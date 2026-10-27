import { useEffect,useState } from 'react'
import { useThreadClient } from './camp-client'
import { PET_SETTINGS_CHANGED,PetThumbnail,type PetSettingsValue } from './PetSettings'
import type { PetState } from './coaxi-pets/animation'
export function PetExtensionHost():React.JSX.Element|null{
  const client=useThreadClient(),native=client.nativePets
  const [settings,setSettings]=useState<PetSettingsValue>({enabled:false,activeId:null,scale:.75}),[state,setState]=useState<PetState>('idle')
  useEffect(()=>{
    let active=true,timer:number|undefined
    const running=new Set<string>(),waiting=new Set<string>()
    const load=():void=>{void (native?native.synchronize():client.request<{settings:PetSettingsValue}>('pets.operation',{operation:'list'}).then(value=>{if(active)setSettings(value.settings)})).catch(()=>{if(active)timer=window.setTimeout(load,15000)})}
    load();document.addEventListener(PET_SETTINGS_CHANGED,load);window.addEventListener('focus',load)
    const changed=native?.onChanged(load)
    const events=native?undefined:client.onEvent?.(event=>{
      const params=event.params&&typeof event.params==='object'?event.params as Record<string,unknown>:{},id=typeof params.agentRunId==='string'?params.agentRunId:null
      if(id&&event.method==='agent_run.started'){running.add(id);waiting.delete(id)}
      if(id&&['agent_run.context_waiting','agent_run.network_recovery_waiting','agent_run.network_recovery_needs_attention'].includes(event.method))waiting.add(id)
      if(id&&['agent_run.terminal','agent_run.failed','agent_run.cancelled'].includes(event.method)){running.delete(id);waiting.delete(id)}
      if(event.method==='runtime.state'&&['shutting_down','stopped'].includes(String(params.status))){running.clear();waiting.clear()}
      setState(waiting.size?'waiting':running.size?'running':'idle')
    })
    return()=>{active=false;if(timer)window.clearTimeout(timer);document.removeEventListener(PET_SETTINGS_CHANGED,load);window.removeEventListener('focus',load);changed?.();events?.()}
  },[client,native])
  if(native||!settings.enabled||!settings.activeId)return null
  return <button className="pet-inline-surface" title="让宠物休息" aria-label="让宠物休息" onClick={()=>{void client.request('pets.operation',{operation:'settings',value:{...settings,enabled:false}}).then(()=>{setSettings({...settings,enabled:false});document.dispatchEvent(new Event(PET_SETTINGS_CHANGED))})}}><PetThumbnail id={settings.activeId} state={state} scale={settings.scale}/></button>
}
