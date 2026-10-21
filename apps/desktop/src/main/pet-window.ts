import { app, BrowserWindow, Menu, screen, type IpcMainInvokeEvent } from 'electron'
import { join } from 'node:path'
import type { CoreEvent, CoreMethod } from '@contracts'
import {clampWindowBounds,readWindowStateFile,sanitizeWindowState,writeWindowStateFile} from './window-state'
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
  private hitRegions:Array<{x:number;y:number;width:number;height:number;mask?:string}>=[]
  private disposed=false
  private serial:Promise<void>=Promise.resolve()
  private pointerTimer:ReturnType<typeof setInterval>|null=null
  private persistTimer:ReturnType<typeof setTimeout>|null=null
  private writeTail:Promise<void>=Promise.resolve()
  private displayChanged:(()=>void)|null=null
  private positionPath():string{return join(app.getPath('userData'),'pet-window-state.json')}
  private async saveScale(scale:number):Promise<void>{const catalog=await this.request<Catalog>('pets.operation',{operation:'list'});await this.request('pets.operation',{operation:'settings',value:{...catalog.settings,scale}});await this.sync();const owner=this.owner();if(owner&&!owner.isDestroyed())owner.webContents.send('rovai:pets-changed')}
  private size(scale:number):{width:number;height:number}{return {width:Math.max(220,Math.ceil(192*scale+16)),height:Math.ceil(208*scale+52)}}
  private place(window:BrowserWindow,width:number,height:number):void{const bounds=window.getBounds(),area=screen.getDisplayMatching(bounds).workArea;window.setBounds(clampWindowBounds({...bounds,width,height},area,1,1))}
  private persist(window:BrowserWindow):void{if(window.isDestroyed())return;const bounds=window.getBounds();this.writeTail=this.writeTail.catch(()=>undefined).then(()=>writeWindowStateFile(this.positionPath(),bounds)).catch(()=>{if(!window.isDestroyed())window.webContents.send('rovai:pet-state',{state:this.state,error:'宠物位置未能保存。'})})}

  constructor(private readonly request:Request,private readonly owner:()=>BrowserWindow|null){}
  private trusted(event:IpcMainInvokeEvent,pet:boolean):void{
    const window=pet?this.window:this.owner()
    if(!window||window.isDestroyed()||event.sender!==window.webContents||event.senderFrame!==window.webContents.mainFrame)throw new Error('pets.window_owner_required')
  }
  async synchronize(event:IpcMainInvokeEvent):Promise<void>{this.trusted(event,false);this.serial=this.serial.catch(()=>undefined).then(()=>this.sync());await this.serial}
  private async sync():Promise<void>{
    const catalog=await this.request<Catalog>('pets.operation',{operation:'list'})
    if(this.disposed)return
    if(!catalog.settings.enabled||!catalog.settings.activeId){this.close();return}
    if(this.window&&!this.window.isDestroyed()){const size=this.size(catalog.settings.scale);this.place(this.window,size.width,size.height);this.window.webContents.send('rovai:pet-state',{state:this.state,changed:true});return}
    const area=screen.getPrimaryDisplay().workArea,{width,height}=this.size(catalog.settings.scale)
    const saved=readWindowStateFile(this.positionPath()),bounds=saved?sanitizeWindowState(saved,screen.getAllDisplays().map(display=>display.workArea),1,1,area):{x:area.x+area.width-width-12,y:area.y+area.height-height-12,width,height}
    const window=new BrowserWindow({...bounds,transparent:true,backgroundColor:'#00000000',frame:false,show:false,resizable:false,alwaysOnTop:true,skipTaskbar:true,hasShadow:false,webPreferences:{preload:join(import.meta.dirname,'../preload/pet.js'),partition:'rovai-pet',nodeIntegration:false,contextIsolation:true,sandbox:true,webSecurity:true}})
    this.window=window;this.place(window,width,height);window.setMenuBarVisibility(false)
    window.on('move',()=>{if(this.persistTimer)clearTimeout(this.persistTimer);this.persistTimer=setTimeout(()=>{this.persistTimer=null;this.persist(window)},220)})
    this.displayChanged=()=>{if(!window.isDestroyed()){const current=window.getBounds();this.place(window,current.width,current.height);this.persist(window)}}
    screen.on('display-removed',this.displayChanged);screen.on('display-metrics-changed',this.displayChanged)
    let ignored=false
    this.pointerTimer=setInterval(()=>{if(window.isDestroyed())return;const cursor=screen.getCursorScreenPoint(),bounds=window.getBounds();const x=cursor.x-bounds.x,y=cursor.y-bounds.y;const inside=this.hitRegions.some(region=>{if(x<region.x||y<region.y||x>=region.x+region.width||y>=region.y+region.height)return false;if(!region.mask)return true;const col=Math.min(31,Math.floor((x-region.x)/region.width*32)),row=Math.min(35,Math.floor((y-region.y)/region.height*36));return region.mask[row*32+col]==='1'});if(ignored===inside){ignored=!inside;window.setIgnoreMouseEvents(ignored,{forward:true})}},200)
    this.pointerTimer.unref()

    window.webContents.setWindowOpenHandler(()=>({action:'deny'}))
    window.webContents.session.setPermissionCheckHandler(()=>false);window.webContents.session.setPermissionRequestHandler((_contents,_permission,callback)=>callback(false))
    window.webContents.on('will-navigate',event=>event.preventDefault())
    window.webContents.on('will-attach-webview',event=>event.preventDefault())
    window.once('closed',()=>{if(this.window===window){this.window=null;this.releaseResources()}})
    window.once('ready-to-show',()=>window.showInactive())
    if(process.env.ELECTRON_RENDERER_URL){const url=new URL('pet.html',process.env.ELECTRON_RENDERER_URL);await window.loadURL(url.href)}else await window.loadFile(join(import.meta.dirname,'../renderer/pet.html'))
  }
  hitTest(event:IpcMainInvokeEvent,input:unknown):void{
    this.trusted(event,true)
    if(!Array.isArray(input)||input.length>3)throw new Error('pets.invalid_hit_regions')
    const bounds=this.window!.getBounds()
    this.hitRegions=input.map(value=>{if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('pets.invalid_hit_region');const region=value as {x:number;y:number;width:number;height:number;mask?:string};if(![region.x,region.y,region.width,region.height].every(Number.isFinite)||region.x<0||region.y<0||region.width<=0||region.height<=0||region.x+region.width>bounds.width+1||region.y+region.height>bounds.height+1||(region.mask!==undefined&&(typeof region.mask!=='string'||!/^[01]{1152}$/.test(region.mask))))throw new Error('pets.invalid_hit_region');return {x:region.x,y:region.y,width:region.width,height:region.height,...(region.mask===undefined?{}:{mask:region.mask})}})
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
    if(action==='menu'&&this.window){const catalog=await this.request<Catalog>('pets.operation',{operation:'list'});const window=this.window;if(!window||window.isDestroyed())return;Menu.buildFromTemplate([{label:'打开工作区',click:()=>this.focusOwner()},{label:'大小',submenu:[.5,.75,1,1.25,1.5,2].map(scale=>({label:`${Math.round(scale*100)}%`,type:'radio' as const,checked:scale===catalog.settings.scale,click:()=>{void this.saveScale(scale).catch(()=>window.webContents.send('rovai:pet-state',{state:this.state,error:'宠物大小未能保存。'}))}}))},{label:'重置位置',click:()=>{const area=screen.getPrimaryDisplay().workArea,bounds=window.getBounds();window.setPosition(area.x+area.width-bounds.width-12,area.y+area.height-bounds.height-12);this.persist(window)}},{type:'separator'},{label:'让宠物休息',click:()=>{void this.hide()}}]).popup({window});return}
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
  private releaseResources():void{this.hitRegions=[];if(this.pointerTimer)clearInterval(this.pointerTimer);this.pointerTimer=null;if(this.persistTimer)clearTimeout(this.persistTimer);this.persistTimer=null;if(this.displayChanged){screen.removeListener('display-removed',this.displayChanged);screen.removeListener('display-metrics-changed',this.displayChanged);this.displayChanged=null}}
  close():void{const window=this.window;if(window&&!window.isDestroyed())this.persist(window);this.window=null;this.releaseResources();if(window&&!window.isDestroyed())window.destroy()}
  dispose():void{this.disposed=true;this.close();this.running.clear();this.waiting.clear()}

}
