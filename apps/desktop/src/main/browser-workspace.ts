import { BrowserWindow, shell, type IpcMainInvokeEvent } from 'electron'
import { BrowserNativeSurface, inspectBrowserPage } from './browser-native-surface'
import { createHash, randomUUID } from 'node:crypto'
import type { BrowserWorkspaceApi, BrowserWorkspaceView, BrowserPreferences, CoreMethod } from '@contracts'
import agentBundle from './codeg-browser-agent.js?raw'
import { BrowserDownloads } from './browser-downloads'
import { BrowserProfiles } from './browser-profiles'
import { BrowserPreferenceStore } from './browser-preferences'
import evalRenderer from './codeg-eval-render.js?raw'
import { InteractionLateResult, sameInteractionScope, type InteractionWork, type InteractionScope, type InteractionGrant } from './interaction-types'

type Request = <T>(method: CoreMethod, params?: unknown) => Promise<T>
type Surface = {
  view:BrowserWorkspaceView; window:BrowserNativeSurface; grantOrigin:string|null;
  console:Array<{level:string;message:string;at:number;seq:number;origin:string|null;top:boolean;url?:string;line?:number}>; consoleSequence:number; tail:Promise<unknown>;
  modelOwner?:{scope:InteractionScope;agentId:string}; modelWorlds:Map<string,number>
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
  private presentationSequence = 0
  private presentationOwner:Electron.WebContents|null=null
  private readonly surfaces=new Map<string,Surface>()
  private readonly downloads=new BrowserDownloads()
  private readonly profiles=new BrowserProfiles(value=>this.downloads.hasActive(value))
  private readonly preferences=new BrowserPreferenceStore(()=>{for(const surface of this.surfaces.values()){this.revoke(surface);if(!surface.window.isDestroyed()){surface.window.webContents.stop();if(this.preferences.action(surface.view.url)!=='builtin')surface.view.error='此地址的浏览规则已变更，请在允许的地址继续浏览'}}})
  constructor(private readonly request:Request,private readonly owner:()=>BrowserWindow|null){}
  onModelResourceChanged:((id:string,origin:string|null)=>void)|null=null
  private trusted(event:IpcMainInvokeEvent):void{
    const owner=this.owner()
    if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame)throw new Error('browser.local_owner_required')
    if(this.presentationOwner!==owner.webContents){
      this.presentationOwner=owner.webContents
      owner.webContents.on('did-start-navigation',(_event,_url,isInPlace,isMainFrame)=>{if(isMainFrame&&!isInPlace){this.presentationSequence=0;for(const surface of this.surfaces.values())surface.window.present(null)}})
    }
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
    this.onModelResourceChanged?.(surface.view.id,null)
  }
  private exclusive<T>(surface:Surface,run:()=>Promise<T>):Promise<T>{
    const result=surface.tail.then(run,run);surface.tail=result.catch(()=>undefined);return result
  }
  async invoke(event:IpcMainInvokeEvent,operation:unknown,input:unknown):Promise<unknown>{
    this.trusted(event)
    return this.invokeOwned(operation,input)
  }
  private async invokeOwned(operation:unknown,input:unknown):Promise<unknown>{
    if(operation==='preferences')return this.preferences.get()
    if(operation==='updatePreferences'){
      if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('browser.invalid_request')
      return this.preferences.update(input as BrowserPreferences)
    }
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
      await this.preferences.get()
      if(this.preferences.action(initial)!=='builtin')throw new Error('browser.address_rule_requires_other_destination')
      const admission=await this.profiles.admit(profileId,id)
      let policy:Awaited<ReturnType<BrowserPreferenceStore['attach']>>
      try{policy=await this.preferences.attach(admission.session)}catch(error){admission.release();throw error}
      let window:BrowserNativeSurface
      try{const parent=this.owner();if(!parent||parent.isDestroyed())throw new Error('browser.local_owner_required');window=new BrowserNativeSurface({webPreferences:{session:admission.session,nodeIntegration:false,contextIsolation:true,sandbox:true,webSecurity:true,allowRunningInsecureContent:false,devTools:policy.devtools}},parent)}catch(error){policy.release();admission.release();throw error}
      const surface:Surface={window,console:[],consoleSequence:0,modelWorlds:new Map(),tail:Promise.resolve(),grantOrigin:null,view:{id,threadId,profileId:admission.profileId,url:initial,title:'浏览器',level:'none',expiresAt:null,loading:true,error:null}}
      this.surfaces.set(id,surface)
      window.webContents.session.setPermissionRequestHandler((_contents,_permission,callback)=>callback(false))
      window.webContents.session.setPermissionCheckHandler(()=>false)
      const unbindDownloads=this.downloads.bind(window.webContents.session,window.webContents,threadId,id)
      window.webContents.setWindowOpenHandler(()=>({action:'deny'}))
      const navigation=(event:Electron.Event,url:string):void=>{try{address(url);if(this.preferences.action(url)!=='builtin')throw new Error('browser.site_blocked')}catch{event.preventDefault();this.revoke(surface);surface.view.error='此地址受到浏览规则限制';return};if(surface.grantOrigin&&new URL(url).origin!==surface.grantOrigin)this.revoke(surface)}
      window.webContents.on('will-navigate',navigation)
      window.webContents.on('will-redirect',navigation)
      window.webContents.on('did-navigate',(_event,url)=>{surface.view.url=url;surface.console=[];this.onModelResourceChanged?.(id,new URL(url).origin);if(surface.grantOrigin&&new URL(url).origin!==surface.grantOrigin)this.revoke(surface)})
      window.webContents.on('did-navigate-in-page',(_event,url,isMainFrame)=>{if(isMainFrame)surface.view.url=url})
      window.webContents.on('did-start-loading',()=>{surface.view.loading=true;surface.view.error=null})
      window.webContents.on('did-stop-loading',()=>{surface.view.loading=false;surface.view.url=window.webContents.getURL();surface.view.title=window.webContents.getTitle()})
      window.webContents.on('render-process-gone',()=>{surface.view.error='页面进程已退出';this.revoke(surface)})
      window.webContents.on('did-fail-load',(_event,code,description,_url,isMainFrame)=>{if(isMainFrame&&code!==-3){surface.view.error=description;surface.view.loading=false;this.revoke(surface)}})
      window.webContents.on('console-message',details=>{
        let origin:string|null=null
        try{origin=new URL(details.frame.url).origin}catch{/* 未证明来源的行不交模型。 */}
        surface.console.push({level:details.level==='warning'?'warn':details.level,message:String(details.message??'').slice(0,16000),at:Date.now(),seq:++surface.consoleSequence,origin,top:details.frame===window.webContents.mainFrame,url:details.sourceId,line:details.lineNumber})
        if(surface.console.length>200)surface.console.shift()
      })
      window.once('closed',()=>{policy.release();admission.release();unbindDownloads();this.revoke(surface);this.surfaces.delete(id)})
      window.once('ready-to-show',()=>window.show())
      void window.loadURL(initial).catch(error=>{surface.view.error=String(error);surface.view.loading=false;window.show()})
      return this.snapshot(surface)
    }
    const id=(input as {id?:unknown}).id
    if(typeof id!=='string')throw new Error('browser.invalid_id')
    const surface=this.find(id)
    if(operation==='control'&&(input as {action?:unknown}).action==='close'){
      this.revoke(surface);this.surfaces.delete(id);surface.window.destroy();return null
    }
    if(operation==='present'){
      const value=input as {sequence:number;bounds:{x:number;y:number;width:number;height:number}|null}
      if(!Number.isSafeInteger(value.sequence)||value.sequence<=this.presentationSequence)return null
      if(value.bounds&&(![value.bounds.x,value.bounds.y,value.bounds.width,value.bounds.height].every(Number.isFinite)||value.bounds.width<0||value.bounds.height<0))throw new Error('browser.invalid_bounds')
      this.presentationSequence=value.sequence
      for(const row of this.surfaces.values())row.window.present(row===surface?value.bounds:null)
      return null
    }
    if(operation==='inspect'){
      const value=input as {action:string;documentId?:string}
      if(value.action==='console'){const origin=new URL(surface.view.url).origin;return {url:surface.view.url,title:surface.view.title,entries:surface.console.filter(row=>row.top&&row.origin===origin&&row.level==='error').map(({level,message,at})=>({level,message,at}))}}
      return inspectBrowserPage(surface.window.webContents,value.action,value.documentId)
    }
    return this.exclusive(surface,async()=>{
      this.find(id)
      if(operation==='control'){
        const value=input as Parameters<BrowserWorkspaceApi['control']>[0]
        switch(value.action){
          case 'devtools':if(!(await this.preferences.get()).devtools)throw new Error('browser.devtools_disabled');surface.window.webContents.openDevTools({mode:'detach'});break
          case 'system':{const target=address(value.url??surface.view.url);if(this.preferences.action(target)==='block')throw new Error('browser.address_blocked');await shell.openExternal(target);break}
          case 'focus':surface.window.show();surface.window.focus();break
          case 'close':surface.window.close();return null
          case 'navigate':{const target=address(value.url??'');if(this.preferences.action(target)!=='builtin')throw new Error('browser.address_rule_requires_other_destination');this.revoke(surface);await surface.window.loadURL(target);break}
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
      if(!receipt.deliverResult||!['read','control'].includes(surface.view.level)||surface.window.isDestroyed()||surface.window.webContents.mainFrame.routingId.toString()!==document||new URL(surface.window.webContents.getURL()).origin!==origin)throw new Error('browser.authority_or_document_changed')
      if(outcome!=='completed')throw new Error('browser.outcome_unknown: '+String((result as {error:string}).error))
      return result
    })
  }
  /** 模型只能从已由 Core 准入的原请求进入；不借用本地 Thread 级分享。 */
  async modelExecute(work:InteractionWork,assertCurrent:()=>Promise<void>):Promise<unknown>{
    const {binding,call,grants}=work,name=call.toolName,args=call.arguments
    await assertCurrent()
    const active=(surface:Surface):InteractionGrant|undefined=>grants.find(grant=>grant.targetKind==='browser_tab'&&grant.targetId===surface.view.id&&grant.agentId===binding.agentId&&grant.desktopPeerId===binding.desktopPeerId&&sameInteractionScope(grant.scope,binding.scope)&&Date.parse(grant.expiresAt)>Date.now()&&grant.origin===new URL(address(surface.window.webContents.getURL())).origin)
    const summary=(surface:Surface):Record<string,unknown>=>{
      const grant=active(surface),url=surface.window.webContents.getURL()
      let origin:string|null=null;try{origin=new URL(address(url)).origin}catch{/* 加载中或错误页不能共享。 */}
      return {tabId:surface.view.id,origin,level:grant?.level??'none',...(grant?{title:surface.window.webContents.getTitle()}: {})}
    }
    if(name==='browser_list_tabs')return {tabs:[...this.surfaces.values()].filter(surface=>!surface.window.isDestroyed()&&surface.view.threadId===binding.scope.threadId&&(!surface.modelOwner||surface.modelOwner.agentId===binding.agentId&&sameInteractionScope(surface.modelOwner.scope,binding.scope)||Boolean(active(surface)))).map(summary)}
    if(name==='browser_open_tab'){
      if(typeof args.url!=='string')throw new Error('invalid_arguments')
      // 不传 profileId：沿本机原设置使用隔离临时身份，不借外部浏览器。
      const view=await this.invokeOwned('open',{threadId:binding.scope.threadId,url:address(args.url)}) as BrowserWorkspaceView
      const surface=this.find(view.id);surface.modelOwner={scope:{...binding.scope},agentId:binding.agentId}
      const original={tabId:view.id,tab:summary(surface),...(view.error?{loadError:'failed'}:{})}
      try{await assertCurrent()}catch{throw new InteractionLateResult(original)}
      return original
    }
    if(typeof args.tabId!=='string')throw new Error('invalid_arguments')
    const surface=this.find(args.tabId)
    if(surface.view.threadId!==binding.scope.threadId)throw new Error('scope_changed')
    return this.exclusive(surface,async()=>{
      await assertCurrent();this.find(surface.view.id)
      const grant=active(surface)
      if(!grant)throw new Error('grant_required')
      const readOnly=['browser_snapshot','browser_console_messages','browser_screenshot'].includes(name)
      if(!readOnly&&grant.level!=='control')throw new Error('grant_required')
      const origin=grant.origin,document=surface.window.webContents.mainFrame.routingId
      const current=async():Promise<void>=>{await assertCurrent();this.find(surface.view.id);if(new URL(address(surface.window.webContents.getURL())).origin!==origin||surface.window.webContents.mainFrame.routingId!==document||Date.parse(grant.expiresAt)<=Date.now())throw new Error('origin_changed')}
      if(name==='browser_close_tab'){await current();surface.window.close();return {tabId:surface.view.id}}
      if(name==='browser_navigate'){
        if(typeof args.url!=='string')throw new Error('invalid_arguments')
        const target=address(args.url);await this.preferences.get()
        if(this.preferences.action(target)!=='builtin')throw new Error('browser.address_rule_requires_other_destination')
        await current();let loadError:string|undefined
        try{await surface.window.loadURL(target)}catch{loadError='failed'}
        const original={tabId:surface.view.id,tab:summary(surface),...(loadError?{loadError}:{})}
        try{await assertCurrent()}catch{throw new InteractionLateResult(original)}
        return original
      }
      let world=surface.modelWorlds.get(binding.bridgeInstanceId)
      if(world===undefined){if(surface.modelWorlds.size>=64)throw new Error('runtime_unavailable');world=WORLD+100+surface.modelWorlds.size;surface.modelWorlds.set(binding.bridgeInstanceId,world)}
      const evaluate=async(code:string):Promise<unknown>=>surface.window.webContents.executeJavaScriptInIsolatedWorld(world!,[{code}],false)
      await evaluate(`if(!globalThis.__codegAgent){${agentBundle}\n}`)
      let result:Record<string,unknown>
      if(name==='browser_snapshot'){
        const snapshot=await evaluate(`globalThis.__codegAgent.snapshot(${JSON.stringify({maxChars:args.maxChars??15000,epoch:binding.bridgeInstanceId})})`)
        result={tabId:surface.view.id,snapshot}
      }else if(name==='browser_console_messages'){
        const levels=['debug','log','info','warn','error'],minimum=levels.indexOf(String(args.minLevel??'debug')),since=Number(args.since??0),limit=Number(args.limit??100)
        const eligible=surface.console.filter(entry=>entry.origin===origin&&entry.seq>since&&levels.indexOf(entry.level)>=minimum)
        const selected=eligible.slice(0,limit)
        result={tabId:surface.view.id,console:{url:surface.window.webContents.getURL(),entries:selected.map(entry=>({seq:entry.seq,at:entry.at,level:entry.level,source:'console',text:entry.message,top:entry.top,...(entry.url?{url:entry.url}:{}),...(entry.line?{line:entry.line}:{})})),dropped:surface.console.length?Math.max(0,surface.console[0].seq-since-1):0,nextSince:selected.at(-1)?.seq??since,more:eligible.length>selected.length}}
      }else if(name==='browser_screenshot'){
        let region=await evaluate('({x:0,y:0,width:innerWidth,height:innerHeight})') as {x:number;y:number;width:number;height:number;ok?:boolean}
        if(args.ref!==undefined){if(typeof args.generation!=='string')throw new Error('generation_changed');region=await evaluate(`globalThis.__codegAgent.rectOf(${JSON.stringify(args.generation)},${JSON.stringify(args.ref)})`) as typeof region;if(region.ok!==true)throw new Error('generation_changed')}
        await current()
        let image=await surface.window.webContents.capturePage({x:Math.floor(region.x),y:Math.floor(region.y),width:Math.ceil(region.width),height:Math.ceil(region.height)})
        const maxWidth=Number(args.maxWidth??1600);if(image.getSize().width>maxWidth)image=image.resize({width:maxWidth})
        const mime=args.format==='jpeg'?'image/jpeg':'image/png',bytes=mime==='image/jpeg'?image.toJPEG(85):image.toPNG(),size=image.getSize()
        if(bytes.length>8*1024*1024)throw new Error('result_too_large')
        result={tabId:surface.view.id,capture:{mime,data:bytes.toString('base64'),width:size.width,height:size.height,url:surface.window.webContents.getURL(),region:{x:region.x,y:region.y,width:region.width,height:region.height},clipped:args.ref!==undefined}}
      }else if(name==='browser_eval'){
        if(typeof args.code!=='string'||!args.code.trim()||Array.from(args.code).length>4000)throw new Error('invalid_arguments')
        await current()
        const raw=await surface.window.webContents.executeJavaScript(`(function(){\n${evalRenderer}\ntry {\nvar __codegEvalValue=(function(){\n${args.code}\n})();\nreturn __codegEvalRender(__codegEvalValue);\n}catch(e){return __codegEvalError(e);}\n})()`,false)
        if(typeof raw!=='string'||Buffer.byteLength(raw)>256*1024)throw new Error('result_too_large')
        const answer=JSON.parse(raw) as {ok:boolean;kind?:string;value?:string;error?:string;truncated?:boolean},value=Array.from(String(answer.ok?answer.value??'':answer.error??'the code threw, and the page did not say what'))
        result={tabId:surface.view.id,result:{kind:answer.ok&&typeof answer.kind==='string'&&Array.from(answer.kind).length<=32?answer.kind:answer.ok?'unknown':'exception',value:value.slice(0,4000).join(''),truncated:Boolean(answer.truncated)||value.length>4000,url:surface.window.webContents.getURL()}}
      }else{
        const kind=({browser_click:'click',browser_hover:'hover',browser_type:'type',browser_press_key:'press',browser_select_option:'select'} as Record<string,string>)[name]
        if(!kind||typeof args.generation!=='string')throw new Error('invalid_arguments')
        await current()
        const action=await evaluate(`globalThis.__codegAgent.act(${JSON.stringify(args.generation)},${JSON.stringify(args.ref??null)},${JSON.stringify({kind,text:args.text,key:args.key,values:args.values,submit:args.submit,button:args.button,count:args.doubleClick?2:1})})`) as {ok:boolean;url:string;error?:string;detail?:string;scrolled?:unknown}
        result=action.ok?{tabId:surface.view.id,action:{fidelity:'synthetic',url:action.url,...(action.scrolled?{scrolled:action.scrolled}:{})}}:{tabId:surface.view.id,error:`browser_${action.error??'failed'}`,note:action.detail??'Browser action did not complete.'}
      }
      if(Buffer.byteLength(JSON.stringify(result))>32*1024*1024)throw new Error('result_too_large')
      try{await current()}catch{throw new InteractionLateResult(result)}
      return result
    })
  }
  revokeAll():void{for(const surface of this.surfaces.values())this.revoke(surface)}
  closeAll():void{this.downloads.close();for(const surface of this.surfaces.values()){this.revoke(surface);if(!surface.window.isDestroyed())surface.window.destroy()}}
}
