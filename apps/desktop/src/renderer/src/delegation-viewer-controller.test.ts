import { describe, expect, it, vi } from 'vitest'
import type { DelegationConversationView, DelegationHistoryPage } from '@contracts'
import { DelegationViewerController, delegationChildren, delegationViewerController } from './delegation-viewer-controller'
import type { ThreadClient } from './camp-client'
import { delegationChildrenController } from './delegation-children-controller'
import { DelegationToolSourceController, assertDelegationToolSource } from './delegation-tool-source-controller'
import type { DelegationToolSourceView } from '@contracts'

const target = { threadId: 'thread-a', parentConversationId: 'parent-a', delegationId: 'delegation-a' }
const detail = (): DelegationConversationView => ({ ...target, ownerAgentId: 'owner-a', rootConversationId: target.parentConversationId, parentDelegationId: null,
  conversationId: 'child-a', depth: 1, currentAttemptNo: 1, controlVersion: 1, takenOver: false, agentRunId: 'run-a', executionEpoch: 1,
  status: 'succeeded', agentType: 'codex', createdAt: '', startedAt: null, endedAt: null, cleanupPending: false, activity: { runs: [], executionEvidence: [], approvals: [] } })
const message = (n: number): DelegationHistoryPage['messages'][number] => ({ id: `message-${n}`, sequence: n, author_type: n % 2 ? 'delegation_parent' : 'assistant', body: `完整消息 ${n}`, created_at: '' })
const page = (values: number[], cursor?: string): DelegationHistoryPage => ({ task_id: target.delegationId, conversation_id: 'child-a', messages: values.map(message), ...(cursor ? { next_cursor: cursor } : {}) })
type Client = Pick<ThreadClient, 'request' | 'onEvent' | 'onInvalidated'>
const sourceTarget = { threadId: target.threadId, evidenceId: 'source-evidence', agentRunId: 'parent-run', executionEpoch: 3 }
const sourceView = (): Extract<DelegationToolSourceView, { state: 'bound' }> => ({ state: 'bound',
  source: { ...sourceTarget, parentConversationId: target.parentConversationId }, operation: 'delegation.start', attemptNo: 1,
  task: '完整原任务中文😀\n'.repeat(8000), reason: null, detail: detail() })

