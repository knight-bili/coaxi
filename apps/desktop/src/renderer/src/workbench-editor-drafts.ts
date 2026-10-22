import type { WorkbenchEditorSnapshot,WorkbenchEditorState } from '@contracts'
import type { ThreadClient } from './camp-client'
import { readCanvasViewScope } from './canvas-view-state'
import { loadEditorRecovery,saveEditorRecovery,type EditorRecoveryRecord } from './workbench-editor-recovery'
const same=(left:unknown,right:unknown):boolean=>JSON.stringify(left)===JSON.stringify(right)
export interface EditorDraftStatus {ready:boolean;saving:boolean;pending:boolean;issue:string|null;revision:string|null}

/** 原文件缓冲与恢复入口共用一个队列；只有原 workbench.files.mutate 会写工程文件。 */
export class WorkbenchEditorDrafts {
  private record:EditorRecoveryRecord|null=null
  private acknowledged:WorkbenchEditorState|null=null
  private disposed=false
  private timer:ReturnType<typeof setTimeout>|null=null
  private cacheTail:Promise<void>=Promise.resolve()
  private flight:Promise<void>|null=null
  private current:EditorDraftStatus={ready:false,saving:false,pending:false,issue:null,revision:null}
  private readonly listeners=new Set<()=>void>()
  constructor(private readonly client:ThreadClient,private readonly threadId:string,private readonly projectPath:string){}
  subscribe=(listener:()=>void):(()=>void)=>{this.listeners.add(listener);return()=>this.listeners.delete(listener)}
  snapshot=():EditorDraftStatus=>this.current
  private publish(change:Partial<EditorDraftStatus>):void{if(this.disposed)return;this.current={...this.current,...change};for(const listener of this.listeners)listener()}
  private cache():Promise<void>{
    if(!this.record)return Promise.reject(new Error('工程草稿尚未准备好。'))
    const value=structuredClone(this.record),write=this.cacheTail.catch(()=>undefined).then(()=>saveEditorRecovery(value));this.cacheTail=write;return write
  }
  async start():Promise<WorkbenchEditorState|null>{
    const identity=await readCanvasViewScope(this.client);if(!identity)throw new Error('无法确定工程草稿所属主机。')
    const key=JSON.stringify([identity.host,identity.core,this.threadId,this.projectPath]),record=await loadEditorRecovery(key)
    const result=await this.client.request<WorkbenchEditorSnapshot>('workbench.editor',{operation:'get',scope:{threadId:this.threadId},editorId:record.editorId})
    if(this.disposed)return null
    if(record.workspaceRoot&&record.workspaceRoot!==result.workspaceRoot)throw new Error('原工程目录已改变，本机原草稿保留，请先恢复原工程。')
    this.record=record;record.workspaceRoot=result.workspaceRoot;this.acknowledged=result.value
    if(record.pending&&same(record.pending.value,result.value)){record.pending=null;record.revision=result.revision}
    else if(!record.pending)record.revision=result.revision
    if(record.pending&&record.pending.expectedRevision!==result.revision){this.publish({ready:true,pending:true,issue:'其他窗口已改变工程草稿。原输入与原请求仍保留，请先查看主机版本。',revision:result.revision});return structuredClone(record.latest??record.pending.value)}
    const restored=record.latest??result.value;record.latest=restored?structuredClone(restored):null;await this.cache()
    this.publish({ready:true,pending:Boolean(record.pending)||!same(restored,this.acknowledged),issue:null,revision:record.revision})
    return restored?structuredClone(restored):null
  }
  update(value:WorkbenchEditorState):void{
    if(!this.record||!this.current.ready||this.disposed)return
    this.record.latest=structuredClone(value);this.publish({pending:Boolean(this.record.pending)||!same(value,this.acknowledged)})
    if(this.timer)clearTimeout(this.timer)
    this.timer=setTimeout(()=>{this.timer=null;void this.flush().catch(()=>undefined)},500)
  }
  async readHost():Promise<WorkbenchEditorSnapshot>{
    if(!this.record)throw new Error('工程草稿尚未读取。')
    return this.client.request('workbench.editor',{operation:'get',scope:{threadId:this.threadId},editorId:this.record.editorId})
  }
  async useHostVersion():Promise<WorkbenchEditorState|null>{
    if(this.flight)throw new Error('工程草稿仍在补存。')
    const host=await this.readHost(),record=this.record!
    if(record.latest)await saveEditorRecovery({...structuredClone(record),key:`${record.key}:copy:${crypto.randomUUID()}`})
    record.pending=null;record.latest=host.value?structuredClone(host.value):null;record.revision=host.revision;record.workspaceRoot=host.workspaceRoot;this.acknowledged=host.value;await this.cache()
    this.publish({ready:true,pending:false,saving:false,issue:null,revision:record.revision});return record.latest?structuredClone(record.latest):null
  }
  flush():Promise<void>{
    if(this.flight)return this.flight
    const flight:Promise<void>=this.persist().finally(()=>{if(this.flight===flight)this.flight=null})
    this.flight=flight;return flight
  }
  private async persist():Promise<void>{
    const record=this.record;if(!record||!this.current.ready)throw new Error('工程编辑草稿尚未读取成功。')
    if(this.timer){clearTimeout(this.timer);this.timer=null}
    this.publish({saving:true,issue:null})
    try{
      for(let attempt=0;attempt<4;attempt++){
        if(!record.pending&&same(record.latest,this.acknowledged))break
        if(!record.pending){if(!record.latest||!record.workspaceRoot)break;record.pending={operation:'save',scope:{threadId:this.threadId},editorId:record.editorId,workspaceRoot:record.workspaceRoot,expectedRevision:record.revision,commandId:crypto.randomUUID(),value:structuredClone(record.latest)}}
        await this.cache()
        const original=record.pending,result=await this.client.request<WorkbenchEditorSnapshot>('workbench.editor',original)
        if(result.workspaceRoot!==record.workspaceRoot||!same(result.value,original.value))throw new Error('主机返回了不同的工程草稿，原请求仍保留。')
        record.revision=result.revision;record.pending=null;this.acknowledged=result.value;await this.cache()
      }
      this.publish({pending:Boolean(record.pending)||!same(record.latest,this.acknowledged),revision:record.revision})
    }catch(error){this.publish({pending:true,issue:String(error)});throw error}finally{this.publish({saving:false})}
  }
  dispose():void{this.disposed=true;if(this.timer)clearTimeout(this.timer);this.timer=null;this.listeners.clear();if(this.record)void this.cache().catch(()=>undefined)}
}
