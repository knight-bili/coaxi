import { describe, expect, it } from 'vitest'
import { jsonChildPath, parseJsonForTree, countJsonNodes } from '../components/ai-elements/json-tree'
import { sourceCodeLines } from './native-syntax'
import { formatSourceCopy } from './source-translations'
import { parseCodexCommandEnvelope } from '../lib/codex-command-action'

describe('原生工具内容边界', () => {
  it('字段名中的点、空名称、斜杠和波浪号不会共享展开身份', () => {
    const root = '', dotted = jsonChildPath(root, 'a.b'), nested = jsonChildPath(jsonChildPath(root, 'a'), 'b')
    const names = [root, dotted, nested, jsonChildPath(root, ''), jsonChildPath(root, 'a/b'), jsonChildPath(root, 'a~b')]
    expect(new Set(names).size).toBe(names.length)
    expect(jsonChildPath(root, 'a/b')).toBe('/a~1b')
    expect(jsonChildPath(root, 'a~b')).toBe('/a~0b')
  })
  it('只把实际对象和数组作为树；其他输入保留原文', () => {
    expect(parseJsonForTree(' {"x":[1,null]}')).toEqual({ x: [1, null] })
    for (const text of ['42', '"text"', '{bad}', '<script>']) expect(parseJsonForTree(text)).toBeUndefined()
    expect(countJsonNodes(Array.from({ length: 3000 }, (_, i) => i))).toBe(2001)
  })
  it('代码首尾与长度相同而中间变化时显示新内容', () => {
    const before = 'x'.repeat(120) + 'OLD' + 'y'.repeat(120)
    const after = 'x'.repeat(120) + 'NEW' + 'y'.repeat(120)
    expect(sourceCodeLines(after, [{ text: before }]).flat().map(x => x.content).join('')).toBe(after)
    expect(sourceCodeLines(before, [{ text: before }]).flat().map(x => x.content).join('')).toBe(before)
  })
  it('语法着色转换保留 CRLF、空行和末尾换行', () => {
    const text = 'a\r\n\n中文😀\n'
    const lines = sourceCodeLines(text, [{ text: 'a\r\n', className: 'syntax' }, { text: '\n中文😀\n' }])
    expect(lines.map(line => line.map(token => token.content).join('')).join('\n')).toBe(text)
    expect(lines[0][0].className).toBe('syntax')
    expect(lines).toHaveLength(4)
  })
  it('原版中英文 ICU 参数和复数完整展开', () => {
    expect(formatSourceCopy('zh-CN', 'Folder.chat.agentPlanOverlay', 'collapsedSummary', { completed: 2, total: 7 })).toBe('计划 2/7')
    expect(formatSourceCopy('en', 'Folder.chat.jsonTree', 'fields', { count: 1 })).toBe('1 field')
    expect(formatSourceCopy('en', 'Folder.chat.jsonTree', 'fields', { count: 2 })).toBe('2 fields')
    expect(formatSourceCopy('en', 'Folder.chat.jsonTree', 'items', { count: 0 })).toBe('0 items')
    expect(formatSourceCopy('zh-CN', 'Folder.chat.contentParts.goal', 'objective')).toBe('目标')
  })
  it('仅解包明确的命令信封，不吞掉相似正文', () => {
    const raw = 'Wall time: 原始文字\nOutput:\n```json\n{"stdout":"保留"}\n```'
    expect(parseCodexCommandEnvelope(raw)).toBeNull()
    expect(parseCodexCommandEnvelope('{"stdout":"保留"}')).toBeNull()
    expect(parseCodexCommandEnvelope(JSON.stringify({ formatted_output: raw, exit_code: 0 }))?.output).toBe(raw)
  })
})
