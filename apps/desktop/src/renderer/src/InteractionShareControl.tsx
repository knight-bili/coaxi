import { useEffect, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Bot, Eye, MousePointerClick, ShieldOff } from 'lucide-react'
import { AppDialogBody,AppDialogContent,AppDialogFooter,AppDialogHeader } from './AppDialog'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { newCommandId } from '../../shared/command-id'
import './interaction-workspace.css'

type Scope={threadId:string;conversationId:string|null;scopeKind:'public'|'single_chat'|'delegation_child'}
interface Target{id:string;agentId:string;displayName:string;createdAt:string;scope:Scope}
interface Grant{schemaVersion:1;grantId:string;scope:Scope;agentId:string;desktopPeerId:string;targetKind:string;targetId:string;origin:string|null;level:'read'|'control';grantGeneration:number;expiresAt:string}
export function InteractionShareControl({threadId,targetId,targetKind,origin=null,disabled=false}:{threadId:string;targetId:string;targetKind:'browser_tab'|'computer_window'|'computer_app'|'computer_desktop';origin?:string|null;disabled?:boolean}):React.JSX.Element|null{
  const api=useThreadClient().interactionWorkspace,t=useUiText(),sequence=useRef(0)
  const [open,setOpen]=useState(false),[targets,setTargets]=useState<Target[]>([]),[next,setNext]=useState<string|null>(null),[selected,setSelected]=useState(''),[grants,setGrants]=useState<Grant[]>([]),[minutes,setMinutes]=useState(30),[busy,setBusy]=useState(false),[error,setError]=useState('')
  const [pending,setPending]=useState<{action:'grant.share'|'grant.revoke';payload:Record<string,unknown>}|null>(null)
  const target=targets.find(target=>target.id===selected)
  const current=grants.find(grant=>grant.targetId===targetId&&grant.targetKind===targetKind&&Date.parse(grant.expiresAt)>Date.now())
  useEffect(()=>{const generation=++sequence.current;setSelected('');setGrants([]);setPending(null);setTargets([]);setNext(null);setError('');if(open)void api?.request<{targets:Target[];next:string|null}>('sharing.targets',{threadId,after:null}).then(value=>{if(sequence.current===generation){setTargets(value.targets);setNext(value.next)}}).catch(error=>{if(sequence.current===generation)setError(String(error))});return()=>{sequence.current++}},[api,open,threadId,targetId,targetKind])
  useEffect(()=>{const generation=++sequence.current;setGrants([]);if(!open||!target||!api)return;void api.request<{grants:Grant[]}>('grant.list',{scope:target.scope,agentId:target.agentId}).then(value=>{if(sequence.current===generation)setGrants(value.grants)}).catch(error=>{if(sequence.current===generation)setError(String(error))})},[api,open,target])
  if(!api)return null
  const commit=async(request:NonNullable<typeof pending>):Promise<void>=>{
    if(busy)return
    const generation=sequence.current;setBusy(true);setError('');setPending(request)
    try{await api.request(request.action,request.payload);if(sequence.current===generation){setPending(null);if(target)setGrants((await api.request<{grants:Grant[]}>('grant.list',{scope:target.scope,agentId:target.agentId})).grants)}}catch(error){if(sequence.current===generation)setError(String(error))}finally{setBusy(false)}
  }
  const share=(level:'read'|'control'):void=>{
    if(!target)return
    void commit({action:'grant.share',payload:{schemaVersion:1,grantId:current?.grantId??newCommandId(),scope:target.scope,agentId:target.agentId,desktopPeerId:'',targetKind,targetId,origin,level,grantGeneration:(current?.grantGeneration??0)+1,expiresAt:new Date(Date.now()+minutes*60_000).toISOString()}})
  }
  const label=(target:Target):string=>`${target.displayName} · ${t(target.scope.scopeKind==='public'?'公共会话':target.scope.scopeKind==='single_chat'?'私聊':'独立帮手')} ${target.scope.scopeKind==='public'?'':new Date(target.createdAt).toLocaleString()}`
  return <Dialog.Root open={open} onOpenChange={value=>{if(!busy&&!pending)setOpen(value)}}>
    <Dialog.Trigger asChild><button type="button" disabled={disabled||targetKind==='browser_tab'&&!origin}><Bot size={14}/>{t('共享给队员')}</button></Dialog.Trigger>
    <Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent tone="neutral" width="compact"><AppDialogHeader title={t('选择会话与队员')} description={t('共享只在所选会话和队员中生效。其他卡片和独立帮手需要分别选择。')} closeDisabled={busy||Boolean(pending)}/>
      <AppDialogBody><div className="interaction-share-body">
        <label>{t('使用这个资源的会话')}<select value={selected} disabled={busy||Boolean(pending)} onChange={event=>setSelected(event.target.value)}><option value="">{t('请选择')}</option>{targets.map(target=><option value={target.id} key={target.id}>{label(target)}</option>)}</select></label>
        {next&&<button type="button" disabled={busy} onClick={()=>{const generation=sequence.current;setBusy(true);void api.request<{targets:Target[];next:string|null}>('sharing.targets',{threadId,after:next}).then(value=>{if(sequence.current===generation){setTargets(items=>[...items,...value.targets]);setNext(value.next)}}).catch(error=>setError(String(error))).finally(()=>setBusy(false))}}>{t('更多会话')}</button>}
        <label>{t('共享时长')}<select value={minutes} disabled={busy||Boolean(pending)} onChange={event=>setMinutes(Number(event.target.value))}>{[1,15,30,60,120].map(value=><option key={value} value={value}>{t('{0} 分钟',String(value))}</option>)}</select></label>
        <p className="interaction-help">{current?t('当前允许：{0}，到期时间：{1}',t(current.level==='read'?'只看':'操作'),new Date(current.expiresAt).toLocaleTimeString()):t('当前未共享。')}{origin?` ${origin}`:''}</p>
        {error&&<p role="alert" className="environment-error">{error}</p>}
        {pending&&<div className="interaction-actions"><button type="button" disabled={busy} onClick={()=>void commit(pending)}>{t('原样重试')}</button><button type="button" disabled={busy} onClick={()=>{setPending(null);setSelected('');setError(t('保留服务器当前授权，请重新选择会话核对。'))}}>{t('先核对当前授权')}</button></div>}
      </div></AppDialogBody>
      <AppDialogFooter><button type="button" disabled={busy||!current||Boolean(pending)} onClick={()=>{if(current)void commit({action:'grant.revoke',payload:{grantId:current.grantId}})}}><ShieldOff size={14}/>{t('收回')}</button><button type="button" disabled={busy||!target||Boolean(pending)} onClick={()=>share('read')}><Eye size={14}/>{t('只看')}</button><button type="button" disabled={busy||!target||Boolean(pending)} onClick={()=>share('control')}><MousePointerClick size={14}/>{t('允许操作')}</button></AppDialogFooter>
    </AppDialogContent></Dialog.Portal>
  </Dialog.Root>
}
