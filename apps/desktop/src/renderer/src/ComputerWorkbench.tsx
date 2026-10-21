import { useCallback, useEffect, useState } from 'react'
import { Monitor, RefreshCw, ShieldOff, Square } from 'lucide-react'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
type Window={targetId:string;app:string;title:string;shareable:boolean;level:'none'|'read'|'control'}
type Status={enabled:boolean;installed:boolean;driverVersion:string;backend:{state:string;detail?:string};shared:Array<{targetId:string;level:'read'|'control'}>}
// Codeg ComputerWindowPicker's app groups/window tiles and per-window levels,
// with Rovai's controls. Listing is an explicit local action.
export function ComputerWorkbench({threadId}:{threadId:string}):React.JSX.Element{
  const api=useThreadClient().computerWorkspace,t=useUiText()
  const [status,setStatus]=useState<Status|null>(null),[windows,setWindows]=useState<Window[]>([]),[busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null)
  const [preview,setPreview]=useState<{targetId:string;image?:string;tree?:string}|null>(null),[permissions,setPermissions]=useState<{required:boolean;accessibility:boolean;screenRecording:boolean}|null>(null)
  const refresh=useCallback(async()=>{if(api)setStatus(await api.request<Status>(threadId,{operation:'status'}))},[api,threadId])
  useEffect(()=>{let active=true;void refresh().catch(failure=>{if(active)setError(String(failure))});return()=>{active=false}},[refresh])
  const run=async(action:()=>Promise<void>):Promise<void>=>{if(busy)return;setBusy(true);setError(null);try{await action();await refresh()}catch(failure){setError(String(failure))}finally{setBusy(false)}}
  const load=async():Promise<void>=>{if(api)setWindows((await api.request<{windows:Window[]}>(threadId,{operation:'windows'})).windows)}
  const share=async(window:Window,level:Window['level']):Promise<void>=>{if(!api)return;await api.request(threadId,{operation:'share',targetId:window.targetId,level});setPreview(null);await load()}
  const read=async(window:Window,operation:'capture'|'snapshot'):Promise<void>=>{
    if(!api)return
    const value=await api.request<{pngBase64?:string;tree?:string}>(threadId,{operation,targetId:window.targetId})
    setPreview({targetId:window.targetId,...(operation==='capture'?{image:'data:image/png;base64,'+value.pngBase64}:{tree:value.tree})})
  }
  if(!api)return <div className="workbench-empty"><Monitor size={30}/><p>{t('电脑窗口的共享和操作在桌面端管理。')}</p></div>
  const groups=Map.groupBy(windows,window=>window.app)
  return <div className="workbench-computer">
    <div className="workbench-toolbar"><Monitor size={17}/><strong>{t('电脑操作')}</strong>
      <label><input type="checkbox" checked={status?.enabled??false} disabled={busy||!status} onChange={event=>{const enabled=event.target.checked;void run(async()=>{await api.request(threadId,{operation:'enable',enabled});setWindows([]);setPreview(null);setPermissions(null)})}}/>{t('允许在此会话共享窗口')}</label>
      <button disabled={busy||!status?.enabled} onClick={()=>void run(load)}><RefreshCw size={14}/>{t('选择窗口')}</button>
      <button disabled={busy||!status?.enabled} onClick={()=>void run(async()=>setPermissions(await api.request(threadId,{operation:'permissions'})))}>{t('系统权限')}</button>
      <button disabled={!status?.enabled} onClick={()=>{setPreview(null);void api.request(threadId,{operation:'stop'}).then(()=>refresh()).catch(failure=>setError(String(failure)))}}><Square size={13}/>{t('立即停止并收回共享')}</button>
    </div>
    <div className="workbench-toolbar"><span>{t('驱动')} {status?.driverVersion??''} · {status?.installed?t('已安装'):t('未安装')} · {status?.backend.state??''}</span><button disabled={busy} onClick={()=>void run(async()=>{await api.request(threadId,{operation:'install'})})}>{t('安装固定版本')}</button><button disabled={busy||status?.enabled||!status?.installed} onClick={()=>void run(async()=>{await api.request(threadId,{operation:'remove'})})}>{t('移除驱动')}</button></div>
    {(error||status?.backend.detail)&&<p role="alert" className="workbench-error">{error??status?.backend.detail}</p>}
    {permissions&&<div className="workbench-toolbar"><span>{t('辅助功能')}: {t(permissions.accessibility?'已授权':'未授权')}</span><span>{t('屏幕录制')}: {t(permissions.screenRecording?'已授权':'未授权')}</span>{permissions.required&&(['accessibility','screenRecording'] as const).map(permission=><button key={permission} disabled={busy||permissions[permission]} onClick={()=>void run(async()=>{await api.request(threadId,{operation:'request_permission',permission});setPermissions(await api.request(threadId,{operation:'permissions'}))})}>{t(permission==='accessibility'?'申请辅助功能权限':'申请屏幕录制权限')}</button>)}</div>}
    <div className="workbench-computer-body"><div className="workbench-window-groups">{[...groups].map(([app,items])=><section key={app}><h3>{app}</h3><div className="workbench-window-grid">{items.map(window=>{
      const level=status?.shared.find(grant=>grant.targetId===window.targetId)?.level??'none'
      return <article className="workbench-window-tile" key={window.targetId}><Monitor size={27}/><strong title={window.title}>{window.title||window.app}</strong><select aria-label={t('窗口共享权限')} value={level} disabled={busy||!window.shareable} onChange={event=>void run(()=>share(window,event.target.value as Window['level']))}><option value="none">{t(window.shareable?'未共享':'此窗口不能共享')}</option><option value="read">{t('允许查看')}</option><option value="control">{t('允许操作')}</option></select>{level!=='none'&&<div className="workbench-toolbar"><button disabled={busy} onClick={()=>void run(()=>read(window,'capture'))}>{t('截图')}</button><button disabled={busy} onClick={()=>void run(()=>read(window,'snapshot'))}>{t('窗口内容')}</button><button disabled={busy} onClick={()=>void run(()=>share(window,'none'))} aria-label={t('收回窗口')}><ShieldOff size={14}/></button></div>}</article>
    })}</div></section>)}{!windows.length&&<div className="workbench-empty"><Monitor size={30}/><p>{t(status?.enabled?'点击“选择窗口”，只共享你选中的窗口。':'开启后选择要共享的窗口。')}</p></div>}</div>
    {preview&&<div className="workbench-browser-preview">{preview.image?<img src={preview.image} alt={t('共享窗口截图')}/>:<pre>{preview.tree}</pre>}</div>}</div>
  </div>
}
