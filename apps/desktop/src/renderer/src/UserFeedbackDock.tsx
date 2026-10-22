import { useEffect,useRef,useState } from 'react'
import type { FeedbackList,FeedbackPreview,FeedbackPreviewRecoveryList,FeedbackScope,LocalAttachmentSourceView,NativeSteerRecoveryPage,SingleChatComposerScope } from '@contracts'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { FeedbackSubmitDialog,feedbackStatusText,type FeedbackDraft } from './FeedbackSubmitDialog'
import { FeedbackNormalContinueDialog,type FeedbackPrivateInput } from './FeedbackNormalContinueDialog'
import { NativeSteerDialog,nativeSteerStatus } from './NativeSteerDialog'
import './user-feedback.css'

/** 工作中补充及原反馈记录是独立输入，不读取或清空普通消息的附件和草稿。 */
export function UserFeedbackDock({scope,members,agentId,getPrivateInput,onContinued,materials=[],composerScope=null}:{scope:FeedbackScope;members:Array<{agentId:string;displayName:string}>;agentId?:string;getPrivateInput?():FeedbackPrivateInput;onContinued?():void;materials?:LocalAttachmentSourceView[];composerScope?:SingleChatComposerScope|null}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0)
  const [open,setOpen]=useState(false),[text,setText]=useState(''),[selected,setSelected]=useState(agentId??''),[records,setRecords]=useState<FeedbackList>({items:[],next:null}),[previews,setPreviews]=useState<FeedbackPreviewRecoveryList>({items:[],next:null})
  const [draft,setDraft]=useState<FeedbackDraft|null>(null),[recovery,setRecovery]=useState<FeedbackPreview|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState(''),[refresh,setRefresh]=useState(0)
  const [continuation,setContinuation]=useState<string|null>(null)
  const [nativeOpen,setNativeOpen]=useState(false),[nativeRecords,setNativeRecords]=useState<NativeSteerRecoveryPage>({schemaVersion:1,items:[],next:null}),[nativeRecovery,setNativeRecovery]=useState<NativeSteerRecoveryPage['items'][number]|null>(null),[nativeError,setNativeError]=useState('')
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
  useEffect(()=>{const observed=generation.current;setNativeRecords({schemaVersion:1,items:[],next:null});setNativeOpen(false);setNativeRecovery(null);setNativeError('');if(open)void client.request<NativeSteerRecoveryPage>('runtime.steer.previews',{schemaVersion:1,scope,after:null,limit:50}).then(value=>{if(observed===generation.current)setNativeRecords(value)}).catch(reason=>{if(observed===generation.current)setNativeError(readErrorMessage(reason))})},[client,scopeKey,target,open,refresh])
  const moreNative=async():Promise<void>=>{if(!nativeRecords.next||busy)return;const observed=generation.current;setBusy(true);try{const value=await client.request<NativeSteerRecoveryPage>('runtime.steer.previews',{schemaVersion:1,scope,after:nativeRecords.next,limit:50});if(observed===generation.current)setNativeRecords(old=>({...value,items:[...old.items,...value.items.filter(row=>!old.items.some(item=>item.preview.previewId===row.preview.previewId))]}))}catch(reason){if(observed===generation.current)setNativeError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}}
  useEffect(()=>{if(!open)return;let timer:ReturnType<typeof setTimeout>|null=null;const schedule=()=>{if(timer||document.hidden)return;timer=setTimeout(()=>{timer=null;setRefresh(value=>value+1)},300)};const off=client.onEvent?.(event=>{if(event.method==='feedback.changed'||event.method==='nativeLifecycle.changed'||event.method==='agent_run.terminal')schedule()});const invalidated=client.onInvalidated?.(schedule);return()=>{off?.();invalidated?.();if(timer)clearTimeout(timer)}},[client,open])
  return <details className="feedback-dock" open={open} onToggle={event=>setOpen(event.currentTarget.open)}><summary>{t('工作中补充和反馈记录')}</summary><div className="feedback-dock-body">
    {!agentId&&<label>{t('查看哪位队员的反馈')}<select value={selected} onChange={event=>setSelected(event.target.value)}><option value="">{t('选择队员')}</option>{members.map(member=><option key={member.agentId} value={member.agentId}>{member.displayName}</option>)}</select></label>}
    <textarea value={text} aria-label={t('补充说明')} placeholder={t('例如：按钮用蓝色，先不要加动画。')} onChange={event=>setText(event.target.value)}/>
    <div className="feedback-dock-actions"><button type="button" className="quiet-button" disabled={!text.trim()||Array.from(text).length>4096||!target} onClick={()=>setDraft({source:{kind:'live_note'},text,title:'',fields:[]})}>{t('交给正在工作的队员')}</button><button type="button" className="quiet-button" disabled={busy||!target} onClick={()=>setRefresh(value=>value+1)}>{t(busy?'正在核对…':'核对记录')}</button><span>{t('{0}/4096 字符',Array.from(text).length)}</span></div>
    <div className="feedback-dock-actions"><button type="button" className="quiet-button" disabled={Array.from(text).length>4096||!target||!text.trim()&&!materials.some(material=>material.kind==='file'&&material.availability==='available')} onClick={()=>setNativeOpen(true)}>{t('预览原生插话')}</button><small>{t('可明确选择当前卡片材料；普通输入仍保留。')}</small></div>
    <p className="feedback-help">{t('先预览完整内容，再提交。队员检查反馈时才会收到；普通输入框继续按原排队规则处理。')}</p>
    {error&&<p className="feedback-error" role="alert">{error}</p>}
    {previews.items.filter(row=>!row.receipt).map(row=><div className="feedback-history-item" key={row.preview.previewId}><strong>{t('有一份原预览可以继续核对')}</strong><pre>{row.preview.text}</pre><button className="quiet-button" type="button" onClick={()=>setRecovery(row.preview)}>{t('查看原预览')}</button></div>)}
    {records.items.map(row=><div className="feedback-history-item" key={row.receipt.feedbackId}><strong>{t(feedbackStatusText[row.receipt.status])}</strong><span> · {new Date(row.createdAt).toLocaleString()}</span><pre>{row.text}</pre><small>{t('编号：{0}',row.receipt.feedbackId)}</small>{(row.receipt.status==='unknown'||row.receipt.status==='response_prepared')&&<p>{t('原结果尚未确定，不会自动重复发送或转给新任务。')}</p>}{row.receipt.status==='requires_user_review'&&<p>{t('原任务已经变化；原文保留，普通消息续办需要重新确认。')}</p>}{['pending','requires_user_review'].includes(row.receipt.status)&&row.receipt.deliveryChannel==='none'&&!row.receipt.bridgeInstanceId&&!row.receipt.resultDigest&&<button type="button" className="quiet-button" disabled={scope.scopeKind!=='public'&&!getPrivateInput} onClick={()=>setContinuation(row.receipt.feedbackId)}>{t('预览普通消息续办')}</button>}</div>)}
    {(records.next||previews.next)&&<button className="quiet-button" type="button" disabled={busy} onClick={()=>void read(true)}>{t('更多记录')}</button>}
    {nativeError&&<p role="alert" className="feedback-error">{nativeError}</p>}
    {nativeRecords.items.filter(row=>!target||row.preview.target.agentId===target).map(row=><div className="feedback-history-item" key={`native:${row.preview.previewId}`}><strong>{t(row.receipt?nativeSteerStatus[row.receipt.status]:'有一份原生插话预览保留')}</strong><pre>{row.preview.input.text}</pre>{row.preview.input.materials.map(material=><small key={material.sourceId}>{material.displayName} · {material.byteSize.toLocaleString()} B </small>)}<button type="button" className="quiet-button" onClick={()=>setNativeRecovery(row)}>{t('查看完整原预览及回执')}</button></div>)}
    {nativeRecords.next&&<button type="button" className="quiet-button" disabled={busy} onClick={()=>void moreNative()}>{t('更多原生插话记录')}</button>}
    {(nativeOpen||nativeRecovery)&&<NativeSteerDialog scope={scope} agentId={target||undefined} text={text} materials={materials} composerScope={composerScope} recovery={nativeRecovery??undefined} onClose={()=>{setNativeOpen(false);setNativeRecovery(null);setRefresh(value=>value+1)}}/>}
  </div>{(draft||recovery)&&<FeedbackSubmitDialog scope={scope} agentId={target||undefined} draft={draft??undefined} recovery={recovery??undefined} onClose={()=>{setDraft(null);setRecovery(null);setRefresh(value=>value+1)}} onSubmitted={()=>{if(draft?.source.kind==='live_note')setText(current=>current===draft.text?'':current);setRefresh(value=>value+1)}}/>}{continuation&&<FeedbackNormalContinueDialog key={continuation} scope={scope} agentId={target} feedbackId={continuation} getPrivateInput={getPrivateInput} onClose={()=>{setContinuation(null);setRefresh(value=>value+1)}} onContinued={()=>{setRefresh(value=>value+1);onContinued?.()}}/>}</details>
}
