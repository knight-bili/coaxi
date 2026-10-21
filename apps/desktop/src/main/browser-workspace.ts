import { BrowserWindow, type IpcMainInvokeEvent } from 'electron'
import { createHash, randomUUID } from 'node:crypto'
import type { BrowserWorkspaceApi, BrowserWorkspaceView, CoreMethod } from '@contracts'
import agentBundle from './codeg-browser-agent.js?raw'
import { BrowserDownloads } from './browser-downloads'
import { BrowserProfiles } from './browser-profiles'

type Request = <T>(method: CoreMethod, params?: unknown) => Promise<T>
type Surface = {
  view:BrowserWorkspaceView; window:BrowserWindow; grantOrigin:string|null;
  console:Array<{level:string;message:string;at:number}>; tail:Promise<unknown>
}
const WORLD=997
const digest=(value:unknown):string=>createHash('sha256').update(JSON.stringify(value)).digest('hex')
const address=(value:string):string=>{
  const url=new URL(value)
  if(!['http:','https:'].includes(url.protocol)||url.username||url.password)throw new Error('browser.invalid_address')
  return url.href
}

/** Codeg's native window surface and isolated snapshot engine, adapted to
 * Electron. Only a trusted local owner can create/share/control a surface. */
export class BrowserWorkspace {
  private readonly surfaces=new Map<string,Surface>()
  private readonly downloads=new BrowserDownloads()
  private readonly profiles=new BrowserProfiles(value=>this.downloads.hasActive(value))
  constructor(private readonly request:Request,private readonly owner:()=>BrowserWindow|null){}
  private trusted(event:IpcMainInvokeEvent):void{
    const owner=this.owner()
    if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame)throw new Error('browser.local_owner_required')
  }
  private find(id:string):Surface{
    const surface=this.surfaces.get(id)
    if(!surface||surface.window.isDestroyed())throw new Error('browser.closed')
    if(surface.view.expiresAt!==null&&surface.view.expiresAt<=Date.now())this.revoke(surface)
    return surface
  }
  private snapshot(surface:Surface):BrowserWorkspaceView{return {...surface.view}}
  private revoke(surface:Surface):void{
    surface.view.level='none';surface.view.expiresAt=null;surface.grantOrigin=null
    void this.request('browser.local.revoke',{scope:{threadId:surface.view.threadId},id:surface.view.id}).catch(()=>undefined)
  }
  private exclusive<T>(surface:Surface,run:()=>Promise<T>):Promise<T>{
    const result=surface.tail.then(run,run);surface.tail=result.catch(()=>undefined);return result
  }
  async invoke(event:IpcMainInvokeEvent,operation:unknown,input:unknown):Promise<unknown>{
    this.trusted(event)
    if(operation==='profiles')return this.profiles.list()
    if(operation==='profileControl'){
      if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('browser.invalid_request')
      return this.profiles.control(input as Parameters<BrowserWorkspaceApi['profileControl']>[0])
    }
    if(operation==='downloads'){
      if(typeof input!=='string'||input.length>200)throw new Error('browser.invalid_thread')
      await this.request('workbench.files.list',{scope:{threadId:input},path:''})
      return this.downloads.list(input)
    }
    if(operation==='downloadControl'){
      if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('browser.invalid_request')
      const value=input as Parameters<BrowserWorkspaceApi['downloadControl']>[0]
      if(typeof value.id!=='string'||typeof value.threadId!=='string')throw new Error('browser.invalid_request')
      await this.request('workbench.files.list',{scope:{threadId:value.threadId},path:''})
      return this.downloads.control(value)
    }
    if(operation==='list'){
      if(typeof input!=='string')throw new Error('browser.invalid_thread')
      return [...this.surfaces.values()].filter(s=>s.view.threadId===input).map(s=>this.snapshot(this.find(s.view.id)))
    }
    if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('browser.invalid_request')
    if(operation==='open'){
      const {threadId,url,profileId}=input as {threadId:string;url:string;profileId?:string}
      if(typeof threadId!=='string'||typeof url!=='string'||this.surfaces.size>=32)throw new Error('browser.invalid_request')
      await this.request('workbench.files.list',{scope:{threadId},path:''})
      const initial=address(url),id=randomUUID()
      if(profileId!==undefined&&typeof profileId!=='string')throw new Error('browser.invalid_profile')
      const admission=await this.profiles.admit(profileId,id)
      let window:BrowserWindow
      try{window=new BrowserWindow({width:1100,height:780,show:false,title:'浏览器',autoHideMenuBar:true,webPreferences:{session:admission.session,nodeIntegration:false,contextIsolation:true,sandbox:true,webSecurity:true,allowRunningInsecureContent:false,devTools:false}})}catch(error){admission.release();throw error}
      const surface:Surface={window,console:[],tail:Promise.resolve(),grantOrigin:null,view:{id,threadId,profileId:admission.profileId,url:initial,title:'浏览器',level:'none',expiresAt:null,loading:true,error:null}}
      this.surfaces.set(id,surface)
      window.webContents.session.setPermissionRequestHandler((_contents,_permission,callback)=>callback(false))
      window.webContents.session.setPermissionCheckHandler(()=>false)
      const unbindDownloads=this.downloads.bind(window.webContents.session,window.webContents,threadId,id)
      window.webContents.setWindowOpenHandler(()=>({action:'deny'}))
      const navigation=(event:Electron.Event,url:string):void=>{try{address(url)}catch{event.preventDefault();return};if(surface.grantOrigin&&new URL(url).origin!==surface.grantOrigin)this.revoke(surface)}
      window.webContents.on('will-navigate',navigation)
      window.webContents.on('will-redirect',navigation)
      window.webContents.on('did-navigate',(_event,url)=>{surface.view.url=url;if(surface.grantOrigin&&new URL(url).origin!==surface.grantOrigin)this.revoke(surface)})
      window.webContents.on('did-navigate-in-page',(_event,url,isMainFrame)=>{if(isMainFrame)surface.view.url=url})
      window.webContents.on('did-start-loading',()=>{surface.view.loading=true;surface.view.error=null})
      window.webContents.on('did-stop-loading',()=>{surface.view.loading=false;surface.view.url=window.webContents.getURL();surface.view.title=window.webContents.getTitle()})
      window.webContents.on('render-process-gone',()=>{surface.view.error='页面进程已退出';this.revoke(surface)})
      window.webContents.on('did-fail-load',(_event,code,description,_url,isMainFrame)=>{if(isMainFrame&&code!==-3){surface.view.error=description;surface.view.loading=false;this.revoke(surface)}})
      window.webContents.on('console-message',(...args:unknown[])=>{
        const details=args[1] as {level?:string;message?:string}|number
        const message=typeof details==='object'?details.message:String(args[2]??'')
        surface.console.push({level:typeof details==='object'?String(details.level):String(details),message:String(message??'').slice(0,16000),at:Date.now()})
        if(surface.console.length>200)surface.console.shift()
      })
      window.once('closed',()=>{admission.release();unbindDownloads();this.revoke(surface);this.surfaces.delete(id)})
      window.once('ready-to-show',()=>window.show())
      void window.loadURL(initial).catch(error=>{surface.view.error=String(error);surface.view.loading=false;window.show()})
      return this.snapshot(surface)
    }
    const id=(input as {id?:unknown}).id
    if(typeof id!=='string')throw new Error('browser.invalid_id')
    const surface=this.find(id)
    return this.exclusive(surface,async()=>{
      this.find(id)
      if(operation==='control'){
        const value=input as Parameters<BrowserWorkspaceApi['control']>[0]
        switch(value.action){
          case 'focus':surface.window.show();surface.window.focus();break
          case 'close':surface.window.close();return null
          case 'navigate':this.revoke(surface);await surface.window.loadURL(address(value.url??''));break
          case 'back':if(surface.window.webContents.navigationHistory.canGoBack())surface.window.webContents.navigationHistory.goBack();break
          case 'forward':if(surface.window.webContents.navigationHistory.canGoForward())surface.window.webContents.navigationHistory.goForward();break
          case 'reload':surface.window.webContents.reload();break
          case 'share':{
            this.revoke(surface)
            if(value.level==='none')break
            if(value.level!=='read'&&value.level!=='control')throw new Error('browser.invalid_level')
            const origin=new URL(address(surface.window.webContents.getURL())).origin
            const grant=await this.request<{expiresAt:number}>('browser.local.grant',{scope:{threadId:surface.view.threadId},id,origin,level:value.level,minutes:value.minutes??30})
            if(surface.window.isDestroyed()||new URL(surface.window.webContents.getURL()).origin!==origin){this.revoke(surface);throw new Error('browser.origin_changed')}
            surface.view.level=value.level;surface.view.expiresAt=grant.expiresAt;surface.grantOrigin=origin;break
          }
          default:throw new Error('browser.operation_not_admitted')
        }
        return this.snapshot(surface)
      }
      if(operation!=='perform')throw new Error('browser.operation_not_admitted')
      const value=input as Parameters<BrowserWorkspaceApi['perform']>[0]
      if(surface.view.level==='none'||surface.grantOrigin!==new URL(surface.window.webContents.getURL()).origin)throw new Error('browser.not_shared')
      if(typeof value.commandId!=='string'||JSON.stringify(input).length>1_048_576)throw new Error('browser.invalid_request')
      const origin=surface.grantOrigin,scope={threadId:surface.view.threadId},inputDigest=digest(value)
      const envelope={scope,id,origin,action:value.action,commandId:value.commandId,inputDigest}
      await this.request('browser.local.begin',envelope)
      let result:unknown,outcome='unknown',document:string|null=null
      try{
        document=surface.window.webContents.mainFrame.routingId.toString()
        const evaluate=(code:string):Promise<unknown>=>surface.window.webContents.executeJavaScriptInIsolatedWorld(WORLD,[{code}],false)
        if(value.action==='console')result=[...surface.console]
        else if(value.action==='screenshot')result={mime:'image/png',data:(await surface.window.webContents.capturePage()).toPNG().toString('base64')}
        else {
          await evaluate(`if(!globalThis.__codegAgent){${agentBundle}\n}`)
          if(value.action==='snapshot')result=await evaluate('globalThis.__codegAgent.snapshot({maxChars:250000})')
          else if(value.action==='scroll'){
            const x=Number(value.x??0),y=Number(value.y??0)
            if(!Number.isFinite(x)||!Number.isFinite(y)||Math.abs(x)>10000||Math.abs(y)>10000)throw new Error('browser.invalid_scroll')
            result=await evaluate(`globalThis.scrollBy(${x},${y});({ok:true})`)
          }else {
            if(typeof value.generation!=='string')throw new Error('browser.snapshot_required')
            const action={kind:value.action,text:value.text,key:value.key,values:value.values}
            result=await evaluate(`globalThis.__codegAgent.act(${JSON.stringify(value.generation)},${JSON.stringify(value.ref??null)},${JSON.stringify(action)})`)
          }
        }
        outcome='completed'
      }catch(error){result={error:String(error)};outcome='unknown'}
      const receipt=await this.request<{deliverResult:boolean}>('browser.local.finish',{...envelope,outcome,resultDigest:digest(result)})
      if(!receipt.deliverResult||surface.view.level==='none'||surface.window.isDestroyed()||surface.window.webContents.mainFrame.routingId.toString()!==document||new URL(surface.window.webContents.getURL()).origin!==origin)throw new Error('browser.authority_or_document_changed')
      if(outcome!=='completed')throw new Error('browser.outcome_unknown: '+String((result as {error:string}).error))
      return result
    })
  }
  revokeAll():void{for(const surface of this.surfaces.values())this.revoke(surface)}
  closeAll():void{this.downloads.close();for(const surface of this.surfaces.values()){this.revoke(surface);if(!surface.window.isDestroyed())surface.window.destroy()}}
}
