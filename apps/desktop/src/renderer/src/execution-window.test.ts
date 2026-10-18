import { describe, expect, it, vi } from 'vitest'
import type { AgentRunExecutionEvidenceView as Evidence, AgentRunExecutionBlock, AgentRunExecutionBlockPage } from '@contracts'
import { ExecutionWindow, ExecutionWindowCache, executionWindowCacheFor, executionWindowPageSize, type ExecutionChangesRequest, type ExecutionWindowRequest } from './execution-window'
import type { AgentRunDelegationBlock, AgentRunDelegationBlockPage, AgentRunDelegationBlockChanges, AgentRunExecutionBlockChanges } from '@contracts'

function evidence(sequence: number): Evidence {
  return { id: `e-${sequence}`, agentRunId: 'run', executionEpoch: 1, sequence,
    operationId: null, revision: 1, changeSequence: sequence,
    eventType: 'agent.text.block', kind: 'narration', phase: 'completed',
    payload: { text: `text ${sequence}` }, contentBlobId: null, contentByteCount: 10,
    isTruncated: false, occurredAt: '2026-09-12T00:00:00Z' }
}
function block(item: Evidence): AgentRunExecutionBlock {
  return { key: `item:${item.sequence}`, kind: 'item', sequence: item.sequence, lastSequence: item.sequence,
    changeSequence: item.changeSequence ?? item.sequence, toolCount: 0,
    counts: { completed: 0, failed: 0, stopped: 0, recorded: 0, running: 0, waiting: 0 }, evidence: [item] }
}
function source(initial = 100) {
  const state = { through: initial, offline: false, refreshed: [] as Evidence[] }
  const request = vi.fn(async (params: Parameters<ExecutionWindowRequest>[0]): Promise<AgentRunExecutionBlockPage> => {
    if (state.offline) throw new Error('offline')
    const start = params.afterSequence === undefined
      ? Math.max(1, Math.min(state.through, (params.beforeSequence ?? state.through + 1) - 1) - params.limit + 1)
      : params.afterSequence + 1
    const end = params.afterSequence === undefined ? Math.min(state.through, (params.beforeSequence ?? state.through + 1) - 1)
      : Math.min(state.through, start + params.limit - 1)
    const hasMore = params.afterSequence === undefined ? start > 1 : end < state.through
    return { schemaVersion: 3, threadId: 'camp', agentRunId: 'run', requestedBeforeSequence: params.beforeSequence,
      requestedAfterSequence: params.afterSequence, nextAfterSequence: params.afterSequence !== undefined && hasMore ? end : null,
      nextBeforeSequence: params.afterSequence === undefined && hasMore ? start : null,
      throughSequence: state.through, throughChangeSequence: state.through, hasMore,
      blocks: Array.from({ length: Math.max(0, end - start + 1) }, (_, offset) => block(evidence(start + offset))) }
  })
  const changes = vi.fn(async (params: Parameters<ExecutionChangesRequest>[0]): Promise<AgentRunExecutionBlockChanges> => {
    if (state.offline) throw new Error('offline')
    const end = Math.min(state.through, params.afterChangeSequence + params.limit)
    return { schemaVersion: 3, threadId: 'camp', agentRunId: 'run', requestedAfterChangeSequence: params.afterChangeSequence,
      nextAfterChangeSequence: end, throughSequence: state.through, throughChangeSequence: state.through,
      hasMore: end < state.through,
      blocks: [...Array.from({ length: end - params.afterChangeSequence }, (_, offset) => block(evidence(params.afterChangeSequence + offset + 1))),
        ...state.refreshed.map(item => block({ ...item, changeSequence: state.through }))] }
  })
  return { state, request, changes }
}

