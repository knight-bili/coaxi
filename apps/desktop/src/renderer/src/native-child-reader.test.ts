import { createHash } from 'node:crypto'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { NativeChildReadPage } from '@contracts'
import { NativeChildReader, nativeTranscript, type NativeTranscript } from './native-child-reader'
import type { ThreadClient } from './camp-client'

const reference = { threadId: 'thread-a', agentRunId: 'run-a', evidenceId: 'evidence-a', receiverIndex: 0 }
const hash = (text: string) => createHash('sha256').update(text).digest('hex')
const deferred = <T>() => { let resolve!: (value: T) => void; let reject!: (reason: unknown) => void; const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject } }
function transcript(text = '完整中文😀'): NativeTranscript {
  return { schema_version: 1, turns: [{ id: 'turn-1', role: 'assistant', timestamp: '2026-10-18T00:00:00Z', blocks: [{ type: 'text', text }] }],
    context_tokens: 123, session_stats: { total_tokens: 123, total_duration_ms: 500 }, summary: null,
    notes: { malformed_records: 1, incomplete_tail: true, unavailable_children: 0, unavailable_advisories: 2 },
    child_references: [{ source_id: 'verified-child', reference: 'opaque-signed-child' }] }
}
function fragments(text: string): string[] {
  const result: string[] = [], encoder = new TextEncoder(); let value = '', bytes = 0
  for (const char of text) {
    const length = encoder.encode(char).length
    if (bytes + length > 32768) { result.push(value); value = ''; bytes = 0 }
    value += char; bytes += length
  }
  if (value) result.push(value)
  return result
}
type Client = Pick<ThreadClient, 'request' | 'onInvalidated'>
function transport() {
  const snapshots = new Map<string, { text: string; fragments: string[] }>()
  const state = { data: transcript(), live: false,
    intercept: null as null | ((method: string, params: Record<string, any>, next: () => unknown) => unknown),
    invalidated: null as null | ((change?: { resync?: boolean }) => void) }
  const request = vi.fn(async (method: string, params: Record<string, any> = {}): Promise<any> => {
    const next = (): unknown => {
      if (method === 'runtime.childReads.close') { snapshots.delete(params.readId); return { closed: true } }
      if (method === 'runtime.childReads.start') { const text = JSON.stringify(state.data); snapshots.set(params.readId, { text, fragments: fragments(text) }); return { readId: params.readId, coreGeneration: 'generation-a', state: 'ready', snapshotReference: `snapshot-${params.readId}`, code: null } }
      if (method === 'runtime.childReads.poll') return { readId: params.readId, coreGeneration: 'generation-a', state: 'ready', snapshotReference: `snapshot-${params.readId}`, code: null }
      if (method === 'runtime.childReads.page') {
        const source = snapshots.get(params.readId)!
        const index = params.cursor ? Number(String(params.cursor).slice('cursor-'.length)) : 0
        const offset = Buffer.byteLength(source.fragments.slice(0, index).join(''))
        return { snapshotReference: `snapshot-${params.readId}`, projectionDigest: hash(source.text), sourceDigest: hash(`source:${source.text}`),
          offset, totalBytes: Buffer.byteLength(source.text), fragment: source.fragments[index], nextCursor: index + 1 < source.fragments.length ? `cursor-${index + 1}` : null,
          complete: index + 1 === source.fragments.length, live: state.live, reportedStatus: state.live ? 'running' : 'completed' } satisfies NativeChildReadPage
      }
      throw new Error('unexpected operation')
    }
    return state.intercept ? state.intercept(method, params, next) : next()
  })
  const client: Client = { request: request as Client['request'], onInvalidated: listener => { state.invalidated = listener; return () => { state.invalidated = null } } }
  return { state, request, client }
}
afterEach(() => vi.useRealTimers())

