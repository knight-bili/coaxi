import {useEffect,useRef,useState} from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import type {LocalAttachmentSourceView,NativeSteerMaterial,NativeSteerPreview,NativeSteerReceipt,NativeSteerScope,NativeSteerTarget,SingleChatComposerScope} from '@contracts'
import {newCommandId} from '../../shared/command-id'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'
import './user-feedback.css'
import {NativeNormalContinueDialog} from './NativeNormalContinueDialog'
import {NativeSteerEvidenceDetails} from './NativeSteerEvidenceDetails'

export const nativeSteerStatus:Record<NativeSteerReceipt['status'],string>={prepared:'已登记，正在发送原请求',connection_written:'已写入原生连接',native_acknowledged:'原生程序已明确接收',not_consumed:'原程序未接收，完整输入保留',requires_user_review:'原程序已接收但另开轮次，需要核对',unknown:'结果待核对，完整输入和编号保留'}
type Original={commandId:string;schemaVersion:1;previewId:string;previewDigest:string}
export function NativeSteerDialog({scope,agentId,text,materials=[],composerScope=null,recovery,onClose}:{scope:NativeSteerScope;agentId?:string;text:string;materials?:LocalAttachmentSourceView[];composerScope?:SingleChatComposerScope|null;recovery?:{preview:NativeSteerPreview;receipt:NativeSteerReceipt|null};onClose():void}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0),busyRef=useRef(false),original=useRef<Original|null>(null)
  const [targets,setTargets]=useState<NativeSteerTarget[]>([]),[selected,setSelected]=useState(''),[checked,setChecked]=useState<string[]>([]),[preview,setPreview]=useState<NativeSteerPreview|null>(recovery?.preview??null),[receipt,setReceipt]=useState<NativeSteerReceipt|null>(recovery?.receipt??null)
  const [busy,setBusy]=useState(false),[error,setError]=useState(''),[ready,setReady]=useState(false),[uncertain,setUncertain]=useState(false)
  const [normal,setNormal]=useState(false)
  const scopeKey=JSON.stringify(scope),key=client.presentationScope?`native-steer:${client.presentationScope}:${scopeKey}:${agentId??''}`:null
  const persist=async(value:Original|null):Promise<void>=>{
    if(!key)throw new Error(t('工作区身份尚未确定，本次未发送。'))
    if(client.workbenchEditorRecovery){await client.workbenchEditorRecovery.put({schemaVersion:1,key,editorId:'native-lifecycle',pending:value});return}
    if(!client.editingRecovery)throw new Error(t('原请求恢复存储尚不可用，本次未发送。'))
    client.editingRecovery.set(key,value)
  }
  useEffect(()=>{
    const observed=++generation.current;setReady(false);setBusy(true);busyRef.current=true
    void(async()=>{
      if(!key)throw new Error(t('工作区身份尚未确定，本次未发送。'))
      let raw:unknown
      if(client.workbenchEditorRecovery){const value=await client.workbenchEditorRecovery.get(key);if(value!==null){const record=value as {key?:string;editorId?:string;pending?:unknown};if(record.key!==key||record.editorId!=='native-lifecycle')throw new Error(t('原请求记录归属不同，保留原记录。'));raw=record.pending}}
      else{if(!client.editingRecovery)throw new Error(t('原请求恢复存储尚不可用，本次未发送。'));raw=client.editingRecovery.get(key)}
      if(observed!==generation.current)return
      if(raw!=null){const row=raw as Partial<Original>;if(row.schemaVersion!==1||typeof row.commandId!=='string'||typeof row.previewId!=='string'||typeof row.previewDigest!=='string')throw new Error(t('原请求记录损坏，保留原记录。'));original.current=row as Original;setUncertain(true)}
      if(recovery?.receipt)original.current={schemaVersion:1,commandId:recovery.receipt.commandId,previewId:recovery.preview.previewId,previewDigest:recovery.preview.previewDigest}
      if(!recovery&&!original.current){const value=await client.request<{schemaVersion:1;targets:NativeSteerTarget[]}>('runtime.steer.targets',{schemaVersion:1,scope});if(observed!==generation.current)return;const matches=value.targets.filter(target=>!agentId||target.agentId===agentId);setTargets(matches);if(matches.length===1)setSelected(matches[0].agentRunId)}
      setReady(true)
    })().catch(reason=>{if(observed===generation.current)setError(readErrorMessage(reason))}).finally(()=>{if(observed===generation.current){busyRef.current=false;setBusy(false)}})
    return()=>{generation.current++}
  },[client,key,scopeKey,agentId,recovery])
  const target=targets.find(target=>target.agentRunId===selected)
  const prepare=async():Promise<void>=>{
    if(!target||busyRef.current||!ready||preview||original.current)return
    busyRef.current=true;setBusy(true);setError('');const observed=generation.current
    try{
      let frozen:NativeSteerMaterial[]=[]
      if(checked.length){const value=await client.request<{schemaVersion:1;materials:NativeSteerMaterial[]}>('runtime.steer.materials.prepare',{schemaVersion:1,target,sourceIds:checked,composerScope});frozen=value.materials}
      const value=await client.request<NativeSteerPreview>('runtime.steer.preview',{schemaVersion:1,target,input:{text,materials:frozen}})
      if(observed===generation.current)setPreview(value)
    }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current){busyRef.current=false;setBusy(false)}}
  }
  const settle=async(value:NativeSteerReceipt,observed:number):Promise<void>=>{
    if(value.commandId!==original.current?.commandId)throw new Error(t('原回执的操作归属不同，请保留编号并核对。'))
    if(observed!==generation.current)return
    setReceipt(value);setUncertain(value.status==='unknown'||value.status==='prepared'||value.status==='connection_written')
    if(['native_acknowledged','not_consumed','requires_user_review'].includes(value.status)){await persist(null);if(observed===generation.current)original.current=null}
  }
  const confirm=async():Promise<void>=>{
    if(!preview||busyRef.current||!ready||receipt||original.current)return
    busyRef.current=true;setBusy(true);setError('');const observed=generation.current
    try{const request:Original={schemaVersion:1,commandId:newCommandId(),previewId:preview.previewId,previewDigest:preview.previewDigest};await persist(request);if(observed!==generation.current)return;original.current=request;setUncertain(true);const value=await client.request<NativeSteerReceipt>('runtime.steer.confirm',request);await settle(value,observed)}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current){busyRef.current=false;setBusy(false)}}
  }
  const reconcile=async():Promise<void>=>{
    if(!original.current||busyRef.current)return
    busyRef.current=true;setBusy(true);setError('');const observed=generation.current,commandId=original.current.commandId
    try{const value=await client.request<NativeSteerReceipt>('runtime.steer.receipt',{schemaVersion:1,commandId});await settle(value,observed)}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current){busyRef.current=false;setBusy(false)}}
  }
  if(normal&&receipt)return <NativeNormalContinueDialog sourceCommandId={receipt.commandId} composerScope={composerScope} onClose={()=>setNormal(false)}/>
  return <Dialog.Root open onOpenChange={open=>{if(!open)onClose()}}><Dialog.Portal><Dialog.Overlay className="dialog-overlay app-dialog-overlay"/><Dialog.Content className="dialog-content app-dialog feedback-dialog" aria-describedby={undefined}><header><Dialog.Title>{t('把完整补充送进当前轮次')}</Dialog.Title><Dialog.Close className="quiet-button" aria-label={t('关闭')}>×</Dialog.Close></header><div className="feedback-dialog-body">
    {!preview&&!original.current&&<><label className="feedback-target-label">{t('接收这份补充的当前任务')}<select value={selected} disabled={busy} onChange={event=>{setSelected(event.target.value);setChecked([])}}><option value="">{t('选择队员和任务')}</option>{targets.map(target=><option key={target.agentRunId} value={target.agentRunId}>{target.agentId} · {target.agentRunId}</option>)}</select></label>{!busy&&!targets.length&&<p>{t('当前没有实际支持原生插话且已启用的任务。原文保留，可按原方式继续。')}</p>}<pre className="feedback-preview-text" dir="auto">{text}</pre>{materials.length>0&&<fieldset className="feedback-card-fields"><legend>{t('只发送你勾选的当前卡片材料')}</legend>{materials.map(material=><label key={material.id}><input type="checkbox" checked={checked.includes(material.id)} disabled={busy||material.kind!=='file'||material.availability!=='available'} onChange={event=>setChecked(current=>event.target.checked?[...current,material.id]:current.filter(id=>id!==material.id))}/>{material.displayName}</label>)}</fieldset>}</>}
    {preview&&<><dl className="feedback-preview-target"><div><dt>{t('队员')}</dt><dd>{preview.target.agentId}</dd></div><div><dt>{t('任务编号')}</dt><dd>{preview.target.agentRunId}</dd></div></dl><p>{t('以下是将发送的完整内容')}</p><pre className="feedback-preview-text" dir="auto">{preview.input.text}</pre>{preview.input.materials.map(material=><p key={material.sourceId}>{material.displayName} · {material.byteSize.toLocaleString()} B · {material.kind}</p>)}<p className="feedback-help">{t('预览有效至 {0}。确认前不会交给模型。',new Date(preview.expiresAt).toLocaleString())}</p></>}
    {receipt&&<p role="status">{t(nativeSteerStatus[receipt.status])} · {t('编号：{0}',receipt.commandId)}</p>}{receipt?.status==='not_consumed'&&<p>{t('整份文字和材料仍保存在原预览。作为普通下一轮发送需要你另行确认。')}</p>}{receipt?.nativeOutcome==='startedNewTurn'&&<p>{t('原程序已消耗这份输入，不要再排队发送；该连接的直接插话已停用。')}</p>}{uncertain&&<p role="status">{t('原结果尚未确定。只核对同一编号，不重发，也不自动换成其他通道。')}</p>}{error&&<p role="alert" className="feedback-error">{error}</p>}
    {receipt&&<NativeSteerEvidenceDetails key={receipt.commandId} commandId={receipt.commandId}/>}
  </div><footer><Dialog.Close className="quiet-button">{t('关闭并保留原文')}</Dialog.Close>{receipt?.status==='not_consumed'&&<button type="button" className="quiet-button" disabled={busy} onClick={()=>setNormal(true)}>{t('预览整份普通下一轮')}</button>}{original.current?<button type="button" className="primary-button" disabled={busy} onClick={()=>void reconcile()}>{t(busy?'正在核对…':'核对原提交')}</button>:!receipt&&(!preview?<button type="button" className="primary-button" disabled={busy||!ready||!target||!text.trim()&&!checked.length||Array.from(text).length>4096} onClick={()=>void prepare()}>{t('查看完整内容')}</button>:<button type="button" className="primary-button" disabled={busy||!ready} onClick={()=>void confirm()}>{t('确认发送完整补充')}</button>)}</footer></Dialog.Content></Dialog.Portal></Dialog.Root>
}
