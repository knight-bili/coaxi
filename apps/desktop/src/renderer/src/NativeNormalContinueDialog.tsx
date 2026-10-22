import {useEffect,useRef,useState} from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import type {NativeNormalConfirm,NativeNormalPreview,NativeNormalReceipt,SingleChatComposerScope} from '@contracts'
import {privateConversationOperations} from '../../shared/private-conversation-operations'
import {newCommandId} from '../../shared/command-id'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'
import './user-feedback.css'

type Saved={schemaVersion:1;preview:NativeNormalPreview;request:NativeNormalConfirm}
export function NativeNormalContinueDialog({sourceCommandId,composerScope=null,onClose}:{sourceCommandId:string;composerScope?:SingleChatComposerScope|null;onClose():void}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0),busyRef=useRef(false),original=useRef<NativeNormalConfirm|null>(null)
  const [preview,setPreview]=useState<NativeNormalPreview|null>(null),[receipt,setReceipt]=useState<NativeNormalReceipt|null>(null),[busy,setBusy]=useState(false),[ready,setReady]=useState(false),[error,setError]=useState('')
  const key=client.presentationScope?`native-normal:${client.presentationScope}:${sourceCommandId}`:null
  const keep=async(value:Saved):Promise<void>=>{
    if(!key)throw new Error(t('工作区身份尚未确定，本次未发送。'))
    if(client.workbenchEditorRecovery){await client.workbenchEditorRecovery.put({schemaVersion:1,key,editorId:'native-normal',pending:value});return}
    if(!client.editingRecovery)throw new Error(t('原请求恢复存储尚不可用，本次未发送。'));client.editingRecovery.set(key,value)
  }
  const accept=(value:NativeNormalReceipt,observed:number):void=>{if(value.commandId!==original.current?.commandId)throw new Error(t('原回执的操作归属不同，请保留编号并核对。'));if(observed===generation.current)setReceipt(value)}
  useEffect(()=>{
    const observed=++generation.current;busyRef.current=true;setBusy(true);setReady(false)
    void(async()=>{
      if(!key)throw new Error(t('工作区身份尚未确定，本次未发送。'))
      let raw:unknown
      if(client.workbenchEditorRecovery){const result=await client.workbenchEditorRecovery.get(key);if(result!==null){const row=result as {key?:string;editorId?:string;pending?:unknown};if(row.key!==key||row.editorId!=='native-normal')throw new Error(t('原请求记录归属不同，保留原记录。'));raw=row.pending}}
      else{if(!client.editingRecovery)throw new Error(t('原请求恢复存储尚不可用，本次未发送。'));raw=client.editingRecovery.get(key)}
      if(observed!==generation.current)return
      if(raw!=null){const saved=raw as Partial<Saved>;if(saved.schemaVersion!==1||saved.request?.reference.sourceCommandId!==sourceCommandId||saved.preview?.sourceCommandId!==sourceCommandId||typeof saved.request.commandId!=='string')throw new Error(t('原请求记录损坏，保留原记录。'));original.current=saved.request;setPreview(saved.preview);accept(await client.request<NativeNormalReceipt>('runtime.steer.normal.receipt',{schemaVersion:1,commandId:saved.request.commandId}),observed)}
      else{const value=await client.request<NativeNormalPreview>('runtime.steer.normal.preview',{schemaVersion:1,sourceCommandId});if(observed===generation.current)setPreview(value)}
      if(observed===generation.current)setReady(true)
    })().catch(reason=>{if(observed===generation.current)setError(readErrorMessage(reason))}).finally(()=>{if(observed===generation.current){busyRef.current=false;setBusy(false)}})
    return()=>{generation.current++}
  },[client,key,sourceCommandId])
  const submit=async():Promise<void>=>{
    if(busyRef.current||!ready||!preview)return;busyRef.current=true;setBusy(true);setError('');const observed=generation.current
    try{
      if(original.current){accept(await client.request<NativeNormalReceipt>('runtime.steer.normal.receipt',{schemaVersion:1,commandId:original.current.commandId}),observed);return}
      const request:NativeNormalConfirm={schemaVersion:1,commandId:newCommandId(),reference:{sourceCommandId,previewId:preview.previewId,previewDigest:preview.previewDigest}}
      if(preview.scope.scopeKind!=='public'){
        const operations=privateConversationOperations(preview.scope.scopeKind==='delegation_child'?'delegation_child':'single_chat')
        const value=await client.request<{conversation:{id:string};draft:{revision:number}}>(operations.get,{conversationId:preview.scope.conversationId,composerScope})
        if(value.conversation.id!==preview.scope.conversationId)throw new Error(t('当前私聊已变化，完整输入保留。'))
        request.draftRevision=value.draft.revision;request.composerScope=composerScope
      }
      if(observed!==generation.current)return;await keep({schemaVersion:1,preview,request});if(observed!==generation.current)return;original.current=request
      accept(await client.request<NativeNormalReceipt>('runtime.steer.normal.confirm',request),observed)
    }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current){busyRef.current=false;setBusy(false)}}
  }
  const result=receipt?.commandResult
  return <Dialog.Root open onOpenChange={open=>{if(!open)onClose()}}><Dialog.Portal><Dialog.Overlay className="app-dialog-overlay"/><Dialog.Content className="app-dialog-content feedback-dialog" aria-describedby={undefined}><header><Dialog.Title>{t('将整份补充作为普通下一轮发送')}</Dialog.Title><Dialog.Close className="quiet-button" aria-label={t('关闭')}>×</Dialog.Close></header><div className="feedback-dialog-body"><p>{t('原程序已明确表示没有接收这份输入。本次确认会按对应会话的普通发送和排队规则处理。')}</p>{preview&&<><pre className="feedback-preview-text" dir="auto">{preview.input.text}</pre>{preview.input.materials.map(material=><p key={material.sourceId}>{material.displayName} · {material.byteSize.toLocaleString()} B</p>)}<p>{t('当前普通输入框的正文、材料和引用保持原状。本次只发送以上整份输入。')}</p></>}{receipt&&<p role="status">{t(receipt.state==='recorded'?result?.status==='rejected'?'本次未发送，完整输入保留':'已按普通消息提交或排队':receipt.state==='failed'?'未完成提交，完整输入保留':receipt.state==='prepared'?'已登记，正在处理原请求':'原结果尚未确定，只核对同一编号')} · {receipt.commandId}</p>}{result?.status==='rejected'&&<p role="alert">{result.code}</p>}{receipt?.error&&<p role="alert">{receipt.error}</p>}{error&&<p role="alert" className="feedback-error">{error}</p>}</div><footer><Dialog.Close className="quiet-button">{t('关闭并保留记录')}</Dialog.Close>{receipt?.state!=='recorded'&&receipt?.state!=='failed'&&<button type="button" className="primary-button" disabled={busy||!ready||!preview} onClick={()=>void submit()}>{t(busy?'正在核对…':original.current?'核对原普通提交':'确认发送整份输入')}</button>}</footer></Dialog.Content></Dialog.Portal></Dialog.Root>
}
