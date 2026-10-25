import {app,BrowserWindow,dialog,type IpcMainInvokeEvent} from 'electron'
import {join} from 'node:path'
import {readFileSync,writeFileSync} from 'node:fs'
import {randomUUID} from 'node:crypto'
import type {CoreMethod} from '@contracts'
import type {CreatedProject,ProjectLauncherWindowApi} from '../shared/project-launcher'

/** Only its own main frame may use the selected directory and template runner. */
export class ProjectLauncherWindow {
  private window:BrowserWindow|null=null
  private directory=''
  private pending:CreatedProject|null=null
  constructor(private request:<T>(method:CoreMethod,params?:unknown)=>Promise<T>,private owner:()=>BrowserWindow|null,private presentation:()=>Partial<Awaited<ReturnType<ProjectLauncherWindowApi['bootstrap']>>>=()=>({})){}
  private isMain(event:IpcMainInvokeEvent,window:BrowserWindow|null):boolean{return !!window&&!window.isDestroyed()&&event.sender===window.webContents&&event.senderFrame===window.webContents.mainFrame}
  async invoke(event:IpcMainInvokeEvent,action:unknown,input?:unknown):Promise<unknown>{
    if(['open','peek','acknowledge'].includes(String(action))){
      if(!this.isMain(event,this.owner()))throw new Error('launcher.local_owner_required')
      if(action==='peek')return this.pending
      if(action==='acknowledge'){if(this.pending?.id===input)this.pending=null;return}
      if(this.window&&!this.window.isDestroyed()){this.window.show();this.window.focus();return}
      try{const value=JSON.parse(readFileSync(join(app.getPath('userData'),'project-launcher.json'),'utf8'));if(typeof value.directory==='string')this.directory=value.directory}catch{}
      const window=new BrowserWindow({title:'项目启动器 · Coaxi',width:1220,height:850,minWidth:880,minHeight:620,show:false,backgroundColor:'#141414',webPreferences:{preload:join(import.meta.dirname,'../preload/project-launcher.js'),contextIsolation:true,nodeIntegration:false,sandbox:true}})
      this.window=window;window.setMenu(null)
      window.webContents.setWindowOpenHandler(()=>({action:'deny'}))
      window.webContents.on('will-navigate',event=>event.preventDefault())
      window.on('close',event=>{event.preventDefault();window.hide()})
      window.on('closed',()=>{if(this.window===window)this.window=null})
      window.once('ready-to-show',()=>window.show())
      if(process.env.ELECTRON_RENDERER_URL)await window.loadURL(new URL('project-launcher.html',process.env.ELECTRON_RENDERER_URL+'/').href)
      else await window.loadFile(join(import.meta.dirname,'../renderer/project-launcher.html'))
      return
    }
    if(!this.isMain(event,this.window))throw new Error('launcher.window_owner_required')
    if(action==='bootstrap')return {...this.presentation(),directory:this.directory,platform:process.platform}
    if(action==='chooseDirectory'){
      const result=await dialog.showOpenDialog(this.window!,{title:'选择新项目的保存目录',properties:['openDirectory','createDirectory']})
      if(result.canceled||!result.filePaths[0])return null
      const inspected=await this.request<{projectPath:string}>('workspaces.inspect',{path:result.filePaths[0]})
      this.directory=inspected.projectPath
      writeFileSync(join(app.getPath('userData'),'project-launcher.json'),JSON.stringify({directory:this.directory}),{mode:0o600})
      return this.directory
    }
    if(action==='request'){
      if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('launcher.invalid_request')
      const value=input as Record<string,unknown>
      if(!['detect','preview','start','status','cancel'].includes(String(value.operation)))throw new Error('launcher.operation_not_admitted')
      if(value.operation!=='detect'&&!this.directory)throw new Error('请先选择保存目录。')
      return this.request('workbench.projectLauncher',{...value,scope:undefined,projectPath:this.directory,options:value.options?{...(value.options as object),scope:undefined}:undefined})
    }
    if(action==='enter'&&typeof input==='string'){
      const result=await this.request<{state:string;path?:string}>('workbench.projectLauncher',{projectPath:this.directory,operation:'status',commandId:input})
      if(result.state!=='completed'||!result.path)throw new Error('项目尚未创建完成，请核对创建结果。')
      const inspected=await this.request<{projectPath:string}>('workspaces.inspect',{path:result.path})
      this.pending={id:randomUUID(),path:inspected.projectPath}
      const main=this.owner();if(!main||main.isDestroyed())throw new Error('主窗口暂不可用，请重新进入项目。')
      if(main.isMinimized())main.restore();main.show();main.focus();main.webContents.send('rovai:project-launcher-available');this.window?.hide();return
    }
    throw new Error('launcher.operation_not_admitted')
  }
  closeAll():void{this.window?.destroy();this.window=null}
}
