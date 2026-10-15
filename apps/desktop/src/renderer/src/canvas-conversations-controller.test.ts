import { expect, it, vi } from 'vitest'
import type { CoreMethod } from '@contracts'
import { CanvasConversationsController } from './canvas-conversations-controller'
import { canvasComposerContextKey, type CanvasComposerContext } from './canvas-composer-drafts'
import type { CanvasConversationTarget } from './canvas-conversation'
import type { ThreadSurfaceSnapshot } from './thread-workspace-data'

const context: CanvasComposerContext = { host: 'host', core: '/core', boardId: 'board', nodeId: 'first', threadId: 'thread', conversationId: null }
const target = (changes: Partial<CanvasComposerContext> = {}): CanvasConversationTarget => {
  const draftContext = { ...context, ...changes }
  return { boardId: draftContext.boardId, nodeId: draftContext.nodeId, draftContext,
    reference: { threadId: draftContext.threadId, conversationId: draftContext.conversationId }, presentation: 'inline' }
}
function fixture(handler: (method: CoreMethod, params?: unknown) => Promise<unknown> = async method => method === 'app.info' ? { dataDir: '/core' } : null, afterCommit: () => Promise<void> = async () => undefined) {
  const requests: Array<{ method: CoreMethod; params: unknown }> = []
  const client = { presentationScope: context.host, request<T>(method: CoreMethod, params?: unknown): Promise<T> {
    requests.push({ method, params }); return handler(method, params) as Promise<T>
  } }
  const reads = { open: vi.fn(async (threadId: string) => ({ snapshot: { thread: { id: threadId }, throughGlobalSequence: 1 } as ThreadSurfaceSnapshot,
    traceId: 'trace', startedAt: 0, receivedAt: 1 })) }
  return { requests, reads, controller: new CanvasConversationsController(client, reads, afterCommit) }
}

it('opens multiple real card contexts, including mirrors, without replacing their instances or entering a Thread', async () => {
  const f = fixture()
  expect(await f.controller.open(target())).toBe(true)
  expect(await f.controller.open(target({ nodeId: 'mirror' }))).toBe(true)
  expect(await f.controller.open(target({ nodeId: 'other-thread', threadId: 'other' }))).toBe(true)
  expect(f.controller.getSnapshot().instances.map(item => item.target.nodeId)).toEqual(['first', 'mirror', 'other-thread'])
  expect(new Set(f.controller.getSnapshot().instances.map(item => item.key)).size).toBe(3)
  expect(f.requests.map(item => item.method)).toEqual(['app.info', 'app.info', 'app.info'])
  expect(f.reads.open.mock.calls.map(([id]) => id)).toEqual(['thread', 'thread', 'other'])
})

it('changes presentation of the same context without reading again, re-keying input or closing another card', async () => {
  const f = fixture()
  await f.controller.open(target()); await f.controller.open(target({ nodeId: 'second' }))
  const first = f.controller.getSnapshot().instances[0], guard = vi.fn()
  f.controller.register(first.key, guard)
  expect(await f.controller.open({ ...first.target, presentation: 'drawer' })).toBe(true)
  const changed = f.controller.getSnapshot().instances.at(-1)!
  expect(changed.key).toBe(first.key)
  expect(changed.target.draftContext).toEqual(first.target.draftContext)
  expect(f.controller.getSnapshot().instances).toHaveLength(2)
  expect(f.reads.open).toHaveBeenCalledTimes(2)
  expect(guard).not.toHaveBeenCalled()
  await f.controller.open({ ...target({ nodeId: 'second' }), presentation: 'drawer' })
  expect(f.controller.getSnapshot().instances.filter(item => item.target.presentation === 'drawer')).toHaveLength(1)
})

it('rejects missing, substituted or foreign Host/Core input ownership before loading a Thread', async () => {
  const f = fixture()
  const wrong = [ { ...target(), draftContext: undefined }, target({ host: 'other' }), target({ core: '/other' }),
    { ...target(), nodeId: 'substituted' }, { ...target(), reference: { threadId: 'other', conversationId: null } } ]
  for (const item of wrong) expect(await f.controller.open(item)).toBe(false)
  expect(f.controller.getSnapshot().instances).toEqual([])
  expect(f.reads.open).not.toHaveBeenCalled()
})

