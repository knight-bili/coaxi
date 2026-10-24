import { useEffect, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Plus, Settings2, Trash2 } from 'lucide-react'
import type { BrowserPreferences, BrowserWorkspaceApi } from '@contracts'
import { AppDialogContent, AppDialogHeader } from './AppDialog'
import { useUiText } from './interface-language'
import { parseHostRulePattern } from '../../shared/codeg-browser-host-rules'

/** Codeg BrowserSettings: setting rows, explicit site rules and native data ownership. */
export function BrowserSettingsDialog({api, onChanged}: {api: BrowserWorkspaceApi; onChanged: (value: BrowserPreferences) => void}): React.JSX.Element {
  const t = useUiText()
  const [open, setOpen] = useState(false), [busy, setBusy] = useState(false)
  const [value, setValue] = useState<BrowserPreferences | null>(null), [error, setError] = useState<string | null>(null)
  const [pattern, setPattern] = useState(''), [action, setAction] = useState<'builtin' | 'system' | 'block'>('block')
  useEffect(() => {
    if (!open) return
    let cancelled = false
    setError(null); setValue(null)
    void api.preferences().then(result => { if (!cancelled) setValue(result) }).catch(failure => { if (!cancelled) setError(String(failure)) })
    return () => { cancelled = true }
  }, [api, open])
  const save = async (): Promise<void> => {
    if (!value || busy) return
    setBusy(true); setError(null)
    try { const result = await api.updatePreferences(value); setValue(result); onChanged(result); setOpen(false) }
    catch (failure) { setError(String(failure)) }
    finally { setBusy(false) }
  }
  return <Dialog.Root open={open} onOpenChange={next => { if (!busy) setOpen(next) }}>
    <Dialog.Trigger asChild><button className="quiet-button" type="button"><Settings2 size={14}/>{t('浏览设置')}</button></Dialog.Trigger>
    <Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent width="wide" tone="neutral"><AppDialogHeader title={t('浏览器设置')} description={t('设置网站规则、浏览代理与开发工具。保存后会收回已共享页面权限。')} closeDisabled={busy}/>
      {error && <p role="alert" className="workbench-error">{error}</p>}
      {value && <div className="browser-settings-content">
        <section className="section-block"><h3>{t('网站规则')}</h3><p>{t('支持 example.com、*.example.com、localhost:3000 和 [::1]:3000。更具体的规则优先；相同范围优先阻止。')}</p>
          <div className="browser-profile-list">{value.hostRules.map((rule, index) => <div className="browser-profile-row" key={`${rule.pattern}:${index}`}><span className="browser-download-name">{rule.pattern}</span><select className="workspace-field" aria-label={t('网站处理方式')} disabled={busy} value={rule.action} onChange={event => setValue({...value, hostRules: value.hostRules.map((row, i) => i === index ? {...row, action: event.target.value as typeof rule.action} : row)})}><option value="builtin">{t('内置浏览器')}</option><option value="system">{t('系统浏览器')}</option><option value="block">{t('阻止')}</option></select><button className="quiet-button" type="button" disabled={busy} aria-label={t('移除网站规则')} onClick={() => setValue({...value, hostRules: value.hostRules.filter((_, i) => i !== index)})}><Trash2 size={14}/></button></div>)}</div>
          <div className="workbench-toolbar"><input className="workspace-field" disabled={busy} aria-label={t('网站规则地址')} placeholder="*.example.com" value={pattern} onChange={event => setPattern(event.target.value)}/><select className="workspace-field" aria-label={t('新增规则处理方式')} disabled={busy} value={action} onChange={event => setAction(event.target.value as typeof action)}><option value="builtin">{t('内置浏览器')}</option><option value="system">{t('系统浏览器')}</option><option value="block">{t('阻止')}</option></select><button className="quiet-button" type="button" disabled={busy || !parseHostRulePattern(pattern) || value.hostRules.length >= 200} onClick={() => {setValue({...value, hostRules: [...value.hostRules, {pattern, action}]}); setPattern('')}}><Plus size={14}/>{t('添加规则')}</button></div>
        </section>
        <section className="section-block"><h3>{t('浏览代理')}</h3><p>{t('留空使用系统代理。填写 HTTP 或 SOCKS5 地址和端口；应用到此浏览器的所有身份和后续请求。')}</p><input className="workspace-field" aria-label={t('浏览器代理地址')} type="text" disabled={busy} placeholder="socks5://127.0.0.1:1080" value={value.proxy ?? ''} onChange={event => setValue({...value, proxy: event.target.value.trim() || null})}/></section>
        <section className="section-block"><label><input className="workspace-check" type="checkbox" disabled={busy} checked={value.devtools} onChange={event => setValue({...value, devtools: event.target.checked})}/>{t('允许打开网页开发工具')}</label><p>{t('更改后对新页面生效。开发工具由你手动打开。')}</p></section>
      </div>}
      <div className="dialog-actions"><button className="quiet-button" type="button" disabled={busy} onClick={() => setOpen(false)}>{t('取消')}</button><button className="quiet-button" type="button" disabled={busy || !value} onClick={() => void save()}>{t('保存')}</button></div>
    </AppDialogContent></Dialog.Portal>
  </Dialog.Root>
}
