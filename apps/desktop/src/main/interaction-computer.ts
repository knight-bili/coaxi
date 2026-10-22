import {defaultInteractionStopShortcut,interactionStopAccelerator} from '../shared/interaction-stop-shortcut'
import { app, globalShortcut, type BrowserWindow, type IpcMainInvokeEvent } from 'electron'
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import { createHash,randomBytes,randomUUID } from 'node:crypto'
import { promises as fs,existsSync } from 'node:fs'
import path from 'node:path'
import { resolveCoreBinary,type CoreClient } from './core-client'
import type { DesktopComputerExecutor } from './interaction-workspace'
import type { InteractionGrant,InteractionWork,InteractionScope } from './interaction-types'

interface Preferences{revision:string;enabled:boolean;launch:boolean;clipboard:boolean;foreground:boolean;screen:boolean;stopShortcut:string}
interface Peer{desktopPeerId:string;coreInstanceId:string;ownerWindowId:string;connectionGeneration:number}
interface NativeWindow{targetId:string;appId:string|null;appName:string;appKey:string|null;title:string;shareable:boolean;identity:unknown;bounds:unknown}
interface Pending{resolve(value:unknown):void;reject(error:Error):void;timer:ReturnType<typeof setTimeout>}
export interface ComputerActivity {scopeKey:string;targetId:string;action:string;outcome:'done'|'refused'|'failed';at:number;app:string|null;count:number}
export interface InteractionComputerMonitor {enabled:boolean;connected:boolean;lastError:string|null;stopShortcut:string|null;grants:InteractionGrant[];activity:Array<ComputerActivity&{scope?:InteractionScope;agentId?:string}>}
const digest=(value:unknown):string=>createHash('sha256').update(JSON.stringify(value)).digest('hex')

