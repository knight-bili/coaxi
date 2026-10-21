import { useEffect, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Archive, Download, Lock, RotateCcw } from 'lucide-react'
import type { BackupSummary } from '@contracts'
import { AppDialogContent, AppDialogHeader } from './AppDialog'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
// Codeg backup-settings: encrypted export, inspect, then explicit restart to
// restore. Passwords stay local to the current input and native helper pipe.
export function BackupSettings():React.JSX.Element{
  const api=useThreadClient().nativeBackups,t=useUiText()
  const [mode,setMode]=useState<'create'|'inspect'|null>(null),[passphrase,setPassphrase]=useState(''),[repeat,setRepeat]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null)
  const [selected,setSelected]=useState<(BackupSummary&{archiveId:string})|null>(null),[created,setCreated]=useState<(BackupSummary&{path:string})|null>(null),[restore,setRestore]=useState(false),[confirmation,setConfirmation]=useState('')
  const [progress,setProgress]=useState<{phase:string;done:number;total:number}|null>(null)
  useEffect(()=>api?.onProgress(setProgress),[api])
  const reset=():void=>{setPassphrase('');setRepeat('');setError(null);setProgress(null);setConfirmation('')}
  const submit=async():Promise<void>=>{
    if(!api||busy)return;setBusy(true);setError(null);setProgress(null)
    try{if(restore&&selected){await api.restore(selected.archiveId,passphrase)}else if(mode==='create'){if(passphrase!==repeat)throw new Error(t('两次输入的口令不一致。'));const value=await api.create(passphrase);if(value){setCreated(value);setMode(null)}}else{const value=await api.inspect(passphrase);if(value){setSelected(value);setMode(null)}}}
    catch(failure){setError(String(failure))}finally{setBusy(false);setPassphrase('');setRepeat('')}
  }
  const description=(value:BackupSummary):string=>`${new Date(value.createdAt).toLocaleString()} · ${value.fileCount} ${t('个文件')} · ${(value.totalBytes/1024/1024).toFixed(1)} MB`
  const roots=(value:BackupSummary):React.JSX.Element=><div>{(value.roots??[]).map(root=><p key={root.id}>{t(root.id==='skill_library'?'技能库':root.id==='runtime_files'?'运行文件':'MCP配置')} · {t(root.present?'已包含':'备份时不存在')}{root.shared?` · ${t('共享位置，恢复将替换这台主机的共用配置')}`:''}</p>)}{value.formatVersion===1&&<p>{t('这是旧格式备份，覆盖原数据目录。')}</p>}</div>
  return <section className="section-block general-settings-section" aria-labelledby="backup-settings-heading">
    <div className="section-heading"><div><h2 id="backup-settings-heading"><Archive size={17}/> {t('备份与恢复')}</h2><p>{t('保存会话、画布、队员、使命及设置。备份加密后保存在你选择的位置。')}</p></div></div>
    {api?<><div className="workbench-toolbar"><button disabled={busy} onClick={()=>{reset();setMode('create')}}><Download size={14}/>{t('创建加密备份')}</button><button disabled={busy} onClick={()=>{reset();setMode('inspect')}}><RotateCcw size={14}/>{t('选择备份并预览')}</button></div>
      {created&&<p role="status">{t('已保存备份')}: {created.path}<br/>{description(created)}</p>}{created&&roots(created)}
      {selected&&<div className="backup-preview-summary"><strong>{t('所选备份')}</strong><p>{description(selected)}</p><p>{t('来源版本')}: {selected.appVersion}</p>{roots(selected)}<button disabled={busy} onClick={()=>{reset();setRestore(true)}}>{t('恢复这个备份…')}</button></div>}
    </>:<p>{t('请在数据所在主机的桌面设置中创建或恢复备份；独立服务器也提供离线备份命令。')}</p>}
    <Dialog.Root open={mode!==null||restore} onOpenChange={open=>{if(!open&&!busy){setMode(null);setRestore(false);reset()}}}>
      <Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent tone={restore?'danger':'neutral'} width="compact"><AppDialogHeader title={t(restore?'恢复并重启应用':mode==='create'?'创建加密备份':'预览加密备份')} description={t(restore?'当前数据会先保留为安全副本，随后恢复所选备份。恢复完成后应用重新启动。':mode==='create'?'工作区会短暂停止，备份保存后自动恢复运行。':'输入口令后选择备份。预览不会更改当前工作区。')} closeDisabled={busy}/>
        <form className="forge-form" onSubmit={event=>{event.preventDefault();void submit()}}><label><Lock size={14}/>{t('加密口令')}<input type="password" autoComplete="off" value={passphrase} disabled={busy} onChange={event=>setPassphrase(event.target.value)} data-dialog-autofocus/></label>
          {mode==='create'&&<label>{t('再次输入口令')}<input type="password" autoComplete="off" value={repeat} disabled={busy} onChange={event=>setRepeat(event.target.value)}/><small>{t('至少8个字符。请自己保管口令，应用不保存口令。')}</small></label>}
          {restore&&<><p>{selected?description(selected):''}</p>{selected&&roots(selected)}<label>{t('输入“恢复”确认替换当前工作区')}<input value={confirmation} disabled={busy} onChange={event=>setConfirmation(event.target.value)}/></label></>}
          {error&&<p className="workbench-error" role="alert">{error}</p>}
          {progress&&<div role="status"><span>{t(progress.phase==='archive'?'正在打包文件':'正在校验文件')} {progress.done}/{progress.total}</span><progress max={Math.max(progress.total,1)} value={progress.done}/></div>}
          <div className="dialog-actions"><button type="button" disabled={busy} onClick={()=>{setMode(null);setRestore(false);reset()}}>{t('取消')}</button><button type="submit" disabled={busy||!passphrase||(mode==='create'&&([...passphrase].length<8||repeat!==passphrase))||(restore&&confirmation!=='恢复')}>{t(busy?'处理中…':restore?'恢复并重启':mode==='create'?'选择保存位置并备份':'选择文件并预览')}</button></div>
        </form>
      </AppDialogContent></Dialog.Portal>
    </Dialog.Root>
  </section>
}
