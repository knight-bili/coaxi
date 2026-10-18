import type { DelegationConversationView, DelegationHistoryPage, DelegationDetail, DelegationListPage, DelegationRuntimeTarget } from '@contracts'
import type { ThreadClient } from './camp-client'

export interface DelegationViewerTarget { delegationId: string; parentConversationId: string; threadId: string }
export interface DelegationViewerState {
  detail: DelegationConversationView | null
  messages: DelegationHistoryPage['messages']
  nextCursor: string | null
  loading: boolean
  paging: boolean
  error: boolean
}
type Client = Pick<ThreadClient, 'request' | 'onEvent' | 'onInvalidated'>
/** Codeg target names are not Rovai adapter-kind identifiers. */
export function delegationRuntimeLabel(target: DelegationRuntimeTarget): string {
  const labels: Record<Exclude<DelegationRuntimeTarget, `custom:${string}`>, string> = {
    antigravity: 'Google Antigravity', claude_code: 'Claude Code', cline: 'Cline', code_buddy: 'CodeBuddy', codex: 'Codex', command_code: 'Command Code',
    copilot: 'GitHub Copilot', cursor: 'Cursor', deepseek: 'DeepSeek Harness', gemini: 'Gemini CLI', grok: 'Grok', hermes: 'Hermes Agent', kimi_code: 'Kimi Code',
    kiro: 'Kiro', open_claw: 'OpenClaw', open_code: 'OpenCode', pi: 'Pi', qoder: 'Qoder', qwen_code: 'Qwen Code', trae_cn: 'TRAE CN', zcode: 'Zcode'
  }
  return labels[target as keyof typeof labels] ?? target
}
export function assertDelegationScope(detail: DelegationDetail, target: DelegationViewerTarget): void {
  if (detail.delegationId !== target.delegationId || detail.threadId !== target.threadId || detail.parentConversationId !== target.parentConversationId) throw new Error('delegation.viewer_scope_mismatch')
}
export function mergeDelegationHistory(previous: DelegationHistoryPage['messages'], next: DelegationHistoryPage['messages']): DelegationHistoryPage['messages'] {
  const rows = new Map(previous.map(message => [message.id, message]))
  for (const message of next) rows.set(message.id, message)
  return [...rows.values()].sort((left, right) => left.sequence - right.sequence || left.id.localeCompare(right.id))
}
/** Reads the exact Core child. No attach/start, implicit takeover, native Session
 * lookup, or fallback from a missing child to an unrelated Camp snapshot. */
