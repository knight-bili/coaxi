// Codeg aff1e6c: message/{sub-agent-session-dialog,session-viewer-host,
// delegated-sub-thread,delegation-card-row}.tsx. Apache-2.0: integration/licenses/codeg/LICENSE.
// Drawer owns presentation only; Core owns every child, Run, approval and answer.
import { useCallback, useContext, useEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import { Drawer } from '@base-ui/react/drawer'
import { ChevronDown, ChevronUp, Loader2, RefreshCw, X } from 'lucide-react'
import type { DelegationDetail, DelegationHistoryPage, SingleChatMessageView } from '@contracts'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { SafeMarkdown } from './SafeMarkdown'
import { SingleChatPanel, SingleChatRunHistory, singleChatEvidenceForRun } from './SingleChatPanel'
import { ApprovalDock, type ThreadLeavePreparation } from './ThreadWorkspace'
import { RuntimeInteractionDock } from './RuntimeInteractionDock'
import { CodegRuntimeMark } from './CodegRuntimeMark'
import { CoreDelegationCard } from './CoreDelegationCard'
import { delegationActionController } from './delegation-action-controller'
import type { DelegationActionMethod } from '../../shared/private-conversation-operations'
import { delegationRuntimeLabel, delegationViewerController, type DelegationViewerTarget } from './delegation-viewer-controller'
import { delegationChildrenController } from './delegation-children-controller'
import { DelegationViewerContext as ViewerContext, type DelegationViewerEnvironment as ViewerEnvironment } from './delegation-viewer-context'
import './delegation-viewer.css'
import { NativeSessionViewer } from './NativeSessionViewer'
import { NativeSessionViewerContext, type NativeSessionTarget } from './native-session-viewer-context'
import { shieldedDismissal, useEscapeShield } from './session-viewer-dismissal'

type LeaveGuard = () => ThreadLeavePreparation
type BindLeaveGuard = (guard: LeaveGuard | null) => void
const EMPTY_ACTION = { pending: false, busy: false, unknown: false, error: null, result: null }
const emptyAction = () => EMPTY_ACTION
const noActionSubscription = () => () => undefined
export function prepareDelegationViewerLeave(...guards: Array<LeaveGuard | null>): ThreadLeavePreparation {
  const prepared: ThreadLeavePreparation[] = []
  try { for (const guard of guards) if (guard) prepared.push(guard()) }
  catch (error) { prepared.reverse().forEach(item => item.complete(false)); throw error }
  let completed = false
  return { complete(didLeave) { if (completed) return; completed = true; prepared.reverse().forEach(item => item.complete(didLeave)) } }
}
function useViewer(): NonNullable<React.ContextType<typeof ViewerContext>> {
  const value = useContext(ViewerContext)
  if (!value) throw new Error('Delegation viewer host is missing')
  return value
}
/** Above virtual rows, one drawer per level. A nested transcript creates the
 * next host; unmounting/replacing a launch row cannot close its open viewer. */
export function DelegationViewerHost({ children, visible = true, onLeaveGuardChange, onViewerOpenChange, ...environment }: ViewerEnvironment & { children: ReactNode; visible?: boolean; onLeaveGuardChange?: BindLeaveGuard; onViewerOpenChange?(open: boolean): void }): React.JSX.Element {
  const ancestor = useContext(ViewerContext)
  const surfaceVisible = visible && (ancestor?.visible ?? true)
  const [target, setTarget] = useState<DelegationViewerTarget | NativeSessionTarget | null>(null)
  const [open, setOpen] = useState(false)
  const [sources, setSources] = useState<Record<string, boolean>>({})
  const sourceVisibility = useCallback((parent: string, next: boolean) => setSources(current => current[parent] === next ? current : { ...current, [parent]: next }), [])
  const drawerGuard = useRef<LeaveGuard | null>(null), openRef = useRef(open)
  openRef.current = open
  const bindDrawerGuard = useCallback<BindLeaveGuard>(guard => { drawerGuard.current = guard }, [])
  const prepareLeave = useCallback<LeaveGuard>(() => openRef.current ? prepareDelegationViewerLeave(drawerGuard.current) : { complete() {} }, [])
  useEffect(() => { onLeaveGuardChange?.(prepareLeave); return () => onLeaveGuardChange?.(null) }, [onLeaveGuardChange, prepareLeave])
  const returnFocus = useRef<HTMLElement | undefined>(undefined)
  const client = useThreadClient()
  useEffect(() => { onViewerOpenChange?.(open) }, [open, onViewerOpenChange])
  useEffect(() => () => onViewerOpenChange?.(false), [onViewerOpenChange])
  const [owner, setOwner] = useState({ client, generation: 0, threadId: environment.threadId })
  if (owner.client !== client || owner.threadId !== environment.threadId) {
    setOwner({ client, threadId: environment.threadId, generation: owner.generation + 1 }); setTarget(null); setOpen(false)
  }
  const changeOpen = (next: boolean): boolean => {
    try { const prepared = !next ? prepareLeave() : null; setOpen(next); prepared?.complete(true); return true }
    catch { environment.onNotify?.('原操作尚未确认，请先核对后再离开。'); return false }
  }
  const value = { environment, visible: surfaceVisible, sourceVisibility, open: (next: DelegationViewerTarget, trigger?: HTMLElement): void => {
    if (next.threadId !== environment.threadId) return
    if (open && target && !('kind' in target) && target.delegationId === next.delegationId && target.parentConversationId === next.parentConversationId) { sourceVisibility(next.parentConversationId, true); return }
    let prepared: ThreadLeavePreparation
    try { prepared = prepareLeave() } catch { environment.onNotify?.('原操作尚未确认，请先核对后再离开。'); return }
    returnFocus.current = trigger
    sourceVisibility(next.parentConversationId, true)
    setTarget(next); setOpen(true)
    prepared.complete(true)
  } }
  const native = { visible: surfaceVisible, open: (next: NativeSessionTarget, trigger?: HTMLElement): void => {
    if (!surfaceVisible || next.threadId !== environment.threadId || next.source.reference.threadId !== environment.threadId) return
    if (open && target && 'kind' in target && target.identity === next.identity && target.parentConversationId === next.parentConversationId) return
    let prepared: ThreadLeavePreparation
    try { prepared = prepareLeave() } catch { environment.onNotify?.('原操作尚未确认，请先核对后再离开。'); return }
    returnFocus.current = trigger; sourceVisibility(next.parentConversationId, true); setTarget(next); setOpen(true); prepared.complete(true)
  } }
  return <ViewerContext.Provider value={value}><NativeSessionViewerContext.Provider value={native}>{children}
    {target && ('kind' in target
      ? <NativeSessionViewer key={`${owner.generation}:${target.identity}`} target={target} open={open} visible={surfaceVisible && sources[target.parentConversationId] !== false} onOpenChange={changeOpen} environment={environment} returnFocus={returnFocus.current} />
      : <DelegationDrawer key={JSON.stringify([owner.generation, target])} target={target} open={open} visible={surfaceVisible && sources[target.parentConversationId] !== false} onOpenChange={changeOpen} onLeaveGuardChange={bindDrawerGuard} environment={environment} returnFocus={returnFocus.current} />)}
  </NativeSessionViewerContext.Provider></ViewerContext.Provider>
}

export function DelegationChildren({ environment, onLeaveGuardChange, ...props }: { parentConversationId: string; visible?: boolean; environment?: ViewerEnvironment; onLeaveGuardChange?: BindLeaveGuard }): React.JSX.Element | null {
  const host = useContext(ViewerContext)
  if (host) return <DelegationChildrenContent {...props} />
  if (!environment) throw new Error('Delegation viewer environment is missing')
  return <DelegationViewerHost key={props.parentConversationId} {...environment} visible={props.visible} onLeaveGuardChange={onLeaveGuardChange}><DelegationChildrenContent {...props} /></DelegationViewerHost>
}
function DelegationChildrenContent({ parentConversationId, visible = true }: { parentConversationId: string; visible?: boolean }): React.JSX.Element | null {
  const client = useThreadClient(), host = useViewer(), t = useUiText()
  const previousParent = useRef(parentConversationId)
  useEffect(() => {
    if (previousParent.current !== parentConversationId) host.sourceVisibility(previousParent.current, false)
    previousParent.current = parentConversationId
    host.sourceVisibility(parentConversationId, visible)
    // A virtual launch-row unmount is not a route hide. Keep its last state.
  }, [parentConversationId, visible, host.sourceVisibility])
  const active = visible && host.visible
  const controller = delegationChildrenController(client, { threadId: host.environment.threadId, parentConversationId })
  const state = useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot)
  const [expanded, setExpanded] = useState(false)
  useEffect(() => active ? controller.retain() : undefined, [controller, active])
  const rows = state.rows
  if (!active || (rows.length === 0 && (state.loading || !state.error))) return null
  const shown = expanded ? rows : rows.slice(0, 3)
  return <section className="delegation-children" aria-label={t('独立帮手')}>
    {state.error && <p role="alert">{t('帮手列表暂时无法读取，将重新核对。')}</p>}
    {shown.map(detail => <div key={detail.delegationId}>
      <CoreDelegationCard detail={detail} onOpen={trigger => host.open({ delegationId: detail.delegationId, parentConversationId, threadId: host.environment.threadId }, trigger)} />
      {(detail.blockedOn && detail.blockedOn.kind !== 'permission' || detail.takenOver || detail.cleanupPending || detail.status === 'waiting' && !detail.blockedOn) && <small className="delegation-status">{t(delegationStatus(detail))}</small>}
    </div>)}
    {rows.length > 3 && <div className="delegation-list-controls"><button type="button" onClick={() => setExpanded(value => !value)}>{expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}{t(expanded ? '收起帮手列表' : '展开帮手列表')}</button></div>}
  </section>
}
function delegationStatus(detail: DelegationDetail): string {
  if (detail.takenOver) return detail.cleanupPending ? '已接管，正在收尾' : '已接管'
  if (detail.blockedOn) return detail.blockedOn.kind === 'permission' ? '等待审批' : detail.blockedOn.kind === 'question' ? '等待回答' : '等待计划确认'
  return ({ queued: '等待开始', running: '正在执行', waiting: '等待继续', succeeded: '已完成', failed: '执行失败', cancelled: '已取消' } as Record<string, string>)[detail.status] ?? '状态待核对'
}

