import { useEffect, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import type { TokenUsageReport } from '@contracts'
import { useThreadClient } from './camp-client'
import { useInterfaceLanguage, useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { CARD_HEIGHT,CARD_WIDTH,ShareCard,resolveShareCardTheme,type ShareCardTheme } from './codeg-native-content/components/token-usage/share-card'

/** 原 Codeg 定尺寸分享卡；打开时冻结一份统计，不在导出途中换成后来筛选。 */
export function UsageShareDialog({report,onClose}:{report:TokenUsageReport;onClose():void}):React.JSX.Element {
  const client=useThreadClient(),language=useInterfaceLanguage(),t=useUiText(),card=useRef<HTMLDivElement|null>(null),probe=useRef<HTMLDivElement|null>(null),generation=useRef(0)
  const [theme,setTheme]=useState<ShareCardTheme|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[names,setNames]=useState(false)
  useEffect(()=>{if(probe.current)setTheme(resolveShareCardTheme(probe.current));return()=>{generation.current++}},[])
  const safeReport=names?report:{...report,by_folder:report.by_folder.map((row,index)=>({...row,label:t('工程 {0}',index+1)})),top_conversations:report.top_conversations.map(row=>({...row,title:null,folder_label:null}))}
  const canCopy=Boolean(client.usageCard)||typeof ClipboardItem!=='undefined'&&typeof navigator.clipboard?.write==='function'
  const exportCard=async(action:'save'|'copy'):Promise<void>=>{
    if(!card.current||!theme||busy)return
    const observed=++generation.current,node=card.current;setBusy(true);setError(null)
    try{
      const {toPng}=await import('html-to-image')
      const dataUrl=await toPng(node,{width:CARD_WIDTH,height:CARD_HEIGHT,pixelRatio:2,backgroundColor:theme.bg,skipFonts:true,style:{transform:'none',transformOrigin:'top left'}})
      if(observed!==generation.current)return
      if(action==='copy'){
        if(client.usageCard)await client.usageCard.copy(dataUrl)
        else {if(!canCopy)throw new Error(t('当前浏览器不支持复制图片，请保存 PNG。'));const bytes=Uint8Array.from(atob(dataUrl.slice(dataUrl.indexOf(',')+1)),character=>character.charCodeAt(0));await navigator.clipboard.write([new ClipboardItem({'image/png':new Blob([bytes],{type:'image/png'})})])}
      }else if(client.usageCard)await client.usageCard.save(dataUrl)
      else{const anchor=document.createElement('a');anchor.href=dataUrl;anchor.download=`codeg-rovai-usage-${new Date().toISOString().slice(0,10)}.png`;anchor.click()}
    }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  return <Dialog.Root open onOpenChange={value=>{if(!value){generation.current++;onClose()}}}><Dialog.Portal><Dialog.Overlay className="app-dialog-overlay"/><Dialog.Content className="app-dialog-content usage-share-dialog" aria-describedby={undefined}>
    <header className="flex items-center justify-between gap-3"><Dialog.Title>{t('用量分享卡片')}</Dialog.Title><Dialog.Close aria-label={t('关闭')} className="quiet-button">×</Dialog.Close></header>
    <div ref={probe} className="codeg-native-rich tu-viz"/>
    <label className="usage-share-names"><input type="checkbox" checked={names} disabled={busy} onChange={event=>setNames(event.target.checked)}/>{t('在分享图中显示工程名称')}</label>
    <div className="usage-share-preview"><div style={{width:CARD_WIDTH/2,height:CARD_HEIGHT/2,position:'relative',flex:'none'}}>{theme&&<div ref={card} style={{position:'relative',width:CARD_WIDTH,height:CARD_HEIGHT,transform:'scale(0.5)',transformOrigin:'top left'}}><ShareCard report={safeReport} locale={language} theme={theme}/>{report.truncated&&<span style={{position:'absolute',bottom:7,left:44,right:44,fontSize:11,lineHeight:'18px',background:theme.bg,color:theme.textMuted}}>{t('此图仅包含已统计的部分记录。')}</span>}</div>}</div></div>
    {report.truncated&&<p role="status">{t('这份统计只覆盖部分记录，分享时请同时说明。')}</p>}
    {error&&<p role="alert">{error}</p>}
    <footer className="flex justify-end gap-2"><button type="button" className="quiet-button" disabled={busy||!theme||!canCopy} onClick={()=>void exportCard('copy')}>{t('复制图片')}</button><button type="button" className="primary-button" disabled={busy||!theme} onClick={()=>void exportCard('save')}>{t(busy?'正在生成…':'保存 PNG')}</button></footer>
  </Dialog.Content></Dialog.Portal></Dialog.Root>
}
