import type { IpcRenderer, WebContents } from 'electron'
import { matchShortcutEvent } from './coaxi-keyboard-shortcuts'

const CLOSE_TAB_CHANNEL = 'coaxi:close-tab-requested'
const CLOSE_WINDOW_CHANNEL = 'coaxi:close-window-shortcut'

export function installCloseTabShortcut(contents: WebContents, platform: NodeJS.Platform, closeWindow: () => void,configured?:()=>{binding:string;recording:boolean}): void {
  contents.on('before-input-event', (event, input) => {
    if(configured){
      const settings=configured();if(settings.recording||input.type!=='keyDown'||input.isComposing)return
      const fields={key:input.key,code:input.code,metaKey:input.meta,ctrlKey:input.control,altKey:input.alt,shiftKey:input.shift,isComposing:input.isComposing}
      const matched=matchShortcutEvent(fields,settings.binding)
      const legacy=(platform==='darwin'?input.meta&&!input.control:input.control&&!input.meta)&&!input.alt&&!input.shift&&input.key.toLowerCase()==='w'
      if(!matched&&!legacy)return
      event.preventDefault();if(input.isAutoRepeat)return
      if(matched)contents.send(CLOSE_TAB_CHANNEL);else contents.send('coaxi:keyboard-input',fields)
      return
    }
    if (input.type !== 'keyDown' || input.isComposing || input.alt || input.shift) return
    const primary = platform === 'darwin' ? input.meta : input.control
    const secondary = platform === 'darwin' ? input.control : input.meta
    if (!primary || secondary || input.key.toLowerCase() !== 'w') return

    // Native menu accelerators run before Renderer keydown handlers.
    event.preventDefault()
    if (!input.isAutoRepeat) contents.send(CLOSE_TAB_CHANNEL)
  })
  contents.on('ipc-message', (_event, channel) => {
    if (channel === CLOSE_WINDOW_CHANNEL) closeWindow()
  })
}

export function createCloseTabShortcutHandler(
  ipc: Pick<IpcRenderer, 'on' | 'send'>
): (listener: () => boolean) => () => void {
  let current: (() => boolean) | null = null
  ipc.on(CLOSE_TAB_CHANNEL, () => {
    if (!current?.()) ipc.send(CLOSE_WINDOW_CHANNEL)
  })
  return (listener) => {
    current = listener
    return () => { if (current === listener) current = null }
  }
}
