import type { SingleChatSnapshot } from '@contracts'
import type { ThreadClient } from './camp-client'
import type { ThreadLeaveGuard, ThreadLeavePreparation } from './ThreadWorkspace'
import { canvasComposerContextKey } from './canvas-composer-drafts'
import { readCanvasConversation, type CanvasConversationTarget } from './canvas-conversation'
import type { ThreadProjectionStore } from './thread-projection-store'
import { readErrorMessage } from './error-message'
import { uiAttribute } from './interface-language'

export interface CanvasConversationInstance {
  key: string
  target: CanvasConversationTarget
  conversation: SingleChatSnapshot['conversation'] | null
}
export interface CanvasConversationsState { instances: CanvasConversationInstance[]; busyKeys: readonly string[]; opening: boolean; issue: string | null }
export const initialCanvasConversations = (): CanvasConversationsState => ({ instances: [], busyKeys: [], opening: false, issue: null })

/** Presentation instances only. Core owns conversations/execution; each real card retains its input context. */
export class CanvasConversationsController {
  private state = initialCanvasConversations()
  private readonly listeners = new Set<() => void>()
  private readonly guards = new Map<string, ThreadLeaveGuard>()
  private disposed = false
  private generation = 0
  constructor(private readonly client: Pick<ThreadClient, 'request' | 'presentationScope'>, private readonly reads: Pick<ThreadProjectionStore, 'open'>, private readonly afterCommit: () => Promise<void>) {}
  getSnapshot = (): CanvasConversationsState => this.state
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener) } }
  private publish(change: Partial<CanvasConversationsState>): void {
    if (this.disposed) return
    this.state = { ...this.state, ...change }
    for (const listener of this.listeners) listener()
  }
  register(key: string, guard: ThreadLeaveGuard | null): void { if (guard) this.guards.set(key, guard); else this.guards.delete(key) }
  async open(input: CanvasConversationTarget): Promise<boolean> {
    if (this.disposed || this.state.opening) return false
    const target = structuredClone(input), context = target.draftContext
    if (!context || context.host !== this.client.presentationScope || context.boardId !== target.boardId || context.nodeId !== target.nodeId
      || context.threadId !== target.reference.threadId || context.conversationId !== (target.reference.conversationId ?? null)) {
      this.publish({ issue: uiAttribute('画布输入的所属信息不完整，请重新读取画板。') }); return false
    }
    const key = canvasComposerContextKey(context), existing = this.state.instances.find(item => item.key === key)
    if (existing) {
      const others = this.state.instances.filter(item => item.key !== key).map(item => target.presentation === 'drawer' && item.target.presentation === 'drawer'
        ? { ...item, target: { ...item.target, presentation: 'inline' as const } } : item)
      this.publish({ instances: [...others, { ...existing, target }], issue: null })
      return true
    }
    const generation = this.generation
    const current = () => !this.disposed && generation === this.generation
    this.publish({ opening: true, issue: null })
    try {
      const info = await this.client.request<{ dataDir: string }>('app.info')
      if (!current()) return false
      if (info.dataDir !== context.core) throw new Error(uiAttribute('画布输入的所属信息不完整，请重新读取画板。'))
      const loaded = await readCanvasConversation(target.reference,
        conversationId => this.client.request<SingleChatSnapshot | null>('singleChat.get', { conversationId }),
        async threadId => (await this.reads.open(threadId)).snapshot, current)
      if (!loaded || !current()) return false
      this.publish({ instances: [...this.state.instances, { key, target, conversation: loaded.conversation }] })
      return true
    } catch (error) { if (current()) this.publish({ issue: readErrorMessage(error) }); return false }
    finally { if (current()) this.publish({ opening: false }) }
  }
  async close(keys: readonly string[] = this.state.instances.map(item => item.key)): Promise<boolean> {
    if (this.disposed || this.state.opening) return false
    const selected = this.state.instances.filter(item => keys.includes(item.key))
    if (!selected.length) return true
    const generation = this.generation, preparations: ThreadLeavePreparation[] = []
    this.publish({ opening: true, busyKeys: selected.map(item => item.key), issue: null })
    try {
      for (const instance of selected) {
        const guard = this.guards.get(instance.key)
        if (guard) preparations.push(await guard())
        if (this.disposed || generation !== this.generation) throw new Error(uiAttribute('会话读取已失效，请重新打开。'))
      }
      const closed = new Set(selected.map(item => item.key))
      this.publish({ instances: this.state.instances.filter(item => !closed.has(item.key)) })
      await this.afterCommit()
      for (const preparation of preparations) preparation.complete(true)
      for (const key of closed) this.guards.delete(key)
      return true
    } catch (error) {
      for (const preparation of preparations) preparation.complete(false)
      this.publish({ issue: readErrorMessage(error) }); return false
    } finally { this.publish({ opening: false, busyKeys: [] }) }
  }
  async prepare(keys: readonly string[]): Promise<ThreadLeavePreparation> {
    if (this.disposed || this.state.opening) throw new Error(uiAttribute('当前对话输入尚未保存，请处理后重试。'))
    const selected = this.state.instances.filter(item => keys.includes(item.key)), preparations: ThreadLeavePreparation[] = []
    const generation = this.generation
    this.publish({ opening: true, busyKeys: selected.map(item => item.key), issue: null })
    try {
      for (const item of selected) {
        const guard = this.guards.get(item.key)
        if (guard) preparations.push(await guard())
        if (this.disposed || generation !== this.generation) throw new Error(uiAttribute('会话读取已失效，请重新打开。'))
      }
    } catch (error) {
      for (const item of preparations) item.complete(false)
      this.publish({ opening: false, busyKeys: [], issue: readErrorMessage(error) }); throw error
    }
    let completed = false
    return { complete: () => {
      if (completed) return
      completed = true
      // A layout operation did not leave the editor. Missing pins retain their original input in the drawer.
      for (const item of preparations) item.complete(false)
      this.publish({ opening: false, busyKeys: [] })
    } }
  }
  dispose(): void { this.generation += 1; this.disposed = true; this.listeners.clear(); this.guards.clear() }
}
