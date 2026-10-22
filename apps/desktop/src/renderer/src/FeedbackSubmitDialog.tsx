import { useEffect,useRef,useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import type { FeedbackPreview,FeedbackPreviewRequest,FeedbackReceipt,FeedbackScope,FeedbackSubmit,FeedbackTarget } from '@contracts'
import { useFeedbackRequest } from './FeedbackRequestProvider'
import { useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import './user-feedback.css'

export const feedbackStatusText:Record<FeedbackReceipt['status'],string>={pending:'等待队员检查反馈',response_prepared:'已准备，正在核对写出结果',connection_written:'已送到智能体连接',unknown:'结果待核对',requires_user_review:'任务已变化，需要重新选择',continued_as_message:'已作为普通消息续办'}
export type FeedbackDraft=Pick<FeedbackPreviewRequest,'source'|'text'|'title'|'fields'>
/** 用户看到 Core 冻结的完整文字后才能提交；重试复用同一预览编号作为命令 UUID。 */
export function FeedbackSubmitDialog({scope,agentId,draft,recovery,onClose,onSubmitted}:{scope:FeedbackScope;agentId?:string;draft?:FeedbackDraft;recovery?:FeedbackPreview;onClose():void;onSubmitted?(receipt:FeedbackReceipt):void}):React.JSX.Element{
  const request=useFeedbackRequest(),t=useUiText(),lifetime=useRef(0)
  const [targets,setTargets]=useState<FeedbackTarget[]>([]),[selected,setSelected]=useState(''),[preview,setPreview]=useState<FeedbackPreview|null>(recovery??null),[receipt,setReceipt]=useState<FeedbackReceipt|null>(null)
  const [busy,setBusy]=useState(false),[error,setError]=useState(''),[uncertain,setUncertain]=useState(false)
  const scopeKey=JSON.stringify(scope)
  useEffect(()=>{const observed=++lifetime.current;if(recovery)return()=>{lifetime.current++};setBusy(true)
    void request<{targets:FeedbackTarget[];truncated:boolean}>('targets',{scope}).then(value=>{if(observed!==lifetime.current)return;const available=value.targets.filter(row=>!agentId||row.agentId===agentId);setTargets(available);if(available.length===1)setSelected(available[0].agentRunId);if(value.truncated)setError(t('任务列表达到读取上限，请在对应私聊中选择队员。'))}).catch(reason=>{if(observed===lifetime.current)setError(readErrorMessage(reason))}).finally(()=>{if(observed===lifetime.current)setBusy(false)});return()=>{lifetime.current++}
  },[request,scopeKey,agentId,recovery])
  const target=targets.find(row=>row.agentRunId===selected)
  const prepare=async():Promise<void>=>{
    if(!target||!draft||busy)return
    const observed=lifetime.current;setBusy(true);setError('')
    try{const value=await request<FeedbackPreview>('preview',{schemaVersion:1,scope:target.scope,agentId:target.agentId,agentRunId:target.agentRunId,executionEpoch:target.executionEpoch,...draft});if(observed===lifetime.current){setPreview(value);setUncertain(false)}}catch(reason){if(observed===lifetime.current)setError(readErrorMessage(reason))}finally{if(observed===lifetime.current)setBusy(false)}
  }
  const submit=async():Promise<void>=>{
    if(!preview||busy||receipt)return
    const original:FeedbackSubmit={schemaVersion:1,commandId:preview.previewId,previewId:preview.previewId,previewDigest:preview.previewDigest};const observed=lifetime.current;setBusy(true);setError('')
    try{const value=await request<FeedbackReceipt>('submit',original);if(observed===lifetime.current){setReceipt(value);setUncertain(false);onSubmitted?.(value)}}catch(reason){if(observed===lifetime.current){setUncertain(true);setError(readErrorMessage(reason))}}finally{if(observed===lifetime.current)setBusy(false)}
  }
  return <Dialog.Root open onOpenChange={value=>{if(!value)onClose()}}><Dialog.Portal><Dialog.Overlay className="app-dialog-overlay"/><Dialog.Content className="app-dialog-content feedback-dialog" aria-describedby={undefined}>
    <header><Dialog.Title>{t('把补充交给队员')}</Dialog.Title><Dialog.Close className="quiet-button" aria-label={t('关闭')}>×</Dialog.Close></header>
    <div className="feedback-dialog-body">
      {!preview&&<><label className="feedback-target-label">{t('接收这份补充的当前任务')}<select value={selected} disabled={busy} onChange={event=>setSelected(event.target.value)}><option value="">{t('选择队员和任务')}</option>{targets.map(row=><option value={row.agentRunId} key={row.agentRunId}>{row.displayName} · {row.feedbackEnabled?t('正在进行的任务'):t('反馈开关未启用')}</option>)}</select></label>{target&&<p>{!target.feedbackEnabled?t('请在队员设置中开启反馈，再从新任务生效；也可以关闭这里，按原方式发送普通消息。'):!target.nativeBindingReady?t('当前任务尚未具备反馈连接。请等待任务就绪，或在开启后新建任务；原任务不会中途增加工具。'):t('先查看将提交的完整内容。队员需要主动检查反馈，才会收到补充。')}</p>}{!busy&&targets.length===0&&<p>{t('这个会话现在没有可接收反馈的运行任务。原输入仍保留，可通过普通消息继续。')}</p>}</>}
      {preview&&<><p>{t('只交给当前会话中的这位队员，不会广播到其他私聊或帮手。')}</p><dl className="feedback-preview-target"><div><dt>{t('队员')}</dt><dd>{target?.displayName??preview.agentId}</dd></div><div><dt>{t('任务编号')}</dt><dd>{preview.agentRunId}</dd></div></dl><p>{t('以下是将发送的完整内容')}</p><pre className="feedback-preview-text" dir="auto">{preview.text}</pre><p className="feedback-help">{t('预览有效至 {0}。确认前不会交给模型。',new Date(preview.expiresAt).toLocaleString())}</p></>}
      {receipt&&<p role="status">{t(feedbackStatusText[receipt.status])} {t('编号：{0}',receipt.feedbackId)}</p>}
      {uncertain&&<p role="status">{t('提交结果尚未确定。原文和编号已保留，请核对原提交；不要另建一份相同反馈。')}</p>}
      {error&&<p role="alert" className="feedback-error">{error}</p>}
    </div>
    <footer><Dialog.Close className="quiet-button">{t(receipt?'完成':'关闭并保留原文')}</Dialog.Close>{!preview?<button type="button" className="primary-button" disabled={busy||!target?.feedbackEnabled||!target.nativeBindingReady} onClick={()=>void prepare()}>{t(busy?'正在准备…':'查看完整内容')}</button>:!receipt&&<button type="button" className="primary-button" disabled={busy} onClick={()=>void submit()}>{t(busy?'正在核对…':uncertain?'核对原提交':'确认交给队员')}</button>}</footer>
  </Dialog.Content></Dialog.Portal></Dialog.Root>
}
