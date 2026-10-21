import { useEffect, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { AppDialogContent, AppDialogHeader } from './AppDialog'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { newCommandId } from '../../shared/command-id'

type Plan = {commandId: string; createdAt: string; items: Array<{id: string; bytes: number; sha256: string; at: string}>; removed: string[]; bytes: number; removedBytes: number; kept?: number; remaining?: number}
export function ConfigurationCacheCleanup({revision, disabled}: {revision: string | null; disabled: boolean}): React.JSX.Element | null {
  const native = useThreadClient().webdavWorkspace, t = useUiText()
  const [plan, setPlan] = useState<Plan | null>(null), [busy, setBusy] = useState(false), [unknown, setUnknown] = useState(false)
  const [error, setError] = useState<string | null>(null), [notice, setNotice] = useState<string | null>(null)
  useEffect(() => {setPlan(null); setUnknown(false); setError(null); setNotice(null)}, [revision])
  const run = async (action: () => Promise<void>): Promise<void> => {if (!native || busy || disabled) return; setBusy(true); setError(null); try {await action()} catch (failure) {setError(String(failure))} finally {setBusy(false)}}
  if (!native) return null
  const removed = new Set(plan?.removed ?? [])
  return <section className="section-block general-settings-section"><div className="section-heading"><div><h3>{t('本机临时配置缓存')}</h3><p>{t('保留最近十份已上传缓存。每批最多清理二十份旧缓存；未完成或结果不明的任务、用户备份、远端副本和传输回执均保留。')}</p></div></div>
    <button disabled={busy || disabled} onClick={() => void run(async () => {const result = await native.request<Plan>({operation: 'config_cache_preview', expectedRevision: revision, commandId: newCommandId()}); if (result) {setPlan(result); setUnknown(false); setNotice(null)}})}>{t('预览可清理缓存')}</button>
    {notice && <p role="status">{notice}</p>}{error && !plan && <p className="workbench-error" role="alert">{error}</p>}
    <Dialog.Root open={plan !== null} onOpenChange={open => {if (!open && !busy) setPlan(null)}}><Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent width="compact" tone="danger"><AppDialogHeader title={t('清理本机临时配置')} description={t('只删除下面列出的自动上传缓存。清理后仍保留原传输回执，远端副本不会改变。')} closeDisabled={busy}/>
      {plan && <><p>{t('本批')} {plan.items.length} {t('份')} · {(plan.bytes / 1048576).toFixed(1)} MB</p>{plan.kept !== undefined && <p>{t('最近保留')} {plan.kept} {t('份')}{plan.remaining !== undefined && plan.remaining > plan.items.length ? ' · ' + t('其余旧缓存可在下一批预览') : ''}</p>}
        <ul>{plan.items.map(item => <li key={item.id}>{new Date(item.at).toLocaleString()} · {(item.bytes / 1048576).toFixed(1)} MB{removed.has(item.id) ? ' · ' + t('已清理') : ''}</li>)}</ul>
        {error && <p role="alert" className="workbench-error">{error}</p>}
        <div className="dialog-actions"><button disabled={busy} onClick={() => setPlan(null)}>{t('关闭')}</button>{unknown && <button disabled={busy || disabled} onClick={() => void run(async () => {const status = await native.request<Plan>({operation: 'config_cache_status', expectedRevision: revision, commandId: plan.commandId}); if (status) {setPlan(status); setNotice(t('已读取原批次记录。'))}})}>{t('核对原批次')}</button>}
          <button disabled={busy || disabled || !plan.items.length || plan.removed.length === plan.items.length} onClick={() => void run(async () => {const selected = plan; try {const result = await native.request<Plan>({operation: 'config_cache_remove', expectedRevision: revision, commandId: selected.commandId, confirmed: true}); if (result) {setPlan(result); setUnknown(false); setNotice(t('已清理本批临时配置，原传输回执仍保留。'))}} catch (failure) {setUnknown(true); throw failure}})}>{t(unknown ? '继续原清理批次' : '确认清理本批')}</button>
        </div></>}
    </AppDialogContent></Dialog.Portal></Dialog.Root>
  </section>
}
