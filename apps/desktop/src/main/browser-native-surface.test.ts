import { EventEmitter } from 'node:events'
import { beforeEach, expect, it, vi } from 'vitest'
import type { BrowserWindow } from 'electron'
import { BrowserNativeSurface } from './browser-native-surface'

vi.mock('electron', () => ({
  WebContentsView: class {
    webContents = Object.assign(new EventEmitter(), { isDestroyed: () => false })
    setBackgroundColor = vi.fn()
    setBounds = vi.fn()
    setVisible = vi.fn()
  }
}))
let owner: BrowserWindow
beforeEach(() => {
  owner = Object.assign(new EventEmitter(), {
    isDestroyed: () => false, isVisible: () => true, isMinimized: () => false,
    getContentBounds: () => ({ width: 1200, height: 900 }),
    webContents: { getZoomFactor: () => 1 },
    contentView: { addChildView: vi.fn(), removeChildView: vi.fn() }
  }) as unknown as BrowserWindow
})
it('keeps the page visible while resynchronizing unchanged or resized bounds', () => {
  const surface = new BrowserNativeSurface({}, owner)
  const bounds = { x: 400, y: 100, width: 700, height: 600 }
  surface.present(bounds)
  vi.mocked(surface.native.setVisible).mockClear()
  vi.mocked(surface.native.setBounds).mockClear()
  surface.present(bounds)
  expect(surface.native.setVisible).not.toHaveBeenCalled()
  expect(surface.native.setBounds).not.toHaveBeenCalled()
  surface.present({ ...bounds, width: 650 })
  expect(surface.native.setVisible).not.toHaveBeenCalled()
  expect(surface.native.setBounds).toHaveBeenCalledWith({ ...bounds, width: 650 })
})
it('hides once on minimize and shows once when the renderer restores the page', () => {
  const surface = new BrowserNativeSurface({}, owner)
  const bounds = { x: 400, y: 100, width: 700, height: 600 }
  surface.present(bounds)
  vi.mocked(surface.native.setVisible).mockClear()
  owner.emit('minimize'); surface.present(null)
  expect(surface.native.setVisible).toHaveBeenCalledTimes(1)
  expect(surface.native.setVisible).toHaveBeenCalledWith(false)
  surface.present(bounds)
  expect(surface.native.setVisible).toHaveBeenCalledTimes(2)
  expect(surface.native.setVisible).toHaveBeenLastCalledWith(true)
})