function DelegationDrawer({ target, open, visible, onOpenChange, onLeaveGuardChange, environment, returnFocus }: { target: DelegationViewerTarget; open: boolean; visible: boolean; onOpenChange(open: boolean): boolean; onLeaveGuardChange: BindLeaveGuard; environment: ViewerEnvironment; returnFocus?: HTMLElement }): React.JSX.Element {
  useEscapeShield()
  const t = useUiText()
  const client = useThreadClient()
  const controller = delegationViewerController(client, target)
  const state = useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot)
  const approvalRef = useRef<HTMLElement | null>(null)
  const headingRef = useRef<HTMLElement | null>(null)
  const [now, setNow] = useState(() => new Date().toISOString())
  const detail = state.detail
  const actions = useMemo(() => detail ? delegationActionController(client, { ...target, conversationId: detail.conversationId }) : null, [client, target, detail?.conversationId])
  const action = useSyncExternalStore(actions?.subscribe ?? noActionSubscription, actions?.getSnapshot ?? emptyAction, actions?.getSnapshot ?? emptyAction)
  const leafGuard = useRef<LeaveGuard | null>(null), nestedGuard = useRef<LeaveGuard | null>(null)
  const bindLeafGuard = useCallback<BindLeaveGuard>(guard => { leafGuard.current = guard }, [])
  const bindNestedGuard = useCallback<BindLeaveGuard>(guard => { nestedGuard.current = guard }, [])
  const prepareLeave = useCallback<LeaveGuard>(() => {
    return prepareDelegationViewerLeave(leafGuard.current, nestedGuard.current)
  }, [])
  useEffect(() => { onLeaveGuardChange(prepareLeave); return () => onLeaveGuardChange(null) }, [onLeaveGuardChange, prepareLeave])
  const ownerMember = environment.members?.find(member => member.agentId === detail?.ownerAgentId && member.membershipStatus === 'active' && member.leaveRequestedAt === null && member.profilePresence === 'present')
  const chatTarget = useMemo(() => detail && ownerMember ? { conversationId: detail.conversationId, agentId: detail.ownerAgentId, agentDisplayName: ownerMember.displayName, requestId: 1 } : null, [detail?.conversationId, detail?.ownerAgentId, ownerMember?.displayName])
  const chatReady = !!detail?.takenOver && !detail.cleanupPending && !!chatTarget
  const [resumeReason, setResumeReason] = useState('')
  const execute = async (method: DelegationActionMethod, command: Record<string, unknown>): Promise<void> => {
    if (!actions) throw new Error('delegation.control_unavailable')
    try { await actions.run(method, command) } finally { await controller.refresh() }
  }
  const recover = async (retry: boolean): Promise<void> => {
    if (!actions) return
    try { if (retry) await actions.retry(); else await actions.reconcile() } finally { await controller.refresh() }
  }
  const live = detail?.activity.runs.some(run => ['queued', 'running', 'waiting'].includes(run.status)) ?? false
  useEffect(() => open ? controller.retain() : undefined, [controller, open])
  useEffect(() => {
    if (!open || !live) return
    const timer = setInterval(() => setNow(new Date().toISOString()), 1_000)
    return () => clearInterval(timer)
  }, [open, live])
  const approvals = detail?.activity.approvals ?? []
  return <Drawer.Root open={open} modal={false} disablePointerDismissal swipeDirection="right" onOpenChange={(next, details) => {
    if (!visible && ['outside-press', 'escape-key', 'focus-out'].includes(details.reason)) { details.cancel(); return }
    if (!next && shieldedDismissal(details)) { details.cancel(); return }
    if (!onOpenChange(next)) details.cancel()
  }}>
    <Drawer.Portal><Drawer.Viewport className="delegation-drawer-viewport" data-host-hidden={!visible || undefined} inert={!visible || undefined}><Drawer.Popup className="delegation-drawer" initialFocus={headingRef}
      finalFocus={() => returnFocus?.isConnected ? returnFocus : false}>
      <Drawer.Content className="delegation-drawer-content">
      <Drawer.Title className="sr-only">{t('独立帮手会话')}</Drawer.Title>
      <header className="delegation-drawer-header" tabIndex={-1} ref={headingRef}><span className="delegation-drawer-avatar">{detail && <CodegRuntimeMark target={detail.agentType} size={16} />}</span>
        <strong>{detail ? delegationRuntimeLabel(detail.agentType) : t('独立帮手会话')}</strong>
        {detail && <span className="delegation-status">{t(delegationStatus(detail))}</span>}
        <button type="button" className="delegation-icon-button" aria-label={t('刷新帮手会话')} onClick={() => void controller.refresh()}><RefreshCw size={15} /></button>
        <Drawer.Close className="delegation-icon-button" aria-label={t('关闭帮手会话')}><X size={16} /></Drawer.Close>
      </header>
      {open && detail && <section className="delegation-controls" aria-label={t('帮手会话控制')}>
        {!detail.takenOver && <>
          <button type="button" className="quiet-button" disabled={environment.busy || state.error || action.pending} onClick={() => void execute('delegations.takeover', { delegationId: detail.delegationId, expectedControlVersion: detail.controlVersion }).catch(() => undefined)}>{t('停止原任务并接管')}</button>
          {['queued', 'running', 'waiting'].includes(detail.status) && <button type="button" className="quiet-button" disabled={environment.busy || state.error || action.pending || detail.cleanupPending} onClick={() => void execute('delegations.cancel', { delegationId: detail.delegationId, attemptNo: detail.currentAttemptNo }).catch(() => undefined)}>{t('停止原任务')}</button>}
          {['cancelled', 'failed'].includes(detail.status) && !detail.cleanupPending && <details><summary>{t('恢复原任务')}</summary><textarea aria-label={t('中断说明（可选）')} value={resumeReason} onChange={event => setResumeReason(event.target.value)} />{new TextEncoder().encode(resumeReason).length > 2048 && <small role="alert">{t('中断说明太长，请缩短后再恢复。')}</small>}<button type="button" disabled={environment.busy || state.error || action.pending || new TextEncoder().encode(resumeReason).length > 2048} onClick={() => void execute('delegations.resume', { delegationId: detail.delegationId, expectedAttemptNo: detail.currentAttemptNo, ...(resumeReason.trim() ? { reason: resumeReason } : {}) }).catch(() => undefined)}>{t('按原身份恢复')}</button></details>}
          <small>{t('接管将停止原任务；清理完成后可在这里继续聊天。')}</small>
        </>}
        {detail.takenOver && detail.cleanupPending && <p role="status">{t('正在停止和清理原任务，完成后可继续聊天。')}</p>}
        {detail.takenOver && !detail.cleanupPending && !ownerMember && <p role="status">{t('发起队员当前不可用，已保留原会话供查看。')}</p>}
        {action.pending && <div role="status">{t('原操作尚未确认，请先核对。')}<button type="button" disabled={action.busy} onClick={() => void recover(false).catch(() => undefined)}>{t('核对原操作')}</button>{action.unknown && <button type="button" disabled={action.busy} onClick={() => void recover(true).catch(() => undefined)}>{t('重试原操作')}</button>}</div>}
        {action.error && !action.pending && <p role="alert">{t(action.error === 'delegation.native_session_rebuild_required' ? '这次任务的工具连接不能安全续接，历史已保留。你可以接管后继续聊天。' : action.error === 'delegation.not_resumable' ? '原任务当前不能恢复，你仍可接管后继续聊天。' : '操作未完成，请刷新状态后再试。')}</p>}
      </section>}
      {open && <div className={`delegation-drawer-body${chatReady ? ' has-chat' : ''}`}>
        {state.loading && <p className="delegation-empty" role="status"><Loader2 size={15} className="native-spinner" />{t('正在读取帮手会话…')}</p>}
        {state.error && <p className="delegation-read-error" role="alert">{t(state.detail ? '会话暂时无法刷新，已保留上次内容。' : '会话暂时无法读取，请重试。')}<button type="button" onClick={() => void controller.refresh()}>{t('重试')}</button></p>}
        {detail && <DelegationViewerHost {...environment} visible={visible} onLeaveGuardChange={bindNestedGuard}>
          {chatReady && chatTarget ? <SingleChatPanel conversationKind="delegation_child" embedded threadId={environment.threadId} members={environment.members ?? []}
            boundConversationId={detail.conversationId} target={chatTarget} visible={visible} onOpen={() => undefined} onClose={() => { onOpenChange(false) }} onLeaveGuardChange={bindLeafGuard}
            onEndConversation={async endTarget => { if (endTarget.conversationId !== detail.conversationId || endTarget.threadId !== detail.threadId) throw new Error('delegation.viewer_scope_mismatch'); await execute('delegations.conversation.end', { conversationId: detail.conversationId }) }}
            profileById={environment.profileById} busy={environment.busy || action.pending} onResolveApproval={environment.onResolveApproval} onNotify={environment.onNotify} /> : <>
          {approvals.length > 0 && <ApprovalDock approvals={approvals} profileById={environment.profileById} busy={environment.busy || state.error}
            onResolve={environment.onResolveApproval} containerRef={approvalRef} focusRequest={null} focusApprovalId={null} />}
          <RuntimeInteractionDock conversationId={detail.conversationId} visible={visible} />
          <div className="delegation-transcript">
            {state.nextCursor && <button type="button" className="delegation-earlier" disabled={state.paging} onClick={() => void controller.loadEarlier()}>{t(state.paging ? '正在读取更早消息…' : '加载更早消息')}</button>}
            <DelegationTranscript detail={detail} messages={state.messages} now={now} onNotify={environment.onNotify} />
            {!state.messages.length && !detail.activity.runs.length && <p className="delegation-empty">{t('还没有可显示的会话内容。')}</p>}
          </div>
          <DelegationChildren parentConversationId={detail.conversationId} visible={visible} />
          </>}
        </DelegationViewerHost>}
      </div>}
      </Drawer.Content>
    </Drawer.Popup></Drawer.Viewport></Drawer.Portal>
  </Drawer.Root>
}
function DelegationTranscript({ detail, messages, now, onNotify }: { detail: NonNullable<ReturnType<ReturnType<typeof delegationViewerController>['getSnapshot']>['detail']>; messages: DelegationHistoryPage['messages']; now: string; onNotify?: (message: string) => void }): React.JSX.Element {
  const t = useUiText(), rendered = new Set<string>()
  const runs = detail.activity.runs
  const final = (id: string | null): SingleChatMessageView | null => {
    const message = messages.find(item => item.id === id && item.author_type === 'assistant')
    return message ? { id: message.id, sequence: message.sequence, authorType: 'agent', authorId: detail.ownerAgentId, body: message.body, attachments: [], quotes: [], agentRunId: null, createdAt: message.created_at } : null
  }
  const renderRun = (run: typeof runs[number]): React.JSX.Element => <SingleChatRunHistory key={run.id} threadId={detail.threadId} conversationId={detail.conversationId}
    conversationKind="delegation_child"
    run={run} evidence={singleChatEvidenceForRun(detail.activity.executionEvidence, run)} finalMessage={final(run.finalConversationMessageId)} now={now} onNotify={onNotify} />
  const rows = messages.map(message => {
    const run = runs.find(item => item.triggerConversationMessageId === message.id)
      ?? (message.author_type === 'assistant' ? runs.find(item => item.finalConversationMessageId === message.id && !rendered.has(item.id)) : undefined)
    const bodyRendered = message.author_type === 'assistant' && runs.some(item => item.finalConversationMessageId === message.id && rendered.has(item.id))
    if (bodyRendered && !message.attachments?.length) return null
    if (run) rendered.add(run.id)
    return <div key={message.id} className={message.author_type === 'assistant' ? 'delegation-assistant-turn' : 'delegation-input-turn'} data-delegation-message-id={message.id}>
      {message.author_type !== 'assistant' && <small>{t(message.author_type === 'delegation_parent' ? '任务说明' : '你')}</small>}
      {!bodyRendered && !(message.author_type === 'assistant' && run) && <SafeMarkdown>{message.body}</SafeMarkdown>}
      {message.attachments?.length ? <ul className="delegation-history-attachments">{message.attachments.map(item => <li key={item.id}>{item.name}{item.kind === 'directory' ? '/' : ''}</li>)}</ul> : null}
      {run && renderRun(run)}
    </div>
  })
  return <>{rows}{runs.filter(run => !rendered.has(run.id) && ['queued', 'running', 'waiting'].includes(run.status)).map(renderRun)}</>
}
