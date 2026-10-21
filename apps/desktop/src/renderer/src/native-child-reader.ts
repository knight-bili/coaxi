// Read-only transport for Codeg's subagent-session-dialog. Core alone resolves
// native addresses. A browser never sends a Session ID or a source path.
import { sha256 } from '@noble/hashes/sha2.js'
import type { NativeAgentReadReference, NativeChildReadPage, NativeChildReadReference, NativeChildReadView } from '@contracts'
import type { ThreadClient } from './camp-client'
import { newCommandId } from '../../shared/command-id'

export interface NativeTranscriptTurn {
  id: string; role: 'user' | 'assistant' | 'system'; timestamp: string
  blocks: NativeTranscriptBlock[]
  usage?: Record<string, number> | null; duration_ms?: number | null; model?: string | null; completed_at?: string | null
  agent_message_id?: string | null
}
/** Exact eight persisted variants from Codeg/Rust. Opaque metadata is retained. */
export type NativeTranscriptBlock =
  | { type: 'text' | 'thinking'; text: string }
  | { type: 'turn_error'; message: string }
  | { type: 'image'; data: string; mime_type: string; uri?: string | null }
  | { type: 'image_generation'; revised_prompt?: string | null; image?: { data: string; mime_type: string; uri?: string | null } | null }
  | { type: 'tool_use'; tool_use_id: string | null; tool_name: string; input_preview: string | null; status?: string | null; meta?: unknown }
  | { type: 'tool_result'; tool_use_id: string | null; output_preview: string | null; is_error: boolean; agent_stats?: Record<string, unknown> | null; images?: Array<{ data: string; mime_type: string; uri?: string | null }> }
export interface NativeTranscript {
  schema_version: 1
  turns: NativeTranscriptTurn[]
  context_tokens: number | null
  session_stats: Record<string, unknown> | null
  summary: { title: string | null; model: string | null; git_branch: string | null; started_at: string; ended_at: string | null; message_count: number } | null
  notes: { malformed_records: number; incomplete_tail: boolean; unavailable_children: number; unavailable_advisories: number }
  child_references: Array<{ source_id: string; reference: string }>
  agent_stats?: Record<string, unknown> | null
}
export interface NativeReaderTarget {
  reference: NativeChildReadReference | NativeAgentReadReference
  kind?: 'session' | 'agent'
  descendant?: { parent: NativeChildReader; sourceId: string }
}
export interface NativeReaderState {
  data: NativeTranscript | null
  loading: boolean
  error: string | null
  live: boolean
  reportedStatus: string | null
}
type Client = Pick<ThreadClient, 'request' | 'onInvalidated'>
type Resource = { id: string; refs: number; retired: boolean; closed: boolean; publishedAt: number }
type Flight = { signal: AbortController; resource: Resource; promise: Promise<void> }
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value)
const count = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
const nullableString = (value: unknown): boolean => value === null || value === undefined || typeof value === 'string'
const digestString = (value: unknown): value is string => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value)
const identifier = (value: unknown): value is string => typeof value === 'string' && /^[a-zA-Z0-9_-]{1,256}$/.test(value)
function failed(code: string): never { throw new Error(code) }

/** The hash validates byte identity; this validates only the display contract,
 * without dropping unknown metadata, clipping strings or manufacturing status. */
