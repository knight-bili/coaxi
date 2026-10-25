import { useEffect, useRef, useState } from 'react'
import * as Popover from '@radix-ui/react-popover'
import { Bot, Eye, ShieldOff } from 'lucide-react'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { newCommandId } from '../../shared/command-id'

type Scope={threadId:string;conversationId:string|null;scopeKind:'public'|'single_chat'|'delegation_child'}
interface Target{id:string;agentId:string;displayName:string;scope:Scope}
interface Grant{grantId:string;agentId:string;targetId:string;targetKind:string;origin:string|null;level:'read'|'control';grantGeneration:number;expiresAt:string}
export function BrowserSharePopover({threadId,targetId,origin}:{threadId:string;targetId:string;origin:string|null}):React.JSX.Element{
  const api=useThreadClient().interactionWorkspace,t=useUiText(),epoch=useRef(0),lock=useRef(false)
  const [open,setOpen]=useState(false),[targets,setTargets]=useState<Target[]>([]),[byTarget,setByTarget]=useState<Record<string,Grant[]>>({})
  const [mode,setMode]=useState<'all'|'selected'>('all'),[selected,setSelected]=useState<string[]>([]),[level,setLevel]=useState<'read'|'control'>('read'),[minutes,setMinutes]=useState(30)
  const [busy,setBusy]=useState(false),[error,setError]=useState(''),[uncertain,setUncertain]=useState(false)
  const refresh=async(generation=epoch.current):Promise<void>=>{
    if(!api)return
    const rows:Target[]=[];let after:string|null=null
    do{const page:{targets:Target[];next:string|null}=await api.request<{targets:Target[];next:string|null}>('sharing.targets',{threadId,after});rows.push(...page.targets.filter(row=>row.scope.threadId===threadId&&row.scope.scopeKind==='public'));after=page.next;if(rows.length>1000)throw Error('可共享队员数量超过本次读取范围。')}while(after)
    const unique=[...new Map(rows.map(row=>[row.id,row])).values()]
    const grants=await Promise.all(unique.map(async row=>[row.id,(await api.request<{grants:Grant[]}>('grant.list',{scope:row.scope,agentId:row.agentId})).grants] as const))
    if(epoch.current===generation){setTargets(unique);setByTarget(Object.fromEntries(grants));setUncertain(false)}
  }
  useEffect(()=>{const generation=++epoch.current;setByTarget({});setTargets([]);setError('');setUncertain(false);void refresh(generation).catch(failure=>{if(epoch.current===generation)setError(readErrorMessage(failure))});return()=>{epoch.current++}},[api,threadId,targetId,origin,open])
  const live=(row:Target):Grant[] => (byTarget[row.id]??[]).filter(grant=>grant.targetKind==='browser_tab'&&grant.targetId===targetId&&grant.origin===origin&&Date.parse(grant.expiresAt)>Date.now())
  const count=targets.filter(row=>live(row).length).length
  const apply=async(stop=false):Promise<void>=>{
    if(!api||lock.current||uncertain||!origin)return
    lock.current=true;setBusy(true);setError('');const generation=epoch.current
    const chosen=mode==='all'?targets:targets.filter(row=>selected.includes(row.id))
    // Freeze the intended recipients before the first write. A failed batch is read back, never replayed silently.
    const removed=stop?targets:targets.filter(row=>!chosen.some(target=>target.id===row.id))
    const revocations=removed.flatMap(row=>live(row).map(grant=>({action:'grant.revoke' as const,payload:{grantId:grant.grantId} as Record<string,unknown>})))
    const requests=stop?targets.flatMap(row=>live(row).map(grant=>({action:'grant.revoke' as const,payload:{grantId:grant.grantId} as Record<string,unknown>}))):chosen.map(row=>{
      const current=live(row)[0],last=(byTarget[row.id]??[]).filter(grant=>grant.targetKind==='browser_tab'&&grant.targetId===targetId).reduce((max,grant)=>Math.max(max,grant.grantGeneration),0)
      return {action:'grant.share' as const,payload:{schemaVersion:1,grantId:current?.grantId??newCommandId(),scope:row.scope,agentId:row.agentId,desktopPeerId:'',targetKind:'browser_tab',targetId,origin,level:mode==='all'?'read':level,grantGeneration:last+1,expiresAt:new Date(Date.now()+minutes*60000).toISOString()} as Record<string,unknown>}
    })
    try{for(const request of stop?requests:[...revocations,...requests])await api.request(request.action,request.payload);await refresh(generation)}catch(failure){if(epoch.current===generation){setError(readErrorMessage(failure));setUncertain(true)}}finally{lock.current=false;if(epoch.current===generation)setBusy(false)}
  }
  return <Popover.Root open={open} onOpenChange={value=>{if(!busy)setOpen(value)}}><Popover.Trigger asChild><button type="button" className={count?'browser-share-pill is-shared':'browser-share-pill'} disabled={!api||!origin} aria-label={t('页面共享权限')}><Bot size={14}/>{t(count?'已共享 · {0} 位队员':'未共享',String(count))}</button></Popover.Trigger><Popover.Portal><Popover.Content className="workspace-popover browser-share-popover" align="start" sideOffset={8}>
    <strong>{t('把当前页面共享给队员')}</strong><p>{origin}</p><p>{t('仅限当前公开对话。新加入的队员和私聊需要重新授权，离开此网站后共享结束。')}</p>
    <label>{t('共享对象')}<select className="workspace-field" value={mode} disabled={busy||uncertain} onChange={event=>{setMode(event.target.value as typeof mode);setLevel('read')}}><option value="all">{t('当前全体队员 · 只读')}</option><option value="selected">{t('指定队员')}</option></select></label>
    {mode==='selected'&&<div className="browser-share-recipients">{targets.map(row=><label key={row.id}><input type="checkbox" checked={selected.includes(row.id)} disabled={busy||uncertain} onChange={event=>setSelected(previous=>event.target.checked?[...previous,row.id]:previous.filter(id=>id!==row.id))}/><span>{row.displayName}</span><small>{t(live(row)[0]?.level==='control'?'可操作':live(row).length?'只读':'未共享')}</small></label>)}</div>}
    {!targets.length&&<p>{t('当前没有可授权的队员会话。请先在本对话中启动队员，再刷新。')}</p>}
    <div className="browser-share-options">{mode==='selected'&&<label>{t('权限')}<select className="workspace-field" value={level} disabled={busy||uncertain} onChange={event=>setLevel(event.target.value as typeof level)}><option value="read">{t('只读')}</option><option value="control">{t('可操作')}</option></select></label>}<label>{t('有效时间')}<select className="workspace-field" value={minutes} disabled={busy||uncertain} onChange={event=>setMinutes(Number(event.target.value))}>{[1,15,30,60,120].map(n=><option value={n} key={n}>{n} {t('分钟')}</option>)}</select></label></div>
    {error&&<p role="alert" className="workbench-error">{error}</p>}{uncertain&&<p>{t('部分操作结果未确认。先核对当前授权，再决定下一步。')}</p>}
    <div className="workspace-popover-actions"><button type="button" className="quiet-button" disabled={busy} onClick={()=>{setBusy(true);void refresh().then(()=>setError('')).catch(failure=>setError(readErrorMessage(failure))).finally(()=>setBusy(false))}}>{t(uncertain?'核对当前授权':'刷新')}</button><button type="button" className="quiet-button" disabled={busy||uncertain||!count} onClick={()=>void apply(true)}><ShieldOff size={14}/>{t('停止共享')}</button><button type="button" className="primary-button" disabled={busy||uncertain||!targets.length||mode==='selected'&&!selected.length} onClick={()=>void apply()}><Eye size={14}/>{t('应用共享')}</button></div>
  </Popover.Content></Popover.Portal></Popover.Root>
}
