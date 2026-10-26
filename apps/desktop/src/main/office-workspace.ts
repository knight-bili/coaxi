import { BrowserWindow, type IpcMainInvokeEvent } from 'electron'
import { randomUUID } from 'node:crypto'
import type { CoreMethod } from '@contracts'
import { BrowserNativeSurface, inspectBrowserPage } from './browser-native-surface'
type Request=<T>(method:CoreMethod,params?:unknown)=>Promise<T>
/** OfficeCLI is the source viewer; it never runs inside the app's origin. */
export class OfficeWorkspace {
  private readonly windows=new Set<BrowserWindow|BrowserNativeSurface>()
  private readonly embedded=new Map<string,BrowserNativeSurface>()
  private readonly watchedOwners=new WeakSet<BrowserWindow>()
  private sequence=0
  private generation=0
  constructor(private readonly request:Request,private readonly owner:()=>BrowserWindow|null){}
  private requireOwner(event:IpcMainInvokeEvent):BrowserWindow{
    const owner=this.owner()
    if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame)throw new Error('office.local_owner_required')
    return owner
  }
  async invoke(event:IpcMainInvokeEvent,input:unknown):Promise<unknown>{
    this.requireOwner(event)
    const value=input as {operation?:string;id?:string;sequence?:number;bounds?:{x:number;y:number;width:number;height:number}|null;action?:string}
    if(!value?.operation||value.operation==='open')return this.open(event,input)
    const surface=typeof value.id==='string'?this.embedded.get(value.id):undefined
    if(!surface||surface.isDestroyed()){if(value.operation==='close'||value.operation==='present'&&value.bounds===null)return;throw new Error('office.preview_missing')}
    if(value.operation==='close'){surface.close();return}
    if(value.operation==='inspect'&&value.action==='screenshot')return inspectBrowserPage(surface.webContents,'screenshot')
    if(value.operation==='present'){
      if(!Number.isSafeInteger(value.sequence)||value.sequence! <= this.sequence)return
      if(value.bounds!==null&&(!value.bounds||![value.bounds.x,value.bounds.y,value.bounds.width,value.bounds.height].every(Number.isFinite)||value.bounds.width<0||value.bounds.height<0))throw new Error('office.invalid_bounds')
      this.sequence=value.sequence!
      for(const current of this.embedded.values())current.present(current===surface?value.bounds!:null)
      return
    }
    throw new Error('office.invalid_operation')
  }
  async open(event:IpcMainInvokeEvent,input:unknown):Promise<{opened:true;id?:string}>{
    const owner=this.requireOwner(event)
    if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('office.invalid_request')
    const {threadId,path,editable,embedded}=input as {threadId:string;path:string;editable:boolean;embedded?:boolean}
    if(typeof threadId!=='string'||typeof path!=='string'||typeof editable!=='boolean'||embedded!==undefined&&typeof embedded!=='boolean')throw new Error('office.invalid_request')
    if(!this.watchedOwners.has(owner)){this.watchedOwners.add(owner);owner.webContents.on('did-start-navigation',(_event,_url,inPlace,mainFrame)=>{if(mainFrame&&!inPlace){this.sequence=0;this.closeAll()}})}
    const scope={threadId},holdId=randomUUID()
    const generation=this.generation
    const lease=await this.request<{port:number}>('office.watch',{scope,operation:'start',holdId,path,editable})
    if(generation!==this.generation||owner.isDestroyed()||event.senderFrame!==owner.webContents.mainFrame){await this.request('office.watch',{scope,operation:'stop',holdId}).catch(()=>undefined);throw new Error('office.preview_cancelled')}
    if(!Number.isInteger(lease.port)||lease.port<1||lease.port>65535){await this.request('office.watch',{scope,operation:'stop',holdId}).catch(()=>undefined);throw new Error('office.invalid_preview')}
    const origin=`http://127.0.0.1:${lease.port}`
    const options={width:1140,height:820,show:false,title:path.split('/').at(-1),autoHideMenuBar:true,webPreferences:{partition:`office-${holdId}`,nodeIntegration:false,contextIsolation:true,sandbox:true,webSecurity:true,devTools:false}}
    let window:BrowserNativeSurface|BrowserWindow
    try{window=embedded?new BrowserNativeSurface(options,owner):new BrowserWindow(options)}catch(error){await this.request('office.watch',{scope,operation:'stop',holdId}).catch(()=>undefined);throw error}
    if(embedded)this.embedded.set(holdId,window as BrowserNativeSurface)
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
    window.once('closed',()=>{clearInterval(heartbeat);this.windows.delete(window);this.embedded.delete(holdId);void this.request('office.watch',{scope,operation:'stop',holdId}).catch(()=>undefined)})
    window.once('ready-to-show',()=>window.show())
    try{await window.loadURL(origin+'/')}catch(error){window.destroy();throw error}
    return {opened:true,...(embedded?{id:holdId}:{})}
  }
  closeAll():void{this.generation++;for(const window of this.windows)if(!window.isDestroyed())window.destroy()}
}
