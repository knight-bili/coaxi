import { randomUUID } from 'node:crypto'
import type { BrowserWindow, IpcMainInvokeEvent } from 'electron'
import type { CoreClient } from './core-client'
import type { BrowserWorkspace } from './browser-workspace'
import type { InteractionGrant, InteractionWork } from './interaction-types'
import { InteractionResultJournal, type InteractionFinish as Finish } from './interaction-result-journal'

type Peer={schemaVersion:1;desktopPeerId:string;coreInstanceId:string;ownerWindowId:string;connectionGeneration:number;capabilities:Array<'browser'|'computer'>;expiresAt:string}
export interface DesktopComputerExecutor {
  attachPeer?(peer:Peer):void
  available():boolean
  execute(work:InteractionWork,assertCurrent:()=>Promise<void>):Promise<unknown>
  stop():Promise<void>
}

/** 原生宿主轮询是唯一动作执行入口。重试只运输既有结果，不重复执行动作。 */
export class InteractionWorkspace {
  private peer:Peer|null=null
  private peerWindow:number|null=null
  private generation=0
  private disposed=false
  private busy=false
  private readonly ownerId=randomUUID()
  private readonly grants=new Map<string,InteractionGrant>()
  private readonly outcomes=new Map<string,Finish>()
  private journalLoaded=false
  private lastError:string|null=null
  private readonly timer:ReturnType<typeof setInterval>
  constructor(private readonly core:CoreClient,private readonly browser:BrowserWorkspace,private readonly owner:()=>BrowserWindow|null,private readonly computer?:DesktopComputerExecutor,private readonly journal?:InteractionResultJournal){
    this.timer=setInterval(()=>{void this.pump().catch(error=>{this.lastError=String(error)})},500)
    this.timer.unref()
    browser.onModelResourceChanged=(id,origin)=>{for(const [grantId,grant] of this.grants){if(grant.targetKind==='browser_tab'&&grant.targetId===id&&(!origin||grant.origin!==origin)){this.grants.delete(grantId);void this.request('grant.revoke',{grantId}).catch(()=>undefined)}}}
  }
  private async request<T>(action:string,payload:unknown):Promise<T>{return this.core.request<T>('interaction.local',{action,payload})}
  private trusted(event:IpcMainInvokeEvent):void{
    const window=this.owner()
    if(!window||window.isDestroyed()||event.sender!==window.webContents||event.senderFrame!==window.webContents.mainFrame)throw new Error('interaction.local_owner_required')
  }
  private clear():void{this.generation++;this.peer=null;this.peerWindow=null;this.grants.clear();void this.computer?.stop().catch(()=>undefined)}
  async invoke(event:IpcMainInvokeEvent,action:unknown,payload:unknown):Promise<unknown>{
    this.trusted(event)
    if(typeof action!=='string'||!payload||typeof payload!=='object'||Array.isArray(payload)||Buffer.byteLength(JSON.stringify(payload))>1024*1024)throw new Error('invalid_arguments')
    if(action==='status')return {peer:this.peer,pendingResults:this.outcomes.size,computerAvailable:Boolean(this.computer?.available()),lastError:this.lastError}
    if(action==='stop'){
      const input=payload as {agentRunId?:unknown}
      if(typeof input.agentRunId!=='string')throw new Error('invalid_arguments')
      const result=await this.request('run.revoke',{agentRunId:input.agentRunId})
      await this.computer?.stop();return result
    }
    if(action==='grant.share'){
      if(!this.peer)throw new Error('desktop_unavailable')
      const input=payload as InteractionGrant
      const result=await this.request<InteractionGrant>('grant.share',{...input,schemaVersion:1,desktopPeerId:this.peer.desktopPeerId})
      this.grants.set(result.grantId,result);return result
    }
    if(action==='grant.revoke'){
      const input=payload as {grantId?:unknown};if(typeof input.grantId!=='string')throw new Error('invalid_arguments')
      const result=await this.request(action,input);this.grants.delete(input.grantId);await this.computer?.stop();return result
    }
    if(action==='grant.list'){
      const result=await this.request<{grants:InteractionGrant[]}>('grant.list',{...payload,desktopPeerId:this.peer?.desktopPeerId??null})
      for(const grant of result.grants)this.grants.set(grant.grantId,grant)
      return result
    }
    if(!['support.get','policy.get','policy.replace','sharing.targets','pending','eval.approve','operation.approve','unknown.reconcile'].includes(action))throw new Error('interaction.operation_not_admitted')
    return this.request(action,payload)
  }
  private async pump():Promise<void>{
    if(this.busy||this.disposed)return
    this.busy=true
    try{
      if(!this.journalLoaded){for(const result of this.journal?.load()??[])this.outcomes.set(result.bridgeCallId,result);this.journalLoaded=true}
      const supervisor=this.core.getSnapshot(),window=this.owner()
      if(!window||window.isDestroyed()||supervisor.fullCoreState!=='ready'){if(this.peer)this.clear();return}
      if(this.peer&&(this.peer.connectionGeneration!==supervisor.generation||this.peerWindow!==window.id||this.peer.capabilities.includes('computer')!==Boolean(this.computer?.available()))){await this.request('peer.detach',{desktopPeerId:this.peer.desktopPeerId}).catch(()=>undefined);this.clear()}
      if(!this.peer||Date.parse(this.peer.expiresAt)-Date.now()<45_000){
        const generation=this.generation
        const peer=await this.request<Peer>('peer.attach',{desktopPeerId:this.peer?.desktopPeerId??null,ownerWindowId:`${window.id}:${this.ownerId}`,connectionGeneration:supervisor.generation,capabilities:this.computer?.available()?['browser','computer']:['browser']})
        if(this.disposed||generation!==this.generation||this.core.getSnapshot().generation!==supervisor.generation||this.owner()!==window)return
        this.peer=peer;this.peerWindow=window.id;this.computer?.attachPeer?.(peer)
      }
      const peer=this.peer,generation=this.generation
      if(!peer)return
      for(const [id,result] of this.outcomes){
        try{
          if(result.desktopPeerId===peer.desktopPeerId)await this.request('desktop.finish',result)
          else await this.request('desktop.archiveLate',result)
          this.journal?.remove(id);this.outcomes.delete(id)
        }catch(error){this.lastError=String(error)}
      }
      const page=await this.request<{calls:InteractionWork[]}>('desktop.poll',{schemaVersion:1,desktopPeerId:peer.desktopPeerId,connectionGeneration:peer.connectionGeneration,limit:16})
      for(const work of page.calls){
        const current=async():Promise<void>=>{
          if(this.disposed||this.generation!==generation||this.peer!==peer||this.core.getSnapshot().generation!==peer.connectionGeneration)throw new Error('desktop_unavailable')
          await this.request('desktop.check',{schemaVersion:1,desktopPeerId:peer.desktopPeerId,connectionGeneration:peer.connectionGeneration,bridgeCallId:work.call.bridgeCallId,bindingDigest:work.receipt.bindingDigest,argumentsDigest:work.call.argumentsDigest})
        }
        let result:unknown|null=null,outcome:Finish['outcome']='unknown'
        const intent:Finish={schemaVersion:1,desktopPeerId:peer.desktopPeerId,connectionGeneration:peer.connectionGeneration,bridgeCallId:work.call.bridgeCallId,bindingDigest:work.receipt.bindingDigest,argumentsDigest:work.call.argumentsDigest,outcome:'unknown',result:null}
        this.journal?.save(intent)
        try{
          await current()
          result=work.call.toolName.startsWith('browser_')?await this.browser.modelExecute(work,current):await this.computer?.execute(work,current)
          if(result===undefined)throw new Error('runtime_unavailable')
          if(Buffer.byteLength(JSON.stringify(result))>32*1024*1024)throw new Error('result_too_large')
          outcome='completed'
        }catch{result=null;outcome='unknown'}
        const finish:Finish={schemaVersion:1,desktopPeerId:peer.desktopPeerId,connectionGeneration:peer.connectionGeneration,bridgeCallId:work.call.bridgeCallId,bindingDigest:work.receipt.bindingDigest,argumentsDigest:work.call.argumentsDigest,outcome,result}
        this.outcomes.set(work.call.bridgeCallId,finish)
        this.journal?.save(finish)
        try{await this.request('desktop.finish',finish);this.journal?.remove(work.call.bridgeCallId);this.outcomes.delete(work.call.bridgeCallId)}catch{try{await this.request('desktop.archiveLate',finish);this.journal?.remove(work.call.bridgeCallId);this.outcomes.delete(work.call.bridgeCallId)}catch{/* 原动作不重做；后续只补存原结果。 */}}
      }
    }finally{this.busy=false}
  }
  async dispose():Promise<void>{
    if(this.disposed)return
    this.disposed=true;clearInterval(this.timer)
    const peer=this.peer;this.clear();await this.computer?.stop()
    if(peer)await this.request('peer.detach',{desktopPeerId:peer.desktopPeerId}).catch(()=>undefined)
    this.browser.onModelResourceChanged=null
  }
}
