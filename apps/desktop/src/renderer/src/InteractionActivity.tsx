import { useEffect, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Bot, Square } from 'lucide-react'
import { AppDialogBody,AppDialogContent,AppDialogHeader } from './AppDialog'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import './interaction-workspace.css'

interface Entry {
  binding:{agentRunId:string;agentId:string;scope:{threadId:string;scopeKind:string;conversationId:string|null}};
  call:{bridgeCallId:string;toolName:string;argumentsDigest:string;arguments:Record<string,unknown>};
  receipt:{state:'prepared'|'started'|'unknown';admittedAt:string}
}
/** 依据 Codeg BrowserAgentActivity 的操作记录与 Rovai 审批弹窗；原代码始终完整展示。 */
export function InteractionActivity({threadId}:{threadId:string}):React.JSX.Element|null{
  const api=useThreadClient().interactionWorkspace,t=useUiText()
  const [open,setOpen]=useState(false),[items,setItems]=useState<Entry[]>([]),[busy,setBusy]=useState<string|null>(null),[error,setError]=useState('')
  const generation=useRef(0)
  const load=async(epoch=generation.current):Promise<void>=>{
    if(!api)return
    const result=await api.request<{items:Entry[]}>('pending',{})
    if(epoch===generation.current)setItems(result.items.filter(item=>item.binding.scope.threadId===threadId))
  }
  useEffect(()=>{const epoch=++generation.current;setItems([]);setError('');let active=true;const refresh=():void=>{void load(epoch).catch(error=>{if(active&&open)setError(String(error))})};refresh();const timer=window.setInterval(refresh,2000);return()=>{active=false;generation.current++;window.clearInterval(timer)}},[api,threadId,open])
  if(!api)return null
  const waiting=items.filter(item=>item.call.toolName==='browser_eval'&&item.receipt.state==='prepared'||item.receipt.state==='unknown').length
  const run=async(id:string,action:()=>Promise<unknown>):Promise<void>=>{
    if(busy)return
    const epoch=generation.current;setBusy(id);setError('')
    try{await action();await load(epoch)}catch(error){if(epoch===generation.current)setError(String(error))}finally{if(epoch===generation.current)setBusy(null)}
  }
  return <Dialog.Root open={open} onOpenChange={value=>{if(!busy)setOpen(value)}}>
    <Dialog.Trigger asChild><button type="button"><Bot size={14}/>{t('队员操作')}{waiting>0?` · ${waiting}`:''}</button></Dialog.Trigger>
    <Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent tone="neutral" width="wide"><AppDialogHeader title={t('队员正在使用的资源')} description={t('每段网页代码都单独确认。停止会收回后续操作资格；已经发生的点击或输入需要核对实际结果。')} closeDisabled={Boolean(busy)}/>
      <AppDialogBody>{error&&<p className="environment-error" role="alert">{error}</p>}
        <div className="interaction-request-list">{items.map(item=>{
          const id=item.call.bridgeCallId,evalRequest=item.call.toolName==='browser_eval',unknown=item.receipt.state==='unknown'
          return <section className="interaction-request" key={id}>
            <header><strong>{t(evalRequest?'运行网页代码':item.call.toolName.startsWith('browser_')?'浏览器操作':'电脑操作')}</strong><span>{t(item.binding.scope.scopeKind==='public'?'公共会话':item.binding.scope.scopeKind==='single_chat'?'私聊':'独立帮手')} · {new Date(item.receipt.admittedAt).toLocaleTimeString()}</span><span>{t(unknown?'完成情况待核对':item.receipt.state==='started'?'正在执行':evalRequest?'等待你的确认':'等待执行')}</span></header>
            {evalRequest?<pre>{String(item.call.arguments.code??'')}</pre>:<details><summary>{t('查看原操作')}</summary><pre>{JSON.stringify(item.call.arguments,null,2)}</pre></details>}
            <div className="interaction-actions">
              {evalRequest&&item.receipt.state==='prepared'&&<button type="button" disabled={Boolean(busy)} onClick={()=>void run(id,()=>api.request('eval.approve',{bridgeCallId:id,argumentsDigest:item.call.argumentsDigest}))}>{t('允许运行这段代码')}</button>}
              {unknown?<button type="button" disabled={Boolean(busy)} onClick={()=>void run(id,()=>api.request('unknown.reconcile',{bridgeCallId:id,argumentsDigest:item.call.argumentsDigest}))}>{t('已核对实际结果，允许继续')}</button>:<button type="button" disabled={Boolean(busy)} onClick={()=>void run(id,()=>api.request('stop',{agentRunId:item.binding.agentRunId}))}><Square size={13}/>{t('停止本轮资源操作')}</button>}
            </div>
          </section>
        })}{!items.length&&<p className="interaction-help">{t('当前没有等待确认或核对的资源操作。')}</p>}</div>
      </AppDialogBody>
    </AppDialogContent></Dialog.Portal>
  </Dialog.Root>
}
