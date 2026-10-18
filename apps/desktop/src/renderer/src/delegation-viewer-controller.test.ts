import { describe, expect, it, vi } from 'vitest'
import type { DelegationConversationView, DelegationHistoryPage } from '@contracts'
import { DelegationViewerController, delegationChildren, delegationViewerController } from './delegation-viewer-controller'
import type { ThreadClient } from './camp-client'

const target = { threadId: 'thread-a', parentConversationId: 'parent-a', delegationId: 'delegation-a' }
const detail = (): DelegationConversationView => ({ ...target, ownerAgentId: 'owner-a', rootConversationId: target.parentConversationId, parentDelegationId: null,
  conversationId: 'child-a', depth: 1, currentAttemptNo: 1, controlVersion: 1, takenOver: false, agentRunId: 'run-a', executionEpoch: 1,
  status: 'succeeded', agentType: 'codex', createdAt: '', startedAt: null, endedAt: null, cleanupPending: false, activity: { runs: [], executionEvidence: [], approvals: [] } })
const message = (n: number): DelegationHistoryPage['messages'][number] => ({ id: `message-${n}`, sequence: n, author_type: n % 2 ? 'delegation_parent' : 'assistant', body: `完整消息 ${n}`, created_at: '' })
const page = (values: number[], cursor?: string): DelegationHistoryPage => ({ task_id: target.delegationId, conversation_id: 'child-a', messages: values.map(message), ...(cursor ? { next_cursor: cursor } : {}) })
type Client = Pick<ThreadClient, 'request' | 'onEvent' | 'onInvalidated'>
describe('delegated conversation viewer authority and paging', () => {
  it('keeps one exact client/Thread/parent/delegation and performs no execution mutation on opening', async () => {
    const calls: string[] = []
    const client: Client = { request: vi.fn(async method => { calls.push(method); return method === 'delegations.get' ? detail() : page([1, 2]) }) as Client['request'] }
    const controller = delegationViewerController(client, target)
    expect(controller).toBe(delegationViewerController(client, target))
    expect(controller).not.toBe(delegationViewerController({ ...client }, target))
    expect(controller).not.toBe(delegationViewerController(client, { ...target, parentConversationId: 'another-parent' }))
    await controller.refresh()
    expect(calls).toEqual(['delegations.get', 'delegations.history'])
    expect(controller.getSnapshot().messages.map(row => row.id)).toEqual(['message-1', 'message-2'])
  })
  it('does not publish another parent, child, Thread or a missing activity projection', async () => {
    for (const changed of [{ parentConversationId: 'parent-b' }, { threadId: 'thread-b' }, { delegationId: 'delegation-b' }, { activity: undefined }]) {
      const client: Client = { request: vi.fn(async () => ({ ...detail(), ...changed })) as Client['request'] }
      const controller = new DelegationViewerController(client, target)
      await controller.refresh()
      expect(controller.getSnapshot().error).toBe(true)
      expect(controller.getSnapshot().detail).toBeNull()
      expect(client.request).toHaveBeenCalledOnce()
    }
    const client: Client = { request: vi.fn(async method => method === 'delegations.get' ? detail() : { ...page([1]), conversation_id: 'another-child' }) as Client['request'] }
    const controller = new DelegationViewerController(client, target)
    await controller.refresh()
    expect(controller.getSnapshot().messages).toEqual([])
    expect(controller.getSnapshot().detail).toBeNull()
  })
  it('retains exact signed older cursor while closing a reconnect gap without duplicates', async () => {
    let latest = false
    const historyCalls: unknown[] = []
    const client: Client = { request: vi.fn(async (method, params) => {
      if (method === 'delegations.get') return detail()
      const input = params as { before_cursor?: string }; historyCalls.push(input)
      if (input.before_cursor === 'older') return page([1, 2])
      if (input.before_cursor === 'gap-1') return page([5, 6], 'gap-2')
      if (input.before_cursor === 'gap-2') return page([3, 4], 'older')
      return latest ? page([7, 8], 'gap-1') : page([3, 4], 'older')
    }) as Client['request'] }
    const controller = new DelegationViewerController(client, target)
    await controller.refresh(); latest = true; await controller.refresh()
    expect(controller.getSnapshot().messages.map(row => row.sequence)).toEqual([3, 4, 5, 6, 7, 8])
    expect(controller.getSnapshot().nextCursor).toBe('older')
    await controller.loadEarlier()
    expect(controller.getSnapshot().messages.map(row => row.sequence)).toEqual([1, 2, 3, 4, 5, 6, 7, 8])
    expect(controller.getSnapshot().nextCursor).toBeNull()
    expect(historyCalls.at(-1)).toEqual({ task_id: target.delegationId, before_cursor: 'older', limit: 20 })
  })
  it('coalesces refreshes, keeps the last valid transcript on error, and releases timers/listeners when closed', async () => {
    vi.useFakeTimers()
    try {
      let fail = false
      const off = vi.fn()
      const client: Client = { onEvent: () => off, onInvalidated: () => off, request: vi.fn(async method => {
        if (fail) throw new Error('offline')
        return method === 'delegations.get' ? detail() : page([1, 2])
      }) as Client['request'] }
      const controller = new DelegationViewerController(client, target)
      const stop = controller.retain()
      const first = controller.refresh(), second = controller.refresh()
      expect(first).toBe(second)
      await first
      fail = true; await controller.refresh()
      expect(controller.getSnapshot().messages.map(row => row.id)).toEqual(['message-1', 'message-2'])
      expect(controller.getSnapshot().error).toBe(true)
      stop(); expect(off).toHaveBeenCalledTimes(2)
      const calls = (client.request as ReturnType<typeof vi.fn>).mock.calls.length
      await vi.advanceTimersByTimeAsync(10_000)
      expect((client.request as ReturnType<typeof vi.fn>).mock.calls.length).toBe(calls)
    } finally { vi.useRealTimers() }
  })
  it('rejects exchanged list pages and repeated history cursors', async () => {
    const badClient: Client = { request: vi.fn(async () => ({ parentConversationId: 'other', items: [] })) as Client['request'] }
    await expect(delegationChildren(badClient, target.parentConversationId)).rejects.toThrow('scope_mismatch')
    let loaded = false
    const client: Client = { request: vi.fn(async method => method === 'delegations.get' ? detail() : loaded ? page([1], 'loop') : page([2], 'loop')) as Client['request'] }
    const controller = new DelegationViewerController(client, target)
    await controller.refresh(); loaded = true; await controller.loadEarlier()
    expect(controller.getSnapshot().error).toBe(true)
    expect(controller.getSnapshot().messages.map(row => row.sequence)).toEqual([2])
  })
})
