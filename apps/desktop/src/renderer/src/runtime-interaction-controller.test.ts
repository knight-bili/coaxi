import { describe, expect, it, vi } from 'vitest'
import type { ResolveRuntimeInteractionParams, RuntimeInteractionView, StoredCommandResult } from '@contracts'
import type { ThreadClient } from './camp-client'
import { RuntimeInteractionController, runtimeInteractionController } from './runtime-interaction-controller'

const question = (conversationId = 'private-a', id = 'question-a'): RuntimeInteractionView => ({
  id, conversationId, kind: 'question', status: 'pending', version: 1, answer: null, closeReason: null,
  body: { kind: 'question', questions: [{ id: 'q1', question: '原问题', header: '范围', multiSelect: false, options: [{ label: 'A (Recommended)', description: '' }] }] }
})
const result = (params: ResolveRuntimeInteractionParams, status: StoredCommandResult['status'] = 'applied'): StoredCommandResult => ({
  commandId: params.commandId, commandType: 'runtime.interactions.resolve', status, code: status === 'applied' ? 'interaction.resolved' : 'interaction.stale',
  payload: {}, resultEntity: null, recordedAt: '2026-10-18T00:00:00Z'
})
type Client = Pick<ThreadClient, 'request' | 'onEvent' | 'onInvalidated' | 'retryPendingCommand'>
const settle = async (): Promise<void> => { for (let n = 0; n < 20; n++) await Promise.resolve() }
describe('native interaction User controller', () => {
  it('scans older metadata but reads only the opened body, never mixes a private or Host scope', async () => {
    const calls: Array<[string, unknown]> = []
    const client: Client = { request: vi.fn(async (method, params) => {
      calls.push([method, params])
      if (method === 'runtime.interactions.list') return { conversationId: 'private-a', items: (params as { cursor: string | null }).cursor
        ? [{ ...question(), createdAt: '2026-10-17' }] : Array.from({ length: 50 }, (_, n) => ({ id: `old-${n}`, kind: 'question', status: 'closed', version: 2, createdAt: '2026-10-18' })),
        nextCursor: (params as { cursor: string | null }).cursor ? null : 'old-49' }
      return question('private-b')
    }) as Client['request'] }
    const controller = runtimeInteractionController(client, 'private-a')
    expect(controller).toBe(runtimeInteractionController(client, 'private-a'))
    expect(controller).not.toBe(runtimeInteractionController({ ...client }, 'private-a'))
    expect(controller).not.toBe(runtimeInteractionController(client, 'private-b'))
    await controller.refresh()
    expect(controller.getSnapshot().summaries).toHaveLength(51)
    expect(calls.map(([method]) => method)).toEqual(['runtime.interactions.list', 'runtime.interactions.list'])
    await controller.open('question-a')
    expect(controller.getSnapshot().error).toBe(true)
    expect(controller.getSnapshot().views).toEqual({})
  })

  it('freezes exact labels, Other, UUID and version across unknowns; never auto redispatches', async () => {
    let view = question()
    let committed: StoredCommandResult | undefined
    const commands: ResolveRuntimeInteractionParams[] = []
    const client: Client = { request: vi.fn(async (method, params) => {
      if (method === 'runtime.interactions.list') return { conversationId: view.conversationId, items: [{ ...view, createdAt: '' }], nextCursor: null }
      if (method === 'runtime.interactions.get') return structuredClone(view)
      if (method === 'commands.reconcile') return committed ? { state: 'recorded', result: committed } : { state: 'unknown' }
      const command = structuredClone(params as ResolveRuntimeInteractionParams); commands.push(command)
      if (commands.length === 1) throw new Error('transport lost')
      view = { ...view, answer: command.command.answer, status: 'resolved', version: 2 }
      committed = result(command)
      return committed
    }) as Client['request'] }
    const controller = new RuntimeInteractionController(client, view.conversationId)
    await controller.refresh(); await controller.open(view.id)
    const answer = { kind: 'question' as const, declined: false, answers: [{ questionId: 'q1', labels: ['A (Recommended)', '保留, 原文'] }] }
    controller.answer(view.id, answer)
    answer.answers[0].labels[0] = '不能污染原提交'
    controller.answer(view.id, { kind: 'question', declined: true, answers: [] })
    await settle()
    expect(commands).toHaveLength(1)
    await controller.recover(view.id)
    expect(commands).toHaveLength(1)
    expect(controller.getSnapshot().intents[view.id].unknown).toBe(true)
    await controller.recover(view.id, true)
    expect(commands).toHaveLength(2)
    expect(commands[1]).toEqual(commands[0])
    expect(commands[1].command.answer).toEqual({ ...answer, answers: [{ questionId: 'q1', labels: ['A (Recommended)', '保留, 原文'] }] })
    expect(controller.getSnapshot().intents).toEqual({})
    expect(controller.getSnapshot().views[view.id].status).toBe('resolved')
    controller.answer(view.id, answer)
    expect(commands).toHaveLength(2)
  })

  it('settles the retained Web promise by read receipt without creating a second command', async () => {
    let view = question()
    let params: ResolveRuntimeInteractionParams | undefined
    let resolveOriginal: (value: StoredCommandResult) => void = () => {}
    const request: Client['request'] = vi.fn(async (method, input) => {
      if (method === 'runtime.interactions.list') return { conversationId: view.conversationId, items: [{ ...view, createdAt: '' }], nextCursor: null }
      if (method === 'runtime.interactions.get') return structuredClone(view)
      if (method === 'commands.reconcile') {
        const receipt = result(params!)
        view = { ...view, version: 2, status: 'resolved', answer: params!.command.answer }
        resolveOriginal(receipt)
        return { state: 'recorded', result: receipt }
      }
      params = structuredClone(input as ResolveRuntimeInteractionParams)
      return await new Promise<StoredCommandResult>(resolve => { resolveOriginal = resolve })
    }) as Client['request']
    const controller = new RuntimeInteractionController({ request }, view.conversationId)
    await controller.refresh(); await controller.open(view.id)
    controller.answer(view.id, { kind: 'question', declined: true, answers: [] })
    await settle()
    expect(controller.getSnapshot().intents[view.id].unknown).toBe(false)
    await controller.recover(view.id, true); await settle()
    expect(controller.getSnapshot().intents).toEqual({})
    expect((request as ReturnType<typeof vi.fn>).mock.calls.filter(call => call[0] === 'runtime.interactions.resolve')).toHaveLength(1)
  })

  it('retains a rejected answer as editable, ignores older bodies and unsubscribes hidden surfaces', async () => {
    let view = question()
    let emit: Parameters<NonNullable<Client['onEvent']>>[0] = () => {}
    const release = vi.fn()
    const client: Client = { onEvent: listener => { emit = listener; return release }, request: vi.fn(async (method, params) => {
      if (method === 'runtime.interactions.list') return { conversationId: view.conversationId, items: [{ ...view, createdAt: '' }], nextCursor: null }
      if (method === 'runtime.interactions.get') return structuredClone(view)
      return result(params as ResolveRuntimeInteractionParams, 'rejected')
    }) as Client['request'] }
    const controller = new RuntimeInteractionController(client, view.conversationId)
    const stop = controller.retain()
    await controller.refresh(); await controller.open(view.id)
    controller.answer(view.id, { kind: 'question', answers: [], declined: true }); await settle()
    expect(controller.getSnapshot().decisionError).toBe(view.id)
    expect(controller.getSnapshot().intents).toEqual({})
    view = { ...view, status: 'closed', version: 3, closeReason: 'core_restarted' }
    emit({ method: 'runtime.interaction.changed', params: { conversationId: view.conversationId, interactionId: view.id, version: 3 } })
    await settle()
    expect(controller.getSnapshot().views[view.id].status).toBe('closed')
    view = { ...view, status: 'pending', version: 1 }
    await controller.open(view.id)
    expect(controller.getSnapshot().views[view.id].version).toBe(3)
    stop(); expect(release).toHaveBeenCalledOnce()
  })
})
