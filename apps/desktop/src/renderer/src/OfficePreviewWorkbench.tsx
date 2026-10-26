import {OfficeCliSettings} from './OfficeCliSettings'
import { useEffect, useState } from 'react'
import { ExternalLink, FileText, RefreshCw } from 'lucide-react'
import { newCommandId } from '../../shared/command-id'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import {NativeBrowserStage} from './NativeBrowserStage'
type Lease={holdId:string;port:number;cap:string;editable:boolean}
// Codeg OfficePreview's watch lifecycle, sandbox and install/retry states.
export function OfficePreviewWorkbench({threadId,path}:{threadId:string;path:string}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText()
  const [editable,setEditable]=useState(false),[attempt,setAttempt]=useState(0),[lease,setLease]=useState<Lease|null>(null),[error,setError]=useState<string|null>(null),[installed,setInstalled]=useState<boolean|null>(null),[opened,setOpened]=useState(false),[busy,setBusy]=useState(false)
  const [surfaceId,setSurfaceId]=useState<string|null>(null)
  const previewFailure=t('文档预览未能打开，请重新连接。')
  useEffect(()=>{
    const api=client.officeWorkspace
    if(!api?.present||!api.close||installed!==true)return
    let active=true,id:string|undefined;setSurfaceId(null);setError(null)
    void api.open(threadId,path,editable,true).then(value=>{id=value.id;if(!id)throw new Error(previewFailure);if(active)setSurfaceId(id);else void api.close!(id)},failure=>{if(active)setError(String(failure))}).catch(failure=>{if(active)setError(String(failure))})
    return()=>{active=false;if(id)void api.close!(id).catch(()=>undefined)}
  },[client,threadId,path,editable,attempt,installed,previewFailure])
  useEffect(()=>{let active=true;void client.request<{installed:boolean}>('office.watch',{scope:{threadId},operation:'detect'}).then(value=>{if(active)setInstalled(value.installed)}).catch(failure=>{if(active)setError(String(failure))});return()=>{active=false}},[client,threadId,attempt])
  useEffect(()=>{
    if(client.officeWorkspace)return
    let active=true,hold:Lease|null=null
    const scope={threadId},holdId=newCommandId();setLease(null);setError(null)
    const opening=client.request<Lease>('office.watch',{scope,operation:'start',holdId,path,editable})
    void opening.then(value=>{hold=value;if(active)setLease(value);else void client.request('office.watch',{scope,operation:'stop',holdId}).catch(()=>undefined)},failure=>{if(active)setError(String(failure))})
    const timer=window.setInterval(()=>{if(active&&hold)void client.request('office.watch',{scope,operation:'heartbeat',holdId}).catch(failure=>{setLease(null);setError(String(failure))})},30000)
    return()=>{active=false;window.clearInterval(timer);if(hold)void client.request('office.watch',{scope,operation:'stop',holdId}).catch(()=>undefined)}
  },[client,threadId,path,editable,attempt])
  const open=async():Promise<void>=>{if(!client.officeWorkspace||busy)return;setBusy(true);setError(null);try{await client.officeWorkspace.open(threadId,path,editable);setOpened(true)}catch(failure){setError(String(failure))}finally{setBusy(false)}}
  return <div className="workbench-office"><div className="workbench-toolbar"><FileText size={16}/><strong title={path}>{path.split('/').at(-1)}</strong><label><input className="workspace-check" type="checkbox" checked={editable} disabled={busy} onChange={event=>{setEditable(event.target.checked);setOpened(false)}}/>{t('允许在预览中编辑此文件')}</label><button className="quiet-button" disabled={busy} onClick={()=>{setError(null);setAttempt(value=>value+1);setOpened(false)}}><RefreshCw size={14}/>{t('重新连接')}</button></div>
    {error&&<p className="workbench-error" role="alert">{error}</p>}
    {installed===false&&<div className="workbench-empty"><FileText size={30}/><p>{t('此主机尚未安装 OfficeCLI，安装后点击重新连接。')}</p><p>{t('支持 Word、Excel 和 PowerPoint 实时预览。')}</p><OfficeCliSettings/></div>}
    {client.officeWorkspace?.present ? surfaceId&&<NativeBrowserStage api={client.officeWorkspace} id={surfaceId}/> : client.officeWorkspace&&installed!==false?<div className="workbench-empty"><FileText size={32}/><p>{t(opened?'预览窗口已打开，文件的变化会在窗口中实时更新。':'此文件使用 OfficeCLI 实时预览。')}</p><button className="quiet-button" disabled={busy} onClick={()=>void open()}><ExternalLink size={14}/>{t('打开预览窗口')}</button></div>:lease?<iframe title={path} className="workbench-office-frame" sandbox="allow-scripts" referrerPolicy="no-referrer" src={`/api/office-watch-proxy/${lease.port}/?cap=${encodeURIComponent(lease.cap)}`}/>:installed!==false&&!error?<div className="workbench-empty">{t('正在准备文档预览…')}</div>:null}
  </div>
}