describe('parent tool sources and shared current metadata', () => {
  it('rejects exchanged source identities, parent details, operation and attempt without publishing bodies', async () => {
    const good = sourceView()
    for (const source of [ { ...good.source, threadId: 'other' }, { ...good.source, evidenceId: 'other' },
      { ...good.source, agentRunId: 'other' }, { ...good.source, executionEpoch: 4 }, { ...good.source, parentConversationId: 'other' } ]) {
      expect(() => assertDelegationToolSource({ ...good, source }, sourceTarget)).toThrow()
    }
    for (const changed of [{ operation: 'native-tool' }, { attemptNo: 2 }, { task: undefined }, { reason: {} },
      { operation: 'delegation.resume', attemptNo: 1 }, { detail: { ...good.detail, threadId: 'other' } }]) {
      const client: Client = { request: vi.fn(async () => ({ ...good, ...changed })) as Client['request'] }
      const controller = new DelegationToolSourceController(client, sourceTarget)
      await controller.refresh()
      expect(controller.getSnapshot()).toMatchObject({ view: null, error: true, loading: false })
    }
  })
  it('keeps full immutable task/reason once and never uses native ids or polling to reread a bound body', async () => {
    const view = { ...sourceView(), operation: 'delegation.resume', attemptNo: 2, detail: { ...detail(), currentAttemptNo: 3 }, reason: '  保留换行\n😀  ' }
    const request = vi.fn(async () => view)
    const client = { request } as unknown as Client
    const controller = new DelegationToolSourceController(client, sourceTarget)
    const stop = controller.retain()
    await controller.refresh()
    expect(controller.getSnapshot().view).toEqual(view)
    controller.evidenceChanged(); controller.evidenceChanged()
    expect(request).toHaveBeenCalledTimes(1)
    expect(request).toHaveBeenCalledWith('delegations.source', { threadId: target.threadId, evidenceId: sourceTarget.evidenceId })
    stop(); expect(controller.getSnapshot().view).toBeNull()
  })
  it('clears private data on authentication invalidation and ignores the old late reply', async () => {
    let invalidate: (() => void) | undefined
    let finishOld: ((value: DelegationToolSourceView) => void) | undefined
    let finishNew: ((value: DelegationToolSourceView) => void) | undefined
    const request = vi.fn().mockImplementationOnce(() => new Promise(resolve => { finishOld = resolve }))
      .mockImplementationOnce(() => new Promise(resolve => { finishNew = resolve }))
    const off = vi.fn()
    const controller = new DelegationToolSourceController({ request, onInvalidated: listener => { invalidate = listener; return off } } as Client, sourceTarget)
    const stop = controller.retain()
    invalidate!()
    expect(controller.getSnapshot().view).toBeNull()
    finishOld!(sourceView()); await Promise.resolve(); await Promise.resolve()
    expect(controller.getSnapshot().view).toBeNull()
    finishNew!({ state: 'unresolved', source: sourceView().source })
    await controller.refresh()
    expect(controller.getSnapshot().view?.state).toBe('unresolved')
    stop(); expect(off).toHaveBeenCalledOnce()
  })
  it('retains source changes arriving during the first unresolved read and clears a settled body on invalidation', async () => {
    let finish: ((value: DelegationToolSourceView) => void) | undefined
    let invalidate: (() => void) | undefined
    const request = vi.fn().mockImplementationOnce(() => new Promise(resolve => { finish = resolve })).mockResolvedValue(sourceView())
    const controller = new DelegationToolSourceController({ request, onInvalidated: listener => { invalidate = listener; return () => {} } } as Client, sourceTarget)
    const stop = controller.retain()
    controller.evidenceChanged()
    finish!({ state: 'unresolved', source: sourceView().source })
    await vi.waitFor(() => expect(controller.getSnapshot().view?.state).toBe('bound'))
    expect(request).toHaveBeenCalledTimes(2)
    invalidate!(); expect(controller.getSnapshot().view).toBeNull()
    await controller.refresh(); expect(controller.getSnapshot().view?.state).toBe('bound')
    stop()
  })
  it('keeps a refused source without enriching it from another existing child', async () => {
    const refused = { state: 'refused', source: sourceView().source, operation: 'delegation.resume', code: 'delegation.not_resumable' }
    const request = vi.fn(async () => refused)
    const controller = new DelegationToolSourceController({ request } as unknown as Client, sourceTarget)
    const stop = controller.retain(); await controller.refresh()
    expect(controller.getSnapshot().view).toEqual(refused)
    controller.evidenceChanged(); expect(request).toHaveBeenCalledTimes(1)
    stop()
  })
  it('shares one exact parent metadata list and timer, with complete pages and zero body reads', async () => {
    vi.useFakeTimers()
    try {
      const request = vi.fn(async (_method, params: { cursor?: string }) => params.cursor
        ? { parentConversationId: target.parentConversationId, items: [{ ...detail(), delegationId: 'second' }] }
        : { parentConversationId: target.parentConversationId, items: [detail()], nextCursor: 'signed-next' })
      const off = vi.fn(), client: Client = { request: request as Client['request'], onEvent: () => off, onInvalidated: () => off }
      const one = delegationChildrenController(client, target), two = delegationChildrenController(client, target)
      expect(one).toBe(two)
      expect(one).not.toBe(delegationChildrenController({ ...client }, target))
      expect(one).not.toBe(delegationChildrenController(client, { ...target, threadId: 'other' }))
      const releaseOne = one.retain(), releaseTwo = two.retain()
      await vi.waitFor(() => expect(one.getSnapshot().rows).toHaveLength(2))
      const initial = request.mock.calls.length
      expect(initial).toBe(2)
      expect(one.getSnapshot().rows.map(row => row.delegationId)).toEqual([target.delegationId, 'second'])
      await vi.advanceTimersByTimeAsync(5_000)
      expect(request.mock.calls.length).toBe(initial + 2)
      expect(request.mock.calls.every(([method]) => method === 'delegations.list')).toBe(true)
      releaseOne(); expect(off).not.toHaveBeenCalled()
      releaseTwo(); expect(off).toHaveBeenCalledTimes(2)
      expect(one.getSnapshot().rows).toEqual([])
      const closed = request.mock.calls.length
      await vi.advanceTimersByTimeAsync(10_000); expect(request.mock.calls.length).toBe(closed)
    } finally { vi.useRealTimers() }
  })
  it('rejects a wrong Thread or repeated cursor and clears metadata before an authenticated reread', async () => {
    for (const wrong of ['thread', 'cursor']) {
      const client = { request: vi.fn(async () => ({ parentConversationId: target.parentConversationId,
        items: [{ ...detail(), ...(wrong === 'thread' ? { threadId: 'other' } : {}) }], ...(wrong === 'cursor' ? { nextCursor: 'loop' } : {}) })) } as Client
      const controller = delegationChildrenController(client, target)
      await controller.refresh(); expect(controller.getSnapshot()).toMatchObject({ error: true, rows: [] })
    }
    let invalidate: (() => void) | undefined
    let finish: ((value: unknown) => void) | undefined
    const request = vi.fn().mockResolvedValueOnce({ parentConversationId: target.parentConversationId, items: [detail()] })
      .mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    const controller = delegationChildrenController({ request, onInvalidated: listener => { invalidate = listener; return () => {} } } as Client, target)
    const stop = controller.retain(); await Promise.resolve(); await Promise.resolve()
    expect(controller.getSnapshot().rows).toHaveLength(1)
    invalidate!(); expect(controller.getSnapshot().rows).toEqual([])
    stop(); finish!({ parentConversationId: target.parentConversationId, items: [detail()] })
    await Promise.resolve(); await Promise.resolve(); expect(controller.getSnapshot().rows).toEqual([])
  })
})
describe('delegated conversation viewer authority and paging', () => {
  it('clears the open transcript on authenticated invalidation and fences old history replies', async () => {
    let invalidate: (() => void) | undefined
    let finishOld: ((value: DelegationHistoryPage) => void) | undefined
    let finishNew: ((value: DelegationHistoryPage) => void) | undefined
    let history = 0
    const client: Client = { onInvalidated: listener => { invalidate = listener; return () => {} }, request: vi.fn(async method => {
      if (method === 'delegations.get') return detail()
      if (++history === 1) return page([1])
      return new Promise(resolve => { if (history === 2) finishOld = resolve; else finishNew = resolve })
    }) as Client['request'] }
    const controller = new DelegationViewerController(client, target), stop = controller.retain()
    await vi.waitFor(() => expect(controller.getSnapshot().messages).toHaveLength(1))
    const old = controller.refresh()
    await vi.waitFor(() => expect(finishOld).toBeTypeOf('function'))
    invalidate!()
    expect(controller.getSnapshot()).toMatchObject({ detail: null, messages: [], loading: true })
    await vi.waitFor(() => expect(finishNew).toBeTypeOf('function'))
    finishNew!(page([3])); await vi.waitFor(() => expect(controller.getSnapshot().messages[0]?.id).toBe('message-3'))
    finishOld!(page([2])); await old
    expect(controller.getSnapshot().messages.map(row => row.id)).toEqual(['message-3'])
    stop()
  })
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
