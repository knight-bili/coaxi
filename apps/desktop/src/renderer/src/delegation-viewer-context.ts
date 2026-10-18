import { createContext } from 'react'
import type { ActionApprovalView, AgentProfile, ThreadMemberView } from '@contracts'
import type { DelegationViewerTarget } from './delegation-viewer-controller'

export interface DelegationViewerEnvironment {
  threadId: string
  members?: ThreadMemberView[]
  profileById: Map<string, AgentProfile>
  busy: boolean
  onResolveApproval(approval: ActionApprovalView, optionId: string): void
  onNotify?(message: string): void
}
export const DelegationViewerContext = createContext<{
  open(target: DelegationViewerTarget, trigger?: HTMLElement): void
  sourceVisibility(parent: string, visible: boolean): void
  visible: boolean
  environment: DelegationViewerEnvironment
} | null>(null)
