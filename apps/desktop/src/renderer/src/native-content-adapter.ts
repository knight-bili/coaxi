import { adaptMessageTurns, type AdaptedMessage, type AdapterMessageText } from './codeg-native-content/lib/adapters/ai-elements-adapter'
import type { ContentBlock, MessageTurn, TurnUsage } from './codeg-native-content/lib/types'
import type { NativeTranscriptTurn } from './native-child-reader'
import { nativeToolPairs } from './native-transcript-model'

/** The donor adapts one turn at a time. Core can retain repeated native call
 * IDs or a result in a later turn. Preserve the viewer's existing arrival-order
 * pairing, without changing the frozen payload or any ID in tool input/meta. */
export function nativeAdapterTurns(turns: NativeTranscriptTurn[]): MessageTurn[] {
  const { results, paired } = nativeToolPairs(turns)
  return turns.map((turn, turnIndex) => ({ ...turn, blocks: turn.blocks.flatMap((block, blockIndex) => {
    if (block.type === 'tool_result' && paired.has(block)) return []
    if (block.type !== 'tool_use' || !block.tool_use_id) return [block]
    const id = `native-view:${turnIndex}:${blockIndex}`, result = results.get(block)
    return [{ ...block, tool_use_id: id }, ...(result ? [{ ...result, tool_use_id: id }] : [])]
  }) })) as unknown as MessageTurn[]
}

export function adaptNativeMessages(turns: NativeTranscriptTurn[], live: boolean, text: AdapterMessageText): AdaptedMessage[] {
  const source = nativeAdapterTurns(turns)
  const progress = new Map<number, Set<string>>()
  if (live) source.forEach((turn, index) => {
    const ids = turn.blocks.filter((block): block is Extract<ContentBlock, { type: 'tool_use' }> => block.type === 'tool_use')
      .filter(block => block.tool_use_id && ['pending', 'in_progress'].includes(block.status ?? ''))
      .map(block => block.tool_use_id!)
    if (ids.length) progress.set(index, new Set(ids))
  })
  return adaptMessageTurns(source, text, undefined, progress, true)
}

/** Invalid optional provider metrics remain in raw metadata, never become zero counts. */
export function nativeTurnStats(turn: NativeTranscriptTurn): { usage: TurnUsage | null; duration_ms: number | null; model: string | null; completedAt: string | null } {
  const usage = turn.usage
  const valid = usage && ['input_tokens', 'output_tokens', 'cache_creation_input_tokens', 'cache_read_input_tokens']
    .every(key => Number.isSafeInteger(usage[key]) && usage[key] >= 0)
  return { usage: valid ? usage as unknown as TurnUsage : null,
    duration_ms: typeof turn.duration_ms === 'number' && Number.isFinite(turn.duration_ms) && turn.duration_ms >= 0 ? turn.duration_ms : null,
    model: typeof turn.model === 'string' ? turn.model : null,
    completedAt: typeof turn.completed_at === 'string' ? turn.completed_at : null }
}
