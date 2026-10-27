import type { DelegationRuntimeTarget } from '@contracts'
import { CoaxiRuntimeMark } from '../../CoaxiRuntimeMark'
import { delegationRuntimeLabel } from '../../delegation-viewer-controller'
import type { AgentType } from '../lib/types'

// Same source Runtime marks and the existing Core Runtime catalog labels.
export function AgentIcon({ agentType, className }: { agentType: AgentType; className?: string }): React.JSX.Element {
  return <span className={className}><CoaxiRuntimeMark target={agentType as DelegationRuntimeTarget} size={20} /></span>
}
export function getAgentLabel(agentType: AgentType): string {
  return delegationRuntimeLabel(agentType as DelegationRuntimeTarget)
}
