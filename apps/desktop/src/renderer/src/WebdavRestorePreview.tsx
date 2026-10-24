import { useEffect, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { RotateCcw } from 'lucide-react'
import type { BackupSummary, ConfigurationSnapshotPreview, PortableArchiveReference } from '@contracts'
import {PortableConfigurationMigration} from './PortableConfigurationMigration'
import { AppDialogContent, AppDialogHeader } from './AppDialog'
import { SnapshotValues } from './ConfigurationSnapshotSettings'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'

type Preview = ConfigurationSnapshotPreview & {kind: 'configuration'; selectionId: string} | BackupSummary & {kind: 'backup'; archiveId: string} | {kind:'configuration-migration';archive:PortableArchiveReference}
type Transfer = {commandId: string; result: {snapshot: {id: string; sha256: string; kind?: 'backup' | 'configuration'}}}
const names: Record<string, string> = {appearance: '外观', general: '界面偏好', windowClose: '关闭窗口', messageLibrary: '全局消息库'}

/** Codeg backup preview/confirm flow, handed off from a verified local pull. */
export function WebdavRestorePreview({transfer, accountRevision, disabled}: {transfer: Transfer; accountRevision: string; disabled: boolean}): React.JSX.Element | null {
  const client = useThreadClient(), native = client.webdavWorkspace, backups = client.nativeBackups, t = useUiText()
  const [open, setOpen] = useState(false), [busy, setBusy] = useState(false), [passphrase, setPassphrase] = useState(''), [confirmation, setConfirmation] = useState('')
  const [preview, setPreview] = useState<Preview | null>(null), [error, setError] = useState<string | null>(null)
  const epoch = useRef(0)
  useEffect(() => {epoch.current++; setOpen(false); setPreview(null); setPassphrase(''); setConfirmation(''); setError(null); return () => {epoch.current++}}, [accountRevision, transfer.commandId])
  const close = (): void => {if (busy) return; epoch.current++; setOpen(false); setPreview(null); setPassphrase(''); setConfirmation(''); setError(null)}
  const submit = async (): Promise<void> => {
    if (!native || !backups || busy || disabled) return
    const current = epoch.current, secret = passphrase; setBusy(true); setError(null)
    try {
      if (!preview) {
        const result = await native.request<Preview>({operation: 'restore_inspect', expectedRevision: accountRevision, commandId: transfer.commandId, snapshotId: transfer.result.snapshot.id, expectedDigest: transfer.result.snapshot.sha256, passphrase: secret})
        if (current === epoch.current && result) setPreview(result)
      } else if (preview.kind === 'configuration-migration') {
        return
      } else if (preview.kind === 'configuration') {
        if (confirmation !== '应用配置') throw new Error(t('请先输入“应用配置”确认。'))
        await backups.applyConfiguration(preview.selectionId, secret)
      } else {
        if (confirmation !== '恢复备份') throw new Error(t('请先输入“恢复备份”确认。'))
        await backups.restore(preview.archiveId, secret)
      }
    } catch (failure) {if (current === epoch.current) setError(String(failure))}
    finally {if (current === epoch.current) {setBusy(false); setPassphrase('')}}
  }
  if (!native || !backups) return null
  if(preview?.kind==='configuration-migration')return <>
    <button className="quiet-button" disabled={disabled||busy} onClick={()=>setOpen(true)}><RotateCcw size={14}/>{t('选择要搬入的配置…')}</button>
    <Dialog.Root open={open} onOpenChange={next=>{if(!next)close()}}><Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent width="wide"><AppDialogHeader title={t('选择要搬入的配置和本机队员')} description={t('副本已核对并登记为候选。每项由你选择，历史任务保留，不会自动运行。')} closeDisabled={busy}/><PortableConfigurationMigration initialArchive={preview.archive} embedded/><div className="dialog-actions"><button className="quiet-button" disabled={busy} onClick={close}>{t('关闭')}</button></div></AppDialogContent></Dialog.Portal></Dialog.Root>
  </>
  return <>
    <button className="quiet-button" disabled={disabled || busy} onClick={() => {setOpen(true); setPreview(null); setPassphrase(''); setError(null); setConfirmation('')}}><RotateCcw size={14}/>{t('预览并恢复…')}</button>
    <Dialog.Root open={open} onOpenChange={next => {if (!next) close()}}><Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent width={preview?.kind === 'configuration' ? 'wide' : 'compact'} tone={preview ? 'danger' : 'neutral'}>
      <AppDialogHeader title={t(!preview ? '读取已下载的加密副本' : preview.kind === 'configuration' ? '确认应用配置并重启' : '确认恢复备份并重启')} description={t(preview ? '只应用下面已预览的副本。恢复前会结束当前工作；如本机内容已变化，需要重新预览。' : '读取本次下载回执对应的本机文件，核对摘要后预览。此时不会应用更改。')} closeDisabled={busy}/>
      <form className="forge-form" onSubmit={event => {event.preventDefault(); void submit()}}>
        <p>{t(transfer.result.snapshot.kind === 'configuration' ? '配置快照' : '完整备份')} · <code>{transfer.result.snapshot.sha256.slice(0, 12)}</code></p>
        {preview && <div className="backup-preview-summary"><p>{t('副本版本')} · {preview.appVersion} · {new Date(preview.createdAt).toLocaleString()}</p>
          {preview.kind === 'configuration' ? preview.changes.map(change => <details key={change.domain}><summary>{t(names[change.domain] ?? change.domain)}</summary><div className="configuration-compare"><div><h4>{t('当前配置')}</h4><SnapshotValues domain={change.domain} value={change.before}/></div><div><h4>{t('应用后的配置')}</h4><SnapshotValues domain={change.domain} value={change.after}/></div></div></details>) : <><p>{t('文件数量')}：{preview.fileCount} · {(preview.totalBytes / 1048576).toFixed(1)} MB</p>{preview.roots?.map(root => <p key={root.id}>{root.id} · {t(root.present ? '包含数据' : '原副本中未启用')}{root.shared ? ' · ' + t('与其他用途共享目录') : ''}</p>)}</>}
        </div>}
        <label>{t(preview ? '重新输入加密口令' : '加密口令')}<input className="workspace-field" type="password" autoComplete="off" disabled={busy} value={passphrase} onChange={event => setPassphrase(event.target.value)} data-dialog-autofocus/></label>
        {preview && <label>{t(preview.kind === 'configuration' ? '输入“应用配置”确认' : '输入“恢复备份”确认')}<input className="workspace-field" disabled={busy} value={confirmation} onChange={event => setConfirmation(event.target.value)}/></label>}
        {error && <p className="workbench-error" role="alert">{error}</p>}
        <div className="dialog-actions"><button className="quiet-button" type="button" disabled={busy} onClick={close}>{t('取消')}</button>{preview && <button className="quiet-button" type="button" disabled={busy} onClick={() => {setPreview(null); setConfirmation(''); setError(null)}}>{t('重新预览')}</button>}<button className="quiet-button" disabled={busy || disabled || !passphrase || preview !== null && confirmation !== (preview.kind === 'configuration' ? '应用配置' : '恢复备份') || preview?.kind === 'configuration' && !preview.changes.length}>{t(busy ? '处理中…' : preview ? '确认并重启' : '读取并预览')}</button></div>
      </form>
    </AppDialogContent></Dialog.Portal></Dialog.Root>
  </>
}
