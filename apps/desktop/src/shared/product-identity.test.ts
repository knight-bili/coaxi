import { describe, expect, it } from 'vitest'
import { PRODUCT_APPLICATION_ID, PRODUCT_DATA_DIRECTORY, PRODUCT_ID, PRODUCT_NAME } from './product-identity'
import { resolveWindowsDataRoot } from '../main/windows-data-root'
import { parseDesktopDeepLink } from './deep-link'

describe('Coaxi 品牌升级兼容', () => {
  it('更名保留安装身份与原数据位置，显式隔离目录继续优先', () => {
    expect(PRODUCT_NAME).toBe('Coaxi')
    expect(PRODUCT_APPLICATION_ID).toBe('app.codegrovai.workbench.desktop')
    expect(PRODUCT_ID).toBe('codeg-rovai-workbench')
    expect(PRODUCT_DATA_DIRECTORY).toBe('Codeg Rovai Workbench')
    expect(resolveWindowsDataRoot(null, 'C:\\Users\\客户\\AppData\\Local')).toBe('C:\\Users\\客户\\AppData\\Local\\Codeg Rovai Workbench')
    expect(resolveWindowsDataRoot('D:\\隔离副本', undefined)).toBe('D:\\隔离副本')
  })

  it('新品牌链接和既有链接使用相同导航边界', () => {
    for (const scheme of ['coaxi', 'rovai']) {
      expect(parseDesktopDeepLink(`${scheme}://tab/canvas`)).toEqual({ kind: 'tab', value: 'canvas' })
      expect(parseDesktopDeepLink(`${scheme}://tab/canvas?command=run`)).toBeNull()
      expect(parseDesktopDeepLink(`${scheme}://token@tab/canvas`)).toBeNull()
    }
  })
})
