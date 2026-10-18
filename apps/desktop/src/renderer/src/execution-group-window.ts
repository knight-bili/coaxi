import type { AgentRunExecutionBlock, AgentRunExecutionEvidenceView as Evidence, AgentRunExecutionGroupPage, AgentRunExecutionWindowChanges, AgentRunDelegationGroupPage, AgentRunDelegationGroupChanges } from '@contracts'
import { validExecutionDeltaPosition } from './execution-block-contract'

export type GroupRequest = (params: {
  threadId: string; agentRunId: string; groupSequence: number; beforeSequence: number | null; afterSequence?: number; limit: number; projection?: 'delegationBlocks'
}) => Promise<AgentRunExecutionGroupPage | AgentRunDelegationGroupPage>
export type GroupChangesRequest = (params: {
  threadId: string; agentRunId: string; groupSequence: number; afterChangeSequence: number
  fromSequence: number; toSequence?: number; limit: number; afterEntrySequence?: number; projection?: 'delegationBlocks'
}) => Promise<AgentRunExecutionWindowChanges | AgentRunDelegationGroupChanges>
export type GroupSnapshot = {
  evidence: Evidence[]; initialized: boolean; hasEarlier: boolean; hasNewer: boolean; changeCursor: number
  projectionVersion?: 4
  loadedFrom?: number
  loadedTo?: number
}
type Direction = 'initial' | 'earlier' | 'newer'
const PAGE_SIZE = 24
const MAX_ITEMS = 512
const MAX_BYTES = 2 * 1024 * 1024

