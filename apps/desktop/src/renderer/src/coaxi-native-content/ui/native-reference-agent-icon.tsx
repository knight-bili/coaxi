import { CoaxiRuntimeMark } from '../../CoaxiRuntimeMark'
import type { DelegationRuntimeTarget } from '@contracts'
import type { AgentType } from '../lib/types'
export function AgentIcon({ agentType, className }: { agentType: AgentType; className?: string }): React.JSX.Element {
  return <span className={className}><CoaxiRuntimeMark target={agentType as DelegationRuntimeTarget} size={14} /></span>
}
