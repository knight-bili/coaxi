import { BrowserWindow, Menu, screen, type IpcMainInvokeEvent } from 'electron'
import { join } from 'node:path'
import type { CoreEvent, CoreMethod } from '@contracts'
type Request=<T>(method:CoreMethod,params?:unknown)=>Promise<T>
type Settings={enabled:boolean;activeId:string|null;scale:number}
type Catalog={entries:Array<{id:string;displayName:string}>;settings:Settings}
/** A source-style transparent pet surface. It has a dedicated preload with no
 * general Core, filesystem, window or external-navigation capability. */
export class PetWindowService{
  private window:BrowserWindow|null=null
  private readonly running=new Set<string>()
  private readonly waiting=new Set<string>()
  private state='idle'
  constructor(private readonly request:Request,private readonly owner:()=>BrowserWindow|null){}
  private trusted(event:IpcMainInvokeEvent,pet:boolean):void{
    const window=pet?this.window:this.owner()
    if(!window||window.isDestroyed()||event.sender!==window.webContents||event.senderFrame!==window.webContents.mainFrame)throw new Error('pets.window_owner_required')
  }
  async synchronize(event:IpcMainInvokeEvent):Promise<void>{this.trusted(event,false);await this.sync()}
  private async sync():Promise<void>{
    const catalog=await this.request<Catalog>('pets.operation',{operation:'list'})
    if(!catalog.settings.enabled||!catalog.settings.activeId){this.close();return}
    if(this.window&&!this.window.isDestroyed()){this.window.webContents.send('rovai:pet-state',{state:this.state,changed:true});return}
    const area=screen.getPrimaryDisplay().workArea,width=410,height=450
    const window=new BrowserWindow({width,height,x:area.x+area.width-width-12,y:area.y+area.height-height-12,transparent:true,backgroundColor:'#00000000',frame:false,show:false,resizable:false,alwaysOnTop:true,skipTaskbar:true,hasShadow:false,webPreferences:{preload:join(import.meta.dirname,'../preload/pet.js'),partition:'rovai-pet',nodeIntegration:false,contextIsolation:true,sandbox:true,webSecurity:true}})
    this.window=window;window.setMenuBarVisibility(false)
    window.webContents.setWindowOpenHandler(()=>({action:'deny'}))
    window.webContents.session.setPermissionCheckHandler(()=>false);window.webContents.session.setPermissionRequestHandler((_contents,_permission,callback)=>callback(false))
    window.webContents.on('will-navigate',event=>event.preventDefault())
    window.webContents.on('will-attach-webview',event=>event.preventDefault())
    window.once('closed',()=>{if(this.window===window)this.window=null})
    window.once('ready-to-show',()=>window.showInactive())
    if(process.env.ELECTRON_RENDERER_URL){const url=new URL('pet.html',process.env.ELECTRON_RENDERER_URL);await window.loadURL(url.href)}else await window.loadFile(join(import.meta.dirname,'../renderer/pet.html'))
  }
  async read(event:IpcMainInvokeEvent,input:unknown):Promise<unknown>{
    this.trusted(event,true)
    if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('pets.invalid_request')
    const value=input as {operation:string;id?:string;offset?:number}
    const catalog=await this.request<Catalog>('pets.operation',{operation:'list'})
    if(value.operation==='list')return {settings:catalog.settings,entries:catalog.entries.filter(entry=>entry.id===catalog.settings.activeId),state:this.state}
    if(value.operation!=='asset'||value.id!==catalog.settings.activeId||!catalog.settings.enabled)throw new Error('pets.asset_not_active')
    return this.request('pets.operation',{operation:'asset',id:value.id,offset:value.offset??0})
  }
  async control(event:IpcMainInvokeEvent,action:unknown):Promise<void>{
    this.trusted(event,true)
    if(action==='focus'){this.focusOwner();return}
    if(action==='hide'){await this.hide();return}
    if(action==='menu'&&this.window){Menu.buildFromTemplate([{label:'打开工作区',click:()=>this.focusOwner()},{label:'让宠物休息',click:()=>{void this.hide()}}]).popup({window:this.window});return}
    throw new Error('pets.operation_not_admitted')
  }
  private focusOwner():void{const owner=this.owner();if(owner&&!owner.isDestroyed()){if(owner.isMinimized())owner.restore();owner.show();owner.focus()}}
  private async hide():Promise<void>{const catalog=await this.request<Catalog>('pets.operation',{operation:'list'});await this.request('pets.operation',{operation:'settings',value:{...catalog.settings,enabled:false}});this.close();const owner=this.owner();if(owner&&!owner.isDestroyed())owner.webContents.send('rovai:pets-changed')}
  onCoreEvent(event:CoreEvent):void{
    const params=event.params&&typeof event.params==='object'?event.params as Record<string,unknown>:{},id=typeof params.agentRunId==='string'?params.agentRunId:null
    if(event.method==='runtime.state'&&['shutting_down','stopped','failed'].includes(String(params.status))){this.running.clear();this.waiting.clear()}
    if(id&&event.method==='agent_run.started'){this.running.add(id);this.waiting.delete(id)}
    if(id&&['agent_run.context_waiting','agent_run.network_recovery_needs_attention','agent_run.network_recovery_waiting'].includes(event.method))this.waiting.add(id)
    if(id&&['agent_run.terminal','agent_run.failed','agent_run.cancelled'].includes(event.method)){this.running.delete(id);this.waiting.delete(id)}
    const state=this.waiting.size?'waiting':this.running.size?'running':'idle'
    if(state!==this.state){this.state=state;this.window?.webContents.send('rovai:pet-state',{state})}
  }
  close():void{const window=this.window;this.window=null;if(window&&!window.isDestroyed())window.destroy()}
}
