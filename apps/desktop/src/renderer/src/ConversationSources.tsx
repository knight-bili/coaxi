import { useEffect, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import type { ConversationGrantPreview, ConversationSourceDescriptor, ConversationSourceGrant, ConversationToolScope, StoredCommandResult } from '@contracts'
import { newCommandId } from '../../shared/command-id'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { conversationOriginal, conversationToolRequest, type ConversationOriginal } from './conversation-tools-client'
import './user-feedback.css'

export function conversationSourceReference(source: Pick<ConversationSourceDescriptor, 'title' | 'sourceSessionId'>): string {
  const title = source.title.replace(/[\r\n\t]+/g, ' ').replace(/([\\\[\]])/g, '\\$1')
  return `[参考：${title}](rovai-source:${source.sourceSessionId})`
}
type Sources = { items: ConversationSourceDescriptor[]; next: string | null }
type HistoryEntry = { id: string; linkedThreadId?: string; revision?: string; summary?: { title?: string } }
const kindText: Record<ConversationSourceDescriptor['sourceKind'], string> = { public: '团队讨论', single_chat: '私聊', delegation_child: '帮手讨论', imported: '导入的历史讨论' }
/** 沿原Rovai反馈弹窗与资料列表容器；引用进入已有消息草稿，不另建模型输入章节。 */
export function ConversationSources({ scope, disabled = false, onInsert, open: controlledOpen, onOpenChange }: { open?: boolean; onOpenChange?(value: boolean): void; scope: ConversationToolScope; disabled?: boolean; onInsert(source: ConversationSourceDescriptor): Promise<void> | void }): React.JSX.Element {
  const client = useThreadClient(), t = useUiText(), generation = useRef(0), locked = useRef(false)
  const [localOpen, setLocalOpen] = useState(false), open = controlledOpen ?? localOpen, setOpen = onOpenChange ?? setLocalOpen
  const [page, setPage] = useState<Sources>({ items: [], next: null }), [history, setHistory] = useState<HistoryEntry[]>([]), [historyLimit, setHistoryLimit] = useState(25)
  const [grants, setGrants] = useState<ConversationSourceGrant[]>([]), [grantNext, setGrantNext] = useState<string | null>(null), [selected, setSelected] = useState<ConversationSourceDescriptor | null>(null), [preview, setPreview] = useState<ConversationGrantPreview | null>(null), [confirmed, setConfirmed] = useState<ConversationSourceGrant | null>(null)
  const [pending, setPending] = useState<ConversationOriginal | null>(null), [busy, setBusy] = useState(false), [ready, setReady] = useState(false), [error, setError] = useState('')
  const scopeKey = JSON.stringify(scope), key = `conversation-sources:${client.presentationScope ?? ''}:${scopeKey}`
  const load = async (more = false): Promise<void> => {
    const observed = generation.current
    const result = await client.request<Sources>('history.library', { scope: { threadId: scope.threadId }, operation: 'sources', targetScope: scope, after: more ? page.next : null, limit: 50 })
    const grantPage = await conversationToolRequest(client, 'conversationTools.grants.list', { scope, after: null, limit: 100 })
    if (observed !== generation.current) return
    setPage(old => more ? { ...result, items: [...old.items, ...result.items.filter(item => !old.items.some(previous => previous.sourceSessionId === item.sourceSessionId))] } : result); setGrants(grantPage.items); setGrantNext(grantPage.next)
  }
  useEffect(() => {
    const observed = ++generation.current; locked.current = false; setBusy(false); setReady(false); setPage({ items: [], next: null }); setHistory([]); setSelected(null); setPreview(null); setConfirmed(null); setPending(null); setError('')
    if (open) void (async () => { const original = await conversationOriginal(client, key); await load(); if (observed === generation.current) { setPending(original); setReady(true) } })().catch(reason => { if (observed === generation.current) setError(readErrorMessage(reason)) })
    return () => { generation.current++ }
  }, [client, scopeKey, key, open])
  const work = async (action: (observed: number) => Promise<void>): Promise<void> => {
    if (locked.current) return
    locked.current = true; setBusy(true); setError(''); const observed = generation.current
    try { await action(observed) } catch (reason) { if (observed === generation.current) setError(readErrorMessage(reason)) }
    finally { if (observed === generation.current) { locked.current = false; setBusy(false) } }
  }
  const choose = (source: ConversationSourceDescriptor): void => { void work(async observed => {
    const value = await conversationToolRequest(client, 'conversationTools.grants.preview', { scope, sourceSessionId: source.sourceSessionId, sourceDigest: source.sourceDigest, throughSequence: source.throughSequence })
    if (observed === generation.current) { setSelected(source); setPreview(value); setConfirmed(null) }
  }) }
  const insert = async (grant: ConversationSourceGrant, observed: number): Promise<void> => {
    if (grant.scope.threadId !== scope.threadId || grant.scope.conversationId !== scope.conversationId || grant.scope.scopeKind !== scope.scopeKind || new Date(grant.expiresAt).getTime() <= Date.now()) throw new Error(t('这份授权已变化或过期，请重新选择资料。'))
    const source = selected?.sourceSessionId === grant.sourceSessionId ? selected : page.items.find(source => source.sourceSessionId === grant.sourceSessionId) ?? { sourceSessionId: grant.sourceSessionId, sourceDigest: grant.sourceDigest, throughSequence: grant.throughSequence, sourceKind: grant.sourceKind, title: t('旧讨论') }
    if (observed !== generation.current || disabled) throw new Error(t('当前输入已经变化，授权记录保留；请回到对应输入后加入。'))
    await onInsert(source); if (observed === generation.current) setOpen(false)
  }
  const confirm = (): void => { void work(async observed => {
    if (!preview || !selected || pending) return
    const original: ConversationOriginal = { operation: 'conversationTools.grants.confirm', commandId: newCommandId() }
    await conversationOriginal(client, key, original); if (observed !== generation.current) return; setPending(original)
    const grant = await conversationToolRequest(client, 'conversationTools.grants.confirm', { commandId: original.commandId, previewId: preview.previewId, previewDigest: preview.previewDigest })
    if (observed !== generation.current) return; setConfirmed(grant)
    await conversationOriginal(client, key, null); if (observed !== generation.current) return; setPending(null)
    await insert(grant, observed)
  }) }
  const reconcile = (): void => { void work(async observed => {
    if (!pending) return; const original = pending
    const result = await client.request<{ state: string; result?: StoredCommandResult }>('commands.reconcile', { operation: original.operation, params: { commandId: original.commandId } })
    if (observed !== generation.current) return
    if (result.state !== 'recorded' || result.result?.commandId !== original.commandId || result.result.commandType !== original.operation) throw new Error(t('原授权请求还没有确定回执，没有重复授权。'))
    await conversationOriginal(client, key, null); if (observed !== generation.current) return; setPending(null)
    if (result.result.status === 'rejected') throw new Error(result.result.code)
    if (original.operation === 'conversationTools.grants.confirm') setConfirmed(result.result.payload as unknown as ConversationSourceGrant)
    await load()
  }) }
  const revoke = (grant: ConversationSourceGrant): void => { void work(async observed => {
    const original: ConversationOriginal = { operation: 'conversationTools.grants.revoke', commandId: newCommandId() }
    await conversationOriginal(client, key, original); if (observed !== generation.current) return; setPending(original)
    await conversationToolRequest(client, 'conversationTools.grants.revoke', { commandId: original.commandId, grantId: grant.grantId, expectedRevision: grant.revision })
    if (observed !== generation.current) return; await conversationOriginal(client, key, null); if (observed !== generation.current) return; setPending(null); if (confirmed?.grantId === grant.grantId) setConfirmed(null); await load()
  }) }
  const readHistory = (): void => { void work(async observed => {
    const value = await client.request<{ entries: HistoryEntry[] }>('history.library', { scope: { threadId: scope.threadId }, operation: 'list', includeArchived: false })
    if (observed === generation.current) setHistory(value.entries)
  }) }
  const chooseImport = (entry: HistoryEntry): void => { void work(async observed => {
    if (!entry.linkedThreadId || !entry.revision) throw new Error(t('这份旧导入缺少归属或修订，请在原历史库中核对。'))
    const value = await client.request<{ source: ConversationSourceDescriptor }>('history.library', { scope: { threadId: entry.linkedThreadId }, operation: 'source.prepare', commandId: newCommandId(), importId: entry.id, expectedRevision: entry.revision })
    if (observed !== generation.current) return
    const preview = await conversationToolRequest(client, 'conversationTools.grants.preview', { scope, sourceSessionId: value.source.sourceSessionId, sourceDigest: value.source.sourceDigest, throughSequence: value.source.throughSequence })
    if (observed === generation.current) { setSelected(value.source); setPreview(preview); setConfirmed(null) }
  }) }
  return <>{controlledOpen === undefined && <button type="button" className="quiet-button" disabled={disabled} onClick={() => setOpen(true)}>{t('参考旧讨论')}</button>}<Dialog.Root open={open} onOpenChange={setOpen}><Dialog.Portal><Dialog.Overlay className="dialog-overlay app-dialog-overlay" /><Dialog.Content className="dialog-content app-dialog feedback-dialog" aria-describedby={undefined}>
    <header><Dialog.Title>{t('选择这次参考的资料')}</Dialog.Title><Dialog.Close className="quiet-button" aria-label={t('关闭')}>×</Dialog.Close></header>
    <div className="feedback-dialog-body"><p>{t('由你选择讨论并确认授权，引用加入当前消息后再按原方式发送。只读取选中时已经保存的可见文字和工具名称；图片、隐藏思考和凭据不会一起交给队员。')}</p>
      <p>{t(scope.scopeKind === 'public' ? '当前是团队公开讨论，只列出可公开参考的来源。私聊和私人导入资料可以在私聊或帮手讨论中选择。' : '资料只授权给当前私聊或帮手讨论，不会因此开放给整个团队。')}</p>
      {page.items.map(source => <div className="feedback-history-item feedback-source-row" key={source.sourceSessionId}><div className="feedback-source-copy"><strong>{source.title}</strong><small>{t(kindText[source.sourceKind])}</small></div><button type="button" className="quiet-button" disabled={busy || !ready || !!pending} onClick={() => choose(source)}>{t('选择并预览')}</button></div>)}
      {!page.items.length && ready && <p>{t('当前还没有可选择的讨论。')}</p>}{page.next && <button type="button" className="quiet-button" disabled={busy || !!pending} onClick={() => void work(async () => load(true))}>{t('更多讨论')}</button>}
      {scope.scopeKind !== 'public' && <details><summary>{t('选择已经导入的历史')}</summary><button type="button" className="quiet-button" disabled={busy || !!pending} onClick={readHistory}>{t('读取原历史库')}</button>{history.slice(0, historyLimit).map(entry => <div className="feedback-history-item feedback-source-row" key={entry.id}><strong>{entry.summary?.title ?? t('历史讨论')}</strong><button type="button" className="quiet-button" disabled={busy || !!pending} onClick={() => chooseImport(entry)}>{t('准备这份引用')}</button></div>)}{history.length > historyLimit && <button type="button" className="quiet-button" onClick={() => setHistoryLimit(limit => limit + 25)}>{t('更多历史')}</button>}</details>}
      {preview && selected && <div className="feedback-preview-target"><strong>{selected.title}</strong><p>{t('授权给当前讨论，最多保留两小时；撤销后不能继续读取。这里只准备引用，不启动队员，也不发送消息。')}</p><p>{t('预览有效至 {0}', new Date(preview.expiresAt).toLocaleTimeString())}</p></div>}
      {!!grants.length && <details><summary>{t('当前讨论已授权的资料')}</summary>{grants.map(grant => <div className="feedback-history-item feedback-source-row" key={grant.grantId}><div className="feedback-source-copy"><strong>{page.items.find(source => source.sourceSessionId === grant.sourceSessionId)?.title ?? t('已授权的讨论')}</strong><small>{t('有效至 {0}', new Date(grant.expiresAt).toLocaleTimeString())}</small></div><button type="button" className="quiet-button" disabled={busy || !!pending} onClick={() => revoke(grant)}>{t('撤销授权')}</button></div>)}{grantNext && <button type="button" className="quiet-button" disabled={busy || !!pending} onClick={() => void work(async observed => { const next = await conversationToolRequest(client, 'conversationTools.grants.list', { scope, after: grantNext, limit: 100 }); if (observed === generation.current) { setGrants(old => [...old, ...next.items]); setGrantNext(next.next) } })}>{t('更多授权记录')}</button>}</details>}
      {error && <p role="alert" className="feedback-error">{error}</p>}
    </div><footer><Dialog.Close className="quiet-button">{t('关闭并保留记录')}</Dialog.Close>{pending ? <button type="button" className="primary-button" disabled={busy} onClick={reconcile}>{t('核对原授权请求')}</button> : confirmed ? <button type="button" className="primary-button" disabled={busy || disabled} onClick={() => void work(observed => insert(confirmed, observed))}>{t('将这份授权加入输入')}</button> : <button type="button" className="primary-button" disabled={busy || !ready || !preview || disabled} onClick={confirm}>{t('授权并加入当前输入')}</button>}</footer>
  </Dialog.Content></Dialog.Portal></Dialog.Root></>
}
