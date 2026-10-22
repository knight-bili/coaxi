import { useEffect,useRef } from 'react'
import { useKeyboardShortcuts } from './useKeyboardShortcuts'
import { registerShortcutActions,type ShortcutActions } from './shortcut-action-driver'
export function useShortcutActions(actions:ShortcutActions,options:{priority?:number;active?:()=>boolean}={}):void{
  const settings=useKeyboardShortcuts(),current=useRef({actions,options,settings})
  current.current={actions,options,settings}
  useEffect(()=>registerShortcutActions({priority:options.priority??0,active:()=>current.current.options.active?.()??!document.querySelector('[role="dialog"][aria-modal="true"], [role="menu"][data-state="open"]'),bindings:()=>current.current.settings.loaded?current.current.settings.bindings:null,actions:()=>current.current.actions}),[options.priority])
}
