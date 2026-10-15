import type { ThreadClient } from './camp-client'

type Owner = Pick<ThreadClient, 'editingRecovery' | 'onInvalidated'>
export interface SingleChatComposerState {
  readonly drafts: Readonly<Record<string, string>>
  readonly ready: boolean
  readonly error: Error | null
  readonly sendingKeys: readonly string[]
}
export interface SingleChatSendLease {
  clearSentBody(originalBody: string): boolean
  release(): void
}
const dictionary = (value: unknown): Record<string, string> => value && typeof value === 'object'
  ? Object.fromEntries(Object.entries(value).filter(([, item]) => typeof item === 'string')) : {}
const same = (left: Readonly<Record<string, string>>, right: Readonly<Record<string, string>>) =>
  Object.keys(left).length === Object.keys(right).length && Object.keys(left).every(key => left[key] === right[key])
const errorValue = (value: unknown) => value instanceof Error ? value : new Error(String(value))

/** One existing client-local recovery dictionary per client/Thread, shared by its mounted mirrors.
 * Body storage keeps the existing single-chat:<Thread> key and string-map shape. No Core draft or model format is added. */
export class SingleChatComposerStore {
  private state: SingleChatComposerState = { drafts: Object.freeze({}), ready: false, error: null, sendingKeys: [] }
  private readonly listeners = new Set<() => void>()
  private readonly sends = new Map<string, object>()
  private unsubscribe: (() => void) | undefined
  private readonly key: string

  constructor(private readonly owner: Owner, threadId: string) {
    this.key = `single-chat:${threadId}`
    this.load()
  }
  getSnapshot = (): SingleChatComposerState => this.state
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    if (this.listeners.size === 1) {
      this.unsubscribe = this.owner.onInvalidated?.(() => this.reconcile())
      this.reconcile()
    }
    return () => {
      this.listeners.delete(listener)
      if (!this.listeners.size) { this.unsubscribe?.(); this.unsubscribe = undefined }
    }
  }
  update(next: (current: Readonly<Record<string, string>>) => Record<string, string>): boolean {
    if (!this.state.ready) return false
    const drafts = next(this.state.drafts)
    if (same(drafts, this.state.drafts)) return !this.state.error
    return this.persist(drafts)
  }
  retry(): boolean { return this.state.ready ? this.persist(this.state.drafts) : this.load() }
  canLeave(key?: string): boolean {
    return this.state.ready && !this.state.error && (key ? !this.sends.has(key) : this.sends.size === 0)
  }
  migratePending(from: string, to: string): boolean {
    if (this.sends.has(from) || this.sends.has(to)) return false
    return this.update(current => {
      if (!(from in current)) return current
      if (current[from] && current[to] && current[from] !== current[to]) return current
      const next = { ...current, [to]: current[to] || current[from] }
      delete next[from]
      return next
    })
  }
  swapBodies(left: string, right: string): boolean {
    if (!this.canLeave(left) || !this.canLeave(right)) return false
    return this.update(current => ({ ...current, [left]: current[right] ?? '', [right]: current[left] ?? '' }))
  }
  restoreReturnedBody(key: string, body: string, commandId: string): boolean {
    const retained = `${key}:returned:${commandId}`
    return this.update(current => {
      // Empty entries also mark an applied receipt; replay must not restore a body the user already sent or edited.
      if (Object.hasOwn(current, retained)) return current
      return current[key] && current[key] !== body
        ? { ...current, [retained]: body }
        : { ...current, [key]: body, [retained]: '' }
    })
  }
  tryBeginSend(key: string): SingleChatSendLease | null {
    if (!this.state.ready || this.state.error || this.sends.has(key)) return null
    const token = {}
    this.sends.set(key, token); this.publish({ sendingKeys: [...this.sends.keys()] })
    return {
      clearSentBody: originalBody => {
        if (this.sends.get(key) !== token || (this.state.drafts[key] ?? '') !== originalBody) return false
        return this.update(current => ({ ...current, [key]: '' }))
      },
      release: () => {
        if (this.sends.get(key) !== token) return
        this.sends.delete(key); this.publish({ sendingKeys: [...this.sends.keys()] })
      }
    }
  }
  reconcile(): boolean {
    // An unrelated Core invalidation must not discard an unsaved body or revive a sending snapshot.
    if (!this.state.ready || this.state.error || this.sends.size || !this.owner.editingRecovery) return false
    try {
      const saved = this.owner.editingRecovery.get(this.key)
      if (!saved || typeof saved !== 'object') return false
      const drafts = dictionary(saved)
      if (!same(drafts, this.state.drafts)) this.publish({ drafts: Object.freeze(drafts) })
      return true
    } catch (error) { this.publish({ error: errorValue(error) }); return false }
  }
  private load(): boolean {
    try {
      const drafts = dictionary(this.owner.editingRecovery?.get(this.key))
      this.publish({ drafts: Object.freeze(drafts), ready: true, error: null })
      return true
    } catch (error) { this.publish({ ready: false, error: errorValue(error) }); return false }
  }
  private persist(drafts: Readonly<Record<string, string>>): boolean {
    const current = Object.freeze({ ...drafts })
    let error: Error | null = null
    try { this.owner.editingRecovery?.set(this.key, current) } catch (value) { error = errorValue(value) }
    // Keep the latest input visible after a write failure; explicit retry writes this value without sending a message.
    this.publish({ drafts: current, error })
    return error === null
  }
  private publish(change: Partial<SingleChatComposerState>): void {
    this.state = { ...this.state, ...change }
    for (const listener of this.listeners) listener()
  }
}
const owners = new WeakMap<object, Map<string, SingleChatComposerStore>>()
export function singleChatComposerStore(owner: Owner, threadId: string): SingleChatComposerStore {
  let threads = owners.get(owner)
  if (!threads) { threads = new Map(); owners.set(owner, threads) }
  let store = threads.get(threadId)
  if (!store) { store = new SingleChatComposerStore(owner, threadId); threads.set(threadId, store) }
  return store
}
