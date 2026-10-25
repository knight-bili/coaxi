// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { ThreadClientProvider, type ThreadClient } from './camp-client'
import { InteractionShareControl } from './InteractionShareControl'
afterEach(cleanup)
it('an explicitly disabled computer tool leaves the sharing dialog recoverable', async () => {
 const request = vi.fn(async (action: string) => {
   if (action === 'sharing.targets') return { targets: [{ id: 'a', agentId: 'a', displayName: '星璃', createdAt: '', scope: { threadId: 'thread', conversationId: null, scopeKind: 'public' } }], next: null }
   if (action === 'grant.list') return { grants: [] }
   if (action === 'grant.share') throw { message: 'interaction_disabled' }
   throw Error(action)
 })
 const view = render(<ThreadClientProvider client={{ interactionWorkspace: { request } } as unknown as ThreadClient}><InteractionShareControl threadId="thread" targetId="window" targetKind="computer_window"/></ThreadClientProvider>)
 fireEvent.click(view.getByText('共享给队员'))
 await waitFor(() => expect(view.getByText('星璃 · 公共会话')).toBeTruthy())
 fireEvent.change(view.getByLabelText('使用这个资源的会话'), { target: { value: 'a' } })
 fireEvent.click(view.getByText('只看'))
 await waitFor(() => expect(view.getByRole('alert').textContent).toContain('启用电脑工具'))
 expect(view.queryByText('原样重试')).toBeNull()
 expect(view.queryByText('[object Object]')).toBeNull()
 expect((view.getByLabelText('关闭') as HTMLButtonElement).disabled).toBe(false)
})
