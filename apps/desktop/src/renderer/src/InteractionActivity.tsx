import { useEffect, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Bot, Square } from 'lucide-react'
import { AppDialogBody,AppDialogContent,AppDialogHeader } from './AppDialog'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import './interaction-workspace.css'

const localApproval=(name:string):boolean=>['browser_eval','computer_launch_app','computer_clipboard_read','computer_clipboard_write'].includes(name)
interface Entry {
  binding:{agentRunId:string;agentId:string;scope:{threadId:string;scopeKind:string;conversationId:string|null}};
  call:{bridgeCallId:string;toolName:string;argumentsDigest:string;arguments:Record<string,unknown>};
  receipt:{state:'prepared'|'started'|'unknown';admittedAt:string}
}
interface ComputerLine {scopeKey:string;scope?:{threadId:string;scopeKind:string};agentId?:string;targetId:string;action:string;outcome:string;at:number;count:number;app:string|null}
const actionNames:Record<string,string>={capture:'截取画面',snapshot:'读取窗口内容',verify:'核对窗口',click:'点击',drag:'拖动',scroll:'滚动',type:'输入',key:'按键','hold-key':'按住按键','set-value':'设置值',restore:'还原窗口',menu:'选择菜单','set-frame':'调整窗口',launch:'启动应用','clipboard-read':'读取剪贴板','clipboard-write':'写入剪贴板'}
/** 依据 Codeg BrowserAgentActivity 的操作记录与 Coaxi 审批弹窗；原代码始终完整展示。 */
export function InteractionActivity({threadId}:{threadId:string}):React.JSX.Element|null{
  const client=useThreadClient(),api=client.interactionWorkspace,computer=client.interactionComputer,t=useUiText()
  const [open,setOpen]=useState(false),[items,setItems]=useState<Entry[]>([]),[busy,setBusy]=useState<string|null>(null),[error,setError]=useState('')
  const generation=useRef(0)
  const [activity,setActivity]=useState<ComputerLine[]>([])
  const load=async(epoch=generation.current):Promise<void>=>{
    if(!api)return
    const result=await api.request<{items:Entry[]}>('pending',{})
    if(epoch===generation.current)setItems(result.items.filter(item=>item.binding.scope.threadId===threadId))
  }
  useEffect(()=>{const epoch=++generation.current;setItems([]);setError('');let active=true;const refresh=():void=>{void load(epoch).catch(error=>{if(active&&open)setError(String(error))})};refresh();const timer=window.setInterval(refresh,2000);return()=>{active=false;generation.current++;window.clearInterval(timer)}},[api,threadId,open])
  useEffect(()=>{let active=true;setActivity([]);if(!computer||!open)return()=>{active=false};const refresh=():void=>{void computer.request<{activity:ComputerLine[]}>('monitor',{}).then(value=>{if(active)setActivity(value.activity.filter(item=>item.scope?.threadId===threadId))}).catch(error=>{if(active)setError(String(error))})};refresh();const timer=window.setInterval(refresh,1000);return()=>{active=false;window.clearInterval(timer)}},[computer,open,threadId])
  if(!api)return null
  const waiting=items.filter(item=>localApproval(item.call.toolName)&&item.receipt.state==='prepared'||item.receipt.state==='unknown').length
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
            <header><strong>{t(evalRequest?'运行网页代码':item.call.toolName.startsWith('browser_')?'浏览器操作':'电脑操作')}</strong><span>{t(item.binding.scope.scopeKind==='public'?'公共会话':item.binding.scope.scopeKind==='single_chat'?'私聊':'独立帮手')} · {new Date(item.receipt.admittedAt).toLocaleTimeString()}</span><span>{t(unknown?'完成情况待核对':item.receipt.state==='started'?'正在执行':localApproval(item.call.toolName)?'等待你的确认':'等待执行')}</span></header>
            {evalRequest?<pre>{String(item.call.arguments.code??'')}</pre>:<details><summary>{t('查看原操作')}</summary><pre>{JSON.stringify(item.call.arguments,null,2)}</pre></details>}
            <div className="interaction-actions">
              {localApproval(item.call.toolName)&&item.receipt.state==='prepared'&&<button type="button" disabled={Boolean(busy)} onClick={()=>void run(id,()=>api.request('operation.approve',{bridgeCallId:id,argumentsDigest:item.call.argumentsDigest}))}>{t(evalRequest?'允许运行这段代码':item.call.toolName==='computer_launch_app'?'允许这次启动应用':item.call.toolName==='computer_clipboard_read'?'允许这次读取剪贴板':'允许这次写入剪贴板')}</button>}
              {unknown?<button type="button" disabled={Boolean(busy)} onClick={()=>void run(id,()=>api.request('unknown.reconcile',{bridgeCallId:id,argumentsDigest:item.call.argumentsDigest}))}>{t('已核对实际结果，允许继续')}</button>:<button type="button" disabled={Boolean(busy)} onClick={()=>void run(id,()=>api.request('stop',{agentRunId:item.binding.agentRunId}))}><Square size={13}/>{t('停止本轮资源操作')}</button>}
            </div>
          </section>
        })}{!items.length&&<p className="interaction-help">{t('当前没有等待确认或核对的资源操作。')}</p>}</div>
        {activity.length>0&&<section className="interaction-activity"><header><strong>{t('本次打开应用后的电脑操作')}</strong><button type="button" disabled={Boolean(busy)} onClick={()=>void run('clear',async()=>{await computer?.request('activity.clear',{threadId,before:Date.now()});setActivity([])})}>{t('清除记录')}</button></header><p className="interaction-help">{t('重复动作合并显示；清除记录不会收回共享权限。')}</p><ol>{activity.map((line,index)=><li key={`${line.scopeKey}:${line.targetId}:${line.action}:${index}`}><span>{t(actionNames[line.action]??line.action)}{line.count>1?` × ${line.count}`:''}</span><span>{line.app??''}</span><span>{t(line.scope?.scopeKind==='public'?'公共会话':line.scope?.scopeKind==='single_chat'?'私聊':'独立帮手')}</span><span>{t(line.outcome==='done'?'已完成':line.outcome==='refused'?'未获授权':'未完成')}</span><time dateTime={new Date(line.at).toISOString()}>{new Date(line.at).toLocaleTimeString()}</time></li>)}</ol></section>}
      </AppDialogBody>
    </AppDialogContent></Dialog.Portal>
  </Dialog.Root>
}
