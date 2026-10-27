import { useEffect, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { AppDialogBody, AppDialogContent, AppDialogFooter, AppDialogHeader } from '../../desktop/src/renderer/src/AppDialog'
import type { ConsoleClient } from './client'

/** Reuses the original Coaxi dialog and sidebar status, without another visual shell. */
export function MissionFilesRecovery({ transport }: { transport: ConsoleClient }) {
  const [, update] = useState(0)
  const [open, setOpen] = useState(false)
  const [busy, setBusy] = useState(false)
  const [issue, setIssue] = useState('')
  const [selected, setSelected] = useState<string | null>(null)
  const input = useRef<HTMLInputElement>(null)
  useEffect(() => transport.onPendingCommandsChanged(() => update(value => value + 1)), [transport])
  const rows = transport.pendingMissionFiles()
  if (!rows.length) return null
  const work = async (action: () => Promise<void>): Promise<void> => {
    if (busy) return
    setBusy(true); setIssue('')
    try { await action() } catch (error) { setIssue(error instanceof Error ? error.message : '原提交尚未核对完成。') }
    finally { setBusy(false); update(value => value + 1) }
  }
  return <>
    <button type="button" className="quiet-button compact" onClick={() => setOpen(true)}>使命提交待核对 · {rows.length}</button>
    <Dialog.Root open={open} onOpenChange={setOpen}><Dialog.Portal><Dialog.Overlay className="dialog-overlay" />
      <AppDialogContent width="wide"><AppDialogHeader title="核对使命提交" description="先查询原结果，再按原请求继续；刷新后需要重新选择原附件。" />
        <AppDialogBody>
          {issue && <p role="alert">{issue}</p>}
          {rows.map(row => <section key={row.commandId} className="settings-section">
            <h3>{row.intent.operation === 'missions.create' ? '创建使命' : '保存使命'} · {'title' in row.intent.command ? row.intent.command.title : '使命附件'}</h3>
            <p>原定义与附件已经保留。已成功时只取回结果；还未知时继续保留，不重新创建。</p>
            <ul>{row.intent.files.map(file => <li key={file.id}>{file.displayName} · {file.byteSize.toLocaleString()} 字节</li>)}</ul>
            <div className="compact-footer">
              <button className="compact-cancel" disabled={busy} onClick={() => void work(() => transport.reconcilePending())}>核对原结果</button>
              <button className="compact-primary" disabled={busy} onClick={() => {
                if (row.needsFiles && row.intent.files.length) { setSelected(row.commandId); input.current?.click() }
                else void work(() => transport.retryMissionFiles(row.commandId))
              }}>{row.needsFiles && row.intent.files.length ? '选择原附件并继续' : '按原请求继续'}</button>
            </div>
          </section>)}
          <input ref={input} type="file" multiple hidden onChange={event => {
            const files = Array.from(event.currentTarget.files ?? []); event.currentTarget.value = ''
            if (selected && files.length) void work(() => transport.resumeMissionFiles(selected, files))
          }} />
        </AppDialogBody><AppDialogFooter><button type="button" className="compact-cancel" onClick={() => setOpen(false)}>稍后处理</button></AppDialogFooter>
      </AppDialogContent>
    </Dialog.Portal></Dialog.Root>
  </>
}
