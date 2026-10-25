// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { cleanup, render, waitFor } from '@testing-library/react'
import type { BrowserWorkspaceApi } from '@contracts'
import { NativeBrowserStage } from './NativeBrowserStage'

beforeEach(() => {
 vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
 vi.stubGlobal('requestAnimationFrame', (fn: FrameRequestCallback) => setTimeout(() => fn(0), 0))
 vi.stubGlobal('cancelAnimationFrame', clearTimeout)
 vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
   return this.classList.contains('native-browser-stage') ? new DOMRect(500, 50, 600, 600) : new DOMRect(550, 60, 200, 100)
 })
})
afterEach(() => { cleanup(); document.querySelectorAll('[role=menu]').forEach(el => el.remove()); vi.restoreAllMocks(); vi.unstubAllGlobals() })
function setup(inspect: ReturnType<typeof vi.fn>) {
 const present = vi.fn(async (_input: Parameters<NonNullable<BrowserWorkspaceApi['present']>>[0]) => undefined)
 const menu = document.createElement('div'); menu.setAttribute('role', 'menu'); document.body.append(menu)
 const view = render(<NativeBrowserStage id="tab" api={{ present, inspect } as unknown as BrowserWorkspaceApi}/>)
 return { menu, view, present }
}
it('a failed frozen-frame capture still hides the native page so menus remain usable', async () => {
 const f = setup(vi.fn(async () => { throw Error('capture failed') }))
 await waitFor(() => expect(f.present).toHaveBeenCalledWith(expect.objectContaining({ bounds: null })))
 expect(f.view.getByRole('alert').textContent).toContain('capture failed')
})
it('a stalled capture is bounded and cannot indefinitely cover a menu', async () => {
 const f = setup(vi.fn(() => new Promise(() => {})))
 await waitFor(() => expect(f.present).toHaveBeenCalledWith(expect.objectContaining({ bounds: null })), { timeout: 1500 })
})
it('a late capture cannot re-hide a page after its menu has closed', async () => {
 let resolve!: (value: unknown) => void
 const f = setup(vi.fn(() => new Promise(done => { resolve = done })))
 f.menu.remove()
 await waitFor(() => expect(f.present).toHaveBeenCalledWith(expect.objectContaining({ bounds: expect.objectContaining({ width: 600 }) })))
 resolve({ data: '' })
 await new Promise(done => setTimeout(done, 30))
 expect(f.present.mock.calls.at(-1)?.[0]).toMatchObject({ bounds: { width: 600 } })
 expect(f.view.queryByRole('alert')).toBeNull()
})
