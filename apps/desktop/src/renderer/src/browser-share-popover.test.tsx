// @vitest-environment jsdom
import { createElement } from 'react'
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { ThreadClientProvider, type ThreadClient } from './camp-client'
import { BrowserSharePopover } from './BrowserSharePopover'
vi.mock('@radix-ui/react-popover', () => ({ Root: ({ children, open, onOpenChange }: any) => createElement('div', null, children, createElement('button', { 'data-testid': 'toggle-share', onClick: () => onOpenChange(!open) }, 'toggle')), Trigger: ({ children }: any) => children, Portal: ({ children }: any) => children, Content: ({ children }: any) => createElement('div', null, children) }))
afterEach(cleanup)
function fixture(enabled = true) {
 let policy = { schemaVersion: 1, revision: 1, members: ['a', 'b'].map(agentId => ({ agentId, features: { browser: enabled, browserEval: false, computer: true, computerLaunch: false, computerClipboard: false } })) }
 const targets = ['a', 'b'].map(agentId => ({ id: agentId, agentId, displayName: agentId === 'a' ? '星璃' : '绘羽', scope: { threadId: 'thread', conversationId: null, scopeKind: 'public' } }))
 const grants = new Map<string, any>()
 let reject = ''
 const request = vi.fn(async (action: string, payload: any) => {
   if (action === 'policy.get') return structuredClone(policy)
   if (action === 'policy.replace') { policy = payload.value; return policy }
   if (action === 'sharing.targets') return { targets: [...targets, { ...targets[0], id: 'private', scope: { ...targets[0].scope, scopeKind: 'single_chat' } }], next: null }
   if (action === 'grant.list') return { grants: [...grants.values()].filter(grant => grant.agentId === payload.agentId) }
   if (action === 'grant.share') { if (reject) throw Error(reject); grants.set(payload.grantId, payload); return payload }
   if (action === 'grant.revoke') { grants.delete(payload.grantId); return {} }
   throw Error(action)
 })
 const client = { interactionWorkspace: { request } } as unknown as ThreadClient
 const view = render(<ThreadClientProvider client={client}><BrowserSharePopover threadId="thread" targetId="tab" origin="https://one.test"/></ThreadClientProvider>)
 return { view, request, grants, policy: () => policy, reject: (value: string) => { reject = value } }
}
it('opening the share control reads state without granting anything', async () => {
 const f = fixture(); await waitFor(() => expect((f.view.getByText('仅查看') as HTMLButtonElement).disabled).toBe(false))
 expect(f.request.mock.calls.filter(([action]) => action === 'grant.share')).toHaveLength(0)
})
it('disabled member tools are explained and no rejected share is sent', async () => {
 const f = fixture(false); await waitFor(() => expect(f.view.getByText('为所选队员启用浏览器工具')).toBeTruthy())
 expect((f.view.getByText('仅查看') as HTMLButtonElement).disabled).toBe(true)
 expect(f.view.queryByText('共享结果尚未核实，请刷新后再操作。')).toBeNull()
 fireEvent.click(f.view.getByText('为所选队员启用浏览器工具'))
 await waitFor(() => expect((f.view.getByText('仅查看') as HTMLButtonElement).disabled).toBe(false))
 expect(f.policy().members.every(member => member.features.browser && member.features.computer && !member.features.browserEval && !member.features.computerClipboard)).toBe(true)
 expect(f.grants.size).toBe(0)
})
it('all recipients can receive explicit read-only grants and stop revokes every grant', async () => {
 const f = fixture(); await waitFor(() => expect((f.view.getByText('仅查看') as HTMLButtonElement).disabled).toBe(false))
 fireEvent.click(f.view.getByText('仅查看')); await waitFor(() => expect(f.grants.size).toBe(2))
 expect([...f.grants.values()].every(grant => grant.level === 'read' && grant.scope.scopeKind === 'public' && grant.origin === 'https://one.test')).toBe(true)
 await waitFor(() => expect((f.view.getByText('停止共享') as HTMLButtonElement).disabled).toBe(false))
 fireEvent.click(f.view.getByText('停止共享')); await waitFor(() => expect(f.grants.size).toBe(0))
})
it('narrowing the share scope revokes the removed recipient and never includes private chats', async () => {
 const f = fixture(); await waitFor(() => expect((f.view.getByText('仅查看') as HTMLButtonElement).disabled).toBe(false))
 fireEvent.click(f.view.getByText('仅查看')); await waitFor(() => expect(f.view.getByLabelText('页面共享权限').textContent).toContain('已共享'))
 fireEvent.click(f.view.getByText('指定队员')); fireEvent.click(f.view.getAllByRole('checkbox')[1])
 fireEvent.click(f.view.getByText('允许操作')); await waitFor(() => expect(f.grants.size).toBe(1))
 expect([...f.grants.values()][0]).toMatchObject({ agentId: 'a', level: 'control' })
 expect(f.view.getAllByRole('checkbox')).toHaveLength(2)
})
it('a definitive disabled rejection is not mislabeled as an unknown result', async () => {
 const f = fixture(); f.reject('interaction_disabled'); await waitFor(() => expect((f.view.getByText('仅查看') as HTMLButtonElement).disabled).toBe(false))
 fireEvent.click(f.view.getByText('仅查看')); await waitFor(() => expect(f.view.getByRole('alert').textContent).toContain('尚未启用浏览器工具'))
 expect(f.view.queryByText('共享结果尚未核实，请刷新后再操作。')).toBeNull()
 expect(f.grants.size).toBe(0)
})
it('changing site during delayed tool setup releases the new menu and never grants the old site', async () => {
 const f = fixture(false)
 await waitFor(() => expect(f.view.getByText('为所选队员启用浏览器工具')).toBeTruthy())
 let finish!: () => void
 const normal = f.request.getMockImplementation()!
 f.request.mockImplementation(async (action: string, payload: any) => {
   if (action === 'policy.replace') await new Promise<void>(resolve => { finish = resolve })
   return normal(action, payload)
 })
 fireEvent.click(f.view.getByText('为所选队员启用浏览器工具'))
 await waitFor(() => expect(typeof finish).toBe('function'))
 const client = { interactionWorkspace: { request: f.request } } as unknown as ThreadClient
 f.view.rerender(<ThreadClientProvider client={client}><BrowserSharePopover threadId="thread" targetId="tab" origin="https://two.test"/></ThreadClientProvider>)
 await waitFor(() => expect((f.view.getByText('为所选队员启用浏览器工具') as HTMLButtonElement).disabled).toBe(false))
 finish()
 await waitFor(() => expect(f.policy().members.every(member => member.features.browser)).toBe(true))
 fireEvent.click(f.view.getByText('刷新状态'))
 await waitFor(() => expect((f.view.getByText('仅查看') as HTMLButtonElement).disabled).toBe(false))
 expect(f.grants.size).toBe(0)
 fireEvent.click(f.view.getByText('仅查看'))
 await waitFor(() => expect(f.grants.size).toBe(2))
 expect([...f.grants.values()].every(grant => grant.origin === 'https://two.test')).toBe(true)
})
it('closing and reopening cannot bypass an unverified share result', async () => {
 const f = fixture()
 await waitFor(() => expect((f.view.getByText('仅查看') as HTMLButtonElement).disabled).toBe(false))
 const normal = f.request.getMockImplementation()!
 let lost = false
 f.request.mockImplementation(async (action: string, payload: any) => {
   if (action === 'grant.share') { lost = true; await normal(action, payload); throw Error('connection lost') }
   if (lost && action === 'grant.list') throw Error('still offline')
   return normal(action, payload)
 })
 fireEvent.click(f.view.getByText('仅查看'))
 await waitFor(() => expect(f.view.getByText('共享结果尚未核实，请刷新后再操作。')).toBeTruthy())
 fireEvent.click(f.view.getByTestId('toggle-share'))
 fireEvent.click(f.view.getByTestId('toggle-share'))
 await waitFor(() => expect(f.view.getByRole('alert').textContent).toContain('still offline'))
 expect((f.view.getByText('仅查看') as HTMLButtonElement).disabled).toBe(true)
 expect((f.view.getByText('允许操作') as HTMLButtonElement).disabled).toBe(true)
 expect(f.request.mock.calls.filter(([action]) => action === 'grant.share')).toHaveLength(1)
})
