import type {
  AgentRunExecutionBlock as Block, AgentRunExecutionEvidenceView as Evidence,
  AgentRunExecutionBlockPage, AgentRunDelegationBlockPage,
  AgentRunExecutionBlockChanges, AgentRunDelegationBlockChanges
} from '@contracts'

export type ExecutionBlockPage = AgentRunExecutionBlockPage | AgentRunDelegationBlockPage
export type ExecutionBlockChanges = AgentRunExecutionBlockChanges | AgentRunDelegationBlockChanges
export type ExecutionProjection = 'blocks' | 'delegationBlocks'

export function executionBlockEvidence(block: Block): Evidence[] {
  return [...(block.supportingEvidence ?? []), ...block.evidence].sort((a, b) => a.sequence - b.sequence)
}

export function validExecutionBlocks(items: Block[], run: string, through: number, version: 3 | 4): boolean {
  return items.every(item => (item.kind === 'item' || item.kind === 'toolGroup') && typeof item.key === 'string'
    && Number.isSafeInteger(item.sequence) && item.sequence > 0 && item.sequence <= through
    && Number.isSafeInteger(item.changeSequence) && item.changeSequence >= 0
    && Number.isSafeInteger(item.lastSequence) && item.lastSequence >= item.sequence && item.lastSequence <= through
    && Number.isSafeInteger(item.toolCount) && item.toolCount >= 0 && Array.isArray(item.evidence) && item.evidence.length <= 4
    && item.counts != null && Object.values(item.counts).every(count => Number.isSafeInteger(count) && count >= 0)
    && item.evidence.every(evidence => evidence.agentRunId === run && Number.isSafeInteger(evidence.sequence)
      && evidence.sequence >= item.sequence && evidence.sequence <= item.lastSequence)
    && (version === 4 ? item.projectionVersion === 4 : item.projectionVersion == null)
    && validSupports(item, run, through, version))
}
function validSupports(block: Block, run: string, through: number, version: 3 | 4): boolean {
  const supports = block.supportingEvidence
  if (supports === undefined) return true
  if (!Array.isArray(supports) || supports.length > 1 || (version === 3 && supports.length > 0)) return false
  if (supports.length === 0) return true
  const primary = block.evidence[0], support = supports[0]
  if (block.kind !== 'item' || block.evidence.length !== 1 || !primary || primary.canonical?.sourceAuthority !== 'core'
    || primary.canonical.credibility !== 'core_verified' || primary.payload == null || typeof primary.payload !== 'object') return false
  const operation = (primary.payload as Record<string, unknown>).canonicalTool
  return (operation === 'delegation.start' || operation === 'delegation.resume')
    && primary.canonical.toolName === operation && (primary.payload as Record<string, unknown>).sourceAuthority === 'core'
    && support.agentRunId === run && support.executionEpoch === primary.executionEpoch
    && Number.isSafeInteger(support.sequence) && support.sequence > 0 && support.sequence <= through
    && support.sequence !== primary.sequence && support.id !== primary.id && support.canonical?.activityDomain === 'shell'
    && support.payload != null && typeof support.payload === 'object'
    && (support.payload as Record<string, unknown>).executionWindowBuiltinOperation === operation
}

type DeltaPosition = {
  requestedAfterChangeSequence: number; requestedAfterEntrySequence?: number | null
  nextAfterChangeSequence: number; nextAfterEntrySequence?: number | null
  throughChangeSequence: number; hasMore: boolean
}
/** The entry is part of the cursor, not an excuse to exceed a response limit. */
export function validExecutionDeltaPosition(page: DeltaPosition, after: number, entry: number | undefined, version: 3 | 4): boolean {
  const next = page.nextAfterChangeSequence, nextEntry = page.nextAfterEntrySequence ?? undefined
  if (page.requestedAfterChangeSequence !== after || !Number.isSafeInteger(next) || next < after
    || !Number.isSafeInteger(page.throughChangeSequence) || next > page.throughChangeSequence
    || (!page.hasMore && next !== page.throughChangeSequence)) return false
  if (version === 3) return !page.hasMore || next > after
  if ((page.requestedAfterEntrySequence ?? undefined) !== entry) return false
  if (!page.hasMore) return nextEntry === undefined
  return Number.isSafeInteger(nextEntry) && nextEntry! > 0
    && (next > after || (entry !== undefined && nextEntry! > entry))
}