export function nativeTranscript(value: unknown): NativeTranscript {
  if (!record(value) || value.schema_version !== 1 || !Array.isArray(value.turns) || !record(value.notes)
    || !Array.isArray(value.child_references) || !(value.context_tokens === null || count(value.context_tokens))
    || !(value.session_stats === null || record(value.session_stats)) || !(value.summary === null || record(value.summary))) failed('native_child.projection_invalid')
  const view = value as Record<string, unknown>
  for (const turn of view.turns as unknown[]) {
    if (!record(turn) || typeof turn.id !== 'string' || !['user', 'assistant', 'system'].includes(String(turn.role))
      || typeof turn.timestamp !== 'string' || !Array.isArray(turn.blocks)) failed('native_child.projection_invalid')
    for (const block of (turn as Record<string, unknown>).blocks as unknown[]) {
      if (!record(block)) failed('native_child.projection_invalid')
      const b = block as Record<string, unknown>
      const image = (v: unknown): boolean => record(v) && typeof v.data === 'string' && typeof v.mime_type === 'string' && nullableString(v.uri)
      const valid = b.type === 'text' || b.type === 'thinking' ? typeof b.text === 'string'
        : b.type === 'turn_error' ? typeof b.message === 'string'
          : b.type === 'image' ? image(b)
            : b.type === 'image_generation' ? nullableString(b.revised_prompt) && (b.image == null || image(b.image))
              : b.type === 'tool_use' ? nullableString(b.tool_use_id) && typeof b.tool_name === 'string' && nullableString(b.input_preview) && nullableString(b.status)
                : b.type === 'tool_result' ? nullableString(b.tool_use_id) && nullableString(b.output_preview) && typeof b.is_error === 'boolean'
                  && (b.agent_stats == null || record(b.agent_stats)) && (b.images == null || Array.isArray(b.images) && b.images.every(image)) : false
      if (!valid) failed('native_child.projection_invalid')
    }
  }
  const notes = view.notes as Record<string, unknown>
  if (!['malformed_records', 'unavailable_children', 'unavailable_advisories'].every(key => count(notes[key])) || typeof notes.incomplete_tail !== 'boolean') failed('native_child.projection_invalid')
  if (view.summary !== null) {
    const summary = view.summary as Record<string, unknown>
    if (!['title', 'model', 'git_branch', 'ended_at'].every(key => nullableString(summary[key])) || typeof summary.started_at !== 'string' || !count(summary.message_count)) failed('native_child.projection_invalid')
  }
  const ids = new Set<string>()
  for (const child of view.child_references as unknown[]) {
    if (!record(child) || !identifier(child.source_id) || typeof child.reference !== 'string' || !child.reference || ids.has(child.source_id)) failed('native_child.projection_invalid')
    ids.add((child as { source_id: string }).source_id)
  }
  return value as unknown as NativeTranscript
}
function abortError(): DOMException { return new DOMException('Reading closed', 'AbortError') }
function check(signal: AbortSignal): void { if (signal.aborted) throw abortError() }
function cancellable<T>(request: () => Promise<T>, signal: AbortSignal): Promise<T> {
  check(signal)
  const promise = request()
  return new Promise((resolve, reject) => {
    if (signal.aborted) { void promise.catch(() => undefined); reject(abortError()); return }
    const aborted = (): void => { signal.removeEventListener('abort', aborted); reject(abortError()) }
    signal.addEventListener('abort', aborted, { once: true })
    promise.then(value => { signal.removeEventListener('abort', aborted); if (signal.aborted) reject(abortError()); else resolve(value) },
      error => { signal.removeEventListener('abort', aborted); reject(error) })
  })
}
function delay(signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    check(signal)
    const aborted = (): void => { clearTimeout(timer); signal.removeEventListener('abort', aborted); reject(abortError()) }
    const timer = setTimeout(() => { signal.removeEventListener('abort', aborted); resolve() }, 150)
    signal.addEventListener('abort', aborted, { once: true })
  })
}

