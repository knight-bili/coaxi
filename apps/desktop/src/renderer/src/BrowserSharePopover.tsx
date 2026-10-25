import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import * as Popover from '@radix-ui/react-popover'
import { Bot, Eye, MousePointerClick, ShieldOff, Check, ChevronDown } from 'lucide-react'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { newCommandId } from '../../shared/command-id'
import type { InteractionFeatures } from './MemberInteractionSettings'

type Scope = { threadId: string; conversationId: string | null; scopeKind: 'public' | 'single_chat' | 'delegation_child' }
interface Target { id: string; agentId: string; displayName: string; scope: Scope }
interface Grant { grantId: string; agentId: string; targetId: string; targetKind: string; origin: string | null; level: 'read' | 'control'; grantGeneration: number; expiresAt: string }
interface Policy { schemaVersion: 1; revision: number; members: Array<{ agentId: string; features: InteractionFeatures }> }
const emptyFeatures = (): InteractionFeatures => ({ browser: false, browserEval: false, computer: false, computerLaunch: false, computerClipboard: false })
const knownRejection = /interaction_disabled|desktop_unavailable|scope_changed|origin_changed|membership_changed|permission_changed|policy_changed|invalid_arguments/

/** Codeg agent-access menu, with Core's per-recipient permissions retained. */
export function BrowserSharePopover({ threadId, targetId, origin }: { threadId: string; targetId: string; origin: string | null }): React.JSX.Element {
  const api = useThreadClient().interactionWorkspace, t = useUiText(), epoch = useRef(0), lock = useRef<{epoch:number}|null>(null)
  const [open, setOpen] = useState(false), [targets, setTargets] = useState<Target[]>([]), [byTarget, setByTarget] = useState<Record<string, Grant[]>>({})
  const [policy, setPolicy] = useState<Policy | null>(null), [mode, setMode] = useState<'all' | 'selected'>('all'), [selected, setSelected] = useState<string[]>([])
  const [busy, setBusy] = useState(false), [loading, setLoading] = useState(true), [error, setError] = useState(''), [uncertain, setUncertain] = useState(false)
  const live = (row: Target): Grant[] => (byTarget[row.id] ?? []).filter(grant => grant.targetKind === 'browser_tab' && grant.targetId === targetId && grant.origin === origin && Date.parse(grant.expiresAt) > Date.now())
  const count = targets.filter(row => live(row).length).length
  const controlCount = targets.filter(row => live(row).some(grant => grant.level === 'control')).length
  const chosen = mode === 'all' ? targets : targets.filter(row => selected.includes(row.id))
  const missing = chosen.filter(row => !policy?.members.find(member => member.agentId === row.agentId)?.features.browser)
  const refresh = async (generation = epoch.current): Promise<void> => {
    if (!api) return
    const nextPolicy = await api.request<Policy>('policy.get', {})
    const rows: Target[] = []; let after: string | null = null
    do {
      const page: { targets: Target[]; next: string | null } = await api.request('sharing.targets', { threadId, after })
      rows.push(...page.targets.filter(row => row.scope.threadId === threadId && row.scope.scopeKind === 'public')); after = page.next
      if (rows.length > 1000) throw Error('可共享队员数量超过本次读取范围。')
    } while (after)
    const unique = [...new Map(rows.map(row => [row.id, row])).values()]
    const grants = await Promise.all(unique.map(async row => [row.id, (await api.request<{ grants: Grant[] }>('grant.list', { scope: row.scope, agentId: row.agentId })).grants] as const))
    if (epoch.current === generation) { setPolicy(nextPolicy); setTargets(unique); setByTarget(Object.fromEntries(grants)); setUncertain(false); setLoading(false) }
  }
  useEffect(() => {
    const generation = ++epoch.current
    setLoading(true); setError('')
    const read = (): void => { if (!lock.current) void refresh(generation).catch(failure => { if (epoch.current === generation) { setError(readErrorMessage(failure)); setLoading(false) } }) }
    read(); const timer = setInterval(read, 5000)
    return () => { epoch.current++; clearInterval(timer) }
  }, [api, threadId, targetId, origin, open])
  useLayoutEffect(() => { epoch.current++; lock.current=null; setBusy(false); setUncertain(false); setTargets([]); setByTarget({}); setPolicy(null); setSelected([]); setMode('all') }, [api, threadId, targetId, origin])
  const apply = async (level: 'read' | 'control' | 'none'): Promise<void> => {
    if (!api || lock.current || uncertain || !origin || !policy || (level !== 'none' && (!chosen.length || missing.length))) return
    const generation = epoch.current, operation = {epoch:generation}; lock.current = operation; setBusy(true); setError('')
    const assertCurrent=():void=>{if(epoch.current!==generation||lock.current!==operation)throw Error('原页面已切换')}
    let writes = 0
    try {
      // Read back current grants before applying a new intent; do not replay an unknown batch.
      const recipients = level === 'none' ? [] : chosen
      for (const row of targets.filter(row => !recipients.some(recipient => recipient.id === row.id))) {
        for (const grant of live(row)) { assertCurrent(); await api.request('grant.revoke', { grantId: grant.grantId }); writes++ }
      }
      for (const row of recipients) {
        const grants = byTarget[row.id] ?? [], current = live(row)[0]
        const last = grants.filter(grant => grant.targetKind === 'browser_tab' && grant.targetId === targetId).reduce((max, grant) => Math.max(max, grant.grantGeneration), 0)
        assertCurrent()
        await api.request('grant.share', { schemaVersion: 1, grantId: current?.grantId ?? newCommandId(), scope: row.scope, agentId: row.agentId, desktopPeerId: '', targetKind: 'browser_tab', targetId, origin, level, grantGeneration: last + 1, expiresAt: new Date(Date.now() + 30 * 60000).toISOString() }); writes++
      }
      assertCurrent(); await refresh(generation)
      if (epoch.current === generation) setOpen(false)
    } catch (failure) {
      if (epoch.current === generation) {
        const message = readErrorMessage(failure)
        setError(message.includes('interaction_disabled') ? t('所选队员尚未启用浏览器工具。请先启用，再共享页面。') : message.includes('desktop_unavailable') ? t('桌面连接正在准备，请稍后重试。') : message)
        if (writes > 0 || !knownRejection.test(message)) {
          setUncertain(true)
          try { await refresh(generation) } catch { /* Preserve explicit recovery until a full read succeeds. */ }
        }
      }
    } finally { if(lock.current===operation)lock.current = null; if (epoch.current === generation) setBusy(false) }
  }
  const enable = async (): Promise<void> => {
    if (!api || !policy || busy || !missing.length) return
    const generation = epoch.current, operation = {epoch:generation}; lock.current = operation; setBusy(true); setError('')
    try {
      const members = new Map(policy.members.map(member => [member.agentId, member]))
      for (const row of missing) members.set(row.agentId, { agentId: row.agentId, features: { ...(members.get(row.agentId)?.features ?? emptyFeatures()), browser: true } })
      await api.request('policy.replace', { expectedRevision: policy.revision, value: { ...policy, revision: policy.revision + 1, members: [...members.values()] } })
      await refresh(generation)
    } catch (failure) { if (epoch.current === generation) setError(t('工具配置未确认，请刷新后核对。') + ' ' + readErrorMessage(failure)) }
    finally { if(lock.current===operation)lock.current = null; if (epoch.current === generation) setBusy(false) }
  }
  const disabled = busy || loading || uncertain || !chosen.length || missing.length > 0
  return <Popover.Root open={open} onOpenChange={value => { if (!busy) setOpen(value) }}><Popover.Trigger asChild><button type="button" className={count ? 'browser-share-pill is-shared' : 'browser-share-pill'} disabled={!api || !origin} aria-label={t('页面共享权限')}><Bot size={14}/>{t(count ? controlCount ? '已共享 · 可操作' : '已共享 · 只读' : '未共享')}<ChevronDown size={12}/></button></Popover.Trigger><Popover.Portal><Popover.Content className="workspace-popover browser-share-popover" align="start" sideOffset={8}>
    <div className="browser-share-summary"><strong>{t(count ? '队员可以读取这个页面' : '把页面共享给队员')}</strong><p>{origin}</p><small>{t('仅在当前对话生效。离开此网站后共享结束。')}</small></div>
    <div className="browser-share-scope"><button type="button" aria-pressed={mode === 'all'} disabled={busy} onClick={() => setMode('all')}>{t('当前全体队员')}{mode === 'all' && <Check size={14}/>}</button><button type="button" aria-pressed={mode === 'selected'} disabled={busy} onClick={() => { setMode('selected'); setSelected(targets.filter(row => live(row).length).map(row => row.id)) }}>{t('指定队员')}{mode === 'selected' && <Check size={14}/>}</button></div>
    {mode === 'selected' && <div className="browser-share-recipients">{targets.map(row => <label key={row.id}><input type="checkbox" checked={selected.includes(row.id)} disabled={busy} onChange={event => setSelected(previous => event.target.checked ? [...previous, row.id] : previous.filter(id => id !== row.id))}/><span>{row.displayName}</span><small>{t(live(row).some(grant => grant.level === 'control') ? '可操作' : live(row).length ? '只读' : '未共享')}</small></label>)}</div>}
    {loading ? <p role="status">{t('正在读取共享状态…')}</p> : !targets.length ? <p>{t('请先在当前对话中启动队员，再共享页面。')}</p> : missing.length > 0 && <div className="browser-share-setup"><p>{t('尚未启用浏览器工具')}：{missing.map(row => row.displayName).join('、')}</p><button type="button" className="quiet-button" disabled={busy} onClick={() => void enable()}>{t('为所选队员启用浏览器工具')}</button><small>{t('工具配置对新执行生效；不会开启网页代码、电脑或剪贴板权限。')}</small></div>}
    {error && <p role="alert" className="workbench-error">{error}</p>}{uncertain && <p>{t('共享结果尚未核实，请刷新后再操作。')}</p>}
    <div className="browser-share-commands"><button type="button" disabled={disabled} onClick={() => void apply('read')}><Eye size={16}/>{t('仅查看')}</button><button type="button" disabled={disabled} onClick={() => void apply('control')}><MousePointerClick size={16}/>{t('允许操作')}</button><button type="button" disabled={busy || loading || uncertain || !count} onClick={() => void apply('none')}><ShieldOff size={16}/>{t('停止共享')}</button></div>
    <div className="browser-share-footnote"><small>{t('30 分钟后自动结束')}</small><button type="button" className="quiet-button" disabled={busy} onClick={() => { setLoading(true); void refresh().then(() => setError('')).catch(failure => { setError(readErrorMessage(failure)); setLoading(false) }) }}>{t('刷新状态')}</button></div>
  </Popover.Content></Popover.Portal></Popover.Root>
}
