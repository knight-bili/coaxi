// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { StrictMode } from 'react'
import '@testing-library/jest-dom/vitest'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { MemoryLibraryView, MemoryRecord } from '@contracts'
import type { ThreadClient } from './camp-client'
import { ThreadClientProvider } from './camp-client'
import { MemoryLibrary } from './MemoryLibrary'

const readScope = vi.hoisted(() => vi.fn())
vi.mock('./canvas-view-state', () => ({ readCanvasViewScope: readScope }))
vi.mock('./MobileLayout', () => ({ MobileBack: () => null, useMobileLayout: () => false }))

afterEach(cleanup)
beforeEach(() => vi.clearAllMocks())

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}

function memory(body: string): MemoryRecord {
  return {
    id: body,
    scope: 'hearth',
    kind: 'agreement',
    creationOrigin: 'user',
    companionAgentId: null,
    relationshipAgentIds: [],
    direction: null,
    directedActorAgentId: null,
    lifecycle: 'active',
    currentRevisionId: `${body}-revision`,
    currentBody: body,
    currentBodyUtf8Bytes: new TextEncoder().encode(body).length,
    currentRetrievalKeys: ['项目约定'],
    reviewAfter: null,
    reviewDue: false,
    outgoingSuccessorIds: [],
    incomingPredecessorIds: [],
    version: 1,
    createdAt: '2026-10-23T00:00:00Z',
    updatedAt: '2026-10-23T00:00:00Z',
    retiredAt: null,
    forgottenAt: null,
    revisions: []
  }
}

const library = (body: string): MemoryLibraryView => ({ memories: [memory(body)], capacities: [] })

describe('memory library owner lifetime', () => {
  it('clears old Core drafts, retains its pending create, and ignores its late response', async () => {
    let core = 'core-a'
    let invalidated: (() => void) | undefined
    const createResponse = deferred<unknown>()
    const stored = new Map<string, unknown>()
    const recovery = {
      get: vi.fn(async (key: string) => stored.get(key) ?? null),
      put: vi.fn(async (value: unknown) => {
        const record = value as { key: string }
        stored.set(record.key, value)
      })
    }
    readScope.mockImplementation(async () => ({ host: 'host-a', core }))
    const request = vi.fn(async (method: string) => {
      if (method === 'memory.list') return library(core === 'core-a' ? 'Core A memory' : 'Core B memory')
      if (method === 'memory.hearthReviewItems.list') return []
      if (method === 'memory.create') return createResponse.promise
      throw new Error(`Unexpected request: ${method}`)
    })
    const client = {
      presentationScope: 'host-a',
      workbenchEditorRecovery: recovery,
      onInvalidated: (listener: () => void) => { invalidated = listener; return () => { invalidated = undefined } },
      request
    } as unknown as ThreadClient

    render(<StrictMode><ThreadClientProvider client={client}><MemoryLibrary agents={[]} /></ThreadClientProvider></StrictMode>)
    await screen.findByRole('button', { name: /Core A memory/ })
    await waitFor(() => expect(invalidated).toBeTypeOf('function'))
    fireEvent.click(screen.getByRole('button', { name: '新增记忆' }))
    fireEvent.change(await screen.findByLabelText('正文'), { target: { value: 'Core A 私有草稿' } })
    fireEvent.change(screen.getByLabelText('Retrieval Keys'), { target: { value: '旧词' } })
    const save = await screen.findByRole('button', { name: '保存记忆' })
    await waitFor(() => expect(save).not.toBeDisabled())
    fireEvent.click(save)
    await waitFor(() => expect(request).toHaveBeenCalledWith('memory.create', expect.anything()))

    const oldKey = 'memory-create:' + JSON.stringify(['host-a', 'core-a'])
    const oldRecord = stored.get(oldKey) as { pending: { request: { commandId: string } } }
    expect(oldRecord.pending.request.commandId).toBeTruthy()

    core = 'core-b'
    act(() => invalidated?.())
    await screen.findByRole('button', { name: /Core B memory/ })
    expect(screen.queryByRole('dialog')).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: '新增记忆' }))
    const newDraft = await screen.findByLabelText('正文') as HTMLTextAreaElement
    expect(newDraft.value).toBe('')

    await act(async () => {
      createResponse.resolve({ status: 'applied', payload: {} })
      await createResponse.promise
    })

    await waitFor(() => expect(screen.getByRole('dialog')).toBeTruthy())
    expect(screen.getByRole('dialog')).toBeTruthy()
    expect(screen.getByLabelText('正文')).toHaveValue('')
    expect((stored.get(oldKey) as { pending: { request: { commandId: string } } }).pending.request.commandId)
      .toBe(oldRecord.pending.request.commandId)
    expect(request.mock.calls.filter(([method]) => method === 'memory.create')).toHaveLength(1)
  })

  it('does not resolve an owner or deliver results after StrictMode cleanup and unmount', async () => {
    const initialRequests: Array<{ method: string; resolve(value: unknown): void; promise: Promise<unknown> }> = []
    const lateResponse = deferred<MemoryLibraryView>()
    let holdNextList = false
    let lateListStarted: (() => void) | undefined
    const lateStarted = new Promise<void>((resolve) => { lateListStarted = resolve })
    readScope.mockImplementation(async () => ({ host: 'host-a', core: 'core-a' }))
    const request = vi.fn((method: string): Promise<unknown> => {
      if (initialRequests.length < 4) {
        const result = deferred<unknown>()
        initialRequests.push({ method, ...result })
        return result.promise
      }
      if (method === 'memory.list' && holdNextList) {
        holdNextList = false
        lateListStarted?.()
        return lateResponse.promise
      }
      if (method === 'memory.list') return Promise.resolve(library('Current memory'))
      if (method === 'memory.hearthReviewItems.list') return Promise.resolve([])
      throw new Error(`Unexpected request: ${method}`)
    })
    const client = {
      presentationScope: 'host-a',
      request
    } as unknown as ThreadClient

    const view = render(<StrictMode><ThreadClientProvider client={client}><MemoryLibrary agents={[]} /></ThreadClientProvider></StrictMode>)
    await waitFor(() => expect(initialRequests).toHaveLength(4))
    expect(readScope).not.toHaveBeenCalled()
    await act(async () => {
      initialRequests[0].resolve(library('Stale StrictMode memory'))
      initialRequests[1].resolve([])
      await Promise.all([initialRequests[0].promise, initialRequests[1].promise])
    })
    expect(readScope).not.toHaveBeenCalled()
    await act(async () => {
      initialRequests[2].resolve(library('Current memory'))
      initialRequests[3].resolve([])
      await Promise.all([initialRequests[2].promise, initialRequests[3].promise])
    })
    await screen.findByRole('button', { name: /Current memory/ })
    expect(screen.queryByText('Stale StrictMode memory')).toBeNull()

    holdNextList = true
    view.rerender(<StrictMode><ThreadClientProvider client={client}><MemoryLibrary agents={[]} refreshSignal={1} /></ThreadClientProvider></StrictMode>)
    await lateStarted
    const readsBeforeUnmount = readScope.mock.calls.length
    view.unmount()

    await act(async () => {
      lateResponse.resolve(library('Late stale memory'))
      await lateResponse.promise
    })

    expect(readScope).toHaveBeenCalledTimes(readsBeforeUnmount)
  })
})
