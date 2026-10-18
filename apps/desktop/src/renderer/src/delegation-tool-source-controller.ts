import type { DelegationToolSourceIdentity, DelegationToolSourceView } from '@contracts'
import type { ThreadClient } from './camp-client'
import { assertDelegationScope } from './delegation-viewer-controller'

export type DelegationToolSourceTarget = Omit<DelegationToolSourceIdentity, 'parentConversationId'>
type Client = Pick<ThreadClient, 'request' | 'onEvent' | 'onInvalidated'>
export interface DelegationToolSourceState { view: DelegationToolSourceView | null; loading: boolean; error: boolean }
const empty = (): DelegationToolSourceState => ({ view: null, loading: true, error: false })

export function assertDelegationToolSource(view: DelegationToolSourceView, target: DelegationToolSourceTarget): void {
  const source = view?.source
  if (!source || source.threadId !== target.threadId || source.evidenceId !== target.evidenceId
    || source.agentRunId !== target.agentRunId || source.executionEpoch !== target.executionEpoch
    || typeof source.parentConversationId !== 'string' || !source.parentConversationId) throw new Error('delegation.tool_source_scope_mismatch')
  if (view.state === 'unresolved') return
  if (!['delegation.start', 'delegation.resume'].includes(view.operation)) throw new Error('delegation.tool_source_operation_mismatch')
  if (view.state === 'refused' && typeof view.code === 'string' && view.code) return
  if (view.state !== 'bound' || typeof view.task !== 'string' || !(view.reason === null || typeof view.reason === 'string')
    || !Number.isSafeInteger(view.attemptNo) || view.attemptNo < 1
    || (view.operation === 'delegation.start' ? view.attemptNo !== 1 : view.attemptNo < 2)
    || !view.detail?.delegationId || !view.detail.conversationId || view.detail.currentAttemptNo < view.attemptNo) throw new Error('delegation.tool_source_incompatible')
  assertDelegationScope(view.detail, { ...source, delegationId: view.detail.delegationId })
}

/** Immutable task/reason is fetched once per visible source, or after explicit
 * retry/auth invalidation. Current status uses the separate shared metadata list. */
export class DelegationToolSourceController {
  private state = empty()
  private listeners = new Set<() => void>()
  private users = 0
  private generation = 0
  private reading: Promise<void> | null = null
  private dirty = false
  private offEvent?: () => void
  private offInvalidation?: () => void
  constructor(private client: Client, readonly target: DelegationToolSourceTarget) {}
  getSnapshot = (): DelegationToolSourceState => this.state
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener) } }
  private publish(state: DelegationToolSourceState): void { this.state = state; this.listeners.forEach(listener => listener()) }
  private clear(): void { ++this.generation; this.reading = null; this.dirty = false; this.publish(empty()) }
  retain(): () => void {
    if (this.users++ === 0) {
      this.offEvent = this.client.onEvent?.(event => {
        const params = event.params as { threadId?: string; agentRunId?: string }
        if (params?.threadId === this.target.threadId && params.agentRunId === this.target.agentRunId) this.evidenceChanged()
      })
      this.offInvalidation = this.client.onInvalidated?.(() => { this.clear(); void this.refresh() })
      void this.refresh()
    }
    let released = false
    return () => { if (released) return; released = true; if (--this.users === 0) { this.offEvent?.(); this.offInvalidation?.(); this.clear() } }
  }
  evidenceChanged(): void {
    if (!this.users || this.state.view?.state === 'bound' || this.state.view?.state === 'refused') return
    if (this.reading) this.dirty = true
    else void this.refresh()
  }
  refresh = (): Promise<void> => {
    if (this.reading) return this.reading
    const generation = this.generation
    this.publish({ ...this.state, loading: true, error: false })
    const promise = this.client.request<DelegationToolSourceView>('delegations.source', { threadId: this.target.threadId, evidenceId: this.target.evidenceId })
      .then(view => {
        assertDelegationToolSource(view, this.target)
        if (generation === this.generation) this.publish({ view, loading: false, error: false })
      }).catch(() => { if (generation === this.generation) this.publish({ view: null, loading: false, error: true }) })
      .finally(() => {
        if (this.reading !== promise) return
        this.reading = null
        const again = this.dirty && this.users > 0 && this.state.view?.state === 'unresolved'
        this.dirty = false
        if (again) void this.refresh()
      })
    this.reading = promise
    return promise
  }
}
