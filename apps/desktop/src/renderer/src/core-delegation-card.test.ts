import { describe, expect, it } from 'vitest'
import type { DelegationDetail } from '@contracts'
import { coreDelegationCardModel } from './core-delegation-card'

const detail = (status: string, blockedOn?: DelegationDetail['blockedOn']): DelegationDetail => ({
  delegationId: 'task-uuid', threadId: 'thread-a', parentConversationId: 'parent-a', conversationId: 'child-uuid',
  status, agentType: 'codex', blockedOn, currentAttemptNo: 1, cleanupPending: false,
} as DelegationDetail)

describe('Core delegation metadata to original presenter', () => {
  it('retains opaque child identity without numeric conversion or report-derived paths', () => {
    const model = coreDelegationCardModel(detail('running'))
    expect(model).toMatchObject({ taskId: 'task-uuid', childConversationId: 'child-uuid', childConnectionId: null, task: null, status: 'running' })
  })
  it.each(['permission', 'question', 'plan_approval'] as const)('keeps %s blocking ahead of an old success state', kind => {
    const blockedOn: DelegationDetail['blockedOn'] = kind === 'permission' ? { kind, request_id: 'request-a', title: 'approval' } : { kind, request_id: 'request-a' }
    expect(coreDelegationCardModel(detail('succeeded', blockedOn)).status).toBe('waiting')
  })
  it('does not present cancelled or unknown tasks as successful', () => {
    expect(coreDelegationCardModel(detail('cancelled'))).toMatchObject({ status: 'err', errorCode: 'canceled' })
    expect(coreDelegationCardModel(detail('future-status'))).toMatchObject({ status: 'err', errorCode: 'unknown' })
    expect(coreDelegationCardModel(detail('failed')).status).toBe('err')
  })
  it('keeps cleanup visible even when the execution has already settled', () => {
    expect(coreDelegationCardModel({ ...detail('cancelled'), takenOver: true, cleanupPending: true }).status).toBe('waiting')
    expect(coreDelegationCardModel(detail('queued')).status).toBe('starting')
    expect(coreDelegationCardModel(detail('succeeded')).status).toBe('ok')
  })
})
