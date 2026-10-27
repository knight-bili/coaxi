import type { DelegationDetail } from '@contracts'
import type { CoreDelegationCardModel } from './coaxi-native-content/ui/source-delegation-host'

/** Metadata from the already-authorized, exact-parent Core list only. No child
 * body prefetch or report/task-id search across conversations. */
export function coreDelegationCardModel(detail: DelegationDetail): CoreDelegationCardModel {
  const status = detail.blockedOn || detail.cleanupPending ? 'waiting'
    : detail.status === 'queued' ? 'starting'
      : detail.status === 'running' ? 'running'
        : detail.status === 'waiting' ? 'waiting'
          : detail.status === 'succeeded' ? 'ok' : 'err'
  return { agentType: detail.agentType, task: null, taskId: detail.delegationId, status,
    errorCode: detail.status === 'cancelled' ? 'canceled' : !['queued', 'running', 'waiting', 'succeeded', 'failed'].includes(detail.status) ? 'unknown' : undefined,
    childConversationId: detail.conversationId, childConnectionId: null, hasModel: true }
}
