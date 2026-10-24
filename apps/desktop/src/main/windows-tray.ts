import { Menu, nativeImage, Tray } from 'electron'
import iconPath from '../../../../build/icon.png?asset'
import { PRODUCT_NAME } from '../shared/product-identity'

/** Called only by the Windows main-window owner, after app.ready. */
export function createWindowsTray(open: () => void, quit: () => void, english: boolean): Tray {
  const icon = nativeImage.createFromPath(iconPath)
  if (icon.isEmpty()) throw new Error('Coaxi tray icon is unavailable')
  const tray = new Tray(icon.resize({ width: 16, height: 16 }))
  try {
    tray.setToolTip(PRODUCT_NAME)
    tray.setContextMenu(Menu.buildFromTemplate([
      { label: english ? `Open ${PRODUCT_NAME}` : `打开 ${PRODUCT_NAME}`, click: open },
      { label: english ? `Quit ${PRODUCT_NAME}` : `退出 ${PRODUCT_NAME}`, click: quit }
    ]))
    tray.on('click', open)
    return tray
  } catch (error) {
    tray.destroy()
    throw error
  }
}
