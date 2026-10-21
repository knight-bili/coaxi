import { BrowserWindow, type IpcMainInvokeEvent } from 'electron'
import { randomUUID } from 'node:crypto'
import type { CoreMethod } from '@contracts'
type Request=<T>(method:CoreMethod,params?:unknown)=>Promise<T>
/** OfficeCLI is the source viewer; it never runs inside the app's origin. */
export class OfficeWorkspace {
  private readonly windows=new Set<BrowserWindow>()
  constructor(private readonly request:Request,private readonly owner:()=>BrowserWindow|null){}
  async open(event:IpcMainInvokeEvent,input:unknown):Promise<{opened:true}>{
    const owner=this.owner()
    if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame)throw new Error('office.local_owner_required')
    if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('office.invalid_request')
    const {threadId,path,editable}=input as {threadId:string;path:string;editable:boolean}
    if(typeof threadId!=='string'||typeof path!=='string'||typeof editable!=='boolean')throw new Error('office.invalid_request')
    const scope={threadId},holdId=randomUUID()
    const lease=await this.request<{port:number}>('office.watch',{scope,operation:'start',holdId,path,editable})
    const origin=`http://127.0.0.1:${lease.port}`
    const window=new BrowserWindow({width:1140,height:820,show:false,title:path.split('/').at(-1),autoHideMenuBar:true,webPreferences:{partition:`office-${holdId}`,nodeIntegration:false,contextIsolation:true,sandbox:true,webSecurity:true,devTools:false}})
    this.windows.add(window)
    const session=window.webContents.session
    session.setPermissionCheckHandler(()=>false);session.setPermissionRequestHandler((_webContents,_permission,callback)=>callback(false))
    session.on('will-download',event=>event.preventDefault())
    session.webRequest.onBeforeRequest((details,callback)=>{
      let allowed=false
      try{allowed=new URL(details.url).origin===origin&&(['GET','HEAD','OPTIONS'].includes(details.method)||editable)}catch{/* no external fallback */}
      callback({cancel:!allowed})
    })
    window.webContents.setWindowOpenHandler(()=>({action:'deny'}))
    window.webContents.on('will-navigate',(navigation,url)=>{if(new URL(url).origin!==origin)navigation.preventDefault()})
    const heartbeat=setInterval(()=>{void this.request('office.watch',{scope,operation:'heartbeat',holdId}).catch(()=>{if(!window.isDestroyed())window.close()})},30000)
    heartbeat.unref()
    window.once('closed',()=>{clearInterval(heartbeat);this.windows.delete(window);void this.request('office.watch',{scope,operation:'stop',holdId}).catch(()=>undefined)})
    window.once('ready-to-show',()=>window.show())
    try{await window.loadURL(origin+'/')}catch(error){window.destroy();throw error}
    return {opened:true}
  }
  closeAll():void{for(const window of this.windows)if(!window.isDestroyed())window.destroy()}
}