function delegationSource() {
  const canonical = (sequence: number, core: boolean): NonNullable<Evidence['canonical']> => ({
    operationId: core ? 'core-start' : 'native-start', classifierVersion: 'activity-v3', activityDomain: core ? 'tool' : 'shell',
    semanticKind: null, toolName: core ? 'delegation.start' : 'shell', presentationHint: null, phase: 'terminal', outcome: 'succeeded',
    credibility: core ? 'core_verified' : 'runtime_structured', coverageLevel: 'fine_grained', sourceAuthority: core ? 'core' : 'runtime',
    sourceEvidenceIds: [`e-${sequence}`], firstEvidenceSequence: sequence, lastEvidenceSequence: sequence, revision: 1
  })
  const primary: Evidence = { ...evidence(3), eventType: 'runtime.action', kind: 'tool_call',
    payload: { sourceAuthority: 'core', canonicalTool: 'delegation.start' }, canonical: canonical(3, true) }
  const support: Evidence = { ...evidence(1), eventType: 'activity.completed', kind: 'command',
    payload: { executionWindowBuiltinOperation: 'delegation.start', item: { type: 'commandExecution', command: 'rovai delegation start --task fixture' } },
    canonical: canonical(1, false) }
  const item: AgentRunDelegationBlock = { ...block(primary), projectionVersion: 4, changeSequence: 5, supportingEvidence: [support] }
  const page: AgentRunDelegationBlockPage = { schemaVersion: 4, delegationBlocksVersion: 4, threadId: 'camp', agentRunId: 'run',
    requestedBeforeSequence: null, throughSequence: 5, throughChangeSequence: 5, hasMore: false, nextBeforeSequence: null, blocks: [item] }
  const legacy = source(5)
  const request = vi.fn<ExecutionWindowRequest>(async params => params.projection === 'delegationBlocks' ? page
    : { ...await legacy.request(params), delegationBlocksVersion: 4 })
  const changes = vi.fn<ExecutionChangesRequest>(async params => ({ schemaVersion: 4, threadId: 'camp', agentRunId: 'run',
    requestedAfterChangeSequence: params.afterChangeSequence, requestedAfterEntrySequence: params.afterEntrySequence,
    throughSequence: 5, throughChangeSequence: 5, nextAfterChangeSequence: 5, hasMore: false, blocks: [] }))
  const window = new ExecutionWindow('camp', 'run', 12, request, () => {}, changes)
  return { request, changes, window, item, primary, support, page }
}

