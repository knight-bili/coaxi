import { InteractionShareControl } from './InteractionShareControl'
import { useEffect, useState } from 'react'
import { ArrowLeft, ArrowRight, ExternalLink, Globe, Plus, RefreshCw, ShieldOff, X } from 'lucide-react'
import type { BrowserWorkspaceView } from '@contracts'
import { BrowserSettingsDialog } from './BrowserSettingsDialog'
import { BrowserProfiles } from './BrowserProfiles'
import { BrowserDownloadBar } from './BrowserDownloadBar'
import { newCommandId } from '../../shared/command-id'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'

/** Address bar and per-tab share menu follow Codeg browser-toolbar and
 * browser-agent-access. Native pages stay in sandboxed Host-owned windows. */
export function BrowserWorkbench({threadId}:{threadId:string}):React.JSX.Element{
  const api=useThreadClient().browserWorkspace,t=useUiText()
  const [tabs,setTabs]=useState<BrowserWorkspaceView[]>([]),[selected,setSelected]=useState<string|null>(null),[url,setUrl]=useState('http://localhost:3000')
  const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[preview,setPreview]=useState<{tree?:string;image?:string;console?:Array<{level:string;message:string;at:number}>}|null>(null)
  const [profileId,setProfileId]=useState('temporary'),[devtools,setDevtools]=useState(false)
  useEffect(()=>{let cancelled=false;void api?.preferences().then(value=>{if(!cancelled)setDevtools(value.devtools)}).catch(()=>undefined);return()=>{cancelled=true}},[api])
  const current=tabs.find(tab=>tab.id===selected)
  useEffect(()=>{let cancelled=false;const refresh=():void=>{void api?.list(threadId).then(value=>{if(!cancelled)setTabs(value)}).catch(failure=>{if(!cancelled)setError(String(failure))})};refresh();const timer=window.setInterval(refresh,2000);return()=>{cancelled=true;window.clearInterval(timer)}},[api,threadId])
  const run=async(action:()=>Promise<void>):Promise<void>=>{if(busy)return;setBusy(true);setError(null);try{await action()}catch(failure){setError(String(failure))}finally{setBusy(false)}}
  const control=async(action:Parameters<NonNullable<typeof api>['control']>[0]['action'],extra:Record<string,unknown>={}):Promise<void>=>{
    if(!api||!current)return
    const result=await api.control({id:current.id,action,...extra});setPreview(null)
    setTabs(previous=>result?previous.map(tab=>tab.id===result.id?result:tab):previous.filter(tab=>tab.id!==current.id))
  }
  const inspect=async(action:'snapshot'|'screenshot'|'console'):Promise<void>=>{
    if(!api||!current)return
    const result=await api.perform({id:current.id,commandId:newCommandId(),action})
    if(action==='snapshot')setPreview({tree:String((result as {tree?:string}).tree??'')})
    else if(action==='screenshot')setPreview({image:'data:image/png;base64,'+String((result as {data:string}).data)})
    else setPreview({console:result as Array<{level:string;message:string;at:number}>})
  }
  if(!api)return <div className="workbench-empty"><Globe size={28}/><p>{t('浏览器页面由桌面主机管理。请在桌面端打开并选择要共享的页面。')}</p></div>
  return <div className="workbench-browser">
    <form className="workbench-toolbar" onSubmit={event=>{event.preventDefault();void run(async()=>{const tab=await api.open(threadId,url,profileId);setTabs(previous=>[...previous,tab]);setSelected(tab.id);setPreview(null)})}}>
      <BrowserSettingsDialog api={api} onChanged={value=>setDevtools(value.devtools)}/><BrowserProfiles api={api} value={profileId} onChange={setProfileId}/><Globe size={16}/><input aria-label={t('网页地址')} value={url} onChange={event=>setUrl(event.target.value)} placeholder="https://"/>
      <button type="submit" disabled={busy}><Plus size={14}/>{t('新页面')}</button>
    </form>
    {error&&<p className="workbench-error" role="alert">{error}</p>}
    <BrowserDownloadBar key={threadId} api={api} threadId={threadId} tabId={current?.id??null}/>
    <div className="workbench-tabs" role="tablist">{tabs.map(tab=><button type="button" role="tab" aria-selected={tab.id===selected} className={tab.id===selected?'workbench-tab is-active':'workbench-tab'} key={tab.id} title={tab.url} onClick={()=>{setSelected(tab.id);setUrl(tab.url);setPreview(null)}}>{tab.title||tab.url}{tab.level!=='none'?' · '+t(tab.level==='read'?'允许查看':'允许操作'):''}</button>)}</div>
    {current?<>
      <div className="workbench-toolbar">
        <button type="button" disabled={busy} aria-label={t('后退')} onClick={()=>void run(()=>control('back'))}><ArrowLeft size={14}/></button>
        <button type="button" disabled={busy} aria-label={t('前进')} onClick={()=>void run(()=>control('forward'))}><ArrowRight size={14}/></button>
        <button type="button" disabled={busy} aria-label={t('重新加载')} onClick={()=>void run(()=>control('reload'))}><RefreshCw size={14}/></button>
        <button type="button" disabled={busy} onClick={()=>void run(()=>control('focus'))}><ExternalLink size={14}/>{t('显示页面')}</button>
        <button type="button" disabled={busy} onClick={()=>void run(()=>control('system'))}>{t('系统浏览器打开')}</button>
        {devtools&&<button type="button" disabled={busy} onClick={()=>void run(()=>control('devtools'))}>{t('开发工具')}</button>}
        <button type="button" disabled={busy} onClick={()=>void run(()=>control('navigate',{url}))}>{t('前往地址')}</button>
        <InteractionShareControl key={current.id} threadId={threadId} targetId={current.id} targetKind="browser_tab" origin={(()=>{try{const url=new URL(current.url);return ['http:','https:'].includes(url.protocol)?url.origin:null}catch{return null}})()} disabled={busy||current.loading}/>
        <select value={current.level} disabled={busy||current.loading} aria-label={t('本机预览权限')} onChange={event=>void run(()=>control('share',{level:event.target.value,minutes:30}))}>
          <option value="none">{t('未共享')}</option><option value="read">{t('允许本会话查看 · 30分钟')}</option><option value="control">{t('允许本会话操作 · 30分钟')}</option>
        </select>
        <button type="button" disabled={busy||current.level==='none'} onClick={()=>void run(()=>control('share',{level:'none'}))}><ShieldOff size={14}/>{t('收回')}</button>
        <button type="button" disabled={busy} onClick={()=>void run(()=>control('close'))}><X size={14}/>{t('关闭页面')}</button>
      </div>
      <div className="workbench-toolbar"><span>{current.profileId==='temporary'?t('临时浏览'):t('独立身份')}</span><button type="button" disabled={busy||current.profileId===profileId} onClick={()=>void run(async()=>{const tab=await api.open(threadId,current.url,profileId);setTabs(previous=>[...previous,tab]);setSelected(tab.id);setPreview(null)})}>{t('在所选身份中打开')}</button><span className="workbench-browser-address" title={current.url}>{current.url}</span>{current.level!=='none'&&<><button disabled={busy} onClick={()=>void run(()=>inspect('snapshot'))}>{t('页面内容')}</button><button disabled={busy} onClick={()=>void run(()=>inspect('screenshot'))}>{t('页面截图')}</button><button disabled={busy} onClick={()=>void run(()=>inspect('console'))}>{t('控制台')}</button></>}</div>
      {current.error&&<p className="workbench-error" role="alert">{current.error}</p>}
      <div className="workbench-browser-preview">{preview?.image?<img src={preview.image} alt={t('当前页面截图')}/>:preview?.tree!==undefined?<pre>{preview.tree}</pre>:preview?.console?<ul>{preview.console.map((entry,index)=><li key={index}><time>{new Date(entry.at).toLocaleTimeString()}</time> <span>{entry.level}</span> {entry.message}</li>)}</ul>:<div className="workbench-empty"><Globe size={32}/><p>{t('点击“显示页面”浏览网页，选择共享权限后可在这里查看页面内容。')}</p></div>}</div>
    </>:<div className="workbench-empty"><Globe size={32}/><p>{t('打开网页或本地开发服务，选择要交给本会话的页面。')}</p></div>}
  </div>
}
