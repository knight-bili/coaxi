// Fixed Codeg aff1e6c; original declaration-only types, Apache-2.0.
import type { AgentType } from "../lib/types"



export type DelegationStatus = "running" | "ok" | "err"


export interface DelegationBinding {
  parentConnectionId: string
  parentToolUseId: string
  childConnectionId: string
  childConversationId: number
  agentType: AgentType
  status: DelegationStatus
  errorCode?: string
  /** Bounded task text from `delegation_started`. The card's fallback when
   *  the tool call's `raw_input` never carried the arguments (Cursor's
   *  identity-less announcements). */
  task: string | null
  /** Broker-minted task id from `delegation_started`. */
  taskId: string | null
}
