// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { cleanup, render, waitFor } from '@testing-library/react'
import type { BrowserWorkspaceApi } from '@contracts'
import { NativeBrowserStage } from './NativeBrowserStage'

beforeEach(() => {
 vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
 vi.stubGlobal('requestAnimationFrame', (fn: FrameRequestCallback) => setTimeout(() => fn(0), 0))
 vi.stubGlobal('cancelAnimationFrame', clearTimeout)
 Object.defineProperty(HTMLImageElement.prototype, 'decode', { configurable: true, value: vi.fn(async () => undefined) })
 vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
   return this.classList.contains('native-browser-stage') ? new DOMRect(500, 50, 600, 600) : new DOMRect(550, 60, 200, 100)
 })
})
afterEach(() => { cleanup(); document.querySelectorAll('[role=menu]').forEach(el => el.remove()); vi.restoreAllMocks(); vi.unstubAllGlobals() })
function setup(inspect: ReturnType<typeof vi.fn>) {
 const present = vi.fn(async (_input: Parameters<NonNullable<BrowserWorkspaceApi['present']>>[0]) => undefined)
 const menu = document.createElement('div'); menu.setAttribute('role', 'menu'); document.body.append(menu)
 const api = { present, inspect } as unknown as BrowserWorkspaceApi
 const view = render(<NativeBrowserStage id="tab" api={api}/>)
 return { menu, view, present, api }
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

it('parent polling and focus restoration never hide a page that remains on screen', async () => {
 const present = vi.fn(async () => undefined)
 const api = { present } as unknown as BrowserWorkspaceApi
 const view = render(<NativeBrowserStage id="tab" api={api}/>)
 await waitFor(() => expect(present).toHaveBeenCalledWith(expect.objectContaining({ bounds: expect.objectContaining({ width: 600 }) })))
 present.mockClear()
 for (let count = 0; count < 3; count++) view.rerender(<NativeBrowserStage id="tab" api={api}/>)
 window.dispatchEvent(new Event('blur')); window.dispatchEvent(new Event('focus'))
 await new Promise(done => setTimeout(done, 40))
 expect(present.mock.calls.some(call => (call as any)[0].bounds === null)).toBe(false)
})

it('paints and keeps a frozen frame for the whole time a menu covers the page', async () => {
 const f = setup(vi.fn(async () => ({ data: 'frame' })))
 await waitFor(() => expect(f.present).toHaveBeenCalledWith(expect.objectContaining({ bounds: null })))
 expect(f.view.container.querySelector('[data-browser-frozen-frame]')).not.toBeNull()
 f.present.mockClear()
 f.view.rerender(<NativeBrowserStage id="tab" api={f.api}/>)
 await new Promise(done => setTimeout(done, 30))
 expect(f.view.container.querySelector('[data-browser-frozen-frame]')).not.toBeNull()
})
