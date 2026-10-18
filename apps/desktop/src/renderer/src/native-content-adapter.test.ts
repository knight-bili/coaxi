import { describe, expect, it } from 'vitest'
import { adaptNativeMessages, nativeAdapterTurns, nativeTurnStats } from './native-content-adapter'
import type { NativeTranscriptTurn, NativeTranscriptBlock } from './native-child-reader'
import type { AdaptedContentPart } from './codeg-native-content/lib/adapters/ai-elements-adapter'

const turn = (blocks: NativeTranscriptBlock[], id = 'turn'): NativeTranscriptTurn => ({ id, role: 'assistant', timestamp: '2026-10-18T00:00:00Z', blocks })
const use = (id: string): NativeTranscriptBlock => ({ type: 'tool_use', tool_use_id: id, tool_name: 'read_file', input_preview: '{"path":"src/保留.ts"}', meta: { untouched: id } })
const result = (id: string, text: string): NativeTranscriptBlock => ({ type: 'tool_result', tool_use_id: id, output_preview: text, is_error: false })
const text = { attachedResources: '材料', toolCallFailed: '失败', pageHandoffName: () => '页面材料' }
it('preserves valid metrics while unknown or malformed metrics stay in raw metadata rather than become zero usage', () => {
  const original = { ...turn([]), usage: { input_tokens: 1200, output_tokens: 340, cache_creation_input_tokens: 0, cache_read_input_tokens: 50 }, model: 'source-model', duration_ms: 4321, completed_at: '2026-10-19T00:00:00Z' }
  const before = JSON.stringify(original)
  expect(nativeTurnStats(original)).toEqual({ usage: original.usage, model: 'source-model', duration_ms: 4321, completedAt: original.completed_at })
  expect(nativeTurnStats({ ...original, usage: { input_tokens: -1 }, duration_ms: Number.NaN })).toMatchObject({ usage: null, duration_ms: null })
  expect(nativeTurnStats({ ...original, usage: null })).toMatchObject({ usage: null })
  expect(JSON.stringify(original)).toBe(before)
})
function calls(parts: AdaptedContentPart[]): Array<Extract<AdaptedContentPart, { type: 'tool-call' }>> {
  return parts.flatMap(part => part.type === 'tool-call' ? [part] : part.type === 'tool-group' ? part.items : part.type === 'goal-run' ? calls([part.start, ...part.items, ...(part.end ? [part.end] : [])]) : [])
}

describe('native snapshot to original Codeg adapter boundary', () => {
  it('keeps arrival-order matches for reused IDs and later-turn results without mutating frozen input', () => {
    const input = [turn([use('same'), use('same'), result('same', '第一份')], 'a'), turn([result('same', '第二份'), result('orphan', '独立结果')], 'b')]
    const before = JSON.stringify(input), output = adaptNativeMessages(input, false, text)
    expect(calls(output.flatMap(message => message.content)).map(item => item.output)).toEqual(['第一份', '第二份'])
    expect(output.flatMap(message => message.content).some(part => part.type === 'tool-result' && part.output === '独立结果')).toBe(true)
    expect(JSON.stringify(input)).toBe(before)
    expect(calls(output.flatMap(message => message.content)).map(item => item.meta)).toEqual([{ untouched: 'same' }, { untouched: 'same' }])
  })
  it('retains complete Unicode tool bodies and tool-result images through the source adapter', () => {
    const long = '完整材料🧩'.repeat(6000), image = { data: 'AQID', mime_type: 'image/png' }
    const input = [turn([use('long'), { ...result('long', long), images: [image] } as NativeTranscriptBlock])]
    const output = adaptNativeMessages(input, false, text).flatMap(message => message.content)
    expect(calls(output)[0]?.output).toBe(long)
    expect(output.find(part => part.type === 'generated-image')).toMatchObject({ type: 'generated-image', image })
  })
  it('lifts proposed plans and checklist records through the unchanged source adaptation rules', () => {
    const input = [turn([{ type: 'text', text: '前言\n<proposed_plan>\n## 实施方案\n保留完整方案\n</proposed_plan>\n后文' },
      { type: 'tool_use', tool_use_id: 'todo', tool_name: 'TodoWrite', input_preview: JSON.stringify({ todos: [{ content: '完成迁移', status: 'completed', priority: 'high' }] }) }, result('todo', '')])]
    const output = adaptNativeMessages(input, false, text).flatMap(message => message.content)
    expect(output.find(part => part.type === 'proposed-plan')).toMatchObject({ markdown: '## 实施方案\n保留完整方案' })
    expect(output.find(part => part.type === 'plan')).toMatchObject({ entries: [{ content: '完成迁移', status: 'completed', priority: 'high' }] })
  })
  it('does not manufacture pairings for ID-less and unmatched results', () => {
    const input = [turn([result('orphan', '保留独立结果')]), turn([{ type: 'tool_use', tool_use_id: null, tool_name: 'read', input_preview: null }, result('different', '原有位置规则')])]
    const output = nativeAdapterTurns(input)
    expect(output[0]!.blocks[0]).toEqual(input[0]!.blocks[0])
    expect(output[1]!.blocks).toEqual(input[1]!.blocks)
  })
  it('keeps all images, complete output and statistics on an orphan result', () => {
    const images = [1, 2, 3].map(index => ({ data: `page-${index}`, mime_type: 'image/png' }))
    const stats = { total_tokens: 42, timeline: [{ type: 'text', text: '独立执行记录' }] }
    const output = adaptNativeMessages([turn([{ type: 'tool_result', tool_use_id: 'missing-use', output_preview: '独立结果完整保留', is_error: true, images, agent_stats: stats }])], false, text).flatMap(message => message.content)
    expect(output.filter(part => part.type === 'generated-image').map(part => part.image?.data)).toEqual(images.map(image => image.data))
    expect(output[0]).toMatchObject({ type: 'tool-result', output: '独立结果完整保留', state: 'output-error', agentStats: stats })
  })
})
