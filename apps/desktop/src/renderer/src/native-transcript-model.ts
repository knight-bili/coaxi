import type { NativeTranscriptBlock, NativeTranscriptTurn } from './native-child-reader'
import { isCodexCollabInput, isCodexThreadId, parseCollabToolInput } from './coaxi-collab-tool'

type Use = Extract<NativeTranscriptBlock, { type: 'tool_use' }>
type Result = Extract<NativeTranscriptBlock, { type: 'tool_result' }>
/** Pair in arrival order, not one global result per textual ID. Reused IDs and
 * orphan results retain every complete output instead of losing earlier work. */
export function nativeToolPairs(turns: NativeTranscriptTurn[]): { results: Map<Use, Result>; paired: Set<Result> } {
  const waiting = new Map<string, Use[]>(), results = new Map<Use, Result>(), paired = new Set<Result>()
  for (const turn of turns) for (const block of turn.blocks) {
    if (block.type === 'tool_use' && block.tool_use_id) {
      const queue = waiting.get(block.tool_use_id) ?? []; queue.push(block); waiting.set(block.tool_use_id, queue)
    } else if (block.type === 'tool_result' && block.tool_use_id) {
      const use = waiting.get(block.tool_use_id)?.shift()
      if (use) { results.set(use, block); paired.add(block) }
    }
  }
  return { results, paired }
}
const object = (value: unknown): Record<string, unknown> | null => value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : null
/** Coaxi agent-tool-call::parseChildSessionId and collab-tool input rules.
 * These are candidate display links only; reader.child must find the signed
 * Core-verified reference before any of them becomes an actionable target. */
export function nativeToolChildren(use?: Use, result?: Result): string[] {
  const ids = new Set<string>()
  const live = object(object(use?.meta)?.grokSubagentSession)?.childSessionId
  if (typeof live === 'string' && live) ids.add(live)
  const saved = result?.agent_stats?.child_session_id
  if (typeof saved === 'string' && saved) ids.add(saved)
  if (isCodexCollabInput(use?.input_preview)) {
    for (const agent of parseCollabToolInput(use?.input_preview)?.agents ?? []) if (isCodexThreadId(agent.threadId)) ids.add(agent.threadId)
  }
  return [...ids]
}
