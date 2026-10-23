// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom/vitest'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { NativeContinuationPreview, NativeContinuationReceipt } from '@contracts'
import type { ThreadClient } from './camp-client'
import { ThreadClientProvider } from './camp-client'
import { NativeContinuationDialog } from './NativeContinuationDialog'
import type { NativeTranscript } from './native-child-reader'

afterEach(cleanup)

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}

describe('native continuation dialog ownership', () => {
  it('does not close a replacement execution when the old open request finishes late', async () => {
    const oldEntry = { id: 'import-old', revision: 'r1', provider: 'codex', summary: { title: '旧来源' }, sourceName: '旧来源' }
    const newEntry = { id: 'import-new', revision: 'r2', provider: 'codex', summary: { title: '新来源' }, sourceName: '新来源' }
    const preview = {
      schemaVersion: 1, previewId: 'preview-old', previewDigest: 'digest', expiresAt: '2026-10-23T00:00:00Z',
      source: { schemaVersion: 1, importId: oldEntry.id, importRevision: oldEntry.revision, provider: 'codex', nativeSessionId: 'native', sourceHandleId: 'handle', sourceRevision: 'source-r1' },
      mode: 'continue', point: null, sourceThreadId: 'source-thread', newThreadId: 'new-thread', newConversationId: 'new-conversation',
      agentId: 'agent', expectedAgentVersion: 1, runtimeConfigDigest: 'runtime', workspaceDigest: 'workspace', title: '新会话',
      historyMessageCount: 1, capabilityStatus: 'available', reason: null
    } as NativeContinuationPreview
    const receipt = {
      schemaVersion: 1, commandId: 'command-old', state: 'binding_recorded', originId: 'origin',
      threadId: 'new-thread', conversationId: 'new-conversation', reportedSessionId: null, error: null
    } as NativeContinuationReceipt
    const recovery = {
      get: vi.fn(async (key: string) => key.endsWith(`:${oldEntry.id}`)
        ? { schemaVersion: 1, key, editorId: 'native-continuation', pending: {
          schemaVersion: 1,
          request: { schemaVersion: 1, commandId: receipt.commandId, previewId: preview.previewId, previewDigest: preview.previewDigest },
          preview
        } }
        : null),
      put: vi.fn(async () => undefined)
    }
    const client = {
      presentationScope: 'desktop',
      workbenchEditorRecovery: recovery,
      request: vi.fn(async (method: string) => {
        if (method === 'history.execution.receipt') return receipt
        if (method === 'members.list') return []
        throw new Error(`Unexpected request: ${method}`)
      })
    } as unknown as ThreadClient
    const openRequest = deferred<boolean>()
    const onOpen = vi.fn(() => openRequest.promise)
    const oldClose = vi.fn()
    const oldExecution = { entry: oldEntry }

    function Harness(): React.JSX.Element {
      const [execution, setExecution] = useState<{ entry: typeof oldEntry } | null>(oldExecution)
      if (!execution) return <button onClick={() => setExecution({ entry: newEntry })}>恢复执行</button>
      const close = execution === oldExecution
        ? oldClose.mockImplementation(() => setExecution((current) => current === oldExecution ? null : current))
        : () => setExecution((current) => current === execution ? null : current)
      return <ThreadClientProvider client={client}>
        <button onClick={() => setExecution({ entry: newEntry })}>打开新执行</button>
        <NativeContinuationDialog
          key={execution.entry.id}
          threadId="source-thread"
          entry={execution.entry}
          data={{ turns: [] } as unknown as NativeTranscript}
          mode="continue"
          onClose={close}
          onOpen={onOpen}
        />
      </ThreadClientProvider>
    }

    render(<Harness />)
    const openButton = await screen.findByRole('button', { name: '打开新工作私聊' })
    await waitFor(() => expect(openButton).not.toBeDisabled())
    fireEvent.click(openButton)
    expect(onOpen).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('button', { name: '打开新执行' }))
    expect(await screen.findByText(/新来源/)).toBeTruthy()

    await act(async () => { openRequest.resolve(true); await openRequest.promise })

    expect(oldClose).not.toHaveBeenCalled()
    expect(screen.getByRole('dialog')).toBeTruthy()
    expect(screen.getByText(/新来源/)).toBeTruthy()
  })
})
