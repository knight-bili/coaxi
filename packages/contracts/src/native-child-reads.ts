/** User-only Core reference. Native addresses and paths are never inputs. */
export interface NativeChildReadReference {
  threadId: string
  agentRunId: string
  evidenceId: string
  receiverIndex: number
}

/** Legacy Agent files have independent Core authority, not a Session receiver. */
export interface NativeAgentReadReference {
  threadId: string
  agentRunId: string
  evidenceId: string
}

export interface NativeAgentReadStart {
  readId: string
  reference: NativeAgentReadReference
}

export interface NativeChildReadStart {
  readId: string
  reference: NativeChildReadReference
  descendant?: { parentReadId: string; descendantReference: string } | null
}

export interface NativeChildReadView {
  readId: string
  coreGeneration: string
  state: 'reading' | 'ready' | 'failed'
  snapshotReference: string | null
  code: string | null
}

export interface NativeChildReadPageRequest {
  readId: string
  snapshotReference: string
  cursor?: string | null
  maxBytes?: number | null
}

/** Reassemble by UTF-8 byte offset and verify projectionDigest before JSON.parse. */
export interface NativeChildReadPage {
  snapshotReference: string
  projectionDigest: string
  sourceDigest: string
  offset: number
  totalBytes: number
  fragment: string
  nextCursor: string | null
  complete: boolean
  reportedStatus: string
  live: boolean
}