export class DelegationViewerController {
  private state: DelegationViewerState = { detail: null, messages: [], nextCursor: null, loading: true, paging: false, error: false }
  private listeners = new Set<() => void>()
  private reading: Promise<void> | null = null
  private dirty = false
  private users = 0
  private generation = 0
  private offEvent?: () => void
  private offInvalidation?: () => void
  private timer: ReturnType<typeof setTimeout> | null = null
  constructor(private client: Client, readonly target: DelegationViewerTarget) {}
  getSnapshot = (): DelegationViewerState => this.state
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener) } }
  private publish(change: Partial<DelegationViewerState>): void { this.state = { ...this.state, ...change }; this.listeners.forEach(listener => listener()) }
  retain(): () => void {
    if (this.users++ === 0) {
      this.offEvent = this.client.onEvent?.(event => {
        const params = event.params as { threadId?: string; delegationId?: string; conversationId?: string; agentRunId?: string }
        if (params?.threadId !== this.target.threadId) return
        if (params.delegationId === this.target.delegationId || params.conversationId === this.state.detail?.conversationId
          || this.state.detail?.activity.runs.some(run => run.id === params.agentRunId)) void this.refresh()
      })
      this.offInvalidation = this.client.onInvalidated?.(() => {
        ++this.generation; this.reading = null; this.dirty = false
        this.publish({ detail: null, messages: [], nextCursor: null, loading: true, paging: false, error: false })
        void this.refresh()
      })
      void this.refresh()
    }
    return () => {
      if (--this.users === 0) {
        ++this.generation; this.reading = null; this.dirty = false
        this.offEvent?.(); this.offInvalidation?.(); if (this.timer) clearTimeout(this.timer); this.timer = null
      }
    }
  }
  private schedule(): void {
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    if (this.users === 0) return
    if (this.state.error || this.state.detail?.cleanupPending || this.state.detail?.activity.runs.some(run => ['queued', 'running', 'waiting'].includes(run.status))) {
      this.timer = setTimeout(() => { this.timer = null; void this.refresh() }, 2_000)
    }
  }
  private validateHistory(page: DelegationHistoryPage, detail: DelegationConversationView): void {
    if (page.task_id !== this.target.delegationId || page.conversation_id !== detail.conversationId) throw new Error('delegation.viewer_history_scope_mismatch')
  }
  refresh = (): Promise<void> => {
    if (this.reading) { this.dirty = true; return this.reading }
    const generation = this.generation
    const reading = (async () => {
      do {
        this.dirty = false
        try {
          const detail = await this.client.request<DelegationConversationView>('delegations.get', { delegationId: this.target.delegationId })
          if (generation !== this.generation) return
          assertDelegationScope(detail, this.target)
          if (!detail.activity || !Array.isArray(detail.activity.runs) || !Array.isArray(detail.activity.approvals) || !Array.isArray(detail.activity.executionEvidence)) throw new Error('delegation.viewer_activity_unavailable')
          if (this.state.detail && this.state.detail.conversationId !== detail.conversationId) throw new Error('delegation.viewer_child_changed')
          let page = await this.client.request<DelegationHistoryPage>('delegations.history', { task_id: this.target.delegationId, limit: 20 })
          if (generation !== this.generation) return
          this.validateHistory(page, detail)
          const known = new Set(this.state.messages.map(message => message.id))
          let fresh = page.messages
          const seen = new Set<string>()
          // Catch up through the signed page chain before preserving the older
          // cursor. A refresh after a long disconnect must not create a silent
          // hole between the cached transcript and the newest twenty messages.
          while (known.size > 0 && page.next_cursor && !page.messages.some(message => known.has(message.id))) {
            if (seen.has(page.next_cursor)) throw new Error('delegation.viewer_cursor_repeated')
            seen.add(page.next_cursor)
            page = await this.client.request<DelegationHistoryPage>('delegations.history', { task_id: this.target.delegationId, before_cursor: page.next_cursor, limit: 20 })
            if (generation !== this.generation) return
            this.validateHistory(page, detail)
            fresh = mergeDelegationHistory(fresh, page.messages)
          }
          const overlaps = fresh.some(message => known.has(message.id))
          this.publish({ detail, messages: mergeDelegationHistory(overlaps ? this.state.messages : [], fresh), nextCursor: overlaps ? this.state.nextCursor : page.next_cursor ?? null,
            loading: false, error: false })
        } catch { if (generation === this.generation) this.publish({ loading: false, error: true }) }
      } while (this.dirty && this.users > 0 && generation === this.generation)
    })().finally(() => { if (this.reading === reading) { this.reading = null; this.schedule() } })
    this.reading = reading
    return reading
  }
  loadEarlier = async (): Promise<void> => {
    const generation = this.generation
    const detail = this.state.detail, cursor = this.state.nextCursor
    if (!detail || !cursor || this.state.paging) return
    this.publish({ paging: true })
    try {
      const page = await this.client.request<DelegationHistoryPage>('delegations.history', { task_id: this.target.delegationId, before_cursor: cursor, limit: 20 })
      if (generation !== this.generation) return
      this.validateHistory(page, detail)
      if (this.state.nextCursor !== cursor) return
      if (page.next_cursor === cursor) throw new Error('delegation.viewer_cursor_repeated')
      this.publish({ messages: mergeDelegationHistory(this.state.messages, page.messages), nextCursor: page.next_cursor ?? null, error: false })
    } catch { if (generation === this.generation) this.publish({ error: true }) }
    finally { if (generation === this.generation) this.publish({ paging: false }) }
  }
}

/** Metadata-only parent list, with exact signed cursor and no body prefetch. */
export async function delegationChildren(client: Pick<Client, 'request'>, parentConversationId: string, cursor?: string): Promise<DelegationListPage> {
  const page = await client.request<DelegationListPage>('delegations.list', { parentConversationId, ...(cursor ? { cursor } : {}), limit: 20 })
  if (page.parentConversationId !== parentConversationId || page.items.some(item => item.parentConversationId !== parentConversationId)) throw new Error('delegation.viewer_list_scope_mismatch')
  return page
}
const controllers = new WeakMap<object, Map<string, DelegationViewerController>>()
export function delegationViewerController(client: Client, target: DelegationViewerTarget): DelegationViewerController {
  let contexts = controllers.get(client)
  if (!contexts) { contexts = new Map(); controllers.set(client, contexts) }
  const key = JSON.stringify([target.threadId, target.parentConversationId, target.delegationId])
  let controller = contexts.get(key)
  if (!controller) { controller = new DelegationViewerController(client, { ...target }); contexts.set(key, controller) }
  return controller
}