describe('negotiated delegation blocks', () => {
  it('probes old blocks once, uses only v4 boundaries, and keeps external support out of the primary cursor', async () => {
    const { request, window, support } = delegationSource()
    await window.latest()
    expect(request.mock.calls.map(([params]) => params.projection)).toEqual(['blocks', 'delegationBlocks'])
    expect(window.blocks.map(item => item.sequence)).toEqual([3])
    expect(window.evidence.map(item => item.sequence)).toEqual([1, 3])
    expect(window.blocks[0].evidence.map(item => item.sequence)).toEqual([3])
    expect(window.blocks[0].supportingEvidence).toEqual([support])
    await window.latest()
    expect(request).toHaveBeenCalledTimes(2)
  })
  it('retries the selected projection after failure without merging the old probe or silently downgrading', async () => {
    const { request, window } = delegationSource()
    const original = request.getMockImplementation()!
    let failed = false
    request.mockImplementation(async params => {
      if (params.projection === 'delegationBlocks' && !failed) { failed = true; throw new Error('offline') }
      return original(params)
    })
    await window.latest()
    expect(window.loaded).toBe(false); expect(window.blocks).toEqual([])
    await window.retry()
    expect(window.loaded).toBe(true)
    expect(request.mock.calls.map(([params]) => params.projection)).toEqual(['blocks', 'delegationBlocks', 'delegationBlocks'])
  })
  it('does not start a capability follow-up after the original window was disposed', async () => {
    const { request, window } = delegationSource()
    let resolve!: (page: AgentRunExecutionBlockPage) => void
    request.mockImplementationOnce(() => new Promise(done => { resolve = done }))
    const pending = window.latest(); window.dispose()
    resolve({ schemaVersion: 3, threadId: 'camp', agentRunId: 'run', requestedBeforeSequence: null,
      nextBeforeSequence: null, throughSequence: 5, throughChangeSequence: 5, hasMore: false, blocks: [], delegationBlocksVersion: 4 })
    await pending
    expect(request).toHaveBeenCalledTimes(1); expect(window.blocks).toEqual([])
    expect(window.projectionVersion).toBe(3)
  })
  it.each(['run', 'epoch', 'claim', 'primary'] as const)('rejects invalid external support: %s', async field => {
    const { window, page, support, primary } = delegationSource()
    page.blocks[0] = { ...page.blocks[0], supportingEvidence: [{ ...support,
      ...(field === 'run' ? { agentRunId: 'other-run' } : {}),
      ...(field === 'epoch' ? { executionEpoch: 2 } : {}),
      ...(field === 'claim' ? { payload: {} } : {}) }],
      ...(field === 'primary' ? { evidence: [{ ...primary, canonical: { ...primary.canonical!, sourceAuthority: 'runtime' } }] } : {}) }
    await window.latest()
    expect(window.loaded).toBe(false); expect(window.blocks).toEqual([])
    expect(window.error).toBe('执行记录分页数据不兼容')
  })
  it('drains tied block changes and removes a revoked support without losing the original carrier', async () => {
    const { window, changes, item, support } = delegationSource()
    await window.latest()
    const native: AgentRunDelegationBlock = { ...block(support), key: 'tools:1', kind: 'toolGroup', projectionVersion: 4,
      changeSequence: 6, toolCount: 1 }
    const make = (params: Parameters<ExecutionChangesRequest>[0], blocks: AgentRunDelegationBlock[], more: boolean): AgentRunDelegationBlockChanges => ({
      schemaVersion: 4, threadId: 'camp', agentRunId: 'run', requestedAfterChangeSequence: params.afterChangeSequence,
      requestedAfterEntrySequence: params.afterEntrySequence, nextAfterChangeSequence: 6,
      ...(more ? { nextAfterEntrySequence: 1 } : {}), throughSequence: 5, throughChangeSequence: 6, hasMore: more, blocks })
    changes.mockImplementation(async params => params.afterEntrySequence === undefined
      ? make(params, [native], true) : make(params, [{ ...item, changeSequence: 6, supportingEvidence: [] }], false))
    await window.refresh()
    expect(changes.mock.calls.map(([params]) => [params.afterChangeSequence, params.afterEntrySequence])).toEqual([[5, undefined], [6, 1]])
    expect(window.blocks.map(item => item.sequence)).toEqual([1, 3])
    expect(window.blocks[1].supportingEvidence).toEqual([])
    expect(window.evidence.filter(item => item.sequence === 1)).toHaveLength(1)
  })
  it('retains the paired root cursor after a later page fails, then resumes the same change watermark', async () => {
    const { window, changes, item } = delegationSource()
    await window.latest()
    const first = { ...item, sequence: 2, lastSequence: 2, key: 'item:2', changeSequence: 6,
      evidence: [{ ...item.evidence[0], sequence: 2 }], supportingEvidence: [] } satisfies AgentRunDelegationBlock
    let failed = false
    changes.mockImplementation(async params => {
      if (params.afterEntrySequence === 2 && !failed) { failed = true; throw new Error('offline') }
      return { schemaVersion: 4, threadId: 'camp', agentRunId: 'run', requestedAfterChangeSequence: params.afterChangeSequence,
        requestedAfterEntrySequence: params.afterEntrySequence, nextAfterChangeSequence: 6, throughSequence: 5, throughChangeSequence: 6,
        hasMore: params.afterEntrySequence === undefined, ...(params.afterEntrySequence === undefined ? { nextAfterEntrySequence: 2 } : {}),
        blocks: params.afterEntrySequence === undefined ? [first] : [{ ...item, changeSequence: 6 }] }
    })
    await window.refresh(); await window.refresh()
    expect(changes.mock.calls.map(([params]) => [params.afterChangeSequence, params.afterEntrySequence])).toEqual([[5, undefined], [6, 2], [6, 2]])
    expect(window.blocks.find(block => block.sequence === 3)?.changeSequence).toBe(6)
  })
})

