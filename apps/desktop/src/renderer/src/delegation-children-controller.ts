import type { DelegationDetail } from '@contracts'
import type { ThreadClient } from './camp-client'
import { delegationChildren } from './delegation-viewer-controller'

type Client = Pick<ThreadClient, 'request' | 'onEvent' | 'onInvalidated'>
type Target = { threadId: string; parentConversationId: string }
export interface DelegationChildrenState { rows: DelegationDetail[]; loading: boolean; initialized: boolean; error: boolean }
const empty = (): DelegationChildrenState => ({ rows: [], loading: true, initialized: false, error: false })

/** One metadata-only page chain per client/Thread/parent, shared by tool cards
 * and the parent list. Full tasks and child transcripts never enter this cache. */
export class DelegationChildrenController {
  private state = empty()
  private listeners = new Set<() => void>()
  private users = 0
  private generation = 0
  private reading: Promise<void> | null = null
  private dirty = false
  private timer: ReturnType<typeof setTimeout> | null = null
  private offEvent?: () => void
  private offInvalidation?: () => void
  constructor(private client: Client, readonly target: Target) {}
  getSnapshot = (): DelegationChildrenState => this.state
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener) } }
  private publish(state: DelegationChildrenState): void { this.state = state; this.listeners.forEach(listener => listener()) }
  private clear(): void {
    ++this.generation; this.reading = null; this.dirty = false
    if (this.timer) clearTimeout(this.timer)
    this.timer = null; this.publish(empty())
  }
  retain(): () => void {
    if (this.users++ === 0) {
      this.offEvent = this.client.onEvent?.(event => {
        const params = event.params as { threadId?: string }
        if (event.method === 'delegation.changed' && params?.threadId === this.target.threadId) void this.refresh()
      })
      this.offInvalidation = this.client.onInvalidated?.(() => { this.clear(); void this.refresh() })
      void this.refresh()
    }
    let released = false
    return () => {
      if (released) return
      released = true
      if (--this.users === 0) { this.offEvent?.(); this.offInvalidation?.(); this.clear() }
    }
  }
  refresh = (): Promise<void> => {
    if (this.reading) { this.dirty = true; return this.reading }
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    const generation = this.generation
    const promise = (async () => {
      do {
        this.dirty = false
        try {
          let page = await delegationChildren(this.client, this.target.parentConversationId)
          const rows = [...page.items], cursors = new Set<string>()
          while (page.nextCursor) {
            if (generation !== this.generation) return
            if (cursors.has(page.nextCursor)) throw new Error('delegation.viewer_cursor_repeated')
            cursors.add(page.nextCursor)
            page = await delegationChildren(this.client, this.target.parentConversationId, page.nextCursor)
            rows.push(...page.items)
          }
          if (rows.some(row => row.threadId !== this.target.threadId)
            || new Set(rows.map(row => row.delegationId)).size !== rows.length) throw new Error('delegation.viewer_list_scope_mismatch')
          if (generation === this.generation) this.publish({ rows, initialized: true, loading: false, error: false })
        } catch { if (generation === this.generation) this.publish({ ...this.state, loading: false, error: true }) }
      } while (this.dirty && this.users > 0 && generation === this.generation)
    })().finally(() => {
      if (this.reading !== promise) return
      this.reading = null
      if (this.users > 0) this.timer = setTimeout(() => { void this.refresh() }, 5_000)
    })
    this.reading = promise
    return promise
  }
}

const controllers = new WeakMap<object, Map<string, DelegationChildrenController>>()
export function delegationChildrenController(client: Client, target: Target): DelegationChildrenController {
  let contexts = controllers.get(client)
  if (!contexts) { contexts = new Map(); controllers.set(client, contexts) }
  const key = JSON.stringify([target.threadId, target.parentConversationId])
  let controller = contexts.get(key)
  if (!controller) { controller = new DelegationChildrenController(client, { ...target }); contexts.set(key, controller) }
  return controller
}
