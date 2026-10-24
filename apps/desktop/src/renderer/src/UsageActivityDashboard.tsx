import { useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from 'react'
import { Activity, ArrowDownRight, ArrowUpRight, RefreshCw } from 'lucide-react'
import type { UsageActivityQuery, UsageActivityReport } from '@contracts'
import { useThreadClient } from './camp-client'
import { useInterfaceLanguage, useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { ActivityHeatmap, RankedBars, RingMeter, TrendChart } from './codeg-native-content/components/token-usage/charts'
import { formatTokenCount } from './codeg-native-content/lib/token-format'
import './codeg-native-content/source-html.css'
import './codeg-native-content/source-usage-colors.css'
import './usage-activity-dashboard.css'
import { UsageShareDialog } from './UsageShareDialog'

const RANGES:Array<{id:UsageActivityQuery['range'];label:string}>=[{id:'7d',label:'近 7 天'},{id:'30d',label:'近 30 天'},{id:'90d',label:'近 90 天'},{id:'thisMonth',label:'本月'},{id:'thisYear',label:'今年'},{id:'all',label:'全部记录'},{id:'custom',label:'自选日期'}]

/** 布局/图表取自 Codeg token-usage-page/charts；事实来自同一次 Core 投影读取。 */
export function UsageActivityDashboard():React.JSX.Element {
  const client=useThreadClient(),t=useUiText(),ids=useId()
  const [query,setQuery]=useState<UsageActivityQuery>(()=>({range:'30d',bucket:'day',tzOffsetMinutes:-new Date().getTimezoneOffset()}))
  const [custom,setCustom]=useState({start:'',end:''}),[customOpen,setCustomOpen]=useState(false)
  const [report,setReport]=useState<UsageActivityReport|null>(null),[error,setError]=useState<string|null>(null),[loading,setLoading]=useState(true),[refresh,setRefresh]=useState(0)
  const [dimension,setDimension]=useState<'project'|'runtime'|'model'>('project'),[exporting,setExporting]=useState(false),[notice,setNotice]=useState<string|null>(null)
  const [share,setShare]=useState<UsageActivityReport|null>(null)
  const sequence=useRef(0),queryKey=JSON.stringify(query)
  useEffect(()=>{
    let alive=true;const observed=++sequence.current;setLoading(true);setError(null);setReport(null)
    const timer=setTimeout(()=>{void client.request<UsageActivityReport>('monitoring.report',query).then(value=>{if(alive&&observed===sequence.current)setReport(value)}).catch(reason=>{if(alive&&observed===sequence.current)setError(readErrorMessage(reason))}).finally(()=>{if(alive&&observed===sequence.current)setLoading(false)})},200)
    return()=>{alive=false;clearTimeout(timer)}
  },[client,queryKey,refresh])
  useEffect(()=>{
    let timer:ReturnType<typeof setTimeout>|undefined
    const schedule=():void=>{if(document.hidden||timer)return;timer=setTimeout(()=>{timer=undefined;setRefresh(value=>value+1)},1000)}
    const unsub=client.onEvent?.(event=>{if(event.method==='agent_run.terminal')schedule()})
    const invalidated=client.onInvalidated?.(schedule)
    document.addEventListener('visibilitychange',schedule)
    return()=>{if(timer)clearTimeout(timer);unsub?.();invalidated?.();document.removeEventListener('visibilitychange',schedule)}
  },[client])
  const selectRange=(range:UsageActivityQuery['range']):void=>{
    if(range==='custom'){setCustomOpen(true);return}
    setCustomOpen(false);setQuery(current=>({...current,range,start:undefined,end:undefined,bucket:range==='90d'?'week':range==='thisYear'||range==='all'?'month':'day'}))
  }
  const exportData=async():Promise<void>=>{
    if(!report||!client.exportUsageActivityReport)return
    setExporting(true);setNotice(null)
    try{const result=await client.exportUsageActivityReport(query);if(result.exported)setNotice(t('用量数据已导出。'))}catch(reason){setNotice(readErrorMessage(reason))}finally{setExporting(false)}
  }
  const changeFacet=(key:'projectRoot'|'runtimeKind'|'modelKey',value:string):void=>setQuery(current=>({...current,[key]:value||undefined}))
  return <div className="usage-activity-dashboard codeg-native-rich tu-viz">
    <header className="usage-dashboard-heading"><div><h1>{t('用量看板')}</h1><p>{t('查看已报告的 Token、缓存与活动分布。')}</p></div><div className="flex gap-2"><button type="button" className="quiet-button" disabled={loading} onClick={()=>setRefresh(value=>value+1)}><RefreshCw size={14}/>{t('刷新')}</button><button type="button" className="quiet-button" disabled={!report||loading||exporting||!client.exportUsageActivityReport} onClick={()=>void exportData()}>{t(exporting?'正在导出…':'导出 JSON')}</button><button type="button" className="primary-button" disabled={!report||loading} onClick={()=>report&&setShare(structuredClone(report))}>{t('分享')}</button></div></header>
    <div className="usage-dashboard-toolbar">
      <div className="usage-dashboard-ranges" role="group" aria-label={t('统计范围')}>{RANGES.map(range=><button className="quiet-button" type="button" key={range.id} aria-pressed={customOpen?range.id==='custom':query.range===range.id} onClick={()=>selectRange(range.id)}>{t(range.label)}</button>)}</div>
      <div className="usage-dashboard-filters">
        <label><span>{t('工程')}</span><input className="workspace-field" list={`${ids}-projects`} value={query.projectRoot??''} placeholder={t('全部工程')} onChange={event=>changeFacet('projectRoot',event.target.value)}/><datalist id={`${ids}-projects`}>{report?.facets.projects.slice(0,500).map(value=><option value={value} key={value}/>)}</datalist></label>
        <label><span>{t('智能体')}</span><select className="workspace-field" value={query.runtimeKind??''} onChange={event=>changeFacet('runtimeKind',event.target.value)}><option value="">{t('全部')}</option>{report?.facets.runtimes.slice(0,500).map(value=><option key={value} value={value}>{value}</option>)}</select></label>
        <label><span>{t('模型')}</span><input className="workspace-field" list={`${ids}-models`} value={query.modelKey??''} placeholder={t('全部模型')} onChange={event=>changeFacet('modelKey',event.target.value)}/><datalist id={`${ids}-models`}>{report?.facets.models.slice(0,500).map(value=><option value={value} key={value}/>)}</datalist></label>
        <label><span>{t('粒度')}</span><select className="workspace-field" value={query.bucket} onChange={event=>setQuery(current=>({...current,bucket:event.target.value as UsageActivityQuery['bucket']}))}><option value="day">{t('按天')}</option><option value="week">{t('按周')}</option><option value="month">{t('按月')}</option></select></label>
        {(query.projectRoot||query.runtimeKind||query.modelKey)&&<button type="button" className="quiet-button" onClick={()=>setQuery(current=>({...current,projectRoot:undefined,runtimeKind:undefined,modelKey:undefined}))}>{t('清除筛选')}</button>}
      </div>
      {customOpen&&<form className="usage-custom-dates" onSubmit={event=>{event.preventDefault();if(!custom.start||!custom.end||custom.start>custom.end)return;setQuery(current=>({...current,range:'custom',...custom}));setCustomOpen(false)}}><label>{t('开始日期')}<input className="workspace-field" type="date" value={custom.start} required onChange={event=>setCustom(current=>({...current,start:event.target.value}))}/></label><label>{t('结束日期')}<input className="workspace-field" type="date" value={custom.end} min={custom.start||undefined} required onChange={event=>setCustom(current=>({...current,end:event.target.value}))}/></label><button className="primary-button" type="submit">{t('应用')}</button></form>}
    </div>
    <main className="usage-dashboard-content" aria-busy={loading}>
      {notice&&<p role="status">{notice}</p>}
      {loading&&<div className="usage-stack" aria-label={t('正在读取用量…')}><div className="h-40 rounded-xl border bg-muted/40 animate-pulse"/><div className="h-64 rounded-xl border bg-muted/40 animate-pulse"/></div>}
      {error&&<div role="alert" className="usage-dashboard-state"><p>{error}</p><button type="button" className="quiet-button" onClick={()=>setRefresh(value=>value+1)}>{t('重试')}</button></div>}
      {report&&!loading&&<UsageActivityView report={report} dimension={dimension} onDimension={setDimension} onFilter={changeFacet}/>}
    </main>
    {share&&<UsageShareDialog report={share} onClose={()=>setShare(null)}/>}
  </div>
}

function Panel({title,hint,children,actions}:{title:string;hint?:string;children:ReactNode;actions?:ReactNode}):React.JSX.Element {
  return <section className="usage-panel"><header className="usage-panel-heading"><div className="min-w-0"><h2 className="flex items-center gap-1.5 text-[0.8125rem] font-semibold">{title}</h2>{hint&&<p className="mt-1 text-xs leading-relaxed text-muted-foreground">{hint}</p>}</div>{actions}</header>{children}</section>
}

export function UsageActivityView({report,dimension,onDimension,onFilter}:{report:UsageActivityReport;dimension:'project'|'runtime'|'model';onDimension(value:'project'|'runtime'|'model'):void;onFilter(key:'projectRoot'|'runtimeKind'|'modelKey',value:string):void}):React.JSX.Element {
  const language=useInterfaceLanguage(),t=useUiText(),total=report.totals,previous=report.previous.knownTotalTokens
  const number=(value:number|null):string=>value===null?'—':new Intl.NumberFormat(language).format(value)
  const date=(value:string):string=>new Date(value).toLocaleDateString(language)
  const ratio=total.inputTokens!==null&&total.inputTokens>0&&total.cacheReadTokens!==null?total.cacheReadTokens/total.inputTokens:null
  const delta=previous>0?(total.knownTotalTokens-previous)/previous:null
  const weekdays=useMemo(()=>Array.from({length:7},(_,index)=>new Intl.DateTimeFormat(language,{weekday:'short',timeZone:'UTC'}).format(new Date(Date.UTC(2024,0,1+index)))),[language])
  const heat=useMemo(()=>{const matrix=Array.from({length:7},()=>Array<number>(24).fill(0));let max=0;for(const cell of report.heatmap){matrix[cell.weekday][cell.hour]=cell.knownTotalTokens;max=Math.max(max,cell.knownTotalTokens)}return {matrix,max}},[report.heatmap])
  const formatAria=useCallback((day:number,hour:number,value:number)=>t('{0} {1} 时，已报告 {2} Token',weekdays[day],hour,number(value)),[language,weekdays])
  const series=report.series.map(point=>{const cache=point.inputTokens===null?0:point.cacheReadTokens??0;return {key:point.key,label:date(point.key+'T12:00:00'),cache,fresh:Math.max(0,point.knownTotalTokens-cache),detail:[{label:t('输入'),value:number(point.inputTokens)},{label:t('输出'),value:number(point.outputTokens)},{label:t('缓存读取'),value:number(point.cacheReadTokens)},{label:t('未完整报告次数'),value:number(point.partialObservations)}]}})
  const rows=dimension==='project'?report.byProject:dimension==='runtime'?report.byRuntime:report.byModel
  const ranked=rows.map(row=>({key:row.key,label:row.key==='__other__'?t('其他'):row.key||t('未记录'),value:row.knownTotalTokens,share:total.knownTotalTokens>0?row.knownTotalTokens/total.knownTotalTokens:null,hint:t('{0} 次运行',row.runs)}))
  const precisionLost=[total.inputTokens,total.outputTokens,total.cacheReadTokens,total.knownTotalTokens].some(value=>value!==null&&!Number.isSafeInteger(value))
  return <div className="usage-stack">
    <section className="usage-panel usage-total-panel">
      <div className="usage-total-heading"><div><p className="text-xs text-muted-foreground">{t('已报告 Token')}</p><div className="mt-2 flex flex-wrap items-center gap-3"><strong className="usage-total-value" title={number(total.knownTotalTokens)}>{formatTokenCount(total.knownTotalTokens)}</strong>{delta!==null&&Math.abs(delta)>=0.005&&<span className="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-1 text-xs">{delta>0?<ArrowUpRight size={13}/>:<ArrowDownRight size={13}/>} {new Intl.NumberFormat(language,{style:'percent',maximumFractionDigits:0,signDisplay:'always'}).format(delta)}</span>}</div><p className="mt-2 text-xs text-muted-foreground">{date(report.range.from)} – {date(report.range.to)} · {t('与上一同长度时段比较')}</p></div><RingMeter ratio={ratio??0} valueText={ratio===null?'—':new Intl.NumberFormat(language,{style:'percent',maximumFractionDigits:1}).format(ratio)} caption={t('缓存读取占比')}/></div>
      <dl className="usage-connected-stats border-t border-border">{[[t('输入 Token'),total.inputTokens],[t('输出 Token'),total.outputTokens],[t('缓存读取'),total.cacheReadTokens],[t('缓存写入'),total.cacheWriteTokens],[t('运行次数'),total.runs]].map(([label,value])=><div className="usage-stat" key={String(label)}><dt className="text-xs text-muted-foreground">{label}</dt><dd className="mt-1.5 text-xl font-semibold leading-tight tracking-tight">{number(value as number|null)}</dd></div>)}</dl>
    </section>
    {(total.partialObservations>0||precisionLost)&&<p className="usage-dashboard-source-note" role="status">{precisionLost?t('数值超过界面整数精度范围，请以原始导出记录核对。'):t('有 {0} 次用量记录未完整报告输入或输出，图中只统计已报告部分。',total.partialObservations)}</p>}
    <Panel title={t('用量趋势')} hint={t('缓存与其他已报告 Token；缺失数据保持未知。')}><TrendChart data={series} label={t('Token 用量趋势')} cacheLabel={t('已报告缓存读取')} freshLabel={t('其他已报告 Token')} emptyLabel={t('此范围还没有分时用量记录。')}/></Panel>
    <div className="usage-dashboard-two-panels">
      <Panel title={t('活动热力图')} hint={t('按星期和小时查看已报告 Token。')}><ActivityHeatmap matrix={heat.matrix} max={heat.max} weekdayLabels={weekdays} formatAria={formatAria} legendLess={t('少')} legendMore={t('多')}/></Panel>
      <Panel title={t('用量分布')} actions={<select className="workspace-field" aria-label={t('排行维度')} value={dimension} onChange={event=>onDimension(event.target.value as typeof dimension)}>{(['project','runtime','model'] as const).map((value,index)=><option value={value} key={value}>{t(['工程','智能体','模型'][index])}</option>)}</select>}>
        <RankedBars data={ranked} showRank emptyLabel={t('此范围还没有分时用量记录。')} onSelect={dimension==='model'?undefined:key=>{if(key&&key!=='__other__')onFilter(dimension==='project'?'projectRoot':'runtimeKind',key)}}/>
      </Panel>
    </div>
    <Panel title={t('运行归因成本')} hint={t('沿用原运行记录的来源和估算标记；跨时段任务按整次运行显示。')}><div className="flex flex-wrap gap-4">{report.runCost.length?report.runCost.map((cost,index)=><div key={index}><strong>{cost.amount} {cost.currency}</strong><p className="mt-1 text-xs text-muted-foreground">{cost.kind} · {cost.source}</p></div>):<p className="text-sm text-muted-foreground">{t('此范围没有可归因成本记录。')}</p>}</div></Panel>
    <p className="usage-dashboard-source-note"><Activity size={13}/>{t('分时记录始于 {0}，保留最近 {1} 天。旧版汇总仍可在运行监控查看；此页不合并外部原生历史。',date(report.recordingStartedAt),report.retentionDays)}</p>
  </div>
}
