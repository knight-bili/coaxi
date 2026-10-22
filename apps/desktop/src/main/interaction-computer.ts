import { app, type BrowserWindow, type IpcMainInvokeEvent } from 'electron'
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import { createHash,randomBytes,randomUUID } from 'node:crypto'
import { promises as fs,existsSync } from 'node:fs'
import path from 'node:path'
import { resolveCoreBinary,type CoreClient } from './core-client'
import type { DesktopComputerExecutor } from './interaction-workspace'
import type { InteractionGrant,InteractionWork,InteractionScope } from './interaction-types'

interface Preferences{revision:string;enabled:boolean;launch:boolean;clipboard:boolean;foreground:boolean;screen:boolean}
interface Peer{desktopPeerId:string;coreInstanceId:string;ownerWindowId:string;connectionGeneration:number}
interface NativeWindow{targetId:string;appId:string|null;appName:string;appKey:string|null;title:string;shareable:boolean;identity:unknown;bounds:unknown}
interface Pending{resolve(value:unknown):void;reject(error:Error):void;timer:ReturnType<typeof setTimeout>}
const digest=(value:unknown):string=>createHash('sha256').update(JSON.stringify(value)).digest('hex')

/** 仅本机窗口拥有此管道。模型和远程 Web 均拿不到子进程的认证材料。 */
export class InteractionComputer implements DesktopComputerExecutor{
  private readonly file=path.join(app.getPath('userData'),'interaction-computer-preferences.json')
  private preferences:Preferences={revision:randomUUID(),enabled:false,launch:false,clipboard:false,foreground:false,screen:false}
  private readonly ready:Promise<void>
  private peer:Peer|null=null
  private child:ChildProcessWithoutNullStreams|null=null
  private token=''
  private opening:Promise<void>|null=null
  private buffer=Buffer.alloc(0)
  private stopGeneration=0
  private readonly pending=new Map<string,Pending>()
  private lastError:string|null=null
  private failed=false
  private writing:Promise<unknown>=Promise.resolve()
  private readonly scopes=new Map<string,{scope:InteractionScope;agentId:string}>()
  constructor(private readonly core:CoreClient,private readonly owner:()=>BrowserWindow|null){this.ready=this.load()}
  available():boolean{try{return this.preferences.enabled&&!this.failed&&existsSync(resolveCoreBinary())}catch{return false}}
  attachPeer(peer:Peer):void{if(this.peer&&(this.peer.desktopPeerId!==peer.desktopPeerId||this.peer.coreInstanceId!==peer.coreInstanceId)){void this.stop();this.end();this.scopes.clear()}this.peer={...peer}}
  private async load():Promise<void>{
    try{const stat=await fs.lstat(this.file);if(!stat.isFile()||stat.isSymbolicLink()||stat.size>16384)throw new Error('computer.invalid_preferences');const value=JSON.parse(await fs.readFile(this.file,'utf8')) as Preferences;this.validate(value);this.preferences=value}catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT'){this.lastError=String(error);this.failed=true}}
  }
  private validate(value:Preferences):void{if(!value||typeof value.revision!=='string'||Object.keys(value).sort().join(',')!=='clipboard,enabled,foreground,launch,revision,screen'||['enabled','launch','clipboard','foreground','screen'].some(key=>typeof value[key as keyof Preferences]!=='boolean'))throw new Error('computer.invalid_preferences')}
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
      if(value.event)continue
      if(!value.id)throw new Error('computer.invalid_response')
      const request=this.pending.get(value.id);if(!request)continue;clearTimeout(request.timer);this.pending.delete(value.id);value.ok?request.resolve(value.result):request.reject(new Error(value.error??'computer.operation_failed'))
    }
  }
  private async open():Promise<void>{
    await this.ready
    if(this.child)return
    if(this.opening)return this.opening
    if(!this.available()||!this.peer)throw new Error('runtime_unavailable')
    const peer=this.peer
    this.opening=(async()=>{
      const child=spawn(resolveCoreBinary(),['--interaction-desktop'],{stdio:['pipe','pipe','pipe'],windowsHide:true,env:{...process.env}})
      this.child=child;this.buffer=Buffer.alloc(0);this.token=randomBytes(32).toString('hex')
      await new Promise<void>((resolve,reject)=>{
        const timer=setTimeout(()=>reject(new Error('computer.handshake_timeout')),15000)
        const fail=(error:Error):void=>{clearTimeout(timer);this.fail(error);reject(error)}
        child.once('error',fail);child.once('exit',()=>{if(this.child===child)this.child=null;fail(new Error('computer.host_exited'))})
        child.stderr.on('data',()=>{/* 原生错误经逐请求回执返回，stderr 不混入私有协议。 */})
        child.stdout.on('data',(chunk:Buffer)=>{try{this.consume(chunk,value=>{const answer=value as {protocolVersion:number;desktopPeerId:string;stopGeneration:number};if(answer.protocolVersion!==1||answer.desktopPeerId!==peer.desktopPeerId)throw new Error('computer.handshake_changed');clearTimeout(timer);this.stopGeneration=answer.stopGeneration;resolve()})}catch(error){fail(error as Error);child.stdin.end()}})
        void this.send({protocolVersion:1,desktopPeerId:peer.desktopPeerId,ownerWindowId:peer.ownerWindowId,token:this.token}).catch(fail)
      })
    })().catch(error=>{this.end();throw error}).finally(()=>{this.opening=null})
    return this.opening
  }
  private async request<T>(method:string,params:unknown):Promise<T>{
    await this.open();const id=randomUUID()
    return new Promise<T>((resolve,reject)=>{const timer=setTimeout(()=>{this.pending.delete(id);reject(new Error('computer.original_request_unknown'))},120000);this.pending.set(id,{resolve:value=>resolve(value as T),reject,timer});void this.send({id,token:this.token,method,params}).catch(error=>{clearTimeout(timer);this.pending.delete(id);reject(error)})})
  }
  private scopeKey(scope:InteractionScope,agentId:string):string{if(!this.peer)throw new Error('desktop_unavailable');return digest({core:this.peer.coreInstanceId,peer:this.peer.desktopPeerId,scope,agentId})}
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
    await assertCurrent()
    const result=await this.request('call',{scopeKey,bridgeInstanceId:work.binding.bridgeInstanceId,expectedStopGeneration:this.stopGeneration,toolName:work.call.toolName,arguments:work.call.arguments})
    await assertCurrent();return result
  }
  async stop():Promise<void>{if(!this.child)return;try{const result=await this.request<{stopGeneration:number}>('stop_all',{});this.stopGeneration=result.stopGeneration}catch(error){this.lastError=String(error);this.end()}}
  private end():void{const child=this.child;this.child=null;if(child){child.stdin.end();const timer=setTimeout(()=>{if(child.exitCode===null)child.kill()},3000);timer.unref();child.once('exit',()=>clearTimeout(timer))}}
  async invoke(event:IpcMainInvokeEvent,method:unknown,input:unknown):Promise<unknown>{
    const owner=this.owner();if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame)throw new Error('computer.local_owner_required')
    await this.ready
    if(method==='preferences.get')return {...this.preferences,lastError:this.lastError}
    if(method==='preferences.set'){
      const value=input as Preferences;this.validate(value);if(value.revision!==this.preferences.revision)throw new Error('computer.preferences_changed')
      await this.stop();const next={...value,revision:randomUUID()},temporary=`${this.file}.${randomUUID()}.tmp`
      try{await fs.writeFile(temporary,JSON.stringify(next),{encoding:'utf8',mode:0o600,flag:'wx'});await fs.rename(temporary,this.file)}finally{await fs.unlink(temporary).catch(()=>undefined)}
      this.preferences=next;this.failed=false;this.lastError=null;return {...next,lastError:null}
    }
    if(method==='stop'){await this.stop();return {stopped:true}}
    if(method==='picker'||method==='status'){
      const value=input as {scope:InteractionScope;agentId:string}
      if(!value?.scope||typeof value.agentId!=='string')throw new Error('invalid_arguments')
      const targets=await this.core.request<{targets:Array<{agentId:string;scope:InteractionScope}>}>('interaction.local',{action:'sharing.targets',payload:{threadId:value.scope.threadId,after:null}})
      if(!targets.targets.some(target=>target.agentId===value.agentId&&JSON.stringify(target.scope)===JSON.stringify(value.scope)))throw new Error('scope_changed')
      const scopeKey=this.scopeKey(value.scope,value.agentId);this.scopes.set(scopeKey,value)
      return this.request(method,{scopeKey})
    }
    throw new Error('computer.operation_not_admitted')
  }
  async dispose():Promise<void>{await this.stop();this.end();this.peer=null;this.scopes.clear()}
}
