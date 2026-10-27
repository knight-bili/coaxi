import { readErrorMessage } from './error-message'
import { InteractionComputerPanel } from './InteractionComputerPanel'
import { InteractionActivity } from './InteractionActivity'
import { useCallback, useEffect, useRef, useState } from 'react'
import { AppWindow, ChevronDown, Layers, Monitor, RefreshCw, Square, X } from 'lucide-react'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'

type Level = 'none' | 'read' | 'control'
type Window = { targetId:string; app:string; appKey?:string; pid?:number; title:string; shareable:boolean; level:Level; minimized?:boolean; hidden?:boolean; wholeApp?:boolean; appId?:string }
type Status = { enabled:boolean; installed:boolean; driverVersion:string; backend:{state:string; detail?:string}; shared:Array<{targetId:string; level:'read'|'control'}> }
type ComputerApi = NonNullable<ReturnType<typeof useThreadClient>['computerWorkspace']>

function Thumbnail({api,threadId,window,round}:{api:ComputerApi;threadId:string;window:Window;round:number}):React.JSX.Element {
  const box=useRef<HTMLDivElement>(null),[image,setImage]=useState<string|null>()
  useEffect(()=>{
    setImage(undefined)
    if(!window.shareable||window.minimized||window.hidden){setImage(null);return}
    let cancelled=false
    const observer=new IntersectionObserver(entries=>{
      if(!entries.some(entry=>entry.isIntersecting))return
      observer.disconnect()
      void api.request<{thumbnail:string|null}>(threadId,{operation:'thumbnail',targetId:window.targetId})
        .then(value=>{if(!cancelled)setImage(value.thumbnail)}).catch(()=>{if(!cancelled)setImage(null)})
    })
    if(box.current)observer.observe(box.current)
    return ()=>{cancelled=true;observer.disconnect()}
  },[api,threadId,window.targetId,window.shareable,window.minimized,window.hidden,round])
  return <div className="workbench-window-picture" ref={box}>{image?<img src={image} alt=""/>:<AppWindow size={28} aria-hidden="true" className={image===undefined?'is-loading':undefined}/>}</div>
}

function LevelControl({value,onChange,disabled,label}:{value:Level;onChange(level:Level):void;disabled:boolean;label:string}):React.JSX.Element {
  const t=useUiText()
  return <div className="workbench-share-level" role="group" aria-label={label}>{(['none','read','control'] as const).map(level=><button type="button" key={level} data-level={level} aria-pressed={value===level} disabled={disabled} onClick={()=>onChange(level)}>{t(level==='none'?'不共享':level==='read'?'可读取':'可操作')}</button>)}</div>
}

