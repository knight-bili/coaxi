import { createRoot } from 'react-dom/client'
import { useEffect,useRef,useState } from 'react'
import { PetSprite } from './codeg-pets/PetSprite'
import {PET_FRAME_DURATIONS_MS,type PetState} from './codeg-pets/animation'
import { readPetSprite } from './pet-sprite-reader'
import './pet-surface.css'
type PetApi={request<T>(input:unknown):Promise<T>;control(action:'focus'|'hide'|'menu'):Promise<void>;onState(listener:(value:{state:string;changed?:boolean;error?:string})=>void):()=>void}
const api=(window as unknown as {rovaiPet:PetApi}).rovaiPet
function PetSurface():React.JSX.Element{
  const [pet,setPet]=useState<{id:string;name:string;scale:number;url:string}|null>(null),[state,setState]=useState<PetState>('idle'),[interaction,setInteraction]=useState<PetState|null>(null),[error,setError]=useState<string|null>(null),timer=useRef<ReturnType<typeof setTimeout>|null>(null)
  const animate=(value:PetState):void=>{if(timer.current)clearTimeout(timer.current);setInteraction(value);timer.current=setTimeout(()=>{setInteraction(null);timer.current=null},PET_FRAME_DURATIONS_MS[value].reduce((a,b)=>a+b,0)*3+80)}
  useEffect(()=>()=>{if(timer.current)clearTimeout(timer.current)},[])
  useEffect(()=>{
    let active=true,controller:AbortController|null=null,url:string|null=null
    const load=async():Promise<void>=>{
      controller?.abort();if(url){URL.revokeObjectURL(url);url=null}const abort=new AbortController();controller=abort
      const catalog=await api.request<{entries:Array<{id:string;displayName:string}>;settings:{activeId:string;enabled:boolean;scale:number};state:PetState}>({operation:'list'})
      const selected=catalog.entries.find(entry=>entry.id===catalog.settings.activeId)
      if(!active||abort.signal.aborted||!catalog.settings.enabled||!selected){if(active)setPet(null);return}
      const asset=await readPetSprite(input=>api.request(input),selected.id,abort.signal)
      if(!active||abort.signal.aborted){URL.revokeObjectURL(asset);return}url=asset;setPet({id:selected.id,name:selected.displayName,scale:catalog.settings.scale,url:asset});setState(catalog.state)
    }
    void load().catch(()=>{if(active)setPet(null)})
    const unsubscribe=api.onState(value=>{if(!active)return;if(value.error)setError(value.error);if(value.changed)void load().catch(()=>setPet(null));if(['idle','waiting','running','failed','review'].includes(value.state))setState(value.state as PetState)})
    return()=>{active=false;unsubscribe();controller?.abort();if(url)URL.revokeObjectURL(url)}
  },[])
  return <main className="pet-native-stage" onContextMenu={event=>{event.preventDefault();void api.control('menu')}}>{pet&&<section className="pet-native-character" title={pet.name}><div className="pet-native-drag" onPointerEnter={()=>animate('waving')} onPointerLeave={()=>{if(timer.current)clearTimeout(timer.current);setInteraction(null)}} onDoubleClick={()=>animate('jumping')}><PetSprite spritesheetUrl={pet.url} state={interaction??state} scale={pet.scale} label={pet.name}/></div><nav title={error??pet.name}><button type="button" onClick={()=>void api.control('focus')} title="打开工作区">{pet.name}</button><button type="button" onClick={()=>void api.control('menu')} aria-label="宠物菜单">···</button></nav></section>}</main>
}
createRoot(document.getElementById('root')!).render(<PetSurface/> )
