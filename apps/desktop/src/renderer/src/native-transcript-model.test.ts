import { describe, expect, it } from 'vitest'
import type { AgentRunExecutionEvidenceView } from '@contracts'
import { nativeSubagentEntries } from './native-subagent-entries'
import { nativeToolChildren, nativeToolPairs } from './native-transcript-model'
import type { NativeTranscriptBlock, NativeTranscriptTurn } from './native-child-reader'

const event = (id: string, sequence: number, children: unknown[], override: Record<string, unknown> = {}): AgentRunExecutionEvidenceView => ({
  id, agentRunId: 'run', executionEpoch: 1, sequence, eventType: 'runtime.activity', kind: 'tool_call', phase: 'completed',
  payload: { nativeSubagents: { schemaVersion: 1, adapterKind: 'codex-cli', source: 'codex-app-server.collabAgentToolCall', relationship: 'runtime_reported_receiver',
    parentNativeSessionId: 'parent', children, ...override } }, contentBlobId: null, contentByteCount: 200, isTruncated: false, occurredAt: '2026-10-18T00:00:00Z'
})
describe('native source metadata stays separate from native read authority', () => {
  it('keeps the original receiver index and only the latest metadata without fetching any transcript', () => {
    const before = event('before', 1, [{ nativeSessionId: 'child', status: 'running' }])
    const after = event('after', 2, [{ nativeSessionId: '../bad', status: 'unknown' }, { nativeSessionId: 'child', status: 'completed' }])
    const rows = nativeSubagentEntries('thread', 'run', [after, before])
    expect(rows.find(row => row.label === '#child')).toEqual({ identity: 'codex:child', runtime: 'codex', label: '#child', status: 'completed', reference: { threadId: 'thread', agentRunId: 'run', evidenceId: 'after', receiverIndex: 1 } })
    expect(rows.some(row => row.reference?.receiverIndex === 0 && row.reference.evidenceId === 'after')).toBe(false)
    expect(nativeSubagentEntries('thread', 'other-run', [before])).toEqual([])
  })
  it('retains a Grok short identity as status only and never sends it as an independent Session', () => {
    const value = event('grok', 1, [{ nativeSubagentId: 'short', status: 'running' }], { adapterKind: 'grok-build', source: 'grok-acp.subagent_notification', relationship: 'runtime_reported_session_child' })
    expect(nativeSubagentEntries('thread', 'run', [value])).toEqual([{ identity: 'grok:short', runtime: 'grok', label: '#short', status: 'running', reference: null }])
    expect(nativeSubagentEntries('thread', 'run', [event('same', 1, [{ nativeSessionId: 'parent', status: 'running' }])])[0]!.reference).toBeNull()
  })
  it.each([{ schemaVersion: 2 }, { adapterKind: 'claude-code-cli' }, { source: 'arbitrary-tool-field' }, { relationship: 'same-directory' }])('does not manufacture a source link for %j', override => {
    expect(nativeSubagentEntries('thread', 'run', [event('invalid', 1, [{ nativeSessionId: 'child', status: 'running' }], override)])).toEqual([])
  })
  it('preserves results from reused IDs and orphan results in chronological matching', () => {
    const use = (): Extract<NativeTranscriptBlock, { type: 'tool_use' }> => ({ type: 'tool_use', tool_use_id: 'same', tool_name: 'read', input_preview: '{}' })
    const result = (text: string): Extract<NativeTranscriptBlock, { type: 'tool_result' }> => ({ type: 'tool_result', tool_use_id: 'same', is_error: false, output_preview: text })
    const orphan = result('orphan first'), first = use(), second = use(), one = result('complete one'), two = result('complete two'), extra = result('orphan last')
    const turns: NativeTranscriptTurn[] = [{ id: 'turn', role: 'assistant', timestamp: '2026-10-18T00:00:00Z', blocks: [orphan, first, second, one, two, extra] }]
    const pairs = nativeToolPairs(turns)
    expect(pairs.results.get(first)).toBe(one); expect(pairs.results.get(second)).toBe(two)
    expect(pairs.paired.has(orphan)).toBe(false); expect(pairs.paired.has(extra)).toBe(false)
    expect(pairs.paired.size).toBe(2)
  })
  it('retains an orphan result and its stats when there is no tool use or child target', () => {
    const fullOutput = 'first line\n' + '完整输出'.repeat(1200)
    const stats = { child_session_id: 'saved-child', duration_ms: 1200, usage: { input_tokens: 15, output_tokens: 8 } }
    const orphan: Extract<NativeTranscriptBlock, { type: 'tool_result' }> = {
      type: 'tool_result', tool_use_id: 'missing-use', output_preview: fullOutput, is_error: false, agent_stats: stats
    }
    const pairs = nativeToolPairs([{ id: 'orphan-turn', role: 'assistant', timestamp: '2026-10-18T00:00:00Z', blocks: [orphan] }])

    expect(pairs.paired.has(orphan)).toBe(false)
    expect(orphan.output_preview).toBe(fullOutput)
    expect(orphan.agent_stats).toEqual(stats)
    expect(nativeToolChildren(undefined, orphan)).toEqual(['saved-child'])
    expect(nativeToolChildren(undefined, undefined)).toEqual([])
  })
  it('uses Coaxi live metadata, saved stats and UUID collab rules as candidates without reading arbitrary input strings', () => {
    const uuid = '00000000-0000-4000-8000-000000000001'
    const use: Extract<NativeTranscriptBlock, { type: 'tool_use' }> = { type: 'tool_use', tool_use_id: 'call', tool_name: 'collab_agent',
      input_preview: JSON.stringify({ senderThreadId: 'parent', receiverThreadIds: [uuid], agentsStates: { [uuid]: { status: 'running' }, 'named-agent': { status: 'completed' } } }),
      meta: { grokSubagentSession: { childSessionId: 'live-child' } } }
    expect(nativeToolChildren(use, { type: 'tool_result', tool_use_id: 'call', output_preview: null, is_error: false, agent_stats: { child_session_id: 'saved-child' } })).toEqual(['live-child', 'saved-child', uuid])
    expect(nativeToolChildren({ ...use, meta: null, input_preview: JSON.stringify({ arbitrary: uuid }) })).toEqual([])
  })
})
