import { isShortcutRecorderArmed,matchShortcutEvent,type ShortcutActionId,type ShortcutEventLike,type ShortcutSettings } from '../../shared/coaxi-keyboard-shortcuts'
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
  if(dispatchShortcutInput(event)){event.preventDefault();event.stopPropagation();event.stopImmediatePropagation()}
}
export function dispatchShortcutInput(event:ShortcutEventLike):boolean{
  if(isShortcutRecorderArmed())return false
  for(const row of [...registrations].sort((left,right)=>right.priority-left.priority)){
    if(!row.active())continue;const bindings=row.bindings();if(!bindings)continue
    for(const [id,callback]of Object.entries(row.actions()))if(callback&&matchShortcutEvent(event,bindings[id as ShortcutActionId])){
      if(callback()===false)continue
      return true
    }
  }
  return false
}
export function registerShortcutActions(value:ShortcutRegistration):()=>void{
  if(!registrations.size)window.addEventListener('keydown',keydown,true)
  registrations.add(value)
  return()=>{registrations.delete(value);if(!registrations.size)window.removeEventListener('keydown',keydown,true)}
}
