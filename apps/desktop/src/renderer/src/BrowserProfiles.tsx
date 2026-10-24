import { useEffect, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Eraser, Plus, Trash2, UserRound } from 'lucide-react'
import type { BrowserProfileView, BrowserWorkspaceApi } from '@contracts'
import { AppDialogContent, AppDialogHeader } from './AppDialog'
import { useUiText } from './interface-language'

export function BrowserProfiles({ api, value, onChange }: { api: BrowserWorkspaceApi; value: string; onChange: (id: string) => void }): React.JSX.Element {
  const t = useUiText()
  const [catalog, setCatalog] = useState<{revision: string; profiles: BrowserProfileView[]} | null>(null)
  const [open, setOpen] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null)
  const [name, setName] = useState(''), [names, setNames] = useState<Record<string, string>>({})
  const [target, setTarget] = useState<{id: string; action: 'clear' | 'remove'; revision: string} | null>(null)
  useEffect(() => {
    let cancelled = false
    void api.profiles().then(value => { if (!cancelled) setCatalog(value) }).catch(failure => { if (!cancelled) setError(String(failure)) })
    return () => { cancelled = true }
  }, [api, open])
  const apply = async (input: Omit<Parameters<BrowserWorkspaceApi['profileControl']>[0], 'expectedRevision'> & {expectedRevision?: string}): Promise<void> => {
    if (!catalog || busy) return
    setBusy(true); setError(null)
    try {
      const next = await api.profileControl({ ...input, expectedRevision: input.expectedRevision ?? catalog.revision })
      setCatalog(next); setTarget(null); setName(''); setNames({})
      if (value !== 'temporary' && !next.profiles.some(row => row.id === value)) onChange('temporary')
    } catch (failure) { setError(String(failure)); try { setCatalog(await api.profiles()) } catch { /* Keep operation failure visible. */ } }
    finally { setBusy(false) }
  }
  const selected = target ? catalog?.profiles.find(row => row.id === target.id) : null
  return <>
    <select className="workspace-field" aria-label={t('新页面的浏览器身份')} value={value} onChange={event => onChange(event.target.value)} disabled={busy}>
      <option value="temporary">{t('临时浏览')}</option>{catalog?.profiles.map(row => <option key={row.id} value={row.id}>{row.name}</option>)}
    </select>
    <button className="quiet-button" type="button" onClick={() => setOpen(true)}><UserRound size={14}/>{t('浏览器身份')}</button>
    {!open && error && <span role="alert">{error}</span>}
    <Dialog.Root open={open} onOpenChange={value => { if (!busy) { setOpen(value); setTarget(null) } }}><Dialog.Portal><Dialog.Overlay className="dialog-overlay"/>
      <AppDialogContent width="wide" tone={target ? 'danger' : 'neutral'}><AppDialogHeader title={t(target ? target.action === 'clear' ? '清除浏览数据' : '删除浏览器身份' : '浏览器身份')} description={t('每个身份独立保存网站登录和浏览数据。页面创建后身份固定；临时浏览关闭后不保存登录。')} closeDisabled={busy}/>
        {error && <p role="alert" className="workbench-error">{error}</p>}
        {target ? <><p><strong>{selected?.name ?? target.id}</strong></p><p>{t('请先关闭这个身份的所有页面并结束下载。此操作会清除网站登录、网站数据和缓存，已下载文件会保留。')}</p><div className="dialog-actions"><button className="quiet-button" type="button" disabled={busy} onClick={() => setTarget(null)}>{t('返回')}</button><button className="quiet-button" type="button" disabled={busy || !selected || target.revision !== catalog?.revision} onClick={() => void apply({ action: target.action, id: target.id, confirmed: true, expectedRevision: target.revision })}>{t(target.action === 'clear' ? '确认清除' : '确认删除')}</button></div></> : <>
          <div className="browser-profile-list">{catalog?.profiles.map(row => <div className="browser-profile-row" key={row.id}>
            <UserRound size={16}/><input className="workspace-field" aria-label={t('浏览器身份名称')} maxLength={80} disabled={busy} value={names[row.id] ?? row.name} onChange={event => setNames(previous => ({ ...previous, [row.id]: event.target.value }))}/>
            <button className="quiet-button" type="button" disabled={busy || !(names[row.id]?.trim()) || names[row.id] === row.name} onClick={() => void apply({action: 'rename', id: row.id, name: names[row.id]})}>{t('保存')}</button>
            <button className="quiet-button" type="button" disabled={busy} onClick={() => setTarget({ id: row.id, action: 'clear', revision: catalog!.revision })}><Eraser size={14}/>{t('清除数据')}</button>
            {row.id !== 'default' && <button className="quiet-button" type="button" disabled={busy} aria-label={t('删除浏览器身份')} onClick={() => setTarget({ id: row.id, action: 'remove', revision: catalog!.revision })}><Trash2 size={14}/></button>}
          </div>)}</div>
          <form className="workbench-toolbar" onSubmit={event => { event.preventDefault(); void apply({action: 'create', name}) }}><input className="workspace-field" aria-label={t('新身份名称')} placeholder={t('新身份名称')} maxLength={80} disabled={busy} value={name} onChange={event => setName(event.target.value)}/><button className="primary-button" type="submit" disabled={busy || !name.trim() || !catalog}><Plus size={14}/>{t('新建身份')}</button></form>
        </>}
      </AppDialogContent>
    </Dialog.Portal></Dialog.Root>
  </>
}
