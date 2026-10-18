import type { DelegationDetail } from './index'

export interface DelegationToolSourceParams { threadId: string; evidenceId: string }
export interface DelegationToolSourceIdentity {
  threadId: string
  evidenceId: string
  agentRunId: string
  executionEpoch: number
  parentConversationId: string
}
export type DelegationToolSourceView = { state: 'unresolved'; source: DelegationToolSourceIdentity }
  | { state: 'refused'; source: DelegationToolSourceIdentity; operation: 'delegation.start' | 'delegation.resume'; code: string }
  | { state: 'bound'; source: DelegationToolSourceIdentity; operation: 'delegation.start' | 'delegation.resume';
      attemptNo: number; detail: DelegationDetail; task: string; reason: string | null }