export class NativeChildReader {
  private state: NativeReaderState = { data: null, loading: false, error: null, live: false, reportedStatus: null }
  private listeners = new Set<() => void>()
  private users = new Set<symbol>()
  private flight: Flight | null = null
  private current: Resource | null = null
  private timer: ReturnType<typeof setTimeout> | undefined
  private off: (() => void) | undefined
  readonly target: NativeReaderTarget
  constructor(private client: Client, target: NativeReaderTarget) {
    this.target = { reference: Object.freeze({ ...target.reference }), kind: target.kind, ...(target.descendant ? { descendant: { ...target.descendant } } : {}) }
  }
  private method(action: 'start' | 'poll' | 'page' | 'close'): string { return `runtime.${this.target.kind === 'agent' ? 'agentReads' : 'childReads'}.${action}` }
  getSnapshot = (): NativeReaderState => this.state
  fileSource(): { threadId: string; readId: string } | null {
    return this.current && !this.current.retired && !this.current.closed && this.state.data
      ? { threadId: this.target.reference.threadId, readId: this.current.id } : null
  }
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener) } }
  private publish(next: Partial<NativeReaderState>): void { this.state = { ...this.state, ...next }; for (const listener of this.listeners) listener() }
  private resource(): Resource { return { id: newCommandId(), refs: 1, retired: false, closed: false, publishedAt: 0 } }
  private close(resource: Resource): void {
    if (!resource.retired || resource.refs > 0 || resource.closed) return
    resource.closed = true
    try { void this.client.request(this.method('close'), { readId: resource.id }).catch(() => undefined) } catch { /* Core/transport may already be gone. */ }
  }
  private retire(resource: Resource | null): void {
    if (!resource || resource.retired) return
    resource.retired = true; --resource.refs; this.close(resource)
  }
  private schedule(): void {
    if (this.timer) clearTimeout(this.timer)
    if (this.users.size && this.state.live) this.timer = setTimeout(() => void this.refresh(), 2000)
  }
  retain(): () => void {
    const token = Symbol(); this.users.add(token)
    if (this.users.size === 1) {
      this.off = this.client.onInvalidated?.(change => {
        if (!change || change.resync) { this.stop(); this.publish({ data: null, error: null, reportedStatus: null }) }
        void this.refresh()
      })
      void this.refresh()
    }
    return () => { if (this.users.delete(token) && this.users.size === 0) { this.off?.(); this.off = undefined; this.stop() } }
  }
  stop(): void {
    if (this.timer) clearTimeout(this.timer)
    this.timer = undefined
    const flight = this.flight; this.flight = null
    flight?.signal.abort(); this.retire(flight?.resource ?? null)
    this.retire(this.current); this.current = null
    this.publish({ loading: false, live: false })
  }
  dispose(): void { this.users.clear(); this.off?.(); this.off = undefined; this.stop(); this.publish({ data: null, error: null, reportedStatus: null }); this.listeners.clear() }
  /** A refresh can retire the parent snapshot while the child start RPC is on
   * the wire. Hold only that parent resource until Core accepts the child. */
  private async childLease(sourceId: string, signal: AbortSignal): Promise<{ parentReadId: string; descendantReference: string; release(): void }> {
    if (!this.current || performance.now() - this.current.publishedAt > 240_000) await cancellable(() => this.refresh(), signal)
    check(signal)
    const resource = this.current, child = this.state.data?.child_references.find(item => item.source_id === sourceId)
    if (!resource || resource.retired || !child) failed('native_child.child_unavailable')
    ++resource!.refs
    let released = false
    return { parentReadId: resource!.id, descendantReference: child!.reference, release: () => {
      if (!released) { released = true; --resource!.refs; this.close(resource!) }
    } }
  }
  child(sourceId: string): NativeReaderTarget | null {
    return this.state.data?.child_references.some(item => item.source_id === sourceId)
      ? { reference: { ...this.target.reference }, descendant: { parent: this, sourceId } } : null
  }
  refresh(): Promise<void> {
    if (this.flight) return this.flight.promise
    const flight: Flight = { signal: new AbortController(), resource: this.resource(), promise: Promise.resolve() }
    this.flight = flight
    this.publish({ loading: true, error: null })
    flight.promise = this.load(flight).catch(error => {
      if (this.flight === flight && !flight.signal.signal.aborted) this.publish({ error: error instanceof Error ? error.message : 'native_child.read_failed' })
    }).finally(() => {
      if (this.flight === flight) { this.flight = null; this.publish({ loading: false }); this.schedule() }
      if (this.current !== flight.resource) this.retire(flight.resource)
    })
    return flight.promise
  }
  private async load(flight: Flight): Promise<void> {
    const signal = flight.signal.signal
    let lease: Awaited<ReturnType<NativeChildReader['childLease']>> | undefined
    const request: { readId: string; reference: NativeReaderTarget['reference']; descendant?: { parentReadId: string; descendantReference: string } } = { readId: flight.resource.id, reference: { ...this.target.reference } }
    try {
      if (this.target.descendant) {
        lease = await this.target.descendant.parent.childLease(this.target.descendant.sourceId, signal)
        request.descendant = { parentReadId: lease.parentReadId, descendantReference: lease.descendantReference }
      }
      let view = await cancellable(() => this.client.request<NativeChildReadView>(this.method('start'), request), signal)
      lease?.release(); lease = undefined
      const generation = view.coreGeneration
      for (;;) {
        if (view.readId !== request.readId || typeof generation !== 'string' || !generation || view.coreGeneration !== generation
          || !['reading', 'ready', 'failed'].includes(view.state)) failed('native_child.read_response_invalid')
        if (view.state === 'failed') failed(typeof view.code === 'string' ? view.code : 'native_child.read_failed')
        if (view.state === 'ready') break
        await delay(signal)
        view = await cancellable(() => this.client.request<NativeChildReadView>(this.method('poll'), { readId: request.readId }), signal)
      }
      if (typeof view.snapshotReference !== 'string' || !view.snapshotReference) failed('native_child.read_response_invalid')
      const hash = sha256.create(), fragments: string[] = [], cursors = new Set<string>(), encoder = new TextEncoder()
      let offset = 0, total: number | null = null, digest: string | null = null, source: string | null = null, cursor: string | null = null
      let live = false, reportedStatus = 'unknown'
      try {
        for (;;) {
          const page: NativeChildReadPage = await cancellable(() => this.client.request<NativeChildReadPage>(this.method('page'), {
            readId: request.readId, snapshotReference: view.snapshotReference, cursor, maxBytes: 32768
          }), signal)
          if (!page || typeof page.fragment !== 'string') failed('native_child.page_invalid')
          const bytes = encoder.encode(page.fragment)
          if (page.snapshotReference !== view.snapshotReference || !count(page.offset) || page.offset !== offset
            || !count(page.totalBytes) || !page.totalBytes || !bytes.length || bytes.length > 32768
            || !digestString(page.projectionDigest) || !digestString(page.sourceDigest) || typeof page.complete !== 'boolean'
            || typeof page.live !== 'boolean' || typeof page.reportedStatus !== 'string') failed('native_child.page_invalid')
          total ??= page.totalBytes; digest ??= page.projectionDigest; source ??= page.sourceDigest
          if (total !== page.totalBytes || digest !== page.projectionDigest || source !== page.sourceDigest
            || offset + bytes.length > total) failed('native_child.page_changed')
          hash.update(bytes); fragments.push(page.fragment); offset += bytes.length
          live = page.live; reportedStatus = page.reportedStatus
          if (page.complete) { if (page.nextCursor !== null || offset !== total) failed('native_child.page_incomplete'); break }
          if (offset >= total || typeof page.nextCursor !== 'string' || !page.nextCursor || cursors.has(page.nextCursor)) failed('native_child.cursor_repeated')
          cursor = page.nextCursor; cursors.add(cursor)
        }
        const actual = Array.from(hash.digest(), byte => byte.toString(16).padStart(2, '0')).join('')
        if (actual !== digest) failed('native_child.digest_mismatch')
        check(signal)
        const data = nativeTranscript(JSON.parse(fragments.join('')))
        check(signal)
        if (this.flight !== flight) return
        const previous = this.current; this.current = flight.resource; flight.resource.publishedAt = performance.now()
        this.publish({ data, live, reportedStatus, error: null }); this.retire(previous)
      } finally { hash.destroy() }
    } finally { lease?.release() }
  }
}
