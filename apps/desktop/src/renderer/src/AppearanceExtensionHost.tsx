import { useEffect } from 'react'
import { useThreadClient } from './camp-client'
import { applyCustomCss, CUSTOM_THEME_TOKENS, sanitizeCustomCss, sanitizeCustomTheme, type CustomTheme } from './codeg-appearance/custom-style'
import { THEME_COLORS } from './codeg-appearance/theme-presets'
import './codeg-appearance/theme-presets.css'
export type AppearanceExtensions={preset:string;customTheme:CustomTheme;customCss:string;customEnabled:boolean}
export type StoredAppearanceExtensions={revision:string|null;value:AppearanceExtensions}
export const APPEARANCE_EXTENSIONS_CHANGED='rovai-appearance-extensions-changed'
export function publishAppearanceExtensions(value:StoredAppearanceExtensions):void{document.dispatchEvent(new CustomEvent(APPEARANCE_EXTENSIONS_CHANGED,{detail:value}))}
export function AppearanceExtensionHost():null{
  const client=useThreadClient()
  useEffect(()=>{
    const root=document.documentElement;let alive=true,current:StoredAppearanceExtensions|null=null,timer:number|undefined
    const apply=():void=>{
      if(!current)return
      const value=current.value,preset=value.preset==='rovai'?'rovai':THEME_COLORS.find(preset=>preset===value.preset)??'neutral'
      root.dataset.codegPreset=preset;root.classList.toggle('dark',root.dataset.theme==='night')
      for(const token of CUSTOM_THEME_TOKENS)root.style.removeProperty(`--cg-${token}`)
      const custom=sanitizeCustomTheme(value.customTheme),side=root.dataset.theme==='night'?custom.dark:custom.light
      if(value.customEnabled)for(const [token,value]of Object.entries(side))if(value)root.style.setProperty(`--cg-${token}`,value)
      const css=value.customEnabled?sanitizeCustomCss(value.customCss):null
      applyCustomCss(css?.css??'')
    }
    const load=():void=>{void client.request<StoredAppearanceExtensions>('appearance.extensions',{operation:'get'}).then(value=>{if(alive){current=value;apply()}},()=>{if(alive)timer=window.setTimeout(load,15000)})}
    const changed=(event:Event):void=>{if(event instanceof CustomEvent&&event.detail){current=event.detail as StoredAppearanceExtensions;apply()}}
    const observer=new MutationObserver(apply);observer.observe(root,{attributes:true,attributeFilter:['data-theme']})
    document.addEventListener(APPEARANCE_EXTENSIONS_CHANGED,changed);window.addEventListener('focus',load);load()
    return()=>{alive=false;if(timer)window.clearTimeout(timer);observer.disconnect();document.removeEventListener(APPEARANCE_EXTENSIONS_CHANGED,changed);window.removeEventListener('focus',load);delete root.dataset.codegPreset;for(const token of CUSTOM_THEME_TOKENS)root.style.removeProperty(`--cg-${token}`);applyCustomCss('')}
  },[client])
  return null
}
