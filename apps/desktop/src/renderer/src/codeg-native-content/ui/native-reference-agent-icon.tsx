import { CodegRuntimeMark } from '../../CodegRuntimeMark'
import type { DelegationRuntimeTarget } from '@contracts'
import type { AgentType } from '../lib/types'
export function AgentIcon({ agentType, className }: { agentType: AgentType; className?: string }): React.JSX.Element {
  return <span className={className}><CodegRuntimeMark target={agentType as DelegationRuntimeTarget} size={14} /></span>
}