// Coaxi ComputerWindowPicker: previews, process/app groups, level controls and bounded scrolling.
// Each request still belongs to this thread's local computer service.
export function ComputerWorkbench({threadId}:{threadId:string}):React.JSX.Element {
  const api=useThreadClient().computerWorkspace,t=useUiText(),bulkCancelled=useRef(false)
  const [status,setStatus]=useState<Status|null>(null),[windows,setWindows]=useState<Window[]>([]),[busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[round,setRound]=useState(0)
  const [preview,setPreview]=useState<{image?:string;tree?:string}|null>(null),[permissions,setPermissions]=useState<{required:boolean;accessibility:boolean;screenRecording:boolean}|null>(null)
  const refresh=useCallback(async()=>{
    if(!api)return
    const value=await api.request<Status>(threadId,{operation:'status'})
    setStatus(value)
    setWindows(value.enabled?(await api.request<{windows:Window[]}>(threadId,{operation:'windows'})).windows:[])
  },[api,threadId])
  useEffect(()=>{
    let active=true
    void (async()=>{
      if(!api)return
      const value=await api.request<Status>(threadId,{operation:'status'})
      if(!active)return
      setStatus(value)
      if(value.enabled){
        const listed=await api.request<{windows:Window[]}>(threadId,{operation:'windows'})
        if(active)setWindows(listed.windows)
      }
    })().catch(failure=>{if(active)setError(readErrorMessage(failure))})
    return ()=>{active=false}
  },[api,threadId])
  const run=async(action:()=>Promise<void>):Promise<void>=>{
    if(busy)return
    setBusy(true);setError(null)
    try{await action()}catch(failure){setError(readErrorMessage(failure))}
    finally{try{await refresh()}catch(failure){setError(readErrorMessage(failure))}setBusy(false)}
  }
  const share=async(window:Window,level:Level,wholeApp=false):Promise<void>=>{
    await api?.request(threadId,{operation:wholeApp?'share_app':'share',targetId:window.targetId,level});setPreview(null)
  }
  const shareAll=async(level:Level):Promise<void>=>{
    bulkCancelled.current=false
    const apps=new Set<string>()
    for(const window of windows.filter(row=>row.shareable)){
      if(bulkCancelled.current)return
      if(window.wholeApp){const key=window.appId??window.appKey??window.app;if(apps.has(key))continue;apps.add(key)}
      await share(window,level,!!window.wholeApp)
    }
  }
  const read=async(window:Window,operation:'capture'|'snapshot'):Promise<void>=>{
    if(!api)return
    const value=await api.request<{pngBase64?:string;tree?:string}>(threadId,{operation,targetId:window.targetId})
    setPreview(operation==='capture'?{image:'data:image/png;base64,'+value.pngBase64}:{tree:value.tree})
  }
  if(!api)return <div className="workbench-empty"><Monitor size={30}/><p>{t('电脑窗口的共享和操作在桌面端管理。')}</p></div>
  const groups=Map.groupBy(windows,window=>JSON.stringify([window.pid,window.appKey??window.app]))
  const available=windows.filter(window=>window.shareable).length,shared=windows.filter(window=>window.level!=='none').length
  return <div className="workbench-computer">
    <header className="workbench-computer-heading"><h2>{t('把窗口共享给队员')}</h2><p>{t('选择队员可以查看或操作的窗口。停止共享、窗口关闭或闲置超过 30 分钟后，共享结束。')}</p>
      <div className="workbench-computer-tools"><label><input className="workspace-check" type="checkbox" checked={status?.enabled??false} disabled={busy||!status} onChange={event=>{const enabled=event.target.checked;void run(async()=>{await api.request(threadId,{operation:'enable',enabled});setWindows([]);setPreview(null);setPermissions(null)})}}/>{t('允许在此会话共享窗口')}</label><InteractionActivity threadId={threadId}/><InteractionComputerPanel threadId={threadId}/></div>
      <details className="workbench-computer-system"><summary>{t('系统与驱动')}</summary><div className="workbench-toolbar"><span>{t('驱动')} {status?.driverVersion??''} · {status?.installed?t('已安装'):t('未安装')} · {status?.backend.state??''}</span><button className="quiet-button" disabled={busy} onClick={()=>void run(async()=>{await api.request(threadId,{operation:'install'})})}>{t('安装固定版本')}</button><button className="quiet-button" disabled={busy||status?.enabled||!status?.installed} onClick={()=>void run(async()=>{await api.request(threadId,{operation:'remove'})})}>{t('移除驱动')}</button><button className="quiet-button" disabled={busy||!status?.enabled} onClick={()=>void run(async()=>setPermissions(await api.request(threadId,{operation:'permissions'})))}>{t('系统权限')}</button></div>
        {permissions&&<div className="workbench-toolbar"><span>{t('辅助功能')}: {t(permissions.accessibility?'已授权':'未授权')}</span><span>{t('屏幕录制')}: {t(permissions.screenRecording?'已授权':'未授权')}</span>{permissions.required&&(['accessibility','screenRecording'] as const).map(permission=><button className="quiet-button" key={permission} disabled={busy||permissions[permission]} onClick={()=>void run(async()=>{await api.request(threadId,{operation:'request_permission',permission});setPermissions(await api.request(threadId,{operation:'permissions'}))})}>{t(permission==='accessibility'?'申请辅助功能权限':'申请屏幕录制权限')}</button>)}</div>}
      </details>
    </header>
    {(error||status?.backend.detail)&&<p role="alert" className="workbench-error">{error??status?.backend.detail}</p>}
    <div className="workbench-computer-summary"><span>{windows.length} {t('个窗口')} · <strong>{t('已共享')} {shared} {t('个')}</strong></span><div>
      <button className="quiet-button workbench-stop-sharing" disabled={!status?.enabled} onClick={()=>{bulkCancelled.current=true;setPreview(null);void api.request(threadId,{operation:'stop'}).then(refresh).catch(failure=>setError(readErrorMessage(failure)))}}><Square size={13}/>{t('全部停止共享')}</button>
      <details className="workbench-bulk-sharing"><summary aria-disabled={busy||!status?.enabled||!available}><Layers size={14}/>{t('全部共享')}<ChevronDown size={13}/></summary><div><button disabled={busy||!status?.enabled||!available} onClick={event=>{event.currentTarget.closest('details')?.removeAttribute('open');void run(()=>shareAll('read'))}}>{t('全部允许读取')}</button><button disabled={busy||!status?.enabled||!available} onClick={event=>{event.currentTarget.closest('details')?.removeAttribute('open');void run(()=>shareAll('control'))}}>{t('全部允许操作')}</button></div></details>
      <button className="quiet-button" aria-label={t('刷新窗口列表')} disabled={busy||!status?.enabled} onClick={()=>void run(async()=>setRound(value=>value+1))}><RefreshCw size={15}/></button>
    </div></div>
    <div className="workbench-computer-body"><div className="workbench-window-groups" role="region" aria-label={t('可共享窗口列表')} tabIndex={0}>{[...groups].map(([key,items])=>{
      const first=items.find(window=>window.shareable),appLevel=items.find(window=>window.wholeApp)?.level??'none'
      return <section key={key}><header className="workbench-window-group-heading"><h3>{items[0].app} <span>· {items.length} {t('个窗口')}</span></h3><div><span>{t('整个应用')}</span><LevelControl value={appLevel} disabled={busy||!first} label={items[0].app+' '+t('应用共享权限')} onChange={level=>{if(first)void run(()=>share(first,level,true))}}/></div></header><div className="workbench-window-grid">{items.map(window=><article className="workbench-window-tile" data-level={window.level} key={window.targetId}>
        <div className="workbench-window-cover"><Thumbnail api={api} threadId={threadId} window={window} round={round}/>{(window.minimized||window.hidden)&&<span className="workbench-window-badge">{t(window.minimized?'已最小化':'已隐藏')}</span>}{window.wholeApp&&<span className="workbench-window-badge is-app">{t('随应用共享')}</span>}</div>
        <div className="workbench-window-info"><strong>{window.app}</strong><p title={window.title}>{window.title||window.app}</p><LevelControl value={window.level} disabled={busy||!window.shareable||!!window.wholeApp} label={(window.title||window.app)+' '+t('窗口共享权限')} onChange={level=>void run(()=>share(window,level))}/>
        {!window.shareable&&<small>{t('此窗口不能共享')}</small>}{window.level!=='none'&&<div className="workbench-window-actions"><button className="quiet-button" disabled={busy} onClick={()=>void run(()=>read(window,'capture'))}>{t('截图')}</button><button className="quiet-button" disabled={busy} onClick={()=>void run(()=>read(window,'snapshot'))}>{t('窗口内容')}</button></div>}</div>
      </article>)}</div></section>
    })}{!windows.length&&<div className="workbench-empty"><Monitor size={30}/><p>{t(!status?'正在读取窗口…':status.enabled?'暂时没有可共享窗口，请刷新列表。':'开启共享后，选择要交给队员的窗口。')}</p></div>}</div>
      {preview&&<div className="workbench-browser-preview"><button className="quiet-button" aria-label={t('关闭窗口预览')} onClick={()=>setPreview(null)}><X size={14}/></button>{preview.image?<img src={preview.image} alt={t('共享窗口截图')}/>:<pre>{preview.tree}</pre>}</div>}
    </div>
  </div>
}
