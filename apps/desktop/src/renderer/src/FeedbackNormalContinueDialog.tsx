import {useEffect,useRef,useState} from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import type {FeedbackNormalPreview,FeedbackNormalSend,FeedbackNormalResult,FeedbackReceipt,FeedbackScope} from '@contracts'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'
import './user-feedback.css'

export type FeedbackPrivateInput=Pick<FeedbackNormalSend,'draftRevision'|'composerScope'>
/** 一次新确认只创建一条普通消息；结果未知时只读取同一编号的回执。 */
export function FeedbackNormalContinueDialog({scope,agentId,feedbackId,getPrivateInput,onClose,onContinued}:{scope:FeedbackScope;agentId:string;feedbackId:string;getPrivateInput?():FeedbackPrivateInput;onClose():void;onContinued?(receipt:FeedbackReceipt):void}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),lifetime=useRef(0),original=useRef<FeedbackNormalSend|null>(null)
  const [preview,setPreview]=useState<FeedbackNormalPreview|null>(null),[busy,setBusy]=useState(true),[error,setError]=useState(''),[uncertain,setUncertain]=useState(false),[receipt,setReceipt]=useState<FeedbackReceipt|null>(null),[rejected,setRejected]=useState(false)
  const request=<T,>(action:string,payload:unknown)=>client.request<T>('feedback.operation',{action,payload})
  useEffect(()=>{const generation=++lifetime.current;void request<FeedbackNormalPreview>('normal.preview',{scope,agentId,feedbackId}).then(value=>{if(generation===lifetime.current)setPreview(value)}).catch(reason=>{if(generation===lifetime.current)setError(readErrorMessage(reason))}).finally(()=>{if(generation===lifetime.current)setBusy(false)});return()=>{lifetime.current++}},[client,JSON.stringify(scope),agentId,feedbackId])
  const submit=async():Promise<void>=>{
    if(!preview||busy||receipt||rejected)return
    const generation=lifetime.current;setBusy(true);setError('')
    try{
      if(!original.current){const input=scope.scopeKind==='public'?{}:getPrivateInput?.();if(!input)throw new Error(t('请回到对应私聊，先处理普通输入中的草稿和材料。'));original.current={commandId:preview.preview.previewId,reference:preview.reference,scope,agentId,body:preview.preview.text,...input}}
      const value=await request<FeedbackNormalResult>(uncertain?'normal.reconcile':'normal.send',original.current)
      if(generation!==lifetime.current)return
      if(value.state==='unknown'){setUncertain(true);setError(t('还没有查到原请求的结果，原文保留；请稍后继续核对。'));return}
      setUncertain(false)
      if(value.commandResult.status==='rejected'){setRejected(true);setError(t('这次续办没有发送：{0}。原反馈仍保留，请关闭后处理当前会话，再重新预览。',value.commandResult.code));return}
      setReceipt(value.receipt);onContinued?.(value.receipt)
    }catch(reason){if(generation===lifetime.current){if(original.current)setUncertain(true);setError(readErrorMessage(reason))}}finally{if(generation===lifetime.current)setBusy(false)}
  }
  return <Dialog.Root open onOpenChange={value=>{if(!value)onClose()}}><Dialog.Portal><Dialog.Overlay className="app-dialog-overlay"/><Dialog.Content className="app-dialog-content feedback-dialog" aria-describedby={undefined}>
    <header><Dialog.Title>{t('改为普通消息继续')}</Dialog.Title><Dialog.Close className="quiet-button" aria-label={t('关闭')}>×</Dialog.Close></header>
    <div className="feedback-dialog-body"><p>{t('这份反馈尚未尝试交给智能体连接。确认后，它会按当前会话的普通消息和排队规则继续处理。')}</p>{preview&&<><p>{t('将发送的完整内容')}</p><pre className="feedback-preview-text" dir="auto">{preview.preview.text}</pre><p className="feedback-help">{t('这条新消息不会自动携带输入框里的其他材料；当前权限和审批仍生效。')}</p></>}{uncertain&&<p role="status">{t('结果尚未确定，现在只核对原请求，不会重复发送。')}</p>}{receipt&&<p role="status">{t(receipt.continuedMessageId?'已作为普通消息续办':'已作为普通消息排队')}{receipt.continuedMessageId&&<small> · {receipt.continuedMessageId}</small>}</p>}{error&&<p role="alert" className="feedback-error">{error}</p>}</div>
    <footer><Dialog.Close className="quiet-button">{t(receipt?'完成':'关闭并保留原文')}</Dialog.Close>{!receipt&&!rejected&&<button type="button" className="primary-button" disabled={busy||!preview} onClick={()=>void submit()}>{t(busy?'正在核对…':uncertain?'核对原请求':'确认作为普通消息发送')}</button>}</footer>
  </Dialog.Content></Dialog.Portal></Dialog.Root>
}
