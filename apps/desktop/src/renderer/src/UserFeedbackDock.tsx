import { useEffect,useRef,useState } from 'react'
import type { FeedbackList,FeedbackPreview,FeedbackPreviewRecoveryList,FeedbackScope } from '@contracts'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { FeedbackSubmitDialog,feedbackStatusText,type FeedbackDraft } from './FeedbackSubmitDialog'
import './user-feedback.css'

/** 工作中补充及原反馈记录是独立输入，不读取或清空普通消息的附件和草稿。 */
export function UserFeedbackDock({scope,members,agentId}:{scope:FeedbackScope;members:Array<{agentId:string;displayName:string}>;agentId?:string}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0)
  const [open,setOpen]=useState(false),[text,setText]=useState(''),[selected,setSelected]=useState(agentId??''),[records,setRecords]=useState<FeedbackList>({items:[],next:null}),[previews,setPreviews]=useState<FeedbackPreviewRecoveryList>({items:[],next:null})
  const [draft,setDraft]=useState<FeedbackDraft|null>(null),[recovery,setRecovery]=useState<FeedbackPreview|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState(''),[refresh,setRefresh]=useState(0)
  const scopeKey=JSON.stringify(scope),target=agentId??selected
  useEffect(()=>{setText('');setDraft(null);setRecovery(null);setRecords({items:[],next:null});setPreviews({items:[],next:null});setSelected(agentId??'')},[scopeKey,agentId])
  const read=async(more=false):Promise<void>=>{
    if(!target)return
    const observed=++generation.current;setBusy(true);setError('')
    const request=<T,>(action:string,after:string|null)=>client.request<T>('feedback.operation',{action,payload:{scope,agentId:target,after}})
    try{const [a,b]=await Promise.all([request<FeedbackList>('list',more?records.next:null),request<FeedbackPreviewRecoveryList>('previews.list',more?previews.next:null)]);if(observed!==generation.current)return
      setRecords(previous=>more?{...a,items:[...previous.items,...a.items.filter(row=>!previous.items.some(old=>old.receipt.feedbackId===row.receipt.feedbackId))]}:a)
      setPreviews(previous=>more?{...b,items:[...previous.items,...b.items.filter(row=>!previous.items.some(old=>old.preview.previewId===row.preview.previewId))]}:b)
    }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  useEffect(()=>{generation.current++;setRecords({items:[],next:null});setPreviews({items:[],next:null});if(open&&target)void read();return()=>{generation.current++}},[client,scopeKey,target,open,refresh])
  useEffect(()=>{if(!open)return;let timer:ReturnType<typeof setTimeout>|null=null;const schedule=()=>{if(timer||document.hidden)return;timer=setTimeout(()=>{timer=null;setRefresh(value=>value+1)},300)};const off=client.onEvent?.(event=>{if(event.method==='feedback.changed'||event.method==='agent_run.terminal')schedule()});const invalidated=client.onInvalidated?.(schedule);return()=>{off?.();invalidated?.();if(timer)clearTimeout(timer)}},[client,open])
  return <details className="feedback-dock" open={open} onToggle={event=>setOpen(event.currentTarget.open)}><summary>{t('工作中补充和反馈记录')}</summary><div className="feedback-dock-body">
    {!agentId&&<label>{t('查看哪位队员的反馈')}<select value={selected} onChange={event=>setSelected(event.target.value)}><option value="">{t('选择队员')}</option>{members.map(member=><option key={member.agentId} value={member.agentId}>{member.displayName}</option>)}</select></label>}
    <textarea value={text} aria-label={t('补充说明')} placeholder={t('例如：按钮用蓝色，先不要加动画。')} onChange={event=>setText(event.target.value)}/>
    <div className="feedback-dock-actions"><button type="button" className="quiet-button" disabled={!text.trim()||Array.from(text).length>4096||!target} onClick={()=>setDraft({source:{kind:'live_note'},text,title:'',fields:[]})}>{t('交给正在工作的队员')}</button><button type="button" className="quiet-button" disabled={busy||!target} onClick={()=>setRefresh(value=>value+1)}>{t(busy?'正在核对…':'核对记录')}</button><span>{t('{0}/4096 字符',Array.from(text).length)}</span></div>
    <p className="feedback-help">{t('先预览完整内容，再提交。队员检查反馈时才会收到；普通输入框继续按原排队规则处理。')}</p>
    {error&&<p className="feedback-error" role="alert">{error}</p>}
    {previews.items.filter(row=>!row.receipt).map(row=><div className="feedback-history-item" key={row.preview.previewId}><strong>{t('有一份原预览可以继续核对')}</strong><pre>{row.preview.text}</pre><button className="quiet-button" type="button" onClick={()=>setRecovery(row.preview)}>{t('查看原预览')}</button></div>)}
    {records.items.map(row=><div className="feedback-history-item" key={row.receipt.feedbackId}><strong>{t(feedbackStatusText[row.receipt.status])}</strong><span> · {new Date(row.createdAt).toLocaleString()}</span><pre>{row.text}</pre><small>{t('编号：{0}',row.receipt.feedbackId)}</small>{(row.receipt.status==='unknown'||row.receipt.status==='response_prepared')&&<p>{t('原结果尚未确定，不会自动重复发送或转给新任务。')}</p>}{row.receipt.status==='requires_user_review'&&<p>{t('原任务已经变化；原文保留，普通消息续办需要重新确认。')}</p>}</div>)}
    {(records.next||previews.next)&&<button className="quiet-button" type="button" disabled={busy} onClick={()=>void read(true)}>{t('更多记录')}</button>}
  </div>{(draft||recovery)&&<FeedbackSubmitDialog scope={scope} agentId={target||undefined} draft={draft??undefined} recovery={recovery??undefined} onClose={()=>{setDraft(null);setRecovery(null);setRefresh(value=>value+1)}} onSubmitted={()=>{if(draft?.source.kind==='live_note')setText(current=>current===draft.text?'':current);setRefresh(value=>value+1)}}/>}</details>
}
