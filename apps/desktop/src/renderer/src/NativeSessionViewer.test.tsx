import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, fireEvent, render } from '@testing-library/react'
import { createElement } from 'react'
import { ThreadClientProvider, type ThreadClient } from './camp-client'
import { NativeTranscriptBody } from './NativeSessionViewer'
import type { NativeTranscript } from './native-child-reader'

afterEach(cleanup)

const client = { attachments: { kind: 'native' } } as unknown as ThreadClient
const completeOutput = `ORPHAN_RESULT_START ${'complete-result-'.repeat(256)} ORPHAN_RESULT_END`
const orphanTranscript: NativeTranscript = {
  schema_version: 1,
  turns: [{
    id: 'assistant-with-orphan-result',
    role: 'assistant',
    timestamp: '2026-10-23T00:00:00Z',
    blocks: [{
      type: 'tool_result',
      tool_use_id: 'missing-tool-use',
      output_preview: completeOutput,
      is_error: false,
      agent_stats: { child_session_id: 'orphan-child-session-marker', total_tokens: 23 }
    }]
  }],
  context_tokens: null,
  session_stats: null,
  summary: null,
  notes: { malformed_records: 0, incomplete_tail: false, unavailable_children: 0, unavailable_advisories: 0 },
  child_references: []
}

describe('NativeTranscriptBody', () => {
  it('shows a complete orphan tool result and stats when no child reader exists', () => {
    const view = render(createElement(ThreadClientProvider, {
      client,
      children: createElement(NativeTranscriptBody, {
        data: orphanTranscript,
        reader: null,
        target: null,
        live: false,
        visible: false
      })
    }))
    const trigger = view.container.querySelector<HTMLButtonElement>('button[aria-controls]')
    expect(trigger).not.toBeNull()
    fireEvent.click(trigger!)

    const renderedText = view.container.textContent ?? ''
    expect(renderedText).toContain(completeOutput)
    expect(renderedText).toContain('orphan-child-session-marker')
    expect(renderedText).toContain('23')
    expect(renderedText).not.toContain('查看下级帮手会话')
  })
})
