import { useEffect, useRef, useState } from 'react'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { publishAppearanceExtensions, type StoredAppearanceExtensions, type AppearanceExtensions } from './AppearanceExtensionHost'
import { THEME_COLORS, THEME_COLOR_PREVIEW } from './codeg-appearance/theme-presets'
import { EMPTY_CUSTOM_THEME, sanitizeCustomCss, sanitizeCustomTheme, parseThemeRegistryItem, toThemeRegistryItem } from './codeg-appearance/custom-style'
import { WorkspaceCodeEditor } from './WorkspaceCodeEditor'
const initial:AppearanceExtensions={preset:'neutral',customTheme:EMPTY_CUSTOM_THEME,customCss:'',customEnabled:false}
// Codeg preset swatches + registry:theme import/export + original CSS sanitizer.
export function AppearanceExtensionsSettings():React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),[saved,setSaved]=useState<StoredAppearanceExtensions>({revision:null,value:initial}),[draft,setDraft]=useState(initial),[tokens,setTokens]=useState('{}')
  const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[notice,setNotice]=useState<string|null>(null),last=useRef(saved),file=useRef<HTMLInputElement>(null)
  const accept=(value:StoredAppearanceExtensions):void=>{last.current=value;setSaved(value);setDraft(value.value);setTokens(JSON.stringify(value.value.customTheme,null,2))}
  useEffect(()=>{let active=true;void client.request<StoredAppearanceExtensions>('appearance.extensions',{operation:'get'}).then(value=>{if(active)accept(value)},failure=>{if(active)setError(String(failure))});return()=>{active=false;publishAppearanceExtensions(last.current)}},[client])
  const prepared=():AppearanceExtensions=>{const customTheme=sanitizeCustomTheme(JSON.parse(tokens));const result=sanitizeCustomCss(draft.customCss);if(result.css===null)throw new Error(t('自定义样式无法应用，请检查内容或长度。'));if(result.removedImports)setNotice(t('样式中的外部导入已移除。'));return {...draft,customTheme,customCss:result.css}}
  const save=async():Promise<void>=>{if(busy)return;setBusy(true);setError(null);try{const value=prepared();const result=await client.request<StoredAppearanceExtensions>('appearance.extensions',{operation:'set',expectedRevision:saved.revision,value});accept(result);publishAppearanceExtensions(result)}catch(failure){setError(String(failure))}finally{setBusy(false)}}
  const preview=():void=>{try{setError(null);publishAppearanceExtensions({revision:saved.revision,value:prepared()})}catch(failure){setError(String(failure))}}
  const exportTheme=():void=>{try{const theme=prepared();const item=toThemeRegistryItem(theme.customTheme,'rovai-custom');const url=URL.createObjectURL(new Blob([item],{type:'application/json'}));const link=document.createElement('a');link.href=url;link.download='rovai-theme.json';link.click();window.setTimeout(()=>URL.revokeObjectURL(url),1000)}catch(failure){setError(String(failure))}}
  return <section className="appearance-section appearance-extensions-section"><div className="appearance-section-heading"><h2>{t('主题与自定义样式')}</h2><p>{t('使用内置主题预设，或导入兼容的主题。日间、夜间设置沿用上方选择。')}</p></div>
    <div className="appearance-preset-swatches" role="group" aria-label={t('主题色')}>
      <button type="button" className={draft.preset==='rovai'?'is-active':''} disabled={busy} onClick={()=>setDraft({...draft,preset:'rovai'})}>{t('Coaxi 默认')}</button>
      {THEME_COLORS.map(preset=><button key={preset} type="button" className={draft.preset===preset?'is-active':''} disabled={busy} aria-pressed={draft.preset===preset} onClick={()=>setDraft({...draft,preset})}><span style={{background:THEME_COLOR_PREVIEW[preset]}}/>{preset}</button>)}
    </div>
    <div className="setting-row"><div className="setting-copy"><label htmlFor="appearance-custom-enabled">{t('使用自定义主题和 CSS')}</label></div><input id="appearance-custom-enabled" type="checkbox" checked={draft.customEnabled} disabled={busy} onChange={event=>setDraft({...draft,customEnabled:event.target.checked})}/></div>
    <details><summary>{t('自定义颜色变量')}</summary><textarea aria-label={t('日间和夜间颜色变量')} value={tokens} disabled={busy} onChange={event=>setTokens(event.target.value)} rows={9}/></details>
    <details><summary>{t('自定义 CSS')}</summary><div className="appearance-custom-css-editor"><WorkspaceCodeEditor path="custom.css" text={draft.customCss} onChange={customCss=>setDraft(previous=>({...previous,customCss}))} onSave={()=>void save()} visible/></div></details>
    <input type="file" accept="application/json,.json" hidden ref={file} onChange={event=>{const selected=event.target.files?.[0];if(!selected)return;void selected.text().then(text=>{if(text.length>256*1024)throw new Error(t('主题文件过大。'));const theme=parseThemeRegistryItem(text);if(!theme)throw new Error(t('无法识别这个主题文件。'));setTokens(JSON.stringify(theme,null,2));setDraft(previous=>({...previous,customEnabled:true}));setNotice(t('主题已导入，预览或保存后应用。'))}).catch(failure=>setError(String(failure)));event.target.value=''}}/>
    <div className="workbench-toolbar"><button disabled={busy} onClick={()=>file.current?.click()}>{t('导入主题')}</button><button disabled={busy} onClick={exportTheme}>{t('导出主题')}</button><button disabled={busy} onClick={preview}>{t('预览')}</button><button disabled={busy} onClick={()=>{accept(saved);publishAppearanceExtensions(saved);setError(null)}}>{t('放弃修改')}</button><button disabled={busy} onClick={()=>void save()}>{t('保存并应用')}</button></div>
    {error&&<p className="workbench-error" role="alert">{error}</p>}{notice&&<p role="status">{notice}</p>}
  </section>
}
