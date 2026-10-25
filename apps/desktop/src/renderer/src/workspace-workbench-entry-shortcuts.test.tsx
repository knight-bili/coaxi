// @vitest-environment jsdom

import { createElement } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import type { CoreMethod } from '@contracts'
import { ThreadClientProvider, type ThreadClient } from './camp-client'
import { desktopThreadClient } from './desktop-camp-client'
import { WorkspaceWorkbenchEntry } from './WorkspaceWorkbenchEntry'
import { dispatchShortcutAction } from './shortcut-action-driver'

vi.mock('./WorkspaceWorkbench', async () => {
  const React = await import('react')
  return {
    WorkspaceWorkbench: (props: {
      threadId: string
      projectPath: string
      initiallyOpen?: boolean
      terminalRequest?: number
      onClosed?: () => void
    }) => React.createElement('section', {
      'data-testid': 'workbench',
      'data-thread': props.threadId,
      'data-root': props.projectPath,
      'data-tab': props.terminalRequest ? 'terminals' : 'files',
      'data-open': String(props.initiallyOpen),
    }, React.createElement('button', { type: 'button', onClick: props.onClosed }, '关闭工作区')),
  }
})

afterEach(cleanup)

function createClient(request: (method: CoreMethod, params?: unknown) => Promise<unknown>): ThreadClient {
  const client = Object.create(desktopThreadClient) as ThreadClient
  Object.defineProperty(client, 'request', { configurable: true, value: request as unknown as ThreadClient['request'] })
  Object.defineProperty(client, 'keyboardShortcuts', { configurable: true, value: undefined })
  return client
}

function ownerTree(client: ThreadClient, threadId: string) {
  return createElement(ThreadClientProvider, {
    client,
    children: createElement(WorkspaceWorkbenchEntry, {
      threadId,
      missionId: 'mission-current',
      projectPath: '/caller-path-must-not-be-used',
      members: [],
      bindLeaveGuard: () => undefined,
    }),
  })
}

