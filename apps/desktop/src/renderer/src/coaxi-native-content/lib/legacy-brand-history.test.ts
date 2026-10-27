import { describe, expect, it } from 'vitest'
import { contextCompactionSummary } from './context-compaction'
import { parseBackgroundTaskMarker } from './background-agent'

describe('旧品牌历史只读兼容', () => {
  it('保留旧压缩摘要，仍要求真实压缩标记', () => {
    expect(contextCompactionSummary({ contextCompaction: true, 'codeg.compactionSummary': true }, '摘要')).toBe('摘要')
    expect(contextCompactionSummary({ 'codeg.compactionSummary': true }, '原文')).toBeNull()
  })
  it('读取旧后台任务，不改写用户正文', () => {
    expect(parseBackgroundTaskMarker('[[codeg-background-task]]{"task_id":"task1","status":"completed"}')?.taskId).toBe('task1')
    expect(parseBackgroundTaskMarker('用户文字 [[codeg-background-task]]{"task_id":"task1"}')).toBeNull()
  })
})
