import {useEffect,useRef,useState} from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import {sha256} from '@noble/hashes/sha2.js'
import type {AgentProfile,NativeContinuationMode,NativeContinuationPreview,NativeContinuationReceipt,NativeForkPoint} from '@contracts'
import {newCommandId} from '../../shared/command-id'
import type {NativeTranscript} from './native-child-reader'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'
import './user-feedback.css'

type Original={schemaVersion:1;commandId:string;previewId:string;previewDigest:string}
type Saved={schemaVersion:1;request:Original;preview:NativeContinuationPreview}
export const nativeContinuationStatus:Record<NativeContinuationReceipt['state'],string>={prepared:'已登记，正在准备原生会话',starting:'原程序正在恢复或分叉',binding_recorded:'原生会话已记录，可在新会话发送第一条消息',failed:'未完成创建，原导入快照保留',unknown:'创建结果待核对，不会重复创建'}

// Codeg 的原助手文字指纹和重复序号；Core 会用只读快照重新计算，前端结果不授予权限。
function forkPoint(provider:string,data:NativeTranscript,selected:string):NativeForkPoint{
  const index=data.turns.findIndex(turn=>turn.id===selected&&turn.role==='assistant')
  if(index<0)throw new Error('请重新选择原助手回答。')
  const text=(turn:NativeTranscript['turns'][number]):string=>turn.blocks.flatMap(block=>block.type==='text'?[block.text]:[]).join('')
  const fingerprint=(text:string):string=>'sha256:'+Array.from(sha256(new TextEncoder().encode(text)),byte=>byte.toString(16).padStart(2,'0')).join('')
  const turn=data.turns[index],content=text(turn),digest=content.trim()?fingerprint(content):null
  const native=turn.agent_message_id?.trim()?turn.agent_message_id:null
  if(!digest&&(provider==='codex'||!native))throw new Error('这条回答暂时无法准确定位，请选择其他回答或明确从末尾分叉。')
  const id=(provider==='codex'?selected:native??selected).replace(/:segment:\d+$/,'')
  return {selectedTurnId:selected,messageId:id,messageFingerprint:digest,messageOccurrence:digest?data.turns.slice(0,index).filter(turn=>turn.role==='assistant'&&fingerprint(text(turn))===digest).length+1:null}
}

