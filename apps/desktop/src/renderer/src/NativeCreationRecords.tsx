import {useEffect,useRef,useState} from 'react'
import type {AgentProfile,CanvasConversationRef,NativeCreationHistory,NativeContinuationReceipt} from '@contracts'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'
import {nativeContinuationStatus} from './NativeContinuationDialog'

/** 复用原记录折叠区和工具栏，原命令只读核对；不从历史按钮重新创建。 */
export function NativeCreationRecords({sourceThreadId,importId,onOpen}:{sourceThreadId:string;importId:string;onOpen?(reference:CanvasConversationRef):Promise<boolean>}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0),busyRef=useRef(false)
  const [open,setOpen]=useState(false),[history,setHistory]=useState<NativeCreationHistory>({schemaVersion:1,items:[],next:null}),[members,setMembers]=useState<AgentProfile[]>([]),[busy,setBusy]=useState(false),[error,setError]=useState('')
  const read=async(more=false):Promise<void>=>{
    if(busyRef.current)return;busyRef.current=true;setBusy(true);setError('');const observed=generation.current
    try{
      const [value,profiles]=await Promise.all([client.request<NativeCreationHistory>('history.execution.records',{schemaVersion:1,sourceThreadId,importId,after:more?history.next:null,limit:50}),client.request<AgentProfile[]>('members.list',{})])
      if(observed!==generation.current)return;setMembers(profiles);setHistory(previous=>more?{...value,items:[...previous.items,...value.items.filter(row=>!previous.items.some(old=>old.preview.previewId===row.preview.previewId))]}:value)
    }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current){busyRef.current=false;setBusy(false)}}
  }
  useEffect(()=>{generation.current++;busyRef.current=false;setHistory({schemaVersion:1,items:[],next:null});if(open)void read();return()=>{generation.current++}},[client,sourceThreadId,importId,open])
  const reconcile=async(receipt:NativeContinuationReceipt):Promise<void>=>{
    if(busyRef.current)return;busyRef.current=true;setBusy(true);setError('');const observed=generation.current
    try{const value=await client.request<NativeContinuationReceipt>('history.execution.receipt',{schemaVersion:1,commandId:receipt.commandId});if(value.commandId!==receipt.commandId)throw new Error(t('原回执的操作归属不同，请保留编号并核对。'));if(observed===generation.current)setHistory(previous=>({...previous,items:previous.items.map(row=>row.receipt?.commandId===value.commandId?{...row,receipt:value}:row)}))}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current){busyRef.current=false;setBusy(false)}}
  }
  return <details className="native-session-reasoning" open={open} onToggle={event=>setOpen(event.currentTarget.open)}><summary>{t('继续与分叉的原创建记录')}</summary><div className="workbench-toolbar"><button disabled={busy} onClick={()=>void read()}>{t('刷新原记录')}</button></div>{error&&<p className="workbench-error" role="alert">{error}</p>}{history.items.map(row=><section key={row.preview.previewId}><strong>{row.preview.title}</strong><p>{t(row.preview.mode==='continue'?'继续原生会话':row.preview.mode==='fork_tail'?'从会话末尾分叉':'从指定回答分叉')} · {members.find(member=>member.agentId===row.preview.agentId)?.displayName??row.preview.agentId}</p><p role="status">{row.receipt?t(nativeContinuationStatus[row.receipt.state]):t('只有预览，尚未确认创建')}</p>{row.receipt?.reportedSessionId&&<p>{t('原程序会话编号：{0}',row.receipt.reportedSessionId)}</p>}{row.receipt?.error&&<p className="workbench-error" role="alert">{row.receipt.error}</p>}{row.preview.reason&&<p>{row.preview.reason}</p>}{row.preview.point&&<p>{t('原助手回答：{0}',row.preview.point.selectedTurnId)}</p>}{row.receipt&&<div className="workbench-toolbar"><button disabled={busy} onClick={()=>void reconcile(row.receipt!)}>{t('核对原创建结果')}</button>{row.receipt.state==='binding_recorded'&&onOpen&&<button disabled={busy} onClick={()=>{void onOpen({threadId:row.receipt!.threadId,conversationId:row.receipt!.conversationId}).catch(reason=>setError(readErrorMessage(reason)))}}>{t('打开新工作私聊')}</button>}</div>}<details><summary>{t('来源与准备记录')}</summary><pre className="feedback-preview-text" dir="ltr">{JSON.stringify({source:row.preview.source,previewId:row.preview.previewId,receipt:row.receipt,preparation:row.preparation},null,2)}</pre></details></section>)}{open&&!busy&&!history.items.length&&<p>{t('这份来源还没有继续或分叉记录。')}</p>}{history.next&&<button disabled={busy} onClick={()=>void read(true)}>{t('更多原记录')}</button>}</details>
}
