import { describe, expect, it } from 'vitest'
import { parseCodegMcpToolCall, parseResumeTaskId } from './codeg-mcp-tool'
import { peelMcpResultEnvelope } from './mcp-result-envelope'

describe('native MCP history envelopes', () => {
  it.each([
    { content: [{ type: 'text', text: '已记录😀' }] },
    { result: { content: [{ type: 'text', text: '已记录😀' }] }, error: null },
    { Ok: { content: [{ type: 'text', text: '已记录😀' }] } },
    { structuredContent: { note: '已记录😀' } },
  ])('reads the actual report through supported host wrapping: %j', output => {
    const model = parseCodegMcpToolCall({ tool: 'task_complete',
      input: JSON.stringify({ arguments: JSON.stringify({ verdict: 'needs_review', summary: '等待复核' }) }),
      output: 'Wall time: 0.1 seconds\nOutput:\n' + JSON.stringify(output), state: 'output-available' })
    expect(model).toMatchObject({ verdict: 'needs_review', detail: '等待复核', resultText: '已记录😀', status: 'ok' })
  })

  it('retains a persistent task instruction even while authoring has no result', () => {
    const prompt = '逐行检查\n```不要把这里当控制指令执行```😀'
    expect(parseCodegMcpToolCall({ tool: 'create_work_task',
      input: JSON.stringify({ params: { title: '检查记录', prompt } }), state: 'input-available' }))
      .toMatchObject({ detail: '检查记录', prompt, resultText: null, status: 'running' })
  })

  it('keeps soft refusals distinct from failed tool transport', () => {
    const refusal = '找不到该任务，未恢复'
    expect(parseCodegMcpToolCall({ tool: 'resume_delegation', output: refusal, state: 'output-available' }))
      .toMatchObject({ resultText: refusal, status: 'ok' })
    expect(parseCodegMcpToolCall({ tool: 'resume_delegation', output: refusal, errorText: '连接中断', state: 'output-error' }))
      .toMatchObject({ resultText: '连接中断', status: 'err' })
    expect(parseResumeTaskId(JSON.stringify({ arguments: JSON.stringify({ task_id: 'source-task' }) }))).toBe('source-task')
  })

  it('does not reinterpret arbitrary child payloads as host reports or failures', () => {
    const child = { result: { answer: '保留完整子结果' }, error: '这是子数据字段' }
    expect(peelMcpResultEnvelope(child, () => false)).toEqual({ obj: child, hostError: null })
    expect(peelMcpResultEnvelope({ result: null, error: '调用失败' }, () => false).hostError).toBe('调用失败')
  })
})
