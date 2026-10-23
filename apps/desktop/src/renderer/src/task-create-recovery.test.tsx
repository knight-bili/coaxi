// @vitest-environment jsdom

import { createElement } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import type { CoreMethod, StoredCommandResult, ThreadSnapshot } from '@contracts'
import { ThreadClientProvider, type ThreadClient } from './camp-client'
import { desktopThreadClient } from './desktop-camp-client'
import { TaskPanel } from './ThreadWorkspace'

afterEach(cleanup)

describe('TaskPanel 创建恢复', () => {
  it('keeps an uncertain Task create retry with its original Host while a late result leaves the new Host editor intact', async () => {
    const recoveryRecords = new Map<string, unknown>()
    const recovery: NonNullable<ThreadClient['workbenchEditorRecovery']> = {
      get: vi.fn(async (key: string) => recoveryRecords.get(key) ?? null),
      put: vi.fn(async (value: unknown) => {
        const record = value as { key?: unknown }
        if (typeof record.key === 'string') recoveryRecords.set(record.key, value)
      }),
      copies: vi.fn(async (_key: string, _after: string | null) => ({ items: [], next: null }))
    }
    const installRecovery = (client: ThreadClient): void => {
      Object.defineProperty(client, 'workbenchEditorRecovery', {
        configurable: true, enumerable: true, value: recovery
      })
    }
    const { client: clientA, request: requestA, rejectFirst, resolveRetry } = (() => {
      let rejectFirst!: (error: Error) => void
      let resolveRetry!: (result: StoredCommandResult) => void
      const firstAttempt = new Promise<StoredCommandResult>((_resolve, reject) => { rejectFirst = reject })
      const retryAttempt = new Promise<StoredCommandResult>((resolve) => { resolveRetry = resolve })
      const attempts = [firstAttempt, retryAttempt]
      const request = vi.fn(async (method: CoreMethod, _params?: unknown) => {
        if (method !== 'tasks.create') throw new Error(`Unexpected TaskPanel request: ${method}`)
        const attempt = attempts.shift()
        if (!attempt) throw new Error('Unexpected additional Task create request')
        return attempt
      })
      const client = Object.create(desktopThreadClient) as ThreadClient
      Object.assign(client, {
        request: request as unknown as ThreadClient['request'],
        presentationScope: 'host-old',
        canvasCoreIdentity: async () => 'core-old'
      })
      installRecovery(client)
      return { client, request, rejectFirst, resolveRetry }
    })()
    const requestB = vi.fn(async (method: CoreMethod) => {
      throw new Error(`New Host unexpectedly received ${method}`)
    })
    const clientB = Object.create(desktopThreadClient) as ThreadClient
    Object.assign(clientB, {
      request: requestB as unknown as ThreadClient['request'],
      presentationScope: 'host-new',
      canvasCoreIdentity: async () => 'core-new'
    })
    installRecovery(clientB)
    const resultFor = (commandId: string): StoredCommandResult => ({
      commandId, commandType: 'tasks.create', status: 'accepted', code: 'task.created', payload: {},
      resultEntity: { entityType: 'task', entityId: 'task-created' }, recordedAt: '2026-10-23T00:00:00Z'
    })
    const snapshot: ThreadSnapshot = {
      schemaVersion: 35, throughGlobalSequence: 1,
      thread: {
        id: 'camp-task-owner', title: 'Owner 切换', activationState: 'active', projectBindingKind: 'quick_chat', projectPath: '/quick-chat',
        defaultLeadAgentId: 'agent_2', membershipGeneration: 1, version: 1,
        createdAt: '2026-07-23T00:00:00Z', updatedAt: '2026-07-23T00:00:00Z'
      },
      membershipReconciliations: [],
      members: [{
        agentId: 'agent_2', displayName: '沐瓦', teamRole: '开发者', avatarRef: null, accent: '#39777a',
        membershipStatus: 'active', leaveRequestedAt: null, profilePresence: 'present', memberOrder: 0,
        isDefaultLead: true, version: 1
      }],
      tasks: [], messages: [], messageDeliveries: [], turns: [], agentRuns: [], contextManifests: [],
      executionEvidence: [], agentRunFileChanges: [], approvals: [], actions: [], timeline: []
    }
    const refreshA = vi.fn(async () => undefined)
    const refreshB = vi.fn(async () => undefined)
    const ownerTree = (client: ThreadClient, onTasksChanged: () => Promise<void>) => createElement(
      ThreadClientProvider,
      { client, children: createElement(TaskPanel, { snapshot, busy: false, onTasksChanged }) }
    )
    const view = render(ownerTree(clientA, refreshA))
    const openCreate = async (): Promise<void> => {
      if (!view.container.querySelector('form.task-editor')) {
        fireEvent.click(view.getByRole('button', { name: /新建/ }))
      }
      await waitFor(() => expect(view.getByLabelText('标题')).toBeTruthy())
    }
    const fillCreate = (title: string, description: string): void => {
      fireEvent.change(view.getByLabelText('标题'), { target: { value: title } })
      fireEvent.change(view.getByLabelText('责任范围与要求'), { target: { value: description } })
      fireEvent.change(view.getByLabelText('负责人'), { target: { value: 'agent_2' } })
    }
    const submitCreate = (): void => {
      const form = view.container.querySelector('form.task-editor')
      if (!form) throw new Error('Task create form is missing')
      fireEvent.submit(form)
    }
    const oldRecoveryKey = 'task-create:' + JSON.stringify(['host-old', 'core-old', snapshot.thread.id])

    await openCreate()
    await waitFor(() => expect((view.getByLabelText('标题') as HTMLInputElement).disabled).toBe(false))
    fillCreate('旧 Host 标题 ', '旧 Host 责任范围 ')
    submitCreate()
    await waitFor(() => expect(requestA).toHaveBeenCalledTimes(1))
    const frozenRequest = requestA.mock.calls[0]?.[1] as {
      commandId: string; threadId: string; title: string; description: string; assigneeAgentId: string
    }
    expect(frozenRequest).toMatchObject({
      threadId: snapshot.thread.id, title: '旧 Host 标题', description: '旧 Host 责任范围', assigneeAgentId: 'agent_2'
    })
    expect(recoveryRecords.get(oldRecoveryKey)).toMatchObject({
      key: oldRecoveryKey, editorId: 'task-create', pending: { request: frozenRequest }
    })

    // TaskPanel itself is not keyed by Thread ID; this changes Host/Core while keeping the same Thread.
    view.rerender(ownerTree(clientB, refreshB))
    await waitFor(() => expect(view.queryByRole('dialog')).toBeNull())
    await openCreate()
    await waitFor(() => expect((view.getByLabelText('标题') as HTMLInputElement).disabled).toBe(false))
    fillCreate('新 Host 当前标题', '新 Host 当前责任范围')
    expect((view.getByLabelText('标题') as HTMLInputElement).value).toBe('新 Host 当前标题')
    await act(async () => { rejectFirst(new Error('连接中断')) })
    expect(view.queryByRole('alert')).toBeNull()
    expect((view.getByLabelText('标题') as HTMLInputElement).value).toBe('新 Host 当前标题')

    view.rerender(ownerTree(clientA, refreshA))
    await waitFor(() => expect(view.queryByRole('dialog')).toBeNull())
    await openCreate()
    await waitFor(() => expect(view.getByText(/上次创建结果待核对/)).toBeTruthy())
    expect((view.getByLabelText('标题') as HTMLInputElement).value).toBe('旧 Host 标题')
    expect((view.getByLabelText('责任范围与要求') as HTMLTextAreaElement).value).toBe('旧 Host 责任范围')
    submitCreate()
    await waitFor(() => expect(requestA).toHaveBeenCalledTimes(2))
    expect(requestA.mock.calls[1]?.[1]).toEqual(frozenRequest)
    expect(requestB).not.toHaveBeenCalled()

    view.rerender(ownerTree(clientB, refreshB))
    await waitFor(() => expect(view.queryByRole('dialog')).toBeNull())
    await openCreate()
    await waitFor(() => expect((view.getByLabelText('标题') as HTMLInputElement).disabled).toBe(false))
    fillCreate('新 Host 编辑后的标题', '新 Host 编辑后的责任范围')
    await act(async () => { resolveRetry(resultFor(frozenRequest.commandId)) })
    expect((view.getByLabelText('标题') as HTMLInputElement).value).toBe('新 Host 编辑后的标题')
    expect((view.getByLabelText('责任范围与要求') as HTMLTextAreaElement).value).toBe('新 Host 编辑后的责任范围')
    expect(view.queryByRole('dialog')).not.toBeNull()
    expect(refreshB).not.toHaveBeenCalled()
    expect(requestB).not.toHaveBeenCalled()
    expect(recoveryRecords.get(oldRecoveryKey)).toMatchObject({ key: oldRecoveryKey, pending: null })
  })

})
