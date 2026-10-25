import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import * as DropdownMenu from '@radix-ui/react-dropdown-menu'
import { ArrowLeft, ArrowRight, RefreshCw, Globe, MoreHorizontal, Send, Camera, MousePointer2, Pencil, Bug, ExternalLink, Plus, X } from 'lucide-react'
import type { BrowserWorkspaceView } from '@contracts'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { useOptionalFilePreview } from './FilePreviewContext'
import { BrowserSettingsDialog } from './BrowserSettingsDialog'
import { BrowserProfiles } from './BrowserProfiles'
import { BrowserDownloadBar } from './BrowserDownloadBar'
import { InteractionActivity } from './InteractionActivity'
import { BrowserSharePopover } from './BrowserSharePopover'
import { NativeBrowserStage } from './NativeBrowserStage'
import { BrowserMarkupDialog,type BrowserCapture } from './BrowserMarkupDialog'

/** Codeg browser-toolbar and page handoff, inside the single retained workspace. */
export function BrowserWorkbench({threadId,resource,active=true,onQuickMessage,onWorkspaceFiles}:{threadId:string;resource?:string;active?:boolean;onQuickMessage?(text:string):boolean;onWorkspaceFiles?(files:File[]):Promise<boolean>}):React.JSX.Element{
  const api=useThreadClient().browserWorkspace,t=useUiText(),preview=useOptionalFilePreview(),epoch=useRef(0),lock=useRef(false)
  const [tabs,setTabs]=useState<BrowserWorkspaceView[]>([]),[url,setUrl]=useState('http://localhost:3000'),[editing,setEditing]=useState(false)
  const [busy,setBusy]=useState(false),[error,setError]=useState(''),[notice,setNotice]=useState(''),[profileId,setProfileId]=useState('temporary'),[devtools,setDevtools]=useState(false)
  const [markup,setMarkup]=useState<BrowserCapture|null>(null),[picker,setPicker]=useState<{id:string;documentId:string}|null>(null),[picked,setPicked]=useState<{url:string;title:string;tag:string;text:string;html:string}|null>(null)
  const current=tabs.find(row=>row.id===resource)
  const deliveryToken=useMemo(()=>({threadId,resource,active}),[threadId,resource,active]),delivery=useRef<typeof deliveryToken|null>(deliveryToken);delivery.current=deliveryToken
  useEffect(()=>()=>{if(delivery.current===deliveryToken)delivery.current=null},[deliveryToken])
  useEffect(()=>{const generation=++epoch.current;void api?.preferences().then(value=>{if(epoch.current===generation)setDevtools(value.devtools)}).catch(()=>undefined);return()=>{epoch.current++}},[api,threadId])
  const refresh=useCallback(async():Promise<void>=>{if(!api)return;const generation=epoch.current,value=await api.list(threadId);if(epoch.current!==generation)return;setTabs(value);for(const row of value){const id=preview?.openWorkspace('browser',row.id,row.title||row.url,true);if(id)preview?.updateWorkspaceTab(id,{label:row.title||row.url})}},[api,threadId,preview?.openWorkspace,preview?.updateWorkspaceTab])
  useEffect(()=>{let live=true;const read=():void=>{void refresh().catch(failure=>{if(live)setError(readErrorMessage(failure))})};read();const timer=setInterval(read,2000);return()=>{live=false;clearInterval(timer)}},[refresh])
  useEffect(()=>{if(current&&!editing)setUrl(current.url)},[resource,current?.url,editing])
  useEffect(()=>{setEditing(false);if(current)setUrl(current.url);setPicked(null);setNotice('');setError('');setMarkup(null)},[resource])
  const run=async(action:()=>Promise<void>):Promise<void>=>{if(lock.current)return;lock.current=true;setBusy(true);setError('');setNotice('');const generation=epoch.current;try{await action()}catch(failure){if(epoch.current===generation)setError(readErrorMessage(failure))}finally{lock.current=false;if(epoch.current===generation)setBusy(false)}}
  const control=async(action:'back'|'forward'|'reload'|'system'|'devtools'):Promise<void>=>{if(api&&current){await api.control({id:current.id,action});await refresh()}}
  const append=(text:string):boolean=>{if(delivery.current!==deliveryToken||!active)return false;const accepted=onQuickMessage?.(text)===true;if(accepted)setNotice(t('已加入当前输入，发送后队员才会收到。'));else setError(t('当前输入暂不可用，材料仍保留，请稍后重试。'));return accepted}
  const capture=async():Promise<BrowserCapture>=>{if(!api?.inspect||!current)throw Error(t('当前页面无法截图，请重新打开页面。'));const result=await api.inspect({id:current.id,action:'screenshot'});if(delivery.current!==deliveryToken||!active)throw Error(t('原页面已切换，请在当前页面重新操作。'));if(!result.data)throw Error(t('截图为空，请稍后重试。'));return {data:result.data,url:result.url,title:result.title}}
  const deliverFile=async(file:File,source:BrowserCapture):Promise<boolean>=>{if(!onWorkspaceFiles||delivery.current!==deliveryToken||!active)return false;const accepted=await onWorkspaceFiles([file]);if(accepted&&delivery.current===deliveryToken){onQuickMessage?.('页面截图：'+source.title+'\n来源：'+source.url);setNotice(t('截图已加入当前输入，发送后队员才会收到。'))}return accepted}
  const startPicker=async():Promise<void>=>{if(!api?.inspect||!current)return;setPicked(null);const result=await api.inspect({id:current.id,action:'pick_start'});if(delivery.current!==deliveryToken||!active){if(result.documentId)await api.inspect({id:current.id,action:'pick_cancel',documentId:result.documentId}).catch(()=>undefined);return}if(!result.documentId)throw Error(t('无法开始选择元素。'));setPicker({id:current.id,documentId:result.documentId})}
  useEffect(()=>{if(!picker||!api?.inspect)return;let live=true,running=false;const read=async():Promise<void>=>{if(running)return;running=true;try{const result=await api.inspect!({id:picker.id,action:'pick_read',documentId:picker.documentId});if(!live)return;if(result.cancelled)setPicker(null);else if(result.result){setPicked({url:result.url,title:result.title,...result.result});setPicker(null)}}catch(failure){if(live){setError(readErrorMessage(failure));setPicker(null)}}finally{running=false}};const timer=setInterval(()=>void read(),350);return()=>{live=false;clearInterval(timer);void api.inspect!({id:picker.id,action:'pick_cancel',documentId:picker.documentId}).catch(()=>undefined)}},[picker,api])
  useEffect(()=>{if(!active||picker&&picker.id!==resource)setPicker(null)},[active,resource])
  if(!api)return <div className="workbench-empty"><Globe size={32}/><strong>{t('浏览器需要桌面端')}</strong><p>{t('请在桌面端打开要使用的页面。')}</p></div>
  let origin:string|null=null;try{if(current&&/^https?:/.test(current.url))origin=new URL(current.url).origin}catch{}
  return <div className="workbench-browser unified-browser">
    <form className="browser-navigation-row" onSubmit={event=>{event.preventDefault();setEditing(false);void run(async()=>{if(current)await api.control({id:current.id,action:'navigate',url});else{const opened=await api.open(threadId,url,profileId);preview?.openWorkspace('browser',opened.id,opened.title||opened.url)}await refresh()})}}>
      <button type="button" className="quiet-button workbench-icon" title={t('后退')} aria-label={t('后退')} disabled={busy||!current} onClick={()=>void run(()=>control('back'))}><ArrowLeft size={16}/></button>
      <button type="button" className="quiet-button workbench-icon" title={t('前进')} aria-label={t('前进')} disabled={busy||!current} onClick={()=>void run(()=>control('forward'))}><ArrowRight size={16}/></button>
      <button type="button" className="quiet-button workbench-icon" title={t('刷新页面')} aria-label={t('刷新页面')} disabled={busy||!current} onClick={()=>void run(()=>control('reload'))}><RefreshCw size={16}/></button>
      <div className="browser-address-field">{current&&<BrowserSharePopover key={current.id} threadId={threadId} targetId={current.id} origin={origin}/>}<input aria-label={t('网页地址')} spellCheck={false} value={url} onFocus={()=>setEditing(true)} onChange={event=>setUrl(event.target.value)}/><button type="submit" className="quiet-button workbench-icon" title={t(current?'前往网页':'打开网页')} aria-label={t(current?'前往网页':'打开网页')} disabled={busy}><ArrowRight size={15}/></button></div>
      <DropdownMenu.Root><DropdownMenu.Trigger asChild><button type="button" className="quiet-button workbench-icon" title={t('把页面加入对话')} aria-label={t('把页面加入对话')} disabled={busy||!current}><Send size={16}/></button></DropdownMenu.Trigger><DropdownMenu.Portal><DropdownMenu.Content className="workspace-menu" align="end" sideOffset={6}>
        <DropdownMenu.Label>{t('加入当前输入')}</DropdownMenu.Label>
        <DropdownMenu.Item onSelect={()=>void run(async()=>{if(!api.inspect||!current)return;const result=await api.inspect({id:current.id,action:'page'});append('页面：'+result.title+'\n来源：'+result.url+'\n\n'+(result.text||t('页面没有可读取的正文。')))})}><Globe size={16}/>{t('页面文字与链接')}</DropdownMenu.Item>
        <DropdownMenu.Item onSelect={()=>void run(startPicker)}><MousePointer2 size={16}/>{t('选择一个元素')}</DropdownMenu.Item>
        <DropdownMenu.Item onSelect={()=>void run(async()=>{const shot=await capture();const file=new File([Uint8Array.from(atob(shot.data),character=>character.charCodeAt(0))],'页面截图.png',{type:'image/png'});if(!await deliverFile(file,shot))setError(t('截图未加入输入，请重试。'))})}><Camera size={16}/>{t('当前页面截图')}</DropdownMenu.Item>
        <DropdownMenu.Item onSelect={()=>void run(async()=>setMarkup(await capture()))}><Pencil size={16}/>{t('标注一张截图')}</DropdownMenu.Item>
        <DropdownMenu.Separator/>
        <DropdownMenu.Item onSelect={()=>void run(async()=>{if(!api.inspect||!current)return;const result=await api.inspect({id:current.id,action:'console'});if(!result.entries?.length){setNotice(t('当前页面没有记录到控制台错误。'));return}append('页面错误：'+result.title+'\n来源：'+result.url+'\n\n'+result.entries.map(row=>row.message).join('\n'))})}><Bug size={16}/>{t('页面错误')}</DropdownMenu.Item>
      </DropdownMenu.Content></DropdownMenu.Portal></DropdownMenu.Root>
      <DropdownMenu.Root><DropdownMenu.Trigger asChild><button type="button" className="quiet-button workbench-icon" title={t('浏览器更多操作')} aria-label={t('浏览器更多操作')}><MoreHorizontal size={17}/></button></DropdownMenu.Trigger><DropdownMenu.Portal><DropdownMenu.Content className="workspace-menu browser-more-menu" align="end" sideOffset={6}>
        <DropdownMenu.Item onSelect={()=>preview?.openWorkspace('browser')}><Plus size={16}/>{t('新页面')}</DropdownMenu.Item>
        <DropdownMenu.Item disabled={!current} onSelect={()=>void run(()=>control('system'))}><ExternalLink size={16}/>{t('在系统浏览器打开')}</DropdownMenu.Item>
        {devtools&&<DropdownMenu.Item disabled={!current} onSelect={()=>void run(()=>control('devtools'))}><Bug size={16}/>{t('开发工具')}</DropdownMenu.Item>}
        <DropdownMenu.Separator/><div className="browser-more-controls"><BrowserSettingsDialog api={api} onChanged={value=>setDevtools(value.devtools)}/><BrowserProfiles api={api} value={profileId} onChange={setProfileId}/><InteractionActivity threadId={threadId}/></div>
      </DropdownMenu.Content></DropdownMenu.Portal></DropdownMenu.Root>
    </form>
    {error&&<div className="workbench-notice" role="alert"><p>{error}</p><button className="quiet-button" type="button" onClick={()=>setError('')}>{t('收起')}</button></div>}
    {notice&&<div className="browser-evidence-notice" role="status">{notice}</div>}
    {picker&&<div className="browser-evidence-notice">{t('在网页上点击一个元素，按 Esc 取消。')}<button className="quiet-button" type="button" onClick={()=>setPicker(null)}><X size={14}/>{t('取消选择')}</button></div>}
    {picked&&<div className="browser-picked-preview"><strong>{t('已选择元素')} · {picked.tag}</strong><p>{picked.text.slice(0,300)}</p><div><button type="button" className="quiet-button" onClick={()=>setPicked(null)}>{t('取消')}</button><button type="button" className="primary-button" onClick={()=>{if(append('页面元素：'+picked.title+'\n来源：'+picked.url+'\n标签：'+picked.tag+'\n文字：'+picked.text+'\n结构：'+picked.html))setPicked(null)}}>{t('加入输入')}</button></div></div>}
    <BrowserDownloadBar api={api} threadId={threadId} tabId={current?.id??null}/>
    {current?<>{current.error&&<p className="workbench-error" role="alert">{current.error}</p>}{active&&<NativeBrowserStage key={current.id} api={api} id={current.id}/>}</>:<div className="workbench-empty browser-start-page"><Globe size={36}/><strong>{t('打开网页，和队员一起工作')}</strong><p>{t('输入网址或本地开发地址。共享后队员可读取页面；截图、元素和页面文字可加入当前对话。')}</p><div className="browser-new-profile"><BrowserProfiles api={api} value={profileId} onChange={setProfileId}/></div><button type="button" className="quiet-button" onClick={()=>{preview?.openWorkspace('computer')}}>{t('也可以共享电脑窗口')}</button></div>}
    {markup&&<BrowserMarkupDialog capture={markup} onClose={()=>setMarkup(null)} onDeliver={file=>deliverFile(file,markup)}/>}
  </div>
}
