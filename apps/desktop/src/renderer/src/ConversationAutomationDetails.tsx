import { useEffect, useRef, useState } from 'react'
import type { ConversationAutomationDetails as Details, ConversationAutomationOwner, ConversationQueuedMission, StoredCommandResult } from '@contracts'
import { useThreadClient } from './camp-client'
import { conversationOriginal, conversationToolRequest, type ConversationOriginal } from './conversation-tools-client'
import { newCommandId } from '../../shared/command-id'
import { useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { dateTimeLabel } from './automation-workspace-model'

/** 仅补充新工具创建的计划；原编辑器继续管理内容、队员和启用状态。 */
export function ConversationAutomationDetails({ automationId, version, disabled, onOpenThread }: { automationId: string; version: number; disabled: boolean; onOpenThread(id: string): void }): React.JSX.Element | null {
  const client = useThreadClient(), t = useUiText(), generation = useRef(0), lock = useRef(false), edited = useRef(false)
  const [owner, setOwner] = useState<ConversationAutomationOwner | null>(null), [action, setAction] = useState<ConversationAutomationOwner['action']>('launch_session'), [timezone, setTimezone] = useState('')
  const [items, setItems] = useState<ConversationQueuedMission[]>([]), [next, setNext] = useState<string | null>(null), [pending, setPending] = useState<ConversationOriginal | null>(null), [busy, setBusy] = useState(false), [error, setError] = useState(''), [ready, setReady] = useState(false)
  const key = `conversation-automation:${client.presentationScope ?? ''}:${automationId}`
  const load = async (more = false, reset = false): Promise<void> => {
    const observed = generation.current
    const value = await client.request<Details | null>('automations.get', { automationId, ...(more ? { queuedMissionAfter: next } : {}) })
    if (observed !== generation.current) return
    if (reset || !edited.current) { const current = value?.conversationToolOwner ?? null; setOwner(current); setAction(current?.action ?? 'launch_session'); setTimezone(current?.timezone ?? ''); edited.current = false }
    setItems(old => more ? [...old, ...(value?.queuedMissions?.items ?? []).filter(item => !old.some(previous => previous.occurrenceId === item.occurrenceId))] : value?.queuedMissions?.items ?? []); setNext(value?.queuedMissions?.next ?? null)
  }
  useEffect(() => {
    const observed = ++generation.current; edited.current = false; lock.current = false; setBusy(false); setReady(false); setOwner(null); setPending(null); setError(''); setItems([]); setNext(null)
    if (/^[0-9a-f-]{36}$/i.test(automationId)) void (async () => {const original = await conversationOriginal(client, key); await load(false, true); if (observed === generation.current) {setPending(original); setReady(true)}})().catch(reason => {if (observed === generation.current) setError(readErrorMessage(reason))})
    return () => {generation.current++}
  }, [client, automationId, key])
  useEffect(() => { if (ready && !lock.current) void load().catch(reason => setError(readErrorMessage(reason))) }, [version])
  const work = async (fn: (observed: number) => Promise<void>): Promise<void> => {if (lock.current) return; lock.current = true; setBusy(true); setError(''); const observed = generation.current; try {await fn(observed)} catch (reason) {if (observed === generation.current) setError(readErrorMessage(reason))} finally {if (observed === generation.current) {lock.current = false; setBusy(false)}}}
  const save = (): void => {void work(async observed => {
    if (!owner || pending) return
    const original: ConversationOriginal = {operation: 'conversationTools.automationOwner.set', commandId: newCommandId()}
    await conversationOriginal(client, key, original); if (observed !== generation.current) return; setPending(original)
    const value = await conversationToolRequest(client, 'conversationTools.automationOwner.set', {commandId: original.commandId, automationId, expectedAutomationVersion: owner.automationVersion, expectedOwnerRevision: owner.revision, action, timezone})
    if (observed !== generation.current) return
    await conversationOriginal(client, key, null); if (observed !== generation.current) return; setPending(null); setOwner(value); setAction(value.action); setTimezone(value.timezone); edited.current = false; await load()
  })}
  const reconcile = (): void => {void work(async observed => {
    if (!pending) return; const original = pending
    const value = await client.request<{state: string; result?: StoredCommandResult}>('commands.reconcile', {operation: original.operation, params: {commandId: original.commandId}})
    if (observed !== generation.current) return
    if (value.state !== 'recorded' || value.result?.commandId !== original.commandId || value.result.commandType !== original.operation) throw new Error(t('原保存请求没有确定回执，没有再次修改计划。'))
    await conversationOriginal(client, key, null); if (observed !== generation.current) return; setPending(null)
    if (value.result.status === 'rejected') throw new Error(value.result.code)
    edited.current = false; await load(false, true)
  })}
  if (!owner && !pending && !error) return null
  return <section className="automation-schedule" aria-label={t('计划执行方式与时区')}>
    <h2>{t('执行方式与时区')}</h2>
    {owner && <div className="automation-schedule-panel">
      <label className="automation-schedule-row"><span>{t('计划到期后')}</span><select className="automation-picker" value={action} disabled={disabled || busy || !!pending} onChange={event => {edited.current = true; setAction(event.target.value as ConversationAutomationOwner['action'])}}><option value="launch_session">{t('启动队员会话')}</option><option value="enqueue_task">{t('生成待启动的使命')}</option></select></label>
      <label className="automation-schedule-row"><span>{t('计划时区')}</span><input aria-label={t('计划时区')} value={timezone} placeholder="Asia/Shanghai" maxLength={128} disabled={disabled || busy || !!pending} onChange={event => {edited.current = true; setTimezone(event.target.value)}} /></label>
      <p className="automation-channel-note">{t(action === 'enqueue_task' ? '按计划生成使命，保留完整任务内容，等待你在使命板启动。' : '按计划使用当前队员和工程启动执行。')}{t('时区由主机核对；夏令时重复时刻只执行一次，跳过不存在的时刻。')}</p>
      {pending ? <button type="button" className="quiet-button" disabled={busy} onClick={reconcile}>{t('核对原保存请求')}</button> : <button type="button" className="quiet-button" disabled={disabled || busy || !timezone.trim()} onClick={save}>{t('保存执行方式与时区')}</button>}
      <button type="button" className="quiet-button" disabled={busy || !!pending} onClick={() => void work(async () => load(false, true))}>{t('重新载入已保存的计划设置')}</button>
    </div>}
    {!!items.length && <div className="automation-history"><h2>{t('按计划生成的使命')}</h2>{items.map(item => <button type="button" className="automation-history-row" key={item.occurrenceId} onClick={() => onOpenThread(item.threadId)}><span><strong>{item.title}</strong><small>{dateTimeLabel(item.scheduledAtUtc)}</small></span><span>{t('打开使命')}</span></button>)}{next && <button type="button" className="quiet-button" disabled={busy} onClick={() => void work(async () => load(true))}>{t('更早的使命')}</button>}</div>}
    {error && <p className="automation-history-error" role="alert">{error}</p>}
  </section>
}