describe('WorkspaceWorkbenchEntry 快捷键', () => {
  it('右上固定控制保持可达，放大、恢复与收起不重建编辑器', async () => {
    const host=document.createElement('span');host.dataset.engineeringEntry='thread-a';document.body.append(host)
    try{
      const view=render(ownerTree(createClient(vi.fn(async()=>({workspaceRoot:'/verified'}))),'thread-a'))
      fireEvent.click(view.getByRole('button',{name:'展开工程区'}))
      const editor=await view.findByTestId('workbench')
      const controls=view.getByRole('group',{name:'工程区显示'})
      expect(controls.parentElement).toBe(host)
      fireEvent.click(view.getByRole('button',{name:'放大工程区'}))
      expect(view.container.querySelector('.engineering-layout.is-maximized')).toBeTruthy()
      expect(host.querySelector('button[aria-label="恢复并排"]')).toBeTruthy()
      fireEvent.click(view.getByRole('button',{name:'恢复并排'}))
      fireEvent.click(view.getByRole('button',{name:'收起工程区'}))
      expect(editor.closest('aside')?.hidden).toBe(true)
      expect(host.querySelector('button[aria-label="展开工程区"]')).toBeTruthy()
      expect(view.getByTestId('workbench')).toBe(editor)
    }finally{host.remove()}
  })
  it('收起保留同一编辑器，重新展开核验目录，目录变更拒绝迁移旧草稿', async () => {
    const roots=['/verified/first','/verified/first','/verified/changed']
    const request=vi.fn(async(method:CoreMethod,params?:unknown)=>{
      expect(method).toBe('workbench.files.list')
      expect(params).toEqual({scope:{threadId:'thread-a',missionId:'mission-current'},path:''})
      return {workspaceRoot:roots.shift()}
    })
    const view=render(ownerTree(createClient(request),'thread-a'))
    await act(async()=>{expect(dispatchShortcutAction('toggle_terminal')).toBe(true)})
    const editor=await view.findByTestId('workbench')
    expect(editor.getAttribute('data-tab')).toBe('terminals')
    expect(editor.closest('aside')?.hidden).toBe(false)
    await act(async()=>{expect(dispatchShortcutAction('toggle_aux_panel')).toBe(true)})
    expect(editor.closest('aside')?.hidden).toBe(true)
    expect(view.getByTestId('workbench')).toBe(editor)
    fireEvent.click(view.getByRole('button',{name:'展开工程区'}))
    await waitFor(()=>expect(editor.closest('aside')?.hidden).toBe(false))
    expect(view.getByTestId('workbench')).toBe(editor)
    fireEvent.click(view.getByRole('button',{name:'关闭工作区'}))
    fireEvent.click(view.getByRole('button',{name:'展开工程区'}))
    await waitFor(()=>expect(view.getByRole('alert').textContent).toContain('工程目录已变化'))
    expect(editor.getAttribute('data-root')).toBe('/verified/first')
    expect(editor.closest('aside')?.hidden).toBe(true)
    expect(request).toHaveBeenCalledTimes(3)
  })

  it('模态框或菜单打开时不核验；Core 未返回已核实目录时不挂载工作区', async () => {
    const request = vi.fn(async () => ({ workspaceRoot: '' }))
    const view = render(ownerTree(createClient(request), 'thread-a'))
    const dialog = document.createElement('div')
    dialog.setAttribute('role', 'dialog')
    dialog.setAttribute('aria-modal', 'true')
    const menu = document.createElement('div')
    menu.setAttribute('role', 'menu')
    menu.setAttribute('data-state', 'open')
    const overlays = [dialog, menu]

    for (const overlay of overlays) {
      document.body.append(overlay)
      await act(async () => { expect(dispatchShortcutAction('toggle_aux_panel')).toBe(false) })
      expect(request).not.toHaveBeenCalled()
      overlay.remove()
    }

    await act(async () => { expect(dispatchShortcutAction('toggle_aux_panel')).toBe(true) })
    await waitFor(() => expect(view.getByRole('alert').textContent).toContain('尚未提供已核实的工程目录'))
    expect(view.queryByTestId('workbench')).toBeNull()
    expect(request).toHaveBeenCalledTimes(1)
  })

  it('Host 同 Thread 切换和 Thread 切换后忽略旧目录核验结果，并合并同一 owner 的重复触发', async () => {
    let resolveOld!: (value: unknown) => void
    const oldRequest = vi.fn((_method: CoreMethod, _params?: unknown) => new Promise<unknown>(resolve => { resolveOld = resolve }))
    const newRequest = vi.fn(async (method: CoreMethod, params?: unknown) => {
      expect(method).toBe('workbench.files.list')
      const threadId = (params as { scope: { threadId: string } }).scope.threadId
      return { workspaceRoot: threadId === 'thread-a' ? '/verified/new-host' : '/verified/new-thread' }
    })
    const view = render(ownerTree(createClient(oldRequest), 'thread-a'))

    await act(async () => { expect(dispatchShortcutAction('toggle_terminal')).toBe(true) })
    await waitFor(() => expect(oldRequest).toHaveBeenCalledTimes(1))
    await act(async () => { expect(dispatchShortcutAction('toggle_terminal')).toBe(true) })
    expect(oldRequest).toHaveBeenCalledTimes(1)

    const clientB = createClient(newRequest)
    view.rerender(ownerTree(clientB, 'thread-a'))
    await act(async () => { expect(dispatchShortcutAction('toggle_terminal')).toBe(true) })
    await waitFor(() => expect(view.getByTestId('workbench').getAttribute('data-root')).toBe('/verified/new-host'))
    expect(view.getByTestId('workbench').getAttribute('data-thread')).toBe('thread-a')
    expect(view.getByTestId('workbench').getAttribute('data-tab')).toBe('terminals')

    view.rerender(ownerTree(clientB, 'thread-b'))
    await act(async () => { expect(dispatchShortcutAction('toggle_aux_panel')).toBe(true) })
    await waitFor(() => expect(view.getByTestId('workbench').getAttribute('data-thread')).toBe('thread-b'))
    expect(view.getByTestId('workbench').getAttribute('data-root')).toBe('/verified/new-thread')
    expect(view.getByTestId('workbench').getAttribute('data-tab')).toBe('files')
    expect(newRequest).toHaveBeenCalledTimes(2)

    await act(async () => { resolveOld({ workspaceRoot: '/stale/old-owner' }) })
    expect(view.getByTestId('workbench').getAttribute('data-thread')).toBe('thread-b')
    expect(view.getByTestId('workbench').getAttribute('data-root')).toBe('/verified/new-thread')
  })
})