describe('Core native read snapshot transport and display lifetime', () => {
  it('reassembles full long Unicode/tool JSON with independent SHA256 before publishing, without native path/session inputs', async () => {
    const server = transport(), long = '长中文😀\\\"\n'.repeat(12000)
    server.state.data = transcript(long)
    server.state.data.turns[0]!.blocks.push({ type: 'tool_use', tool_use_id: 'tool', tool_name: 'read', input_preview: JSON.stringify({ nested: long }), meta: { unknown: { retained: long } } },
      { type: 'tool_result', tool_use_id: 'tool', output_preview: long, is_error: false })
    const reader = new NativeChildReader(server.client, { reference })
    const refresh = reader.refresh(); expect(reader.refresh()).toBe(refresh); await refresh
    expect(reader.getSnapshot().data).toEqual(server.state.data)
    expect(server.request.mock.calls.filter(([method]) => method === 'runtime.childReads.page').length).toBeGreaterThan(3)
    expect(server.request.mock.calls[0]).toEqual(['runtime.childReads.start', { readId: expect.any(String), reference }])
    expect(JSON.stringify(server.request.mock.calls)).not.toMatch(/nativeSessionId|homePath|workingDirectory|adapterKind/)
    reader.dispose()
  })
  it.each(['offset', 'snapshot', 'digest', 'source', 'total', 'empty', 'oversized', 'early-end', 'terminal-cursor', 'generation'])('rejects a mixed or incomplete %s response without publishing a partial document', async broken => {
    const server = transport(); server.state.data = transcript('甲😀'.repeat(15000))
    server.state.intercept = (method, params, next) => {
      const value = next() as any
      if (method === 'runtime.childReads.start' && broken === 'generation') return { ...value, readId: 'other-read' }
      if (method !== 'runtime.childReads.page') return value
      if (broken === 'offset') value.offset += 1
      if (broken === 'snapshot') value.snapshotReference = 'another-snapshot'
      if (broken === 'digest') value.projectionDigest = '0'.repeat(64)
      if (broken === 'source' && params.cursor) value.sourceDigest = '1'.repeat(64)
      if (broken === 'total' && params.cursor) value.totalBytes += 1
      if (broken === 'empty') value.fragment = ''
      if (broken === 'oversized') value.fragment = 'a'.repeat(32769)
      if (broken === 'early-end') { value.complete = true; value.nextCursor = null }
      if (broken === 'terminal-cursor' && value.complete) value.nextCursor = 'unexpected'
      return value
    }
    const reader = new NativeChildReader(server.client, { reference }); await reader.refresh()
    expect(reader.getSnapshot().data).toBeNull(); expect(reader.getSnapshot().error).toBeTruthy()
    expect(server.request.mock.calls.some(([method]) => method === 'runtime.childReads.close')).toBe(true)
    reader.dispose()
  })
  it('preserves the last complete body after a refresh failure and releases the failed candidate only', async () => {
    const server = transport(), reader = new NativeChildReader(server.client, { reference })
    await reader.refresh(); const saved = reader.getSnapshot().data, first = server.request.mock.calls[0]![1]!.readId
    server.state.intercept = method => { if (method === 'runtime.childReads.close') return { closed: true }; throw new Error('native_child.source_changed') }
    await reader.refresh()
    expect(reader.getSnapshot().data).toBe(saved); expect(reader.getSnapshot().error).toBe('native_child.source_changed')
    expect(server.request.mock.calls.filter(([m, p]) => m === 'runtime.childReads.close' && p!.readId === first)).toHaveLength(0)
    reader.dispose()
  })
  it('closes a pending start immediately and ignores its late success, then allows a new reader after stop', async () => {
    const server = transport(), pending = deferred<unknown>(), entered = deferred<void>()
    server.state.intercept = (method, _, next) => { const value = next(); if (method === 'runtime.childReads.start') { entered.resolve(); return pending.promise.then(() => value) }; return value }
    const reader = new NativeChildReader(server.client, { reference }), old = reader.refresh()
    await entered.promise; reader.stop(); await old
    expect(server.request.mock.calls.map(([m]) => m)).toContain('runtime.childReads.close')
    server.state.intercept = null; server.state.data = transcript('new read')
    await reader.refresh(); const current = reader.getSnapshot().data
    pending.resolve({}); await Promise.resolve(); await Promise.resolve()
    expect(reader.getSnapshot().data).toBe(current); expect(current?.turns[0]!.blocks[0]).toEqual({ type: 'text', text: 'new read' })
    reader.dispose()
  })
  it('polls one pending read and rejects a Core generation change', async () => {
    vi.useFakeTimers()
    const server = transport()
    server.state.intercept = (method, _, next) => {
      const value = next() as any
      return method === 'runtime.childReads.start' ? { ...value, state: 'reading', snapshotReference: null }
        : method === 'runtime.childReads.poll' ? { ...value, coreGeneration: 'generation-b' } : value
    }
    const reader = new NativeChildReader(server.client, { reference }), run = reader.refresh()
    await vi.advanceTimersByTimeAsync(150); await run
    expect(reader.getSnapshot().data).toBeNull(); expect(reader.getSnapshot().error).toBe('native_child.read_response_invalid')
    expect(server.request.mock.calls.filter(([m]) => m === 'runtime.childReads.start')).toHaveLength(1)
    reader.dispose()
  })
  it('refreshes only while retained and Core reports live, and dispose/release is safe in StrictMode order', async () => {
    vi.useFakeTimers()
    const server = transport(); server.state.live = true
    const reader = new NativeChildReader(server.client, { reference }), release = reader.retain()
    await reader.refresh(); await vi.advanceTimersByTimeAsync(2000)
    expect(server.request.mock.calls.filter(([m]) => m === 'runtime.childReads.start')).toHaveLength(2)
    server.state.live = false; await vi.advanceTimersByTimeAsync(2000)
    const starts = server.request.mock.calls.filter(([m]) => m === 'runtime.childReads.start').length
    await vi.advanceTimersByTimeAsync(10000)
    expect(server.request.mock.calls.filter(([m]) => m === 'runtime.childReads.start')).toHaveLength(starts)
    reader.dispose(); release(); release()
    const again = reader.retain(); await reader.refresh(); again(); reader.dispose()
    expect(reader.getSnapshot().loading).toBe(false)
  })
  it('holds an old parent snapshot through a child start while a parent refresh replaces the displayed body', async () => {
    const server = transport(), parent = new NativeChildReader(server.client, { reference })
    await parent.refresh(); const parentId = server.request.mock.calls[0]![1]!.readId
    const entered = deferred<void>(), releaseStart = deferred<void>()
    server.state.intercept = async (method, params, next) => {
      if (method === 'runtime.childReads.start' && params.descendant) { entered.resolve(); await releaseStart.promise }
      return next()
    }
    const target = parent.child('verified-child')!; expect(parent.child('unverified-child')).toBeNull()
    const child = new NativeChildReader(server.client, target), opening = child.refresh()
    await entered.promise; await parent.refresh()
    expect(server.request.mock.calls.filter(([m, p]) => m === 'runtime.childReads.close' && p!.readId === parentId)).toHaveLength(0)
    releaseStart.resolve(); await opening
    expect(server.request.mock.calls.filter(([m, p]) => m === 'runtime.childReads.close' && p!.readId === parentId)).toHaveLength(1)
    const start = server.request.mock.calls.find(([m, p]) => m === 'runtime.childReads.start' && p!.descendant)![1]!
    expect(start.reference).toEqual(reference); expect(start.descendant).toEqual({ parentReadId: parentId, descendantReference: 'opaque-signed-child' })
    child.dispose(); parent.dispose()
  })
  it('clears old scoped data on resync before loading and detaches all invalidation listeners on release', async () => {
    const server = transport(), reader = new NativeChildReader(server.client, { reference }), release = reader.retain()
    await reader.refresh(); expect(reader.getSnapshot().data).not.toBeNull()
    const waiting = deferred<unknown>()
    server.state.intercept = (method, _, next) => method === 'runtime.childReads.start' ? waiting.promise : next()
    server.state.invalidated?.({ resync: true }); expect(reader.getSnapshot().data).toBeNull()
    release(); expect(server.state.invalidated).toBeNull(); reader.dispose(); waiting.reject(new Error('old connection'))
    await Promise.resolve(); await Promise.resolve()
  })
  it.each([['schema_version', 2], ['context_tokens', -1], ['turns', [{}]], ['child_references', [{ source_id: '../escape', reference: 'opaque' }]]])('rejects an unsupported display contract %s while keeping all known metadata', (key, value) => {
    expect(() => nativeTranscript({ ...transcript(), [key as string]: value })).toThrow('native_child.projection_invalid')
  })
})
