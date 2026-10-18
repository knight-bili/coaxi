import { expect, it, vi } from 'vitest'
import type { StoredCommandResult } from '@contracts'
import { DelegationActionController, delegationActionController } from './delegation-action-controller'
import type { ThreadClient } from './camp-client'

const origin = { threadId: 'thread-a', parentConversationId: 'parent-a', delegationId: 'delegation-a', conversationId: 'child-a' }
function fixture() {
  const values = new Map<string, unknown>(), receipts = new Map<string, StoredCommandResult>()
  let lose: 'before' | 'after' | null = null, failSave = false, failClear = false, wrongReceipt = false, effects = 0
  const storage = { get: (key: string) => structuredClone(values.get(key)), set: (key: string, value: unknown) => {
    if (failSave || failClear && value === null) throw Error('storage denied')
    values.set(key, structuredClone(value))
  } }
  const request = vi.fn(async (method: string, params: unknown): Promise<unknown> => {
    const input = params as { commandId: string; operation?: string; params?: { commandId: string } }
    if (method === 'commands.reconcile') { const result = receipts.get(input.params!.commandId); return result ? { state: 'recorded', result } : { state: 'unknown' } }
    if (lose === 'before') throw Error('disconnected')
    let result = receipts.get(input.commandId)
    if (!result) { effects++; result = { commandId: input.commandId, commandType: method, status: 'applied', code: 'applied', payload: {}, resultEntity: null, recordedAt: 'now' }; receipts.set(input.commandId, result) }
    if (lose === 'after') throw Error('disconnected')
    return wrongReceipt ? { ...result, commandType: 'delegations.cancel' } : result
  })
  const client = { request: request as ThreadClient['request'], editingRecovery: storage }
  return { client, request, values, receipts, controller: new DelegationActionController(client, origin),
    effects: () => effects, lose: (mode: typeof lose) => { lose = mode }, failSave: (next: boolean) => { failSave = next }, failClear: (next: boolean) => { failClear = next }, wrongReceipt: () => { wrongReceipt = true } }
}
it('读取状态不控制执行，首次操作保存确切身份和完整参数再派发', async () => {
  const f = fixture()
  expect(f.controller.getSnapshot().pending).toBe(false)
  expect(f.request).not.toHaveBeenCalled()
  const command = { delegationId: origin.delegationId, attemptNo: 7 }
  const pending = f.controller.run('delegations.cancel', command)
  command.attemptNo = 9
  await pending
  expect(f.request).toHaveBeenCalledExactlyOnceWith('delegations.cancel', { commandId: expect.any(String), command: { delegationId: origin.delegationId, attemptNo: 7 } })
  expect(f.controller.getSnapshot()).toMatchObject({ pending: false, busy: false, result: { commandType: 'delegations.cancel' } })
})
it('丢回执后只核对原UUID，不产生第二次控制；pending拒绝新操作', async () => {
  const f = fixture(); f.lose('after')
  await expect(f.controller.run('delegations.takeover', { delegationId: origin.delegationId, expectedControlVersion: 4 })).rejects.toThrow('disconnected')
  const original = structuredClone(f.request.mock.calls[0][1])
  await expect(f.controller.run('delegations.cancel', { delegationId: origin.delegationId, attemptNo: 1 })).rejects.toThrow('control_pending')
  await f.controller.reconcile()
  expect(f.request).toHaveBeenLastCalledWith('commands.reconcile', { operation: 'delegations.takeover', params: original })
  expect(f.effects()).toBe(1)
  expect(f.controller.getSnapshot().pending).toBe(false)
})
it('重启后的未知操作显式重试保留原UUID/版本/原因；身份属性顺序不改变恢复位置', async () => {
  const f = fixture(); f.lose('before')
  await expect(f.controller.run('delegations.resume', { delegationId: origin.delegationId, expectedAttemptNo: 3, reason: '原始中断说明' })).rejects.toThrow()
  const original = structuredClone(f.request.mock.calls[0][1])
  const restored = new DelegationActionController({ ...f.client }, { conversationId: 'child-a', delegationId: 'delegation-a', parentConversationId: 'parent-a', threadId: 'thread-a' })
  await restored.reconcile()
  expect(restored.getSnapshot().unknown).toBe(true)
  expect(f.request.mock.calls.filter(([method]) => method === 'delegations.resume')).toHaveLength(1)
  f.lose(null); await restored.retry()
  expect(f.request.mock.calls.filter(([method]) => method === 'delegations.resume').map(([, params]) => params)).toEqual([original, original])
  expect(f.effects()).toBe(1)
  expect(restored.getSnapshot().pending).toBe(false)
})
it('错误回执类型不能清原操作，仍可从Core核对正确回执', async () => {
  const f = fixture(); f.wrongReceipt()
  await expect(f.controller.run('delegations.conversation.end', { conversationId: origin.conversationId })).rejects.toThrow('recovery_invalid')
  expect(f.controller.getSnapshot().pending).toBe(true)
  await f.controller.reconcile()
  expect(f.effects()).toBe(1)
  expect(f.controller.getSnapshot().result?.commandType).toBe('delegations.conversation.end')
})
it('不能保存原操作时没有派发；已保存回执但清理本机失败时重启不再派发或查询', async () => {
  const f = fixture(); f.failSave(true)
  await expect(f.controller.run('delegations.cancel', { delegationId: origin.delegationId, attemptNo: 1 })).rejects.toThrow('control_not_saved')
  expect(f.request).not.toHaveBeenCalled()
  f.failSave(false); f.failClear(true)
  await expect(f.controller.run('delegations.cancel', { delegationId: origin.delegationId, attemptNo: 1 })).rejects.toThrow('storage denied')
  const restored = new DelegationActionController({ ...f.client }, origin)
  f.failClear(false); await restored.reconcile()
  expect(f.request).toHaveBeenCalledTimes(1)
  expect(restored.getSnapshot().pending).toBe(false)
})
it('拒绝错误子会话/Thread覆盖/超限说明；持久身份被篡改不能恢复', async () => {
  const f = fixture()
  expect(() => f.controller.run('delegations.conversation.end', { conversationId: 'other' })).toThrow('recovery_invalid')
  expect(() => f.controller.run('delegations.cancel', { delegationId: origin.delegationId, attemptNo: 1, threadId: 'other' })).toThrow()
  expect(() => f.controller.run('delegations.resume', { delegationId: origin.delegationId, expectedAttemptNo: 1, reason: '字'.repeat(683) })).toThrow()
  expect(f.request).not.toHaveBeenCalled()
  f.lose('before'); await expect(f.controller.run('delegations.cancel', { delegationId: origin.delegationId, attemptNo: 1 })).rejects.toThrow()
  const [key, value] = [...f.values.entries()][0]
  f.values.set(key, { ...value as object, origin: { ...origin, parentConversationId: 'other' } })
  const restored = new DelegationActionController(f.client, origin)
  await expect(restored.reconcile()).rejects.toThrow('recovery_invalid')
  expect(f.request).toHaveBeenCalledTimes(1)
})
it('同一身份共享控制器，换Host或父会话不共用控制状态', () => {
  const a = fixture(), b = fixture()
  const first = delegationActionController(a.client, origin)
  expect(delegationActionController(a.client, { ...origin })).toBe(first)
  expect(delegationActionController(b.client, origin)).not.toBe(first)
  expect(delegationActionController(a.client, { ...origin, parentConversationId: 'other' })).not.toBe(first)
})
