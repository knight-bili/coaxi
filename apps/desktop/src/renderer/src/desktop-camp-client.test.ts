import {afterEach, expect, it, vi} from 'vitest'
import {desktopThreadClient} from './desktop-camp-client'

afterEach(() => vi.unstubAllGlobals())

it('没有桌面桥时可读取可选能力且不伪造实现', () => {
  vi.stubGlobal('window', undefined)
  expect(desktopThreadClient.keyboardShortcuts).toBeUndefined()
  expect(desktopThreadClient.nativePets).toBeUndefined()
  expect(desktopThreadClient.privateInputRecovery).toBeUndefined()
  expect(desktopThreadClient.platform).toBeUndefined()
})

it('能力从当前桥惰性读取，不缓存已替换的桥', () => {
  vi.stubGlobal('window', {rovai: {platform: 'win32'}})
  expect(desktopThreadClient.platform).toBe('win32')
  vi.stubGlobal('window', {rovai: {platform: 'darwin'}})
  expect(desktopThreadClient.platform).toBe('darwin')
})