export function NativeContinuationDialog({threadId,entry,data,mode,selectedTurnId,onClose,onCreated,onOpen}:{threadId:string;entry:{id:string;revision:string;provider:string;summary:{title:string|null};sourceName:string};data:NativeTranscript;mode:NativeContinuationMode;selectedTurnId?:string;onClose():void;onCreated?(receipt:NativeContinuationReceipt):void;onOpen?(receipt:NativeContinuationReceipt):Promise<boolean>}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0),busyRef=useRef(false),original=useRef<Original|null>(null)
  const [members,setMembers]=useState<AgentProfile[]>([]),[memberId,setMemberId]=useState(''),[title,setTitle]=useState(entry.summary?.title||entry.sourceName)
  const [preview,setPreview]=useState<NativeContinuationPreview|null>(null),[receipt,setReceipt]=useState<NativeContinuationReceipt|null>(null),[historyReceipts,setHistoryReceipts]=useState<NativeContinuationReceipt[]>([]),[busy,setBusy]=useState(false),[ready,setReady]=useState(false),[error,setError]=useState('')
  const key=client.presentationScope?`native-continuation:${client.presentationScope}:${threadId}:${entry.id}`:null
  const loadMembers=async(observed:number):Promise<AgentProfile[]>=>{
    const list=await client.request<AgentProfile[]>('members.list',{})
    if(observed!==generation.current)return []
    const available=list.filter(member=>member.presence==='present'&&member.runtimeConfiguration!==null)
    setMembers(available)
    setMemberId(current=>available.some(member=>member.agentId===current)?current:available.length===1?available[0].agentId:'')
    return available
  }
  const persist=async(saved:Saved|null):Promise<void>=>{
    if(!key)throw new Error(t('工作区身份尚未确定，本次未发送。'))
    if(client.workbenchEditorRecovery){await client.workbenchEditorRecovery.put({schemaVersion:1,key,editorId:'native-continuation',pending:saved});return}
    if(!client.editingRecovery)throw new Error(t('原请求恢复存储尚不可用，本次未发送。'))
    client.editingRecovery.set(key,saved)
  }
  const settle=async(value:NativeContinuationReceipt,observed:number):Promise<void>=>{
    if(value.commandId!==original.current?.commandId)throw new Error(t('原回执的操作归属不同，请保留编号并核对。'))
    if(observed!==generation.current)return
    if(value.state==='binding_recorded'||value.state==='failed'){
      await persist(null)
      if(observed!==generation.current)return
      original.current=null
    }
    setReceipt(value)
    if(value.state==='binding_recorded'||value.state==='failed'){
      setHistoryReceipts(current=>current.some(item=>item.commandId===value.commandId)?current:[...current,value])
    }
    if(value.state==='binding_recorded')onCreated?.(value)
    if(value.state==='binding_recorded'||value.state==='failed')await loadMembers(observed)
  }
  useEffect(()=>{
    const observed=++generation.current;setReady(false);setBusy(true);busyRef.current=true;setError('')
    void(async()=>{
      if(!key)throw new Error(t('工作区身份尚未确定，本次未发送。'))
      let raw:unknown
      if(client.workbenchEditorRecovery){const value=await client.workbenchEditorRecovery.get(key);if(value!==null){const record=value as {key?:string;editorId?:string;pending?:unknown};if(record.key!==key||record.editorId!=='native-continuation')throw new Error(t('原请求记录归属不同，保留原记录。'));raw=record.pending}}
      else{if(!client.editingRecovery)throw new Error(t('原请求恢复存储尚不可用，本次未发送。'));raw=client.editingRecovery.get(key)}
      if(observed!==generation.current)return
      if(raw!=null){
        const saved=raw as Partial<Saved>
        if(saved.schemaVersion!==1||!saved.request||saved.request.schemaVersion!==1||typeof saved.request.commandId!=='string'||typeof saved.request.previewId!=='string'||typeof saved.request.previewDigest!=='string'||saved.preview?.source.importId!==entry.id||saved.preview.sourceThreadId!==threadId)throw new Error(t('原请求记录损坏，保留原记录。'))
        original.current=saved.request;setPreview(saved.preview)
        const value=await client.request<NativeContinuationReceipt>('history.execution.receipt',{schemaVersion:1,commandId:saved.request.commandId});await settle(value,observed)
      }else{
        await loadMembers(observed)
      }
      if(observed===generation.current)setReady(true)
    })().catch(reason=>{if(observed===generation.current)setError(readErrorMessage(reason))}).finally(()=>{if(observed===generation.current){busyRef.current=false;setBusy(false)}})
    return()=>{generation.current++}
  },[client,key,threadId,entry.id])
  const run=async(action:(observed:number)=>Promise<void>):Promise<void>=>{
    if(busyRef.current)return;busyRef.current=true;setBusy(true);setError('');const observed=generation.current
    try{await action(observed)}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current){busyRef.current=false;setBusy(false)}}
  }
  const prepare=():Promise<void>=>run(async observed=>{
    const member=members.find(member=>member.agentId===memberId);if(!member||!ready||original.current)return
    const point=mode==='fork_point'?forkPoint(entry.provider,data,selectedTurnId??''):null
    const next=await client.request<NativeContinuationPreview>('history.execution.preview',{schemaVersion:1,sourceThreadId:threadId,importId:entry.id,expectedImportRevision:entry.revision,agentId:member.agentId,expectedAgentVersion:member.version,mode,point,title})
    if(observed===generation.current)setPreview(next)
  })
  const confirm=():Promise<void>=>run(async observed=>{
    if(!ready||!preview||preview.capabilityStatus!=='available'||original.current)return
    const request:Original={schemaVersion:1,commandId:newCommandId(),previewId:preview.previewId,previewDigest:preview.previewDigest}
    await persist({schemaVersion:1,request,preview});if(observed!==generation.current)return;original.current=request
    const value=await client.request<NativeContinuationReceipt>('history.execution.confirm',request);await settle(value,observed)
  })
  const reconcile=():Promise<void>=>run(async observed=>{if(!original.current)return;const value=await client.request<NativeContinuationReceipt>('history.execution.receipt',{schemaVersion:1,commandId:original.current.commandId});await settle(value,observed)})
  const beginNewCreation=():void=>{void run(async observed=>{
    const available=await loadMembers(observed)
    if(observed!==generation.current)return
    if(!available.length)throw new Error(t('当前没有可用队员，原创建回执已保留。'))
    setReady(true)
    original.current=null;setPreview(null);setReceipt(null);setError('')
  })}
  const terminal=receipt?.state==='binding_recorded'||receipt?.state==='failed'
  return <Dialog.Root open onOpenChange={open=>{if(!open)onClose()}}><Dialog.Portal><Dialog.Overlay className="app-dialog-overlay"/><Dialog.Content className="app-dialog-content feedback-dialog" aria-describedby={undefined}><header><Dialog.Title>{t(mode==='continue'?'从原生会话继续工作':mode==='fork_point'?'从这条回答分叉':'从会话末尾分叉')}</Dialog.Title><Dialog.Close className="quiet-button" aria-label={t('关闭')}>×</Dialog.Close></header><div className="feedback-dialog-body">
    {!preview&&!original.current&&<><label className="feedback-target-label">{t('使用哪位队员')}<select value={memberId} disabled={busy} onChange={event=>setMemberId(event.target.value)}><option value="">{t('选择队员')}</option>{members.map(member=><option key={member.agentId} value={member.agentId}>{member.displayName}</option>)}</select></label><label className="feedback-target-label">{t('新会话名称')}<input value={title} disabled={busy} onChange={event=>setTitle(event.target.value)}/></label><p>{entry.summary?.title||entry.sourceName} · {entry.provider}</p><p>{t('会在原工程创建只包含所选队员的新会话。原记录保留，不会隐式发送第一条消息。')}</p></>}
    {preview&&<><dl className="feedback-preview-target"><div><dt>{t('新会话')}</dt><dd>{preview.title}</dd></div><div><dt>{t('来源程序')}</dt><dd>{preview.source.provider}</dd></div><div><dt>{t('历史消息')}</dt><dd>{preview.historyMessageCount.toLocaleString()}</dd></div><div><dt>{t('队员')}</dt><dd>{members.find(member=>member.agentId===preview.agentId)?.displayName??preview.agentId}</dd></div></dl>{preview.point&&<p>{t('从原助手回答 {0} 分叉；不能准确定位时不会改为末尾分叉。',preview.point.selectedTurnId)}</p>}{preview.reason&&<p role="status">{preview.reason}</p>}<p>{t('不改变 Git 分支和工程文件。原程序可能继续写它自己的日志，已导入的快照保持只读。')}</p><p className="feedback-help">{t('预览有效至 {0}。',new Date(preview.expiresAt).toLocaleString())}</p></>}
    {historyReceipts.filter(item=>item.commandId!==receipt?.commandId).map(item=><p key={item.commandId} role="status">{t(nativeContinuationStatus[item.state])} · {t('编号：{0}',item.commandId)}</p>)}{receipt&&<p role="status">{t(nativeContinuationStatus[receipt.state])} · {t('编号：{0}',receipt.commandId)}</p>}{receipt?.reportedSessionId&&<p>{t('原程序会话编号：{0}',receipt.reportedSessionId)}</p>}{receipt?.state==='binding_recorded'&&<p>{t('新会话已创建，可从会话列表打开。发送第一条消息前仍使用当前工程的权限和审批。')}</p>}{(receipt?.state==='unknown'||original.current&&!receipt)&&<p role="status">{t('原结果尚未确定。只核对同一编号，不重复创建，也不把只读记录当作普通消息发送。')}</p>}{receipt?.error&&<p role="alert" className="feedback-error">{receipt.error}</p>}{error&&<p role="alert" className="feedback-error">{error}</p>}
  </div><footer><Dialog.Close className="quiet-button">{t('关闭并保留记录')}</Dialog.Close>{receipt?.state==='binding_recorded'&&onOpen&&<button type="button" className="primary-button" disabled={busy} onClick={()=>void run(async observed=>{if(await onOpen(receipt)&&observed===generation.current)onClose()})}>{t('打开新工作私聊')}</button>}{original.current?<button type="button" className="quiet-button" disabled={busy} onClick={()=>void reconcile()}>{t(busy?'正在核对…':'核对原创建结果')}</button>:terminal?<button type="button" className="primary-button" disabled={busy} onClick={beginNewCreation}>{t('开始新的创建')}</button>:!preview?<button type="button" className="primary-button" disabled={busy||!ready||!memberId||Array.from(title).length>80} onClick={()=>void prepare()}>{t('预览新会话')}</button>:<button type="button" className="primary-button" disabled={busy||!ready||preview.capabilityStatus!=='available'} onClick={()=>void confirm()}>{t('确认创建，不发送消息')}</button>}</footer></Dialog.Content></Dialog.Portal></Dialog.Root>
}