it('verifies the exact private conversation before using shared public Thread data and never creates a replacement', async () => {
  const f = fixture(async (method, params) => method === 'app.info' ? { dataDir: '/core' } : {
    conversation: { id: (params as { conversationId: string }).conversationId, threadId: 'thread', agentId: 'member', status: 'active' }
  })
  expect(await f.controller.open(target({ conversationId: 'private' }))).toBe(true)
  expect(f.requests).toEqual([{ method: 'app.info', params: undefined }, { method: 'singleChat.get', params: { conversationId: 'private' } }])
  expect(f.controller.getSnapshot().instances[0].conversation?.id).toBe('private')
  const ended = fixture(async method => method === 'app.info' ? { dataDir: '/core' } : { conversation: { id: 'private', threadId: 'thread', status: 'ended' } })
  expect(await ended.controller.open(target({ conversationId: 'private' }))).toBe(false)
  expect(ended.reads.open).not.toHaveBeenCalled()
})

it('prepares a whole close batch and rolls every prepared editor back if a later editor cannot leave', async () => {
  const f = fixture(), complete = vi.fn()
  await f.controller.open(target()); await f.controller.open(target({ nodeId: 'second' }))
  const before = f.controller.getSnapshot().instances
  f.controller.register(before[0].key, async () => ({ complete }))
  f.controller.register(before[1].key, async () => { throw new Error('unsaved input') })
  expect(await f.controller.close()).toBe(false)
  expect(f.controller.getSnapshot().instances).toBe(before)
  expect(complete).toHaveBeenCalledExactlyOnceWith(false)
  expect(f.controller.getSnapshot()).toMatchObject({ opening: false, busyKeys: [] })
})

it('closes only selected cards and completes leave only after their committed unmount checkpoint', async () => {
  const complete = vi.fn()
  let release!: () => void
  const committed = new Promise<void>(resolve => { release = resolve })
  const f = fixture(undefined, () => committed)
  await f.controller.open(target()); await f.controller.open(target({ nodeId: 'second' }))
  const first = f.controller.getSnapshot().instances[0]
  f.controller.register(first.key, async () => ({ complete }))
  const closing = f.controller.close([first.key])
  await vi.waitFor(() => expect(f.controller.getSnapshot().instances.map(item => item.target.nodeId)).toEqual(['second']))
  expect(complete).not.toHaveBeenCalled()
  expect(await f.controller.open(target({ nodeId: 'third' }))).toBe(false)
  release(); expect(await closing).toBe(true)
  expect(complete).toHaveBeenCalledExactlyOnceWith(true)
})

it('does not publish a delayed open after client disposal and blocks conflicting close during that open', async () => {
  let release!: (value: unknown) => void
  const f = fixture(async () => new Promise(resolve => { release = resolve }))
  const opening = f.controller.open(target())
  expect(await f.controller.close()).toBe(false)
  f.controller.dispose(); release({ dataDir: '/core' })
  expect(await opening).toBe(false)
  expect(f.reads.open).not.toHaveBeenCalled()
  expect(f.controller.getSnapshot().instances).toEqual([])
})

it('uses exact context identities when one pin is rebound and retains the old input as a separate instance', async () => {
  const f = fixture()
  await f.controller.open(target()); await f.controller.open(target({ threadId: 'replacement' }))
  const keys = f.controller.getSnapshot().instances.map(item => item.key)
  expect(keys).toEqual([canvasComposerContextKey(context), canvasComposerContextKey({ ...context, threadId: 'replacement' })])
})

it('flushes cards for a graph mutation but keeps their instances and unlocks them when pins disappear', async () => {
  const f = fixture(), complete = vi.fn()
  await f.controller.open(target())
  const before = f.controller.getSnapshot().instances
  f.controller.register(before[0].key, async () => ({ complete }))
  const preparation = await f.controller.prepare([before[0].key])
  expect(f.controller.getSnapshot().instances).toBe(before)
  expect(f.controller.getSnapshot().busyKeys).toEqual([before[0].key])
  preparation.complete(true); preparation.complete(false)
  expect(complete).toHaveBeenCalledExactlyOnceWith(false)
  expect(f.controller.getSnapshot()).toMatchObject({ instances: before, opening: false, busyKeys: [] })
})

it('rolls back a failed graph preparation without closing any of its cards', async () => {
  const f = fixture(), complete = vi.fn()
  await f.controller.open(target()); await f.controller.open(target({ nodeId: 'second' }))
  const before = f.controller.getSnapshot().instances
  f.controller.register(before[0].key, async () => ({ complete }))
  f.controller.register(before[1].key, async () => { throw new Error('storage quota') })
  await expect(f.controller.prepare(before.map(item => item.key))).rejects.toThrow('storage quota')
  expect(f.controller.getSnapshot().instances).toBe(before)
  expect(complete).toHaveBeenCalledExactlyOnceWith(false)
})
