// Codeg aff1e6c: message/{sub-agent-session-dialog,session-viewer-host,
// delegated-sub-thread,delegation-card-row}.tsx. Apache-2.0: integration/licenses/codeg/LICENSE.
// Drawer owns presentation only; Core owns every child, Run, approval and answer.
import { createContext, useContext, useEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import { Drawer } from '@base-ui/react/drawer'
import { ChevronDown, ChevronUp, Eye, Loader2, RefreshCw, X } from 'lucide-react'
import type { ActionApprovalView, AgentProfile, DelegationDetail, DelegationHistoryPage, SingleChatMessageView } from '@contracts'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { SafeMarkdown } from './SafeMarkdown'
import { SingleChatRunHistory, singleChatEvidenceForRun } from './SingleChatPanel'
import { ApprovalDock } from './ThreadWorkspace'
import { RuntimeInteractionDock } from './RuntimeInteractionDock'
import { CodegRuntimeMark } from './CodegRuntimeMark'
import { delegationChildren, delegationRuntimeLabel, delegationViewerController, type DelegationViewerTarget } from './delegation-viewer-controller'
import './delegation-viewer.css'

interface ViewerEnvironment {
  threadId: string
  profileById: Map<string, AgentProfile>
  busy: boolean
  onResolveApproval(approval: ActionApprovalView, optionId: string): void
  onNotify?(message: string): void
}
const ViewerContext = createContext<{ open(target: DelegationViewerTarget, trigger?: HTMLElement): void; environment: ViewerEnvironment } | null>(null)
function useViewer(): NonNullable<React.ContextType<typeof ViewerContext>> {
  const value = useContext(ViewerContext)
  if (!value) throw new Error('Delegation viewer host is missing')
  return value
}
/** Above virtual rows, one drawer per level. A nested transcript creates the
 * next host; unmounting/replacing a launch row cannot close its open viewer. */
export function DelegationViewerHost({ children, onViewerOpenChange, ...environment }: ViewerEnvironment & { children: ReactNode; onViewerOpenChange?(open: boolean): void }): React.JSX.Element {
  const [target, setTarget] = useState<DelegationViewerTarget | null>(null)
  const [open, setOpen] = useState(false)
  const returnFocus = useRef<HTMLElement | undefined>(undefined)
  const client = useThreadClient()
  useEffect(() => { onViewerOpenChange?.(open) }, [open, onViewerOpenChange])
  useEffect(() => () => onViewerOpenChange?.(false), [onViewerOpenChange])
  const [owner, setOwner] = useState({ client, generation: 0, threadId: environment.threadId })
  if (owner.client !== client || owner.threadId !== environment.threadId) {
    setOwner({ client, threadId: environment.threadId, generation: owner.generation + 1 }); setTarget(null); setOpen(false)
  }
  const value = { environment, open: (next: DelegationViewerTarget, trigger?: HTMLElement): void => {
    if (next.threadId !== environment.threadId) return
    returnFocus.current = trigger
    setTarget(next); setOpen(true)
  } }
  return <ViewerContext.Provider value={value}>{children}
    {target && <DelegationDrawer key={JSON.stringify([owner.generation, target])} target={target} open={open} onOpenChange={setOpen} environment={environment} returnFocus={returnFocus.current} />}
  </ViewerContext.Provider>
}

export function DelegationChildren({ environment, ...props }: { parentConversationId: string; visible?: boolean; environment?: ViewerEnvironment }): React.JSX.Element | null {
  const host = useContext(ViewerContext)
  if (host) return <DelegationChildrenContent {...props} />
  if (!environment) throw new Error('Delegation viewer environment is missing')
  return <DelegationViewerHost key={props.parentConversationId} {...environment}><DelegationChildrenContent {...props} /></DelegationViewerHost>
}
function DelegationChildrenContent({ parentConversationId, visible = true }: { parentConversationId: string; visible?: boolean }): React.JSX.Element | null {
  const client = useThreadClient(), host = useViewer(), t = useUiText()
  const [state, setState] = useState<{ owner: object; parent: string; rows: DelegationDetail[]; error: boolean; loading: boolean }>({ owner: client, parent: parentConversationId, rows: [], error: false, loading: true })
  const [expanded, setExpanded] = useState(false)
  const sequence = useRef(0), inFlight = useRef(false), readAgain = useRef(false)
  const current = state.owner === client && state.parent === parentConversationId
  const rows = current ? state.rows : []
  useEffect(() => {
    if (!visible) return
    let disposed = false, timer: ReturnType<typeof setTimeout> | null = null
    const refresh = async (): Promise<void> => {
      if (inFlight.current) { readAgain.current = true; return }
      inFlight.current = true
      const request = ++sequence.current
      try {
        let page = await delegationChildren(client, parentConversationId)
        const items = [...page.items], cursors = new Set<string>()
        while (page.nextCursor) {
          if (disposed || request !== sequence.current) return
          if (cursors.has(page.nextCursor)) throw new Error('delegation.viewer_cursor_repeated')
          cursors.add(page.nextCursor)
          page = await delegationChildren(client, parentConversationId, page.nextCursor)
          items.push(...page.items)
        }
        if (items.some(item => item.threadId !== host.environment.threadId)) throw new Error('delegation.viewer_thread_mismatch')
        if (!disposed && request === sequence.current) setState({ owner: client, parent: parentConversationId, rows: items, loading: false, error: false })
      } catch { if (!disposed && request === sequence.current) setState(previous => ({ ...previous, rows: previous.owner === client && previous.parent === parentConversationId ? previous.rows : [], owner: client, parent: parentConversationId, loading: false, error: true })) }
      finally {
        if (request === sequence.current) inFlight.current = false
        if (!disposed) {
          if (readAgain.current) { readAgain.current = false; void refresh() }
          else timer = setTimeout(() => void refresh(), 5_000)
        }
      }
    }
    const off = client.onEvent?.(event => { if (event.method === 'delegation.changed') { if (timer) clearTimeout(timer); void refresh() } })
    const offInvalidation = client.onInvalidated?.(() => { if (timer) clearTimeout(timer); void refresh() })
    void refresh()
    return () => { disposed = true; ++sequence.current; inFlight.current = false; readAgain.current = false; if (timer) clearTimeout(timer); off?.(); offInvalidation?.() }
  }, [client, parentConversationId, visible])
  if (!visible || (rows.length === 0 && (!current || state.loading || !state.error))) return null
  const shown = expanded ? rows : rows.slice(0, 3)
  return <section className="delegation-children" aria-label={t('独立帮手')}>
    {state.error && <p role="alert">{t('帮手列表暂时无法读取，将重新核对。')}</p>}
    {shown.map(detail => <div className="delegation-card" key={detail.delegationId}>
      <span className="delegation-avatar"><CodegRuntimeMark target={detail.agentType} /></span>
      <div className="delegation-card-description"><div><strong>{delegationRuntimeLabel(detail.agentType)}</strong><code title={detail.delegationId}>#{detail.delegationId.slice(-8)}</code>
        <span className={`delegation-status is-${detail.status}`}>{t(delegationStatus(detail))}</span></div>
        {detail.blockedOn && <small>{t(detail.blockedOn.kind === 'permission' ? '等待审批' : detail.blockedOn.kind === 'question' ? '等待回答' : '等待计划确认')}</small>}
      </div>
      <button type="button" className="delegation-open" aria-label={t('查看帮手会话')} onClick={event => host.open({ delegationId: detail.delegationId, parentConversationId, threadId: host.environment.threadId }, event.currentTarget)}><Eye size={14} /><span>{t('查看会话')}</span></button>
    </div>)}
    {rows.length > 3 && <div className="delegation-list-controls"><button type="button" onClick={() => setExpanded(value => !value)}>{expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}{t(expanded ? '收起帮手列表' : '展开帮手列表')}</button></div>}
  </section>
}
function delegationStatus(detail: DelegationDetail): string {
  if (detail.takenOver) return detail.cleanupPending ? '已接管，正在收尾' : '已接管'
  if (detail.blockedOn) return detail.blockedOn.kind === 'permission' ? '等待审批' : detail.blockedOn.kind === 'question' ? '等待回答' : '等待计划确认'
  return ({ queued: '等待开始', running: '正在执行', waiting: '等待继续', succeeded: '已完成', failed: '执行失败', cancelled: '已取消' } as Record<string, string>)[detail.status] ?? '状态待核对'
}

// Ported from Codeg ui/drawer.tsx: Radix and Base UI have independent dismissal
// stacks. Capture the shield before Radix synchronously unmounts its modal.
let escapeProbeUsers = 0
let lastEscape: { event: KeyboardEvent; shielded: boolean } | null = null
const radixModalOpen = (): boolean => document.body.style.pointerEvents === 'none'
function sampleEscapeShield(event: KeyboardEvent): void {
  if (event.key === 'Escape') lastEscape = { event, shielded: radixModalOpen() }
}
function useEscapeShield(): void {
  useEffect(() => {
    if (escapeProbeUsers++ === 0) document.addEventListener('keydown', sampleEscapeShield, true)
    return () => { if (--escapeProbeUsers === 0) { document.removeEventListener('keydown', sampleEscapeShield, true); lastEscape = null } }
  }, [])
}
function shieldedDismissal(details: Drawer.Root.ChangeEventDetails): boolean {
  if (!['outside-press', 'escape-key', 'focus-out'].includes(details.reason)) return false
  return details.reason === 'escape-key' && lastEscape?.event === details.event ? lastEscape.shielded : radixModalOpen()
}

function DelegationDrawer({ target, open, onOpenChange, environment, returnFocus }: { target: DelegationViewerTarget; open: boolean; onOpenChange(open: boolean): void; environment: ViewerEnvironment; returnFocus?: HTMLElement }): React.JSX.Element {
  useEscapeShield()
  const t = useUiText()
  const controller = delegationViewerController(useThreadClient(), target)
  const state = useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot)
  const approvalRef = useRef<HTMLElement | null>(null)
  const headingRef = useRef<HTMLElement | null>(null)
  const [now, setNow] = useState(() => new Date().toISOString())
  const detail = state.detail
  const live = detail?.activity.runs.some(run => ['queued', 'running', 'waiting'].includes(run.status)) ?? false
  useEffect(() => open ? controller.retain() : undefined, [controller, open])
  useEffect(() => {
    if (!open || !live) return
    const timer = setInterval(() => setNow(new Date().toISOString()), 1_000)
    return () => clearInterval(timer)
  }, [open, live])
  const approvals = detail?.activity.approvals ?? []
  return <Drawer.Root open={open} modal={false} disablePointerDismissal swipeDirection="right" onOpenChange={(next, details) => {
    if (!next && shieldedDismissal(details)) { details.cancel(); return }
    onOpenChange(next)
  }}>
    <Drawer.Portal><Drawer.Viewport className="delegation-drawer-viewport"><Drawer.Popup className="delegation-drawer" initialFocus={headingRef}
      finalFocus={() => returnFocus?.isConnected ? returnFocus : false}>
      <Drawer.Content className="delegation-drawer-content">
      <Drawer.Title className="sr-only">{t('独立帮手会话')}</Drawer.Title>
      <header className="delegation-drawer-header" tabIndex={-1} ref={headingRef}><span className="delegation-drawer-avatar">{detail && <CodegRuntimeMark target={detail.agentType} size={16} />}</span>
        <strong>{detail ? delegationRuntimeLabel(detail.agentType) : t('独立帮手会话')}</strong>
        {detail && <span className="delegation-status">{t(delegationStatus(detail))}</span>}
        <button type="button" className="delegation-icon-button" aria-label={t('刷新帮手会话')} onClick={() => void controller.refresh()}><RefreshCw size={15} /></button>
        <Drawer.Close className="delegation-icon-button" aria-label={t('关闭帮手会话')}><X size={16} /></Drawer.Close>
      </header>
      {open && <div className="delegation-drawer-body">
        {state.loading && <p className="delegation-empty" role="status"><Loader2 size={15} className="native-spinner" />{t('正在读取帮手会话…')}</p>}
        {state.error && <p className="delegation-read-error" role="alert">{t('会话暂时无法刷新，已保留上次内容。')}<button type="button" onClick={() => void controller.refresh()}>{t('重试')}</button></p>}
        {detail && <DelegationViewerHost {...environment}>
          {approvals.length > 0 && <ApprovalDock approvals={approvals} profileById={environment.profileById} busy={environment.busy || state.error}
            onResolve={environment.onResolveApproval} containerRef={approvalRef} focusRequest={null} focusApprovalId={null} />}
          <RuntimeInteractionDock conversationId={detail.conversationId} />
          <div className="delegation-transcript">
            {state.nextCursor && <button type="button" className="delegation-earlier" disabled={state.paging} onClick={() => void controller.loadEarlier()}>{t(state.paging ? '正在读取更早消息…' : '加载更早消息')}</button>}
            <DelegationTranscript detail={detail} messages={state.messages} now={now} onNotify={environment.onNotify} />
            {!state.messages.length && !detail.activity.runs.length && <p className="delegation-empty">{t('还没有可显示的会话内容。')}</p>}
          </div>
          <DelegationChildren parentConversationId={detail.conversationId} />
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
