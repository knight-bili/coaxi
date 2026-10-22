import { isShortcutRecorderArmed,matchShortcutEvent,type ShortcutActionId,type ShortcutSettings } from '../../shared/codeg-keyboard-shortcuts'
export type ShortcutActions=Partial<Record<ShortcutActionId,()=>boolean|void>>
export interface ShortcutRegistration {priority:number;active():boolean;bindings():ShortcutSettings|null;actions():ShortcutActions}
const registrations=new Set<ShortcutRegistration>()
export function dispatchShortcutAction(action:ShortcutActionId):boolean{
  if(isShortcutRecorderArmed())return true
  for(const row of [...registrations].sort((left,right)=>right.priority-left.priority))if(row.active()){
    const callback=row.actions()[action];if(callback&&callback()!==false)return true
  }
  return false
}
function keydown(event:KeyboardEvent):void{
  if(event.defaultPrevented||event.isComposing||event.repeat||isShortcutRecorderArmed())return
  for(const row of [...registrations].sort((left,right)=>right.priority-left.priority)){
    if(!row.active())continue;const bindings=row.bindings();if(!bindings)continue
    for(const [id,callback]of Object.entries(row.actions()))if(callback&&matchShortcutEvent(event,bindings[id as ShortcutActionId])){
      if(callback()===false)continue
      event.preventDefault();event.stopPropagation();event.stopImmediatePropagation();return
    }
  }
}
export function registerShortcutActions(value:ShortcutRegistration):()=>void{
  if(!registrations.size)window.addEventListener('keydown',keydown)
  registrations.add(value)
  return()=>{registrations.delete(value);if(!registrations.size)window.removeEventListener('keydown',keydown)}
}
