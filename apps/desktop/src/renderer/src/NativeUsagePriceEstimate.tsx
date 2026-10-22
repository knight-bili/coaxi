import { useEffect,useMemo,useRef,useState } from 'react'
import type { PublicUsagePrices,TokenUsageReport } from '@contracts'
import { useThreadClient } from './camp-client'
import { useInterfaceLanguage,useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { buildRateIndex,estimateReportCost } from './codeg-native-content/lib/model-api-rates'

/** 与原 TokenUsagePage 相同的公开价目估算，不覆盖实际成本或订阅账单。 */
export function NativeUsagePriceEstimate({report}:{report:TokenUsageReport}):React.JSX.Element {
  const client=useThreadClient(),language=useInterfaceLanguage(),t=useUiText(),generation=useRef(0)
  const [catalog,setCatalog]=useState<PublicUsagePrices|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null)
  useEffect(()=>{const observed=++generation.current;setCatalog(null);void client.request<PublicUsagePrices>('monitoring.prices',{}).then(value=>{if(observed===generation.current)setCatalog(value)}).catch(reason=>{if(observed===generation.current)setError(readErrorMessage(reason))});return()=>{generation.current++}},[client])
  const index=useMemo(()=>catalog?.available?buildRateIndex(catalog.providers):null,[catalog])
  const estimate=useMemo(()=>index?estimateReportCost(report.by_model,index):null,[report,index])
  const refresh=async():Promise<void>=>{
    if(busy)return;const observed=++generation.current;setBusy(true);setError(null)
    try{const value=await client.request<PublicUsagePrices>('monitoring.prices',{refresh:true});if(observed===generation.current)setCatalog(value)}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  return <section className="rounded-xl border border-border bg-card p-4"><header className="mb-3 flex flex-wrap items-center justify-between gap-3"><div><h2 className="text-[0.8125rem] font-semibold">{t('API 价目估算')}</h2><p className="mt-1 text-xs text-muted-foreground">{t('按公开每百万 Token 价目换算，不代表订阅扣款或剩余额度。')}</p></div><button type="button" className="quiet-button" disabled={busy} onClick={()=>void refresh()}>{t(busy?'正在读取…':catalog?.available?'更新公开价目':'读取公开价目')}</button></header>
    <strong className="text-xl font-semibold">{estimate&&estimate.coverage>0?new Intl.NumberFormat(language,{style:'currency',currency:'USD',maximumFractionDigits:4}).format(estimate.usd):'—'}</strong>
    {estimate&&<p className="mt-2 text-xs text-muted-foreground">{t('已定价覆盖 {0}，尚未定价 {1} Token。',new Intl.NumberFormat(language,{style:'percent',maximumFractionDigits:1}).format(estimate.coverage),new Intl.NumberFormat(language).format(estimate.unpricedTokens))}</p>}
    {catalog?.fetchedAt&&<p className="mt-2 text-xs text-muted-foreground">{t('价目来源：{0} · 读取于 {1}',catalog.source,new Date(catalog.fetchedAt).toLocaleString(language))}{catalog.stale&&' · '+t('价目已超过 24 小时')}</p>}
    {(error||catalog?.refreshError)&&<p className="mt-2 text-xs text-muted-foreground" role="status">{t('公开价目暂时无法更新；已保存的价目和原用量均保留。')} {error??catalog?.refreshError}</p>}
  </section>
}