/** One contiguous child window, independent of the outer block count and output cache. */
export class ExecutionGroupWindow {
  state: GroupSnapshot
  loading = false
  error: string | null = null
  direction: Direction = 'initial'
  private generation = 0
  private active = true
  private refreshing = false
  private refreshBlock: AgentRunExecutionBlock | null = null
  private pendingRefresh: AgentRunExecutionBlock | null = null
  private viewport: [number, number] | null = null
  private failedOperation: Direction | 'refresh' | null = null
  constructor(
    readonly threadId: string, readonly runId: string, readonly groupSequence: number,
    private readonly liveTail: () => boolean, private readonly request: GroupRequest,
    private readonly changes: GroupChangesRequest, private readonly changed: () => void,
    initial?: GroupSnapshot, readonly projectionVersion: 3 | 4 = 3
  ) {
    this.state = initial && (initial.projectionVersion ?? 3) === projectionVersion ? initial
      : { evidence: [], initialized: false, hasEarlier: false, hasNewer: false, changeCursor: 0,
        ...(projectionVersion === 4 ? { projectionVersion: 4 } : {}) }
  }
  resume(): void { this.active = true }
  dispose(): void { this.active = false; this.generation++; this.loading = false; this.refreshing = false; this.refreshBlock = null; this.pendingRefresh = null }
  setViewport(first: number, last: number): void { this.viewport = [first, last] }
  private merge(items: Evidence[]): Evidence[] {
    const merged = new Map(this.state.evidence.map(item => [item.sequence, item]))
    for (const item of items) {
      const prior = merged.get(item.sequence)
      if (prior && ((prior.revision ?? 0) > (item.revision ?? 0)
        || ((prior.revision ?? 0) === (item.revision ?? 0) && (prior.changeSequence ?? 0) > (item.changeSequence ?? 0)))) continue
      merged.set(item.sequence, item)
    }
    return [...merged.values()].sort((a, b) => a.sequence - b.sequence)
  }
  private valid(items: Evidence[]): boolean {
    return items.every(item => item.agentRunId === this.runId && Number.isSafeInteger(item.sequence) && item.sequence >= this.groupSequence)
  }
  async read(direction: Direction = 'initial'): Promise<void> {
    if (!this.active || this.loading || (direction === 'initial' && this.state.initialized)) return
    if ((direction === 'earlier' && !this.state.hasEarlier) || (direction === 'newer' && !this.state.hasNewer)) return
    // Navigation changes the loaded range. Abort the old range refresh and
    // replay its uncommitted watermark after the new page has arrived.
    if (this.refreshing) {
      this.generation++; this.refreshing = false
      this.pendingRefresh ??= this.refreshBlock; this.refreshBlock = null
    }
    this.loading = true; this.direction = direction; this.error = null; this.failedOperation = null; this.changed()
    const generation = this.generation
    const forward = direction === 'newer' || (direction === 'initial' && !this.liveTail())
    const after = forward ? (direction === 'initial' ? this.groupSequence - 1 : this.state.loadedTo ?? this.state.evidence.at(-1)!.sequence) : undefined
    const before = direction === 'earlier' ? this.state.loadedFrom ?? this.state.evidence[0].sequence : null
    try {
      const page = await this.request({ threadId: this.threadId, agentRunId: this.runId, groupSequence: this.groupSequence,
        beforeSequence: before, ...(after === undefined ? {} : { afterSequence: after }), limit: PAGE_SIZE,
        ...(this.projectionVersion === 4 ? { projection: 'delegationBlocks' } : {}) })
      if (generation !== this.generation) return
      if (page.schemaVersion !== this.projectionVersion || page.threadId !== this.threadId || page.agentRunId !== this.runId
        || page.groupSequence !== this.groupSequence || page.requestedBeforeSequence !== before
        || (page.requestedAfterSequence ?? undefined) !== after || page.evidence.length > PAGE_SIZE
        || !Number.isSafeInteger(page.throughChangeSequence) || page.throughChangeSequence < 0 || !this.valid(page.evidence)
        || page.evidence.some((item, index) => (index > 0 && item.sequence <= page.evidence[index - 1].sequence)
          || (before !== null && item.sequence >= before) || (after !== undefined && item.sequence <= after))
        || (forward ? (page.hasMore ? page.nextAfterSequence !== page.evidence.at(-1)?.sequence : page.nextAfterSequence !== null)
          : (page.hasMore ? page.nextBeforeSequence !== page.evidence[0]?.sequence : page.nextBeforeSequence !== null))) {
        throw new Error('执行记录分页数据不兼容')
      }
      const bounds = this.projectionVersion === 4 ? {
        loadedFrom: direction === 'newer' ? this.state.loadedFrom ?? this.groupSequence
          : forward || !page.hasMore ? this.groupSequence : page.evidence[0]?.sequence ?? this.groupSequence,
        loadedTo: direction === 'earlier' ? this.state.loadedTo ?? this.state.evidence.at(-1)?.sequence ?? this.groupSequence
          : page.evidence.at(-1)?.sequence ?? this.state.loadedTo ?? this.groupSequence
      } : {}
      this.state = { ...this.state, ...bounds, evidence: this.merge(page.evidence), initialized: true,
        hasEarlier: forward ? this.state.hasEarlier : page.hasMore,
        hasNewer: forward ? page.hasMore : this.state.hasNewer,
        changeCursor: this.state.initialized ? Math.min(this.state.changeCursor, page.throughChangeSequence) : page.throughChangeSequence }
      this.prune(direction === 'earlier')
    } catch (error) {
      if (generation === this.generation) {
        this.failedOperation = direction
        this.error = error instanceof Error ? error.message : '读取执行记录失败'
      }
    } finally {
      if (generation === this.generation) {
        this.loading = false; this.changed()
        const pending = this.pendingRefresh; this.pendingRefresh = null
        if (pending && !this.error) void this.refresh(pending)
      }
    }
  }
  async refresh(block: AgentRunExecutionBlock): Promise<void> {
    if (!this.active || this.error || !this.state.initialized || block.changeSequence <= this.state.changeCursor) return
    if (this.loading || this.refreshing) { this.pendingRefresh = block; return }
    const generation = this.generation
    this.refreshing = true; this.refreshBlock = block
    try {
      // Keep this range fixed while a tied cursor is draining. Suppressing its
      // first/last row must not change the meaning of the next request.
      const from = this.projectionVersion === 4
        ? this.state.hasEarlier ? this.state.loadedFrom ?? this.state.evidence[0]?.sequence ?? this.groupSequence : this.groupSequence
        : this.state.evidence[0]?.sequence ?? this.groupSequence
      const to = this.state.hasNewer ? this.state.loadedTo ?? this.state.evidence.at(-1)?.sequence : undefined
      let after = this.state.changeCursor, entry: number | undefined
      let more = true
      while (more && generation === this.generation) {
        const page = await this.changes({ threadId: this.threadId, agentRunId: this.runId, groupSequence: this.groupSequence,
          fromSequence: from, ...(to === undefined ? {} : { toSequence: to }), afterChangeSequence: after, limit: 96,
          ...(entry === undefined ? {} : { afterEntrySequence: entry }),
          ...(this.projectionVersion === 4 ? { projection: 'delegationBlocks' } : {}) })
        if (generation !== this.generation) return
        const suppressed = page.schemaVersion === 4 ? page.suppressedOperations ?? [] : []
        if (page.schemaVersion !== (this.projectionVersion === 4 ? 4 : 2) || page.threadId !== this.threadId || page.agentRunId !== this.runId
          || !validExecutionDeltaPosition(page, after, entry, this.projectionVersion)
          || page.evidence.length + suppressed.length > 96 || !this.valid(page.evidence)
          || (page.schemaVersion === 4 && !this.validDelta(page, after, entry, from, to))
          || page.evidence.some(item => item.sequence < from || (to !== undefined && item.sequence > to))) {
          throw new Error('执行记录增量数据不兼容')
        }
        this.state = { ...this.state, evidence: this.merge(page.evidence).filter(item => !suppressed.some(value =>
          item.sequence === value.sequence && item.executionEpoch === value.executionEpoch)),
          // On failure/reopen, replay the bounded original range. Never persist
          // half a tied watermark without its range and entry position.
          ...(this.projectionVersion === 4 && page.hasMore ? {} : { changeCursor: page.nextAfterChangeSequence }) }
        after = page.nextAfterChangeSequence
        entry = page.schemaVersion === 4 ? page.nextAfterEntrySequence ?? undefined : undefined
        this.prune(false); this.changed(); more = page.hasMore
        if (more) await new Promise<void>(resolve => setTimeout(resolve, 0))
      }
    } catch (error) {
      if (generation === this.generation) {
        this.failedOperation = 'refresh'
        this.error = error instanceof Error ? error.message : '读取执行记录失败'; this.changed()
      }
    } finally {
      if (generation === this.generation) {
        this.refreshing = false; this.refreshBlock = null
        const pending = this.pendingRefresh; this.pendingRefresh = null
        if (pending && !this.error) void this.refresh(pending)
      }
    }
  }
  private validDelta(page: AgentRunDelegationGroupChanges, after: number, entry: number | undefined, from: number, to: number | undefined): boolean {
    const suppressed = page.suppressedOperations ?? []
    const positions = [...page.evidence, ...suppressed]
    if (!Number.isSafeInteger(page.throughSequence) || page.throughSequence < 0
      || new Set(positions.map(item => item.sequence)).size !== positions.length) return false
    if (suppressed.some(item => !Number.isSafeInteger(item.sequence) || item.sequence < from || item.sequence > page.throughSequence
      || (to !== undefined && item.sequence > to) || !Number.isSafeInteger(item.executionEpoch) || item.executionEpoch < 0
      || typeof item.operationId !== 'string' || !item.operationId
      || this.state.evidence.some(prior => prior.sequence === item.sequence && (prior.executionEpoch !== item.executionEpoch
        || (prior.canonical?.operationId ?? prior.operationId) != null
          && (prior.canonical?.operationId ?? prior.operationId) !== item.operationId)))) return false
    if (page.evidence.some(item => !Number.isSafeInteger(item.changeSequence) || item.changeSequence! > page.throughChangeSequence
      || !(item.changeSequence! > after || (entry !== undefined && item.changeSequence === after && item.sequence > entry))
      || (page.hasMore && !(item.changeSequence! < page.nextAfterChangeSequence
        || item.changeSequence === page.nextAfterChangeSequence && item.sequence <= page.nextAfterEntrySequence!)))) return false
    return !page.hasMore || positions.some(item => item.sequence === page.nextAfterEntrySequence)
  }
  async retry(block: AgentRunExecutionBlock): Promise<void> {
    const failed = this.failedOperation
    this.error = null
    this.failedOperation = null
    if (failed === 'refresh') await this.refresh(block)
    else { await this.read(failed ?? this.direction); await this.refresh(block) }
  }
  private prune(fromTail: boolean): void {
    const items = this.state.evidence
    let bytes = items.reduce((sum, item) => sum + JSON.stringify(item).length * 2, 0)
    let start = 0, end = items.length
    while (end - start > 1 && (end - start > MAX_ITEMS || bytes > MAX_BYTES)) {
      const item = items[fromTail ? end - 1 : start]
      if (this.viewport && item.sequence >= this.viewport[0] && item.sequence <= this.viewport[1]) break
      bytes -= JSON.stringify(item).length * 2
      if (fromTail) end--; else start++
    }
    if (start || end < items.length) this.state = { ...this.state, evidence: items.slice(start, end),
      ...(this.projectionVersion === 4 ? { loadedFrom: items[start].sequence, loadedTo: items[end - 1].sequence } : {}),
      hasEarlier: this.state.hasEarlier || start > 0, hasNewer: this.state.hasNewer || end < items.length }
  }
}
