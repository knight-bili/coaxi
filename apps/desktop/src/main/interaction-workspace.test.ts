import { afterEach, expect, it, vi } from 'vitest'
import type { BrowserWindow, IpcMainInvokeEvent } from 'electron'
import type { CoreClient } from './core-client'
import type { BrowserWorkspace } from './browser-workspace'
import { InteractionWorkspace } from './interaction-workspace'

const workspaces: InteractionWorkspace[] = []
afterEach(async () => { for (const workspace of workspaces.splice(0)) await workspace.dispose() })
function fixture() {
 const contents = { mainFrame: {} }, window = { id: 1, isDestroyed: () => false, webContents: contents }
 let origin = 'https://one.test'
 const browser = { onModelResourceChanged: null, assertShareTarget: vi.fn((grant) => { if (grant.origin !== origin) throw Error('origin_changed') }) } as unknown as BrowserWorkspace
 let complete!: (value: unknown) => void
 const request = vi.fn(async (_method: string, params: { action: string; payload: any }) => {
   if (params.action === 'grant.share') return new Promise(resolve => { complete = resolve })
   if (params.action === 'grant.list') return { grants: [] }
   return {}
 })
 const core = { request, getSnapshot: () => ({ fullCoreState: 'starting', generation: 1 }) } as unknown as CoreClient
 const workspace = new InteractionWorkspace(core, browser, () => window as unknown as BrowserWindow)
 Object.assign(workspace, { peer: { desktopPeerId: 'peer' } })
 workspaces.push(workspace)
 const input = { grantId: 'grant', targetId: 'tab', targetKind: 'browser_tab', origin, scope: { threadId: 'thread', scopeKind: 'public', conversationId: null }, agentId: 'a' }
 const event = { sender: contents, senderFrame: contents.mainFrame } as unknown as IpcMainInvokeEvent
 return { workspace, request, input, event, finish: () => complete(input), navigate: (next: string) => { origin = next; browser.onModelResourceChanged?.('tab', next) } }
}
it('revokes a delayed share response after navigating away, even when the page returns to the same origin', async () => {
 const f = fixture()
 const promise = f.workspace.invoke(f.event, 'grant.share', f.input)
 f.navigate('https://two.test'); f.navigate('https://one.test'); f.finish()
 await expect(promise).rejects.toThrow('origin_changed')
 expect(f.request).toHaveBeenCalledWith('interaction.local', { action: 'grant.revoke', payload: { grantId: 'grant' } })
})
it('rejects a new browser grant before Core when the page origin already changed', async () => {
 const f = fixture(); f.navigate('https://two.test')
 await expect(f.workspace.invoke(f.event, 'grant.share', f.input)).rejects.toThrow('origin_changed')
 expect(f.request.mock.calls.some(([, value]) => value.action === 'grant.share')).toBe(false)
})

it('keeps an uncertain committed grant blocked until failed revocation can be retried', async () => {
 const f = fixture()
 f.request.mockImplementation(async (_method, params) => {
  if (params.action === 'grant.share') throw Error('reply lost')
  if (params.action === 'grant.revoke') throw Error('offline')
  if (params.action === 'grant.list') return { grants: [f.input] }
  return {}
 })
 await expect(f.workspace.invoke(f.event, 'grant.share', f.input)).rejects.toThrow('reply lost')
 expect(f.request.mock.calls.some(([, value]) => value.action === 'grant.revoke')).toBe(true)
 f.navigate('https://two.test'); f.navigate('https://one.test')
 const listed = await f.workspace.invoke(f.event, 'grant.list', {})
 expect(listed).toEqual({ grants: [] })
 expect((f.workspace as unknown as { pendingRevocations: Set<string> }).pendingRevocations.has('grant')).toBe(true)
})
