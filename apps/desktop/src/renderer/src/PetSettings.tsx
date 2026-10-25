import { PetMarketplace } from './PetMarketplace'
import { useCallback, useEffect, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Cat, FolderOpen, Pencil, Trash2 } from 'lucide-react'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { AppDialogContent, AppDialogHeader, AppDialogBody, AppDialogFooter } from './AppDialog'
import { PetSprite } from './codeg-pets/PetSprite'
import type { PetState } from './codeg-pets/animation'
import { readPetSprite } from './pet-sprite-reader'
import './pet-settings.css'
export type PetSettingsValue={enabled:boolean;activeId:string|null;scale:number}
type Pet={id:string;displayName:string;description?:string|null;unavailable?:boolean;message?:string}
export const PET_SETTINGS_CHANGED='rovai-pets-changed'
export function publishPetsChanged():void{document.dispatchEvent(new Event(PET_SETTINGS_CHANGED))}
export function PetThumbnail({id,state='idle',scale=.5}:{id:string;state?:PetState;scale?:number}):React.JSX.Element{
  const client=useThreadClient(),[url,setUrl]=useState<string|null>(null),[error,setError]=useState(false),element=useRef<HTMLDivElement>(null)
  useEffect(()=>{
    const abort=new AbortController();let url:string|null=null,started=false
    const load=():void=>{if(started)return;started=true;void readPetSprite(input=>client.request('pets.operation',input),id,abort.signal).then(value=>{if(abort.signal.aborted)URL.revokeObjectURL(value);else{url=value;setUrl(value)}},failure=>{if(!(failure instanceof DOMException&&failure.name==='AbortError'))setError(true)})}
    const observer=new IntersectionObserver(entries=>{if(entries.some(entry=>entry.isIntersecting)){observer.disconnect();load()}},{rootMargin:'160px'})
    if(element.current)observer.observe(element.current)
    return()=>{abort.abort();observer.disconnect();if(url)URL.revokeObjectURL(url)}
  },[client,id])
  return <div className="pet-thumbnail" ref={element}>{url?<PetSprite spritesheetUrl={url} state={state} scale={scale} label={id}/>:error?<Cat size={26}/>:<span aria-hidden>…</span>}</div>
}
export function PetSettings():React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),[pets,setPets]=useState<Pet[]>([]),[settings,setSettings]=useState<PetSettingsValue>({enabled:false,activeId:null,scale:.75})
  const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[editing,setEditing]=useState<Pet|null>(null),[preview,setPreview]=useState<{pet:Pet;state:PetState}|null>(null)
  const load=useCallback(async()=>{const value=await client.request<{entries:Pet[];settings:PetSettingsValue}>('pets.operation',{operation:'list'});setPets(value.entries);setSettings(value.settings)},[client])
  useEffect(()=>{void load().catch(failure=>setError(String(failure)))},[load])
  const run=async(action:()=>Promise<void>):Promise<void>=>{if(busy)return;setBusy(true);setError(null);try{await action();await load();publishPetsChanged()}catch(failure){setError(String(failure))}finally{setBusy(false)}}
  const saveSettings=async(value:PetSettingsValue):Promise<void>=>{await client.request('pets.operation',{operation:'settings',value});setSettings(value)}
  return <section className="appearance-section pet-settings-section"><div className="appearance-section-heading"><h2><Cat size={17}/> {t('桌面宠物')}</h2><p>{t('使用原版精灵动画。宠物只显示工作状态，不会替你发送消息或启动任务。')}</p></div>
    <div className="workbench-toolbar"><PetMarketplace onInstalled={load}/>{client.nativePets&&<button className="quiet-button" disabled={busy} onClick={()=>void run(async()=>{await client.nativePets!.importDirectory()})}><FolderOpen size={14}/>{t('从原生宠物目录导入')}</button>}<label><input className="workspace-check" type="checkbox" checked={settings.enabled} disabled={busy||!settings.activeId} onChange={event=>void run(()=>saveSettings({...settings,enabled:event.target.checked}))}/>{t('显示宠物')}</label><label>{t('大小')}<input className="workspace-field" type="range" min="0.25" max="2" step="0.05" value={settings.scale} disabled={busy} onChange={event=>setSettings({...settings,scale:Number(event.target.value)})} onPointerUp={()=>void run(()=>saveSettings(settings))} onKeyUp={event=>{if(['ArrowLeft','ArrowRight','Home','End'].includes(event.key))void run(()=>saveSettings(settings))}}/></label></div>
    {error&&<p className="workbench-error" role="alert">{error}</p>}
    <div className="pet-catalog-grid">{pets.map(pet=><article className={pet.id===settings.activeId?'pet-catalog-card is-active':'pet-catalog-card'} key={pet.id}><button className="pet-preview-button" disabled={pet.unavailable} onClick={()=>setPreview({pet,state:'idle'})}><PetThumbnail id={pet.id}/></button><strong>{pet.displayName}</strong>{pet.description&&<small>{pet.description}</small>}{pet.unavailable?<p role="alert">{pet.message}</p>:<div className="workbench-toolbar"><button className="quiet-button" disabled={busy} onClick={()=>void run(()=>saveSettings({...settings,enabled:pet.id===settings.activeId?!settings.enabled:true,activeId:pet.id}))}>{t(pet.id===settings.activeId&&settings.enabled?'收起':'召唤')}</button><button className="quiet-button" aria-label={t('编辑宠物资料')} disabled={busy} onClick={()=>setEditing({...pet})}><Pencil size={13}/></button><button className="quiet-button" aria-label={t('移除宠物')} disabled={busy} onClick={()=>{if(window.confirm(t('移除这个宠物？已保存的会话不会改变。')))void run(async()=>{await client.request('pets.operation',{operation:'remove',id:pet.id})})}}><Trash2 size={13}/></button></div>}</article>)}</div>
    {pets.length===0&&<p>{t('还没有宠物。可导入包含 pet.json 与 spritesheet.webp 的原生目录。')}</p>}
    <Dialog.Root open={editing!==null||preview!==null} onOpenChange={open=>{if(!open&&!busy){setEditing(null);setPreview(null)}}}><Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent tone="neutral"><AppDialogHeader title={t(editing?'编辑宠物资料':'动作预览')} description={editing?.id??preview?.pet.displayName??''} hideDescription closeDisabled={busy}/>{editing?<form className="app-dialog-form" onSubmit={event=>{event.preventDefault();void run(async()=>{await client.request('pets.operation',{operation:'meta',id:editing.id,displayName:editing.displayName,description:editing.description??''});setEditing(null)})}}><AppDialogBody className="forge-form"><label>{t('名称')}<input className="workspace-field" value={editing.displayName} disabled={busy} onChange={event=>setEditing({...editing,displayName:event.target.value})}/></label><label>{t('描述')}<textarea className="workspace-field" value={editing.description??''} disabled={busy} onChange={event=>setEditing({...editing,description:event.target.value})}/></label></AppDialogBody><AppDialogFooter><button className="primary-button" type="submit" disabled={busy||!editing.displayName.trim()}>{t('保存')}</button></AppDialogFooter></form>:<AppDialogBody className="app-dialog-stack">{preview&&<div className="pet-action-preview"><PetThumbnail id={preview.pet.id} state={preview.state} scale={1}/><select className="workspace-field" aria-label={t('宠物动作')} value={preview.state} onChange={event=>setPreview({...preview,state:event.target.value as PetState})}>{Object.entries({idle:'待机',running_right:'向右跑',running_left:'向左跑',waving:'挥手',jumping:'跳跃',failed:'失败',waiting:'等待',running:'工作中',review:'检查中'}).map(([value,label])=><option value={value} key={value}>{t(label)}</option>)}</select></div>}</AppDialogBody>}</AppDialogContent></Dialog.Portal></Dialog.Root>
  </section>
}