/** 仅本机窗口拥有此管道。模型和远程 Web 均拿不到子进程的认证材料。 */
export class InteractionComputer implements DesktopComputerExecutor{
  private readonly file=path.join(app.getPath('userData'),'interaction-computer-preferences.json')
  private preferences:Preferences={revision:randomUUID(),enabled:false,launch:false,clipboard:false,foreground:false,screen:false,stopShortcut:defaultInteractionStopShortcut(process.platform)}
  private readonly ready:Promise<void>
  private peer:Peer|null=null
  private child:ChildProcessWithoutNullStreams|null=null
  private token=''
  private opening:Promise<void>|null=null
  private buffer=Buffer.alloc(0)
  private stopGeneration=0
  private catalogSequence=0
  private readonly pending=new Map<string,Pending>()
  private lastError:string|null=null
  private failed=false
  private disposed=false
  private registeredShortcut:string|null=null
  private readonly running=new Map<string,{scopeKey:string;bridgeInstanceId:string}>()
  private readonly unsubscribe:()=>void
  private writing:Promise<unknown>=Promise.resolve()
  private readonly scopes=new Map<string,{scope:InteractionScope;agentId:string}>()
  private readonly activity:ComputerActivity[]=[]
  private readonly nativeStates=new Map<string,unknown>()
  constructor(private readonly core:CoreClient,private readonly owner:()=>BrowserWindow|null){this.ready=this.load();this.unsubscribe=core.onEvent(event=>{if(event.method!=='interaction.run.revoked')return;const input=event.params as {agentRunId?:string;bridgeInstanceId?:string},running=input.agentRunId?this.running.get(input.agentRunId):null;if(running&&running.bridgeInstanceId===input.bridgeInstanceId){this.running.delete(input.agentRunId!);if(this.child)void this.request('release_scope',running).catch(()=>undefined)}});void app.whenReady().then(()=>this.ready).then(()=>this.updateShortcut()).catch(()=>undefined)}
  available():boolean{try{return !this.disposed&&this.preferences.enabled&&!this.failed&&existsSync(resolveCoreBinary())}catch{return false}}
  attachPeer(peer:Peer):void{if(this.peer&&(this.peer.desktopPeerId!==peer.desktopPeerId||this.peer.coreInstanceId!==peer.coreInstanceId)){this.end();this.scopes.clear();this.activity.length=0;this.nativeStates.clear()}this.peer={...peer}}
  private async load():Promise<void>{
    try{const stat=await fs.lstat(this.file);if(!stat.isFile()||stat.isSymbolicLink()||stat.size>16384)throw new Error('computer.invalid_preferences');const value=JSON.parse(await fs.readFile(this.file,'utf8')) as Preferences;if(value.stopShortcut===undefined)value.stopShortcut=defaultInteractionStopShortcut(process.platform);this.validate(value);this.preferences=value}catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT'){this.lastError=String(error);this.failed=true}}
  }
  private validate(value:Preferences):void{if(!value||typeof value.revision!=='string'||Object.keys(value).sort().join(',')!=='clipboard,enabled,foreground,launch,revision,screen,stopShortcut'||['enabled','launch','clipboard','foreground','screen'].some(key=>typeof value[key as keyof Preferences]!=='boolean'))throw new Error('computer.invalid_preferences');if(typeof value.stopShortcut!=='string'||value.stopShortcut.length>128)throw new Error('computer.invalid_stop_shortcut');interactionStopAccelerator(value.stopShortcut,process.platform)}
  private updateShortcut():void{
    if(!app.isReady())return
    const shortcut=this.preferences.enabled?interactionStopAccelerator(this.preferences.stopShortcut,process.platform):null
    if(shortcut===this.registeredShortcut)return
    if(this.registeredShortcut)globalShortcut.unregister(this.registeredShortcut)
    this.registeredShortcut=null
    if(shortcut){if(globalShortcut.register(shortcut,()=>{void this.core.request('interaction.local',{action:'computer.stop',payload:{}}).finally(()=>this.stop()).catch(error=>{this.lastError=String(error)})}))this.registeredShortcut=shortcut;else this.lastError='停止快捷键未注册成功，请在设置中更换组合或使用立即停止。'}
  }
  private fail(error:Error):void{this.failed=true;this.lastError=error.message;for(const request of this.pending.values()){clearTimeout(request.timer);request.reject(error)}this.pending.clear()}
  private frame(value:unknown):Buffer{const body=Buffer.from(JSON.stringify(value));if(body.length>1024*1024)throw new Error('computer.request_too_large');const header=Buffer.alloc(4);header.writeUInt32LE(body.length);return Buffer.concat([header,body])}
  private send(value:unknown):Promise<void>{
    const bytes=this.frame(value),child=this.child
    if(!child)throw new Error('desktop_unavailable')
    const write=this.writing.then(()=>new Promise<void>((resolve,reject)=>{if(this.child!==child||child.stdin.destroyed){reject(new Error('desktop_unavailable'));return}child.stdin.write(bytes,error=>error?reject(error):resolve())}))
    this.writing=write.catch(()=>undefined);return write
  }
  private consume(chunk:Buffer,hello:(value:unknown)=>void):void{
    this.buffer=Buffer.concat([this.buffer,chunk]);if(this.buffer.length>64*1024*1024)throw new Error('computer.frame_limit')
    while(this.buffer.length>=4){const size=this.buffer.readUInt32LE();if(size>32*1024*1024)throw new Error('computer.frame_limit');if(this.buffer.length<size+4)return;const value=JSON.parse(this.buffer.subarray(4,size+4).toString('utf8')) as {id?:string;ok?:boolean;result?:unknown;error?:string;event?:string;ready?:boolean};this.buffer=this.buffer.subarray(size+4)
      if(value.ready){hello(value);continue}
      if(value.event){this.nativeEvent(value as {event:string;payload?:unknown});continue}
      if(!value.id)throw new Error('computer.invalid_response')
      const request=this.pending.get(value.id);if(!request)continue;clearTimeout(request.timer);this.pending.delete(value.id);value.ok?request.resolve(value.result):request.reject(new Error(value.error??'computer.operation_failed'))
    }
  }
  private nativeEvent(event:{event:string;payload?:unknown}):void{
    const envelope=event.payload as {scopeKey?:unknown;payload?:unknown}
    if(!envelope||typeof envelope.scopeKey!=='string'||!this.scopes.has(envelope.scopeKey))return
    if(event.event==='computer://state'){this.nativeStates.set(envelope.scopeKey,envelope.payload);return}
    if(event.event==='computer://backend-status'){const state=envelope.payload as {state?:string;detail?:string};if(state.state==='unavailable'||state.state==='failed')this.lastError=state.detail??'computer.backend_unavailable';return}
    if(event.event!=='computer://agent-activity')return
    const payload=envelope.payload as Omit<ComputerActivity,'scopeKey'|'count'>
    const actions=['capture','snapshot','verify','click','drag','scroll','type','key','hold-key','set-value','restore','menu','set-frame','launch','clipboard-read','clipboard-write']
    if(!payload||typeof payload.targetId!=='string'||!actions.includes(payload.action)||!['done','refused','failed'].includes(payload.outcome)||!Number.isSafeInteger(payload.at))return
    const head=this.activity[0]
    if(head&&head.scopeKey===envelope.scopeKey&&head.targetId===payload.targetId&&head.action===payload.action&&head.outcome===payload.outcome){head.at=payload.at;head.count++}
    else this.activity.unshift({scopeKey:envelope.scopeKey,targetId:payload.targetId,action:payload.action,outcome:payload.outcome,at:payload.at,app:typeof payload.app==='string'?payload.app:null,count:1})
    this.activity.splice(50)
  }
  async monitor():Promise<InteractionComputerMonitor>{
    await this.ready
    const peer=this.peer
    const all=peer?await this.core.request<{grants:InteractionGrant[]}>('interaction.local',{action:'grant.list',payload:{desktopPeerId:peer.desktopPeerId}}):{grants:[]}
    if(this.peer!==peer)throw new Error('desktop_unavailable')
    return {enabled:this.preferences.enabled,connected:Boolean(this.child),lastError:this.lastError,stopShortcut:this.registeredShortcut?this.preferences.stopShortcut:null,
      grants:all.grants.filter(grant=>grant.targetKind.startsWith('computer_')&&Date.parse(grant.expiresAt)>Date.now()),
      activity:this.activity.map(item=>({...item,...this.scopes.get(item.scopeKey)}))}
  }
  async clearActivity(before:number,threadId:string):Promise<void>{if(!Number.isSafeInteger(before)||typeof threadId!=='string')throw new Error('invalid_arguments');for(let index=this.activity.length-1;index>=0;index--)if(this.activity[index].at<=before&&this.scopes.get(this.activity[index].scopeKey)?.scope.threadId===threadId)this.activity.splice(index,1)}
  private async open():Promise<void>{
    await this.ready
    if(this.opening)return this.opening
    if(this.child)return
    if(!this.available()||!this.peer)throw new Error('runtime_unavailable')
    const peer=this.peer
    let child:ChildProcessWithoutNullStreams|null=null
    const opening:Promise<void>=(async()=>{
      const processChild=spawn(resolveCoreBinary(),['--interaction-desktop'],{stdio:['pipe','pipe','pipe'],windowsHide:true,env:{...process.env}})
      child=processChild;this.child=processChild;this.buffer=Buffer.alloc(0);this.token=randomBytes(32).toString('hex')
      await new Promise<void>((resolve,reject)=>{
        const fail=(error:Error):void=>{clearTimeout(timer);if(this.child===processChild){this.fail(error);this.end()}reject(error)}
        const timer=setTimeout(()=>fail(new Error('computer.handshake_timeout')),15000)
        processChild.once('error',fail);processChild.once('exit',()=>fail(new Error('computer.host_exited')))
        processChild.stderr.on('data',()=>{/* 原生错误经逐请求回执返回，stderr 不混入私有协议。 */})
        processChild.stdout.on('data',(chunk:Buffer)=>{if(this.child!==processChild)return;try{this.consume(chunk,value=>{const answer=value as {protocolVersion:number;desktopPeerId:string;stopGeneration:number};if(answer.protocolVersion!==1||answer.desktopPeerId!==peer.desktopPeerId||this.peer?.desktopPeerId!==peer.desktopPeerId)throw new Error('computer.handshake_changed');clearTimeout(timer);this.stopGeneration=answer.stopGeneration;resolve()})}catch(error){fail(error as Error)}})
        void this.send({protocolVersion:1,desktopPeerId:peer.desktopPeerId,ownerWindowId:peer.ownerWindowId,token:this.token}).catch(fail)
      })
    })().catch(error=>{if(this.child===child)this.end();throw error}).finally(()=>{if(this.opening===opening)this.opening=null})
    this.opening=opening;return opening
  }
  private async request<T>(method:string,params:unknown):Promise<T>{
    await this.open();const id=randomUUID()
    return new Promise<T>((resolve,reject)=>{const timer=setTimeout(()=>{this.pending.delete(id);reject(new Error('computer.original_request_unknown'))},120000);this.pending.set(id,{resolve:value=>resolve(value as T),reject,timer});void this.send({id,token:this.token,method,params}).catch(error=>{clearTimeout(timer);this.pending.delete(id);reject(error)})})
  }
  private scopeKey(scope:InteractionScope,agentId:string):string{if(!this.peer)throw new Error('desktop_unavailable');return digest({core:this.peer.coreInstanceId,peer:this.peer.desktopPeerId,scope,agentId})}
  private async registerTargets(windows:NativeWindow[],screenId:string):Promise<void>{
    if(!this.peer)throw new Error('desktop_unavailable')
    const targets=new Map<string,{targetId:string;targetKind:string;appId:string|null;shareable:boolean}>()
    for(const window of windows){
      targets.set(window.targetId,{targetId:window.targetId,targetKind:'computer_window',appId:window.appId,shareable:window.shareable})
      if(window.appId){const old=targets.get(window.appId);targets.set(window.appId,{targetId:window.appId,targetKind:'computer_app',appId:null,shareable:Boolean(old?.shareable)||window.shareable})}
    }
    if(screenId)targets.set(screenId,{targetId:screenId,targetKind:'computer_desktop',appId:null,shareable:this.preferences.screen&&process.platform!=='linux'})
    await this.core.request('interaction.local',{action:'desktop.targets',payload:{desktopPeerId:this.peer.desktopPeerId,connectionGeneration:this.peer.connectionGeneration,sequence:++this.catalogSequence,targets:[...targets.values()]}})
  }
  async execute(work:InteractionWork,assertCurrent:()=>Promise<void>):Promise<unknown>{
    await this.ready;await assertCurrent()
    if(!this.peer||this.peer.desktopPeerId!==work.binding.desktopPeerId)throw new Error('desktop_unavailable')
    const scopeKey=this.scopeKey(work.binding.scope,work.binding.agentId)
    this.scopes.set(scopeKey,{scope:work.binding.scope,agentId:work.binding.agentId})
    const policy=await this.core.request<{revision:number;members:Array<{agentId:string;features:{computer:boolean;computerLaunch:boolean;computerClipboard:boolean}}>} >('interaction.local',{action:'policy.get',payload:{}})
    const features=policy.members.find(member=>member.agentId===work.binding.agentId)?.features
    if(policy.revision!==work.binding.permissionRevision||!features?.computer)throw new Error('permission_changed')
    const all=await this.core.request<{grants:InteractionGrant[]}>('interaction.local',{action:'grant.list',payload:{scope:work.binding.scope,agentId:work.binding.agentId,desktopPeerId:this.peer.desktopPeerId}})
    await this.request('configure',{scopeKey,bridgeInstanceId:work.binding.bridgeInstanceId,expectedStopGeneration:this.stopGeneration,enabled:true,launchEnabled:features.computerLaunch&&this.preferences.launch,clipboardEnabled:features.computerClipboard&&this.preferences.clipboard,allowForeground:this.preferences.foreground,screenEnabled:this.preferences.screen,grants:all.grants.filter(grant=>grant.targetKind.startsWith('computer_')).map(grant=>({targetKind:grant.targetKind,targetId:grant.targetId,level:grant.level,expiresAt:grant.expiresAt,grantGeneration:grant.grantGeneration}))})
    const catalog=await this.request<{windows:NativeWindow[];screenId:string}>('picker',{scopeKey})
    await this.registerTargets(catalog.windows,catalog.screenId)
    await assertCurrent()
    const running={scopeKey,bridgeInstanceId:work.binding.bridgeInstanceId}
    for(const [id,previous]of this.running)if(previous.scopeKey===scopeKey&&id!==work.binding.agentRunId)this.running.delete(id)
    this.running.set(work.binding.agentRunId,running)
    let checking=false
    const watcher=setInterval(()=>{if(checking)return;checking=true;void assertCurrent().catch(()=>this.request('stop_scope',running)).catch(()=>undefined).finally(()=>{checking=false})},500);watcher.unref()
    try{const result=await this.request('call',{...running,expectedStopGeneration:this.stopGeneration,toolName:work.call.toolName,arguments:work.call.arguments});await assertCurrent();return result}
    finally{clearInterval(watcher)}
  }
  async stop():Promise<void>{const child=this.child;if(!child)return;try{const result=await this.request<{stopGeneration:number}>('stop_all',{});if(this.child===child)this.stopGeneration=result.stopGeneration}catch(error){this.lastError=String(error);if(this.child===child)this.end()}}
  private end():void{const child=this.child;this.child=null;this.opening=null;this.running.clear();this.nativeStates.clear();for(const pending of this.pending.values()){clearTimeout(pending.timer);pending.reject(new Error('computer.original_request_unknown'))}this.pending.clear();if(child){child.stdin.end();const timer=setTimeout(()=>{if(child.exitCode===null)child.kill()},3000);timer.unref();child.once('exit',()=>clearTimeout(timer))}}
  async invoke(event:IpcMainInvokeEvent,method:unknown,input:unknown):Promise<unknown>{
    const owner=this.owner();if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame)throw new Error('computer.local_owner_required')
    await this.ready
    if(method==='monitor')return this.monitor()
    if(method==='activity.clear'){const value=input as {before:number;threadId:string};await this.clearActivity(value?.before,value?.threadId);return {cleared:true}}
    if(method==='preferences.get')return {...this.preferences,lastError:this.lastError}
    if(method==='preferences.set'){
      const value=input as Preferences;this.validate(value);if(value.revision!==this.preferences.revision)throw new Error('computer.preferences_changed')
      await this.stop();const next={...value,revision:randomUUID()},temporary=`${this.file}.${randomUUID()}.tmp`
      try{await fs.writeFile(temporary,JSON.stringify(next),{encoding:'utf8',mode:0o600,flag:'wx'});await fs.rename(temporary,this.file)}finally{await fs.unlink(temporary).catch(()=>undefined)}
      this.preferences=next;this.failed=false;this.lastError=null;this.updateShortcut();return {...next,lastError:null}
    }
    if(method==='stop'){try{await this.core.request('interaction.local',{action:'computer.stop',payload:{}})}finally{await this.stop()}return {stopped:true}}
    if(method==='picker'||method==='status'){
      const value=input as {scope:InteractionScope;agentId:string}
      if(!value?.scope||typeof value.agentId!=='string')throw new Error('invalid_arguments')
      const targets=await this.core.request<{targets:Array<{agentId:string;scope:InteractionScope}>}>('interaction.local',{action:'sharing.targets',payload:{threadId:value.scope.threadId,after:null}})
      if(!targets.targets.some(target=>target.agentId===value.agentId&&JSON.stringify(target.scope)===JSON.stringify(value.scope)))throw new Error('scope_changed')
      const scopeKey=this.scopeKey(value.scope,value.agentId);this.scopes.set(scopeKey,value)
      const pickerKey=digest({peer:this.peer?.desktopPeerId,scope:value.scope,agentId:value.agentId,kind:'local_picker'})
      if(method==='picker')await this.request('configure',{scopeKey:pickerKey,bridgeInstanceId:randomUUID(),expectedStopGeneration:this.stopGeneration,enabled:this.preferences.enabled,launchEnabled:false,clipboardEnabled:false,allowForeground:false,screenEnabled:false,grants:[]})
      try{const result=await this.request<{windows:NativeWindow[];screenId:string}>(method,{scopeKey:pickerKey});if(method==='picker')await this.registerTargets(result.windows,result.screenId);return result}
      finally{if(this.child)await this.request('release_scope',{scopeKey:pickerKey}).catch(()=>undefined)}
    }
    throw new Error('computer.operation_not_admitted')
  }
  async dispose():Promise<void>{this.disposed=true;this.unsubscribe();if(this.registeredShortcut)globalShortcut.unregister(this.registeredShortcut);this.registeredShortcut=null;await this.stop();this.end();this.peer=null;this.scopes.clear()}
}
