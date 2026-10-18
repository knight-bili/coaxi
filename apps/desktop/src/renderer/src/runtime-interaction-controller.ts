import type { ResolveRuntimeInteractionParams, RuntimeInteractionAnswer, RuntimeInteractionPage, RuntimeInteractionView, StoredCommandResult } from '@contracts'
import type { ThreadClient } from './camp-client'
import { newCommandId } from '../../shared/command-id'

type Client = Pick<ThreadClient, 'request' | 'onEvent' | 'onInvalidated' | 'retryPendingCommand'>
type Intent = { params: ResolveRuntimeInteractionParams; unknown: boolean }
export interface RuntimeInteractionState {
  summaries: RuntimeInteractionPage['items']
  views: Readonly<Record<string, RuntimeInteractionView>>
  intents: Readonly<Record<string, Intent>>
  loading: boolean
  error: boolean
  decisionError: string | null
}

/** One authenticated client / real Conversation. A remounted canvas card keeps
 * the original UUID; presentation never constructs native ownership or RPC IDs. */
export class RuntimeInteractionController {
  private state: RuntimeInteractionState = { summaries: [], views: {}, intents: {}, loading: true, error: false, decisionError: null }
  private listeners = new Set<() => void>()
  private reading: Promise<void> | null = null
  private dirty = false
  private opened = new Set<string>()
  private users = 0
  private releaseEvents?: () => void
  private releaseInvalidation?: () => void
  private recovering = new Set<string>()
  constructor(private client: Client, readonly conversationId: string) {}
  getSnapshot = (): RuntimeInteractionState => this.state
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener) } }
  private publish(change: Partial<RuntimeInteractionState>): void {
    this.state = { ...this.state, ...change }
    this.listeners.forEach(listener => listener())
  }
  retain(): () => void {
    if (this.users++ === 0) {
      this.releaseEvents = this.client.onEvent?.(event => {
        if (event.method === 'runtime.interaction.changed' && (event.params as { conversationId?: string })?.conversationId === this.conversationId) void this.refresh()
      })
      this.releaseInvalidation = this.client.onInvalidated?.(() => { void this.refresh() })
      void this.refresh()
    }
    return () => {
      if (--this.users === 0) { this.releaseEvents?.(); this.releaseInvalidation?.() }
    }
  }
  /** Scan bounded summary pages so an older blocking request cannot disappear
   * behind newer history. Bodies are read only for the explicitly opened card. */
  refresh = (): Promise<void> => {
    if (this.reading) { this.dirty = true; return this.reading }
    this.reading = (async () => {
      do {
        this.dirty = false
        try {
          let cursor: string | null = null
          const summaries: RuntimeInteractionPage['items'] = []
          const cursors = new Set<string>()
          do {
            const page: RuntimeInteractionPage = await this.client.request('runtime.interactions.list', { conversationId: this.conversationId, limit: 50, cursor })
            if (page.conversationId !== this.conversationId) throw new Error('interaction.scope_mismatch')
            summaries.push(...page.items)
            cursor = page.nextCursor
            if (cursor && cursors.has(cursor)) throw new Error('interaction.cursor_repeated')
            if (cursor) cursors.add(cursor)
          } while (cursor)
          this.publish({ summaries, loading: false, error: false })
          for (const id of this.opened) await this.read(id)
        } catch { this.publish({ loading: false, error: true }) }
      } while (this.dirty && this.users > 0)
    })().finally(() => { this.reading = null })
    return this.reading
  }
  open = async (id: string): Promise<void> => {
    this.opened.clear()
    this.opened.add(id)
    try { await this.read(id) } catch { this.publish({ error: true }) }
  }
  private async read(id: string): Promise<void> {
    const view = await this.client.request<RuntimeInteractionView | null>('runtime.interactions.get', { interactionId: id })
    if (view === null) {
      const views = { ...this.state.views }; delete views[id]
      this.publish({ views, summaries: this.state.summaries.filter(item => item.id !== id) })
      return
    }
    if (view.conversationId !== this.conversationId || view.id !== id) throw new Error('interaction.scope_mismatch')
    if ((this.state.views[id]?.version ?? 0) > view.version) return
    this.publish({ views: { ...this.state.views, [id]: view } })
  }
  answer = (id: string, answer: RuntimeInteractionAnswer): void => {
    const view = this.state.views[id]
    if (!view || view.status !== 'pending' || this.state.intents[id] || this.state.error
      || this.state.summaries.some(item => item.id === id && item.version > view.version)) return
    const intent: Intent = { params: { commandId: newCommandId(), command: { interactionId: id, expectedVersion: view.version, answer: structuredClone(answer) } }, unknown: false }
    this.publish({ intents: { ...this.state.intents, [id]: intent }, decisionError: null })
    void this.dispatch(id, intent)
  }
  private async dispatch(id: string, intent: Intent): Promise<void> {
    try {
      const result = await this.client.request<StoredCommandResult>('runtime.interactions.resolve', structuredClone(intent.params))
      await this.accept(id, intent, result)
    } catch {
      if (this.state.intents[id]?.params.commandId === intent.params.commandId) {
        this.publish({ intents: { ...this.state.intents, [id]: { ...intent, unknown: true } } })
      }
    }
  }
  private async accept(id: string, intent: Intent, result: StoredCommandResult): Promise<void> {
    if (this.state.intents[id]?.params.commandId !== intent.params.commandId) return
    // Read before unlocking: a committed answer must never flash editable while
    // waiting for a new Core view. Failure leaves the original UUID recoverable.
    await this.read(id)
    if (this.state.intents[id]?.params.commandId !== intent.params.commandId) return
    if (result.status !== 'rejected' && this.state.views[id]?.status === 'pending') throw new Error('interaction.read_not_caught_up')
    const intents = { ...this.state.intents }; delete intents[id]
    this.publish({ intents, decisionError: result.status === 'rejected' ? id : null })
    await this.refresh()
  }
  /** Reconcile is read-only. Only an explicit retry may redispatch the SAME
   * complete command, including the frozen answer/version, after an unknown. */
  recover = async (id: string, retry = false): Promise<void> => {
    const intent = this.state.intents[id]
    if (!intent || this.recovering.has(id)) return
    this.recovering.add(id)
    try {
      const reply = await this.client.request<{ state: 'unknown' | 'recorded'; result?: StoredCommandResult }>('commands.reconcile', { operation: 'runtime.interactions.resolve', params: structuredClone(intent.params) })
      if (this.state.intents[id]?.params.commandId !== intent.params.commandId) return
      if (reply.state === 'recorded' && reply.result) await this.accept(id, intent, reply.result)
      else if (reply.state === 'unknown') {
        this.publish({ intents: { ...this.state.intents, [id]: { ...intent, unknown: true } } })
        if (retry) {
          if (this.client.retryPendingCommand) {
            await this.client.retryPendingCommand('runtime.interactions.resolve', structuredClone(intent.params))
            const retried = await this.client.request<{ state: string; result?: StoredCommandResult }>('commands.reconcile', { operation: 'runtime.interactions.resolve', params: structuredClone(intent.params) })
            if (retried.state === 'recorded' && retried.result) await this.accept(id, intent, retried.result)
          } else await this.dispatch(id, intent)
        }
      }
    } catch {
      if (this.state.intents[id]?.params.commandId === intent.params.commandId) this.publish({ intents: { ...this.state.intents, [id]: { ...intent, unknown: true } } })
    } finally { this.recovering.delete(id) }
  }
}

const controllers = new WeakMap<object, Map<string, RuntimeInteractionController>>()
export function runtimeInteractionController(client: Client, conversationId: string): RuntimeInteractionController {
  let conversations = controllers.get(client)
  if (!conversations) { conversations = new Map(); controllers.set(client, conversations) }
  let controller = conversations.get(conversationId)
  if (!controller) { controller = new RuntimeInteractionController(client, conversationId); conversations.set(conversationId, controller) }
  return controller
}
