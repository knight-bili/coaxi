import { useMemo } from 'react'
import type { UsageActivityReport } from '@contracts'
import { useUiText } from './interface-language'
import { formatTokenCount } from './coaxi-native-content/lib/token-format'
import { CARD_HEIGHT,CARD_WIDTH,RankRow,StripCell,TrendBars,type ShareCardTheme } from './coaxi-native-content/components/token-usage/share-card'

/** 沿用原分享卡的尺寸、色板、趋势、连带指标与排行；Core 未采集的会话时长/画像不编造。 */
export function UsageActivityShareCard({report,locale,theme,showProjectNames}:{report:UsageActivityReport;locale:string;theme:ShareCardTheme;showProjectNames:boolean}):React.JSX.Element {
  const t=useUiText(),total=report.totals
  const number=(value:number|null)=>value===null?'—':new Intl.NumberFormat(locale).format(value)
  const date=(value:string)=>new Date(value).toLocaleDateString(locale,{year:'numeric',month:'short',day:'numeric'})
  const cache=total.inputTokens!==null&&total.inputTokens>0&&total.cacheReadTokens!==null?total.cacheReadTokens/total.inputTokens:null
  const trend=useMemo(()=>{
    const points=report.series.map(row=>{const cache=row.inputTokens===null?0:row.cacheReadTokens??0;return {cache,fresh:Math.max(0,row.knownTotalTokens-cache)}})
    if(points.length<=48)return points
    const size=Math.ceil(points.length/48),merged:Array<{cache:number;fresh:number}>=[]
    for(let offset=0;offset<points.length;offset+=size){const chunk=points.slice(offset,offset+size);merged.push({cache:chunk.reduce((sum,row)=>sum+row.cache,0),fresh:chunk.reduce((sum,row)=>sum+row.fresh,0)})}
    return merged
  },[report.series])
  const panel={borderRadius:16,backgroundColor:theme.panel,border:`1px solid ${theme.border}`}
  const ranks=(rows:UsageActivityReport['byModel'],projects:boolean)=>rows.slice(0,4).map((row,index)=><RankRow key={row.key} rank={index+1} label={row.key==='__other__'?t('其他'):projects&&!showProjectNames?t('工程 {0}',index+1):row.key||t('未记录')} value={formatTokenCount(row.knownTotalTokens)} share={total.knownTotalTokens>0?row.knownTotalTokens/total.knownTotalTokens:0} theme={theme}/>)
  return <div style={{width:CARD_WIDTH,height:CARD_HEIGHT,position:'relative',overflow:'hidden',backgroundColor:theme.bg,color:theme.text,fontFamily:theme.fontFamily,display:'flex',flexDirection:'column',padding:'44px 44px 36px',boxSizing:'border-box'}}>
    <div style={{position:'absolute',inset:0,background:`radial-gradient(760px 420px at 12% -6%, ${theme.washA}, transparent 62%), radial-gradient(680px 460px at 105% 104%, ${theme.washB}, transparent 60%)`,pointerEvents:'none'}}/>
    <div style={{position:'relative',flex:1,display:'flex',flexDirection:'column',minHeight:0}}>
      <div style={{display:'flex',justifyContent:'space-between',gap:16}}><div><div style={{fontSize:27,fontWeight:700,letterSpacing:'-.02em'}}>{t('用量看板')}</div><div style={{marginTop:6,fontSize:14,color:theme.textMuted}}>{date(report.range.from)} – {date(new Date(new Date(report.range.to).getTime()-1).toISOString())}</div></div><span style={{alignSelf:'flex-start',padding:'8px 14px',borderRadius:999,background:theme.accentSoft,border:`1px solid ${theme.border}`,fontSize:13}}>{t('融合运行')}</span></div>
      <div style={{marginTop:34}}><div style={{fontSize:13,letterSpacing:'.08em',color:theme.textDim}}>{t('已报告 Token')}</div><div style={{marginTop:4,fontSize:84,fontWeight:750,letterSpacing:'-.04em',lineHeight:1}}>{formatTokenCount(total.knownTotalTokens)}</div><div style={{marginTop:12,fontSize:14,color:theme.textMuted}}>{t('仅统计已报告部分，未知数值显示为横线。')}</div></div>
      {trend.length>0&&<div style={{marginTop:26}}><TrendBars series={trend} theme={theme}/><div style={{display:'flex',gap:16,marginTop:10,fontSize:12,color:theme.textMuted}}><span>{t('缓存读取')}</span><span>{t('其他已报告 Token')}</span></div></div>}
      <div style={{...panel,marginTop:18,padding:'16px 18px',display:'flex',justifyContent:'space-between',gap:18}}><div><div style={{fontSize:11,color:theme.textDim}}>{t('缓存读取占比')}</div><div style={{marginTop:4,fontSize:30,fontWeight:700}}>{cache===null?'—':new Intl.NumberFormat(locale,{style:'percent',maximumFractionDigits:1}).format(cache)}</div></div><div style={{alignSelf:'center',fontSize:12,color:theme.textMuted}}>{t('缓存读取')} {number(total.cacheReadTokens)}<br/>{t('未完整报告次数')} {number(total.partialObservations)}</div></div>
      <div style={{...panel,marginTop:12,display:'flex'}}><StripCell label={t('输入 Token')} value={number(total.inputTokens)} theme={theme}/><StripCell divided label={t('输出 Token')} value={number(total.outputTokens)} theme={theme}/><StripCell divided label={t('运行次数')} value={number(total.runs)} theme={theme}/></div>
      <div style={{display:'flex',gap:28,marginTop:30}}><div style={{flex:1,minWidth:0}}><div style={{fontSize:12,color:theme.textDim,marginBottom:12}}>{t('模型排行')}</div>{ranks(report.byModel,false)}</div><div style={{flex:1,minWidth:0}}><div style={{fontSize:12,color:theme.textDim,marginBottom:12}}>{t('工程排行')}</div>{ranks(report.byProject,true)}</div></div>
      <div style={{marginTop:'auto',paddingTop:10,borderTop:`1px solid ${theme.border}`,fontSize:11,lineHeight:1.6,color:theme.textDim}}>{t('数据来源：融合 Core，分时精度 {0} 秒。',report.resolutionSeconds)}<br/>{t('生成于 {0}；不含导入原生历史，不代表账单。',new Date(report.generatedAt).toLocaleString(locale))}</div>
    </div>
  </div>
}
