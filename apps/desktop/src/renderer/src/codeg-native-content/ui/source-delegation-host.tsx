import { createContext, useContext, useEffect, type ReactNode } from 'react'
import type { DelegationCardStatus } from '../lib/delegation-card'
import type { AgentType } from '../lib/types'
import type { ToolCallState } from '../lib/adapters/ai-elements-adapter'

export interface DelegationCardSource {
  parentToolUseId: string
  input?: string | null; output?: string | null; errorText?: string | null
  state?: ToolCallState; meta?: Record<string, unknown> | null; taskIdHint?: string | null
}
export interface CoreDelegationCardModel {
  agentType: AgentType | null; task: string | null; taskId: string | null
  status: DelegationCardStatus; errorCode?: string
  childConversationId: string | null; childConnectionId: null; hasModel: boolean
}
type SourceTarget = { kind: 'delegation'; source: DelegationCardSource }
type SourceHost = { sourceId: string; model: CoreDelegationCardModel; open(target: SourceTarget): void }
const Context = createContext<SourceHost | null>(null)
export function SourceDelegationProvider({ value, children }: { value: SourceHost; children: ReactNode }): React.JSX.Element {
  return <Context.Provider value={value}>{children}</Context.Provider>
}
function useHost(): SourceHost {
  const host = useContext(Context)
  if (!host) throw new Error('Source delegation cards require their scoped Core model and viewer')
  return host
}
// The original presenter stays intact. Core's UUID and current status replace
// the donor's numeric DB identity and global connection store, without coercion.
export function useDelegationCardModel(source: DelegationCardSource): CoreDelegationCardModel {
  const host = useHost()
  if (source.parentToolUseId !== host.sourceId || source.taskIdHint != null && source.taskIdHint !== host.model.taskId) throw new Error('delegation.viewer_scope_mismatch')
  return host.model
}
export function useSessionViewerHost(): Pick<SourceHost, 'open'> { return useHost() }
export function SubAgentSessionDialog({ open, onOpenChange, childConversationId }: {
  open: boolean; onOpenChange(open: boolean): void; childConversationId: string
  childConnectionId: string | null; agentType: AgentType | null; kickoffTask: string | null
}): null {
  const host = useHost()
  useEffect(() => {
    if (!open) return
    if (childConversationId !== host.model.childConversationId) throw new Error('delegation.viewer_scope_mismatch')
    host.open({ kind: 'delegation', source: { parentToolUseId: host.sourceId } })
    onOpenChange(false)
  }, [open, host, childConversationId, onOpenChange])
  return null
}