describe('continuous execution history', () => {
  it('recovers a failed initial tail on invalidation without polling or warming an unopened window', async () => {
    const { state, request, changes } = source(1)
    const current = new ExecutionWindow('camp', 'run', 1, request, () => {}, changes)
    await current.refresh(() => true, true)
    expect(request).not.toHaveBeenCalled()
    state.offline = true
    await current.latest()
    expect(current.loaded).toBe(false)
    await current.refresh() // A historical reader still waits for explicit retry.
    expect(request).toHaveBeenCalledTimes(1)
    await current.refresh(() => true, true)
    expect(request).toHaveBeenCalledTimes(2)
    expect(current.loading).toBe(false)
    expect(current.error).toBe('offline')
    state.offline = false
    await current.refresh(() => true, true)
    expect(request).toHaveBeenCalledTimes(3)
    expect(changes).not.toHaveBeenCalled()
    expect(current.loaded).toBe(true)
    expect(current.error).toBeNull()
    expect(current.evidence.map(item => item.sequence)).toEqual([1])
  })

  it('coalesces invalidations during a slow first page and catches up immediately after it returns', async () => {
    const { state, request, changes } = source(1)
    let resolve!: (page: AgentRunExecutionBlockPage) => void
    const page = await request({ threadId: 'camp', agentRunId: 'run', beforeSequence: null, limit: 1 })
    const current = new ExecutionWindow('camp', 'run', 1, () => new Promise(done => { resolve = done }), () => {}, changes)
    const initial = current.latest()
    state.through = 2
    await Promise.all(Array.from({ length: 3 }, () => current.refresh(() => true, true)))
    expect(changes).not.toHaveBeenCalled()
    resolve(page)
    await initial
    await vi.waitFor(() => expect(current.evidence.at(-1)?.sequence).toBe(2))
    expect(changes).toHaveBeenCalledTimes(1)
    expect(changes.mock.calls[0][0].afterChangeSequence).toBe(1)
  })

  it('uses an in-flight invalidation once if the first page fails, then waits for another signal', async () => {
    const { request, changes } = source(1)
    let reject!: (reason: Error) => void
    request.mockImplementationOnce(() => new Promise((_done, fail) => { reject = fail }))
    request.mockRejectedValueOnce(new Error('still offline'))
    const current = new ExecutionWindow('camp', 'run', 1, request, () => {}, changes)
    const initial = current.latest()
    await current.refresh(() => true, true)
    await current.refresh(() => true, true)
    reject(new Error('offline'))
    await initial
    await vi.waitFor(() => expect(current.error).toBe('still offline'))
    expect(request).toHaveBeenCalledTimes(2)
    expect(current.loading).toBe(false)
    await current.refresh(() => true, true)
    expect(current.loaded).toBe(true)
    expect(request).toHaveBeenCalledTimes(3)
  })

  it('appends the thirteenth live record without hiding the first twelve or shifting the history cursor', async () => {
    const { state, request, changes } = source(12)
    const current = new ExecutionWindow('camp', 'run', 12, request, () => {}, changes)
    await current.latest()
    state.through = 13
    await current.refresh()
    expect(current.evidence.map(item => item.sequence)).toEqual(Array.from({ length: 13 }, (_, i) => i + 1))
    expect(current.hasEarlier).toBe(false)
    state.through = 240
    await current.refresh()
    expect(current.evidence.map(item => item.sequence)).toEqual(Array.from({ length: 240 }, (_, i) => i + 1))
    expect(request).toHaveBeenCalledTimes(1)
    expect(changes.mock.calls.map(([params]) => params.afterChangeSequence)).toEqual([12, 13, 109, 205])
    state.through = 10_000
    await current.refresh()
    expect(current.evidence.at(-1)?.sequence).toBe(10_000)
    expect(current.evidence.length).toBeLessThanOrEqual(2048)
    expect(changes).toHaveBeenCalledTimes(5)
    expect(request.mock.calls.filter(([params]) => params.beforeSequence === null)).toHaveLength(2)
  })

  it('updates an unfinished row in place while preserving older history and a stable prefetch boundary', async () => {
    const { state, request, changes } = source()
    const current = new ExecutionWindow('camp', 'run', 12, request, () => {}, changes)
    await current.latest()
    await current.earlier()
    const first = current.evidence[0].sequence
    current.evidence[0].phase = 'updated'
    current.evidence[0].revision = null
    state.refreshed = [{ ...current.evidence[0], phase: 'completed', payload: { text: 'complete body' } }]
    state.through = 102
    await current.refresh(() => false)
    expect(current.evidence[0].sequence).toBe(first)
    expect(current.evidence[0].payload).toEqual({ text: 'complete body' })
    expect(current.evidence.at(-1)?.sequence).toBe(102)
    expect(changes.mock.calls[0][0].refreshEvidenceIds).toEqual([`e-${first}`])
    expect(request.mock.calls.map(([params]) => params.beforeSequence)).toEqual([null, 89, 77])
  })

  it('refreshes a streaming block without a durable change or a new display position', async () => {
    const { request, changes } = source(1)
    const streaming: Evidence = { ...evidence(1), phase: 'updated', payload: { status: 'streaming', text: 'hello' } }
    const current = new ExecutionWindow('camp', 'run', 12, async params => ({ ...await request(params), blocks: [block(streaming)] }), () => {},
      async params => {
        expect(params.refreshEvidenceIds).toEqual(['e-1'])
        return { ...await changes(params), refreshedBlocks: [block({ ...streaming, payload: { status: 'streaming', text: 'hello world' } })] }
      })
    await current.latest()
    await current.refresh()
    expect(current.blocks).toHaveLength(1)
    expect(current.evidence[0].payload).toEqual({ status: 'streaming', text: 'hello world' })
    expect(current.blocks[0].changeSequence).toBe(1)
    expect(request).toHaveBeenCalledTimes(1)
  })

  it('refreshes and clears a transient thinking title without changing evidence or its cursor', async () => {
    const { request, changes } = source(1)
    let runtimePhase: 'thinking' | 'executing' = 'thinking'
    const current = new ExecutionWindow('camp', 'run', 12, request, () => {}, async params => ({
      ...await changes(params), runtimePhase, runtimeThinkingTitle: '检查调用链'
    }))
    await current.latest()
    const evidenceBefore = current.evidence
    await current.refresh()
    expect(current.runtimePhase).toBe('thinking')
    expect(current.runtimeThinkingTitle).toBe('检查调用链')
    expect(current.evidence).toBe(evidenceBefore)
    runtimePhase = 'executing'
    await current.refresh()
    expect(current.runtimeThinkingTitle).toBeNull()
    expect(current.evidence).toBe(evidenceBefore)
    expect(changes.mock.calls.every(([params]) => params.afterChangeSequence === 1)).toBe(true)
  })

  it('reads larger historical batches, prefetches one neighbor and revisits loaded data offline', async () => {
    const { state, request, changes } = source(1000)
    const current = new ExecutionWindow('camp', 'run', 12, request, () => {}, changes)
    await current.latest()
    await vi.waitFor(() => expect(request).toHaveBeenCalledTimes(2))
    expect(current.evidence).toHaveLength(12)
    expect(request.mock.calls.map(([params]) => params.limit)).toEqual([12, 12])
    await current.earlier(); await current.earlier(); await current.earlier()
    expect(current.evidence).toHaveLength(48)
    const count = request.mock.calls.length
    state.offline = true
    await current.latest(); await current.newer()
    expect(current.error).toBeNull()
    expect(current.evidence).toHaveLength(48)
    expect(current.evidence.at(-1)?.sequence).toBe(1000)
    expect(request).toHaveBeenCalledTimes(count)
    expect(executionWindowPageSize(300)).toBeLessThan(executionWindowPageSize(900))
    expect(executionWindowPageSize(4000)).toBe(24)
  })

  it('keeps successful data on failure, retries the same direction and validates Thread/cursor boundaries', async () => {
    const { state, request, changes } = source(1000)
    state.offline = true
    const current = new ExecutionWindow('camp', 'run', 12, request, () => {}, changes)
    await current.latest()
    expect(current.error).toBe('offline')
    state.offline = false; await current.retry()
    await vi.waitFor(() => expect(request).toHaveBeenCalledTimes(3))
    await current.earlier()
    state.offline = true
    await current.earlier(); await current.earlier()
    const retained = current.evidence
    expect(current.error).toBe('offline')
    expect(current.direction).toBe('earlier')
    state.offline = false; await current.retry()
    expect(current.error).toBeNull()
    expect(current.evidence.length).toBeGreaterThan(retained.length)
    const invalid = new ExecutionWindow('camp', 'run', 12, async params => ({ ...await request(params), threadId: 'other' }), () => {}, changes)
    await invalid.latest()
    expect(invalid.error).toContain('不兼容')
    expect(invalid.evidence).toEqual([])
  })

  it('bounds retained records and bytes while preserving the reading viewport and reloads evicted forward ranges', async () => {
    const { state, request, changes } = source(1000)
    const current = new ExecutionWindow('camp', 'run', 12, request, () => {}, changes, { maxItems: 24, maxBytes: 100_000 })
    await current.latest(); await current.earlier()
    current.setViewport(978, 980)
    await current.earlier()
    expect(current.evidence.length).toBeLessThanOrEqual(24)
    expect(current.evidence.some(item => item.sequence === 978)).toBe(true)
    expect(current.hasNewer).toBe(true)
    const requests = request.mock.calls.length
    state.through = 1100
    await current.refresh(() => false)
    expect(request).toHaveBeenCalledTimes(requests)
    current.setViewport(984, 986)
    await current.newer()
    expect(request.mock.calls.some(([params]) => params.afterSequence !== undefined)).toBe(true)
    expect(current.evidence.length).toBeLessThanOrEqual(24)
    await current.latest()
    expect(current.evidence.at(-1)?.sequence).toBe(1100)
    expect(current.byteSize).toBeLessThan(100_000)
    const bytes = new ExecutionWindow('camp', 'run', 12, request, () => {}, changes, { maxItems: 2048, maxBytes: 1500 })
    await bytes.latest()
    expect(bytes.evidence.length).toBeLessThan(12)
  })

  it('fences late responses after unmount and reuses a released Run cache across Thread switches', async () => {
    const { request, changes } = source()
    let resolve!: (value: AgentRunExecutionBlockPage) => void
    const changed = vi.fn()
    const late = new ExecutionWindow('camp', 'run', 12, () => new Promise(done => { resolve = done }), changed, changes)
    const loading = late.latest()
    await late.refresh(() => true, true)
    late.dispose(); changed.mockClear()
    resolve(await request({ threadId: 'camp', agentRunId: 'run', beforeSequence: null, limit: 12 }))
    await loading
    expect(late.evidence).toEqual([]); expect(changed).not.toHaveBeenCalled()
    expect(changes).not.toHaveBeenCalled()
    const cache = new ExecutionWindowCache(2)
    const create = vi.fn((notify: () => void) => new ExecutionWindow('camp', 'run', 12, request, notify, changes))
    const a = cache.acquire('a', create, changed); await a.window.latest(); a.release()
    const b = cache.acquire('b', create, changed); await b.window.latest(); b.release()
    const calls = request.mock.calls.length
    const returned = cache.acquire('a', create, changed); await returned.window.latest()
    expect(returned.window).toBe(a.window); expect(request).toHaveBeenCalledTimes(calls)
    returned.release()
    const c = cache.acquire('c', create, changed); c.release()
    const old = cache.acquire('b', create, changed)
    expect(old.window).not.toBe(b.window); old.release()
    // Identical Run IDs on a replacement Host/client must load through the new
    // transport, while revisiting a Thread on the same client still hits cache.
    const firstClient = {}, nextClient = {}
    const firstSource = source(3), nextSource = source(7)
    const first = executionWindowCacheFor(firstClient).acquire('same-run', notify => new ExecutionWindow('camp', 'run', 12, firstSource.request, notify, firstSource.changes), changed)
    await first.window.latest(); first.release()
    const same = executionWindowCacheFor(firstClient).acquire('same-run', create, changed)
    expect(same.window).toBe(first.window); same.release()
    const next = executionWindowCacheFor(nextClient).acquire('same-run', notify => new ExecutionWindow('camp', 'run', 12, nextSource.request, notify, nextSource.changes), changed)
    expect(next.window.evidence).toEqual([])
    await next.window.latest()
    expect(next.window.evidence.at(-1)?.sequence).toBe(7)
    expect(firstSource.request).toHaveBeenCalledTimes(1)
    expect(nextSource.request).toHaveBeenCalledTimes(1)
    next.release()
  })
})
