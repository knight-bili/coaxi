import { useEffect,useMemo,useSyncExternalStore } from 'react'
import type { KeyboardShortcutsApi,KeyboardShortcutsSnapshot } from '@contracts'
import { useThreadClient } from './camp-client'
import { DEFAULT_SHORTCUTS,SHORTCUTS_STORAGE_KEY,SHORTCUTS_UPDATED_EVENT,normalizeShortcutSettings,type ShortcutSettings } from '../../shared/coaxi-keyboard-shortcuts'

interface State {revision:string;bindings:ShortcutSettings;loaded:boolean;error:string|null}
class ShortcutEntry {
  state:State={revision:'',bindings:{...DEFAULT_SHORTCUTS},loaded:false,error:null}
  readonly listeners=new Set<()=>void>()
  private tail:Promise<unknown>=Promise.resolve()
  private loading:Promise<void>|null=null
  constructor(private readonly api:KeyboardShortcutsApi|undefined){}
  private publish(next:State):void{this.state=next;for(const listener of this.listeners)listener()}
  async load():Promise<void>{
    if(this.loading)return this.loading
    const load=(async()=>{try{
      let snapshot:KeyboardShortcutsSnapshot
      if(this.api)snapshot=await this.api.get()
      else{const raw=window.localStorage.getItem(SHORTCUTS_STORAGE_KEY);snapshot={revision:raw??'defaults',bindings:raw?normalizeShortcutSettings(JSON.parse(raw)):{...DEFAULT_SHORTCUTS}}}
      this.publish({revision:snapshot.revision,bindings:normalizeShortcutSettings(snapshot.bindings),loaded:true,error:null})
    }catch(error){this.publish({...this.state,error:String(error)})}})().finally(()=>{if(this.loading===load)this.loading=null})
    this.loading=load;return load
  }
  save(bindings:ShortcutSettings):Promise<void>{
    const next={...bindings}
    const save=this.tail.catch(()=>undefined).then(async()=>{
      await this.loading;if(!this.state.loaded)throw new Error('快捷键尚未读取成功，请先重新读取。')
      try{
        let snapshot:KeyboardShortcutsSnapshot
        if(this.api)snapshot=await this.api.save({revision:this.state.revision,bindings:next})
        else{const raw=window.localStorage.getItem(SHORTCUTS_STORAGE_KEY);if((raw??'defaults')!==this.state.revision)throw new Error('快捷键已在其他窗口更改，请重新读取。');const serialized=JSON.stringify(next);window.localStorage.setItem(SHORTCUTS_STORAGE_KEY,serialized);if(window.localStorage.getItem(SHORTCUTS_STORAGE_KEY)!==serialized)throw new Error('快捷键未保存成功。');snapshot={revision:serialized,bindings:next};window.dispatchEvent(new Event(SHORTCUTS_UPDATED_EVENT))}
        this.publish({revision:snapshot.revision,bindings:normalizeShortcutSettings(snapshot.bindings),loaded:true,error:null})
      }catch(error){this.publish({...this.state,error:String(error)});throw error}
    })
    this.tail=save;return save
  }
  subscribe=(listener:()=>void):(()=>void)=>{this.listeners.add(listener);return()=>this.listeners.delete(listener)}
  snapshot=():State=>this.state
}
const nativeEntries=new WeakMap<KeyboardShortcutsApi,ShortcutEntry>()
let browserEntry:ShortcutEntry|undefined
export function useKeyboardShortcuts():State&{save(bindings:ShortcutSettings):Promise<void>;reload():Promise<void>}{
  const api=useThreadClient().keyboardShortcuts
  const entry=useMemo(()=>{if(!api)return browserEntry??(browserEntry=new ShortcutEntry(undefined));let value=nativeEntries.get(api);if(!value){value=new ShortcutEntry(api);nativeEntries.set(api,value)}return value},[api])
  const state=useSyncExternalStore(entry.subscribe,entry.snapshot,entry.snapshot)
  useEffect(()=>{if(!entry.state.loaded)void entry.load();if(api)return;const refresh=(event:StorageEvent):void=>{if(!event.key||event.key===SHORTCUTS_STORAGE_KEY)void entry.load()};window.addEventListener('storage',refresh);return()=>window.removeEventListener('storage',refresh)},[api,entry])
  return {...state,save:value=>entry.save(value),reload:()=>entry.load()}
}
