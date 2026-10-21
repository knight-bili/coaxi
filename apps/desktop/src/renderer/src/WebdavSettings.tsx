import { useCallback, useEffect, useState } from 'react'
import { Cloud, Download, RefreshCw, Upload } from 'lucide-react'
import { newCommandId } from '../../shared/command-id'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
type Settings={revision:string|null;endpoint:string;username:string;passwordSet:boolean;allowInsecure:boolean}
type Snapshot={id:string;createdAt:string;sha256:string;bytes:number}
const empty:Settings={revision:null,endpoint:'',username:'',passwordSet:false,allowInsecure:false}
// Codeg config-sync-settings: account, saved-password state and explicit remote
// operations. Encrypted backup archives are published as immutable snapshots.
export function WebdavSettings():React.JSX.Element{
  const client=useThreadClient(),native=client.webdavWorkspace,t=useUiText()
  const [saved,setSaved]=useState<Settings>(empty),[draft,setDraft]=useState<Settings>(empty),[password,setPassword]=useState(''),[clearPassword,setClearPassword]=useState(false)
  const [entries,setEntries]=useState<Snapshot[]>([]),[busy,setBusy]=useState(false),[dirty,setDirty]=useState(false),[error,setError]=useState<string|null>(null),[message,setMessage]=useState<string|null>(null)
  const [pending,setPending]=useState<{commandId:string;operation:'push'|'pull'}|null>(null)
  const accept=(value:Settings):void=>{setSaved(value);setDraft(value);setPassword('');setClearPassword(false);setDirty(false)}
  const load=useCallback(async()=>accept(await client.request<Settings>('backup.webdav.settings',{operation:'get'})),[client])
  useEffect(()=>{void load().catch(failure=>setError(String(failure)))},[load])
  useEffect(()=>{const guard=(event:BeforeUnloadEvent):void=>{if(dirty||busy){event.preventDefault();event.returnValue=''}};window.addEventListener('beforeunload',guard);return()=>window.removeEventListener('beforeunload',guard)},[dirty,busy])
  const change=(value:Partial<Settings>):void=>{setDraft(previous=>({...previous,...value}));setDirty(true)}
  const run=async(action:()=>Promise<void>):Promise<void>=>{if(busy)return;setBusy(true);setError(null);setMessage(null);try{await action()}catch(failure){setError(String(failure))}finally{setBusy(false)}}
  const list=async():Promise<void>=>{if(!native||!saved.revision)return;const result=await native.request<{entries:Snapshot[]}>({operation:'list',expectedRevision:saved.revision});if(result)setEntries(result.entries)}
  const transfer=async(operation:'push'|'pull',snapshot?:Snapshot):Promise<void>=>{
    if(!native||!saved.revision)return
    const commandId=newCommandId();setPending({commandId,operation})
    const result=await native.request<{path:string;snapshot:Snapshot}>({operation,expectedRevision:saved.revision,commandId,...(snapshot?{snapshotId:snapshot.id,expectedDigest:snapshot.sha256}:{})})
    if(result){setMessage(t(operation==='push'?'加密备份已上传。':'加密备份已下载，可在“备份与恢复”中预览并选择恢复。')+(operation==='pull'?` ${result.path}`:''));await list()}
    setPending(null)
  }
  return <section className="section-block general-settings-section" aria-labelledby="webdav-settings-heading">
    <div className="section-heading"><div><h2 id="webdav-settings-heading"><Cloud size={17}/> WebDAV</h2><p>{t('将加密备份保存到你的 WebDAV 文件夹，并在其他主机下载。')}</p></div></div>
    <form className="forge-form" onSubmit={event=>{event.preventDefault();void run(async()=>{const result=await client.request<Settings>('backup.webdav.settings',{operation:'set',expectedRevision:saved.revision,endpoint:draft.endpoint,username:draft.username,password,clearPassword,allowInsecure:draft.allowInsecure});accept(result);setEntries([]);setMessage(t('WebDAV 设置已保存。'))})}}>
      <label>{t('远程文件夹地址')}<input type="url" value={draft.endpoint} disabled={busy} placeholder="https://dav.example.com/Rovai/" onChange={event=>change({endpoint:event.target.value})}/></label>
      <label>{t('账号')}<input autoComplete="off" value={draft.username} disabled={busy} onChange={event=>change({username:event.target.value})}/></label>
      <label>{t('密码或应用专用密码')}<input type="password" autoComplete="off" value={password} disabled={busy} placeholder={saved.passwordSet?t('已保存，留空保留当前账号密码'):''} onChange={event=>{setPassword(event.target.value);setDirty(true)}}/></label>
      <label><input type="checkbox" disabled={busy} checked={clearPassword} onChange={event=>{setClearPassword(event.target.checked);setDirty(true)}}/>{t('清除保存的密码')}</label>
      <label><input type="checkbox" disabled={busy} checked={draft.allowInsecure} onChange={event=>change({allowInsecure:event.target.checked})}/>{t('允许使用 HTTP 地址连接此服务器')}</label>
      <div className="workbench-toolbar"><button disabled={busy||!dirty||!draft.endpoint.trim()} type="submit">{t('保存')}</button><button disabled={busy||!dirty} type="button" onClick={()=>accept(saved)}>{t('放弃修改')}</button></div>
    </form>
    {error&&<p className="workbench-error" role="alert">{error}</p>}{message&&<p role="status">{message}</p>}
    {native?<div className="workbench-toolbar"><button disabled={busy||dirty||!saved.revision} onClick={()=>void run(async()=>{await native.request({operation:'probe',expectedRevision:saved.revision!});setMessage(t('已连接远程文件夹。'))})}>{t('检查连接')}</button><button disabled={busy||dirty||!saved.revision} onClick={()=>void run(async()=>{await native.request({operation:'create_collection',expectedRevision:saved.revision!});setMessage(t('远程文件夹已准备好。'))})}>{t('创建远程文件夹')}</button><button disabled={busy||dirty||!saved.revision} onClick={()=>void run(list)}><RefreshCw size={14}/>{t('读取远程备份')}</button><button disabled={busy||dirty||!saved.revision||pending!==null} onClick={()=>void run(()=>transfer('push'))}><Upload size={14}/>{t('上传加密备份')}</button></div>:<p>{t('文件传输在桌面主机完成；独立服务器提供对应的 WebDAV 命令。')}</p>}
    {pending&&<div role="status"><p>{t(busy?'正在传输加密备份…':'原传输的结果需要核对。')} {pending.commandId}</p><button disabled={busy||!saved.revision} onClick={()=>void run(async()=>{const result=await native?.request<{state:string;message?:string;result?:{path:string}}>({operation:'status',expectedRevision:saved.revision!,commandId:pending.commandId});if(result?.state==='completed'){setPending(null);setMessage(t('原传输已经完成。'));await list()}else setMessage(t(result?.state==='started'?'原传输仍在进行，请稍后再核对。':'原传输的结果尚不能确定，已保留记录，没有重复发送。'))})}>{t('核对原传输')}</button></div>}
    {entries.length>0&&<div className="webdav-snapshot-list">{entries.map(snapshot=><div key={snapshot.id} className="workbench-import-row"><Cloud size={18}/><div><strong>{new Date(snapshot.createdAt).toLocaleString()}</strong><small>{(snapshot.bytes/1024/1024).toFixed(1)} MB · {snapshot.sha256.slice(0,12)}</small></div><button disabled={busy||dirty||pending!==null} onClick={()=>void run(()=>transfer('pull',snapshot))}><Download size={14}/>{t('下载')}</button></div>)}</div>}
  </section>
}
