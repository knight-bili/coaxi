import { SettingsDisclosureHint } from './SettingsDisclosureHint'
import {useEffect,useRef,useState} from 'react'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'

type Settings={level:string;targets:Array<{target:string;level:string}>}
type View={settings:Settings;digest:string;writable?:boolean;scope?:'structuredHost'}
const options=[['off','关闭'],['error','错误'],['warn','警告'],['info','信息'],['debug','调试'],['trace','追踪']]

/** 参考 Codeg LogsSettings 的级别选择和按目标覆盖，放在原 Rovai 诊断容器。 */
export function LocalLoggingSettings():React.JSX.Element|null{
  const client=useThreadClient(),api=client.portableConfiguration?.logging,t=useUiText(),generation=useRef(0)
  const [saved,setSaved]=useState<View|null>(null),[draft,setDraft]=useState<Settings>({level:'trace',targets:[]}),[busy,setBusy]=useState(false),[error,setError]=useState('')
  const dirty=saved!==null&&JSON.stringify(draft)!==JSON.stringify(saved.settings)
  useEffect(()=>{const observed=++generation.current;setSaved(null);setError('');if(api)void api({operation:'get'}).then(value=>{if(observed===generation.current){setSaved(value);setDraft(value.settings)}}).catch(reason=>{if(observed===generation.current)setError(readErrorMessage(reason))});return()=>{generation.current++}},[client,client.presentationScope])
  useEffect(()=>{const before=(event:BeforeUnloadEvent):void=>{if(dirty||busy){event.preventDefault();event.returnValue=''}};window.addEventListener('beforeunload',before);return()=>window.removeEventListener('beforeunload',before)},[dirty,busy])
  if(!api||saved?.writable===false)return null
  const save=async():Promise<void>=>{
    if(!saved||busy)return
    const observed=generation.current;setBusy(true);setError('')
    try{const seen=new Set<string>();if(draft.targets.some(row=>!(/^[a-zA-Z0-9_.:-]{1,256}$/).test(row.target)||seen.has(row.target)||!seen.add(row.target)))throw new Error(t('请填写不重复的日志目标名称，只使用字母、数字、下划线、点、冒号和横线。'));const value=await api({operation:'set',expectedDigest:saved.digest,settings:draft});if(observed===generation.current){setSaved(value);setDraft(value.settings)}}
    catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  const change=(index:number,patch:Partial<Settings['targets'][number]>):void=>setDraft(value=>({...value,targets:value.targets.map((row,rowIndex)=>rowIndex===index?{...row,...patch}:row)}))
  return <section className="diagnostics-section" aria-label={t('日志输出设置')}>
    <div className="section-heading"><h3>{t('日志输出')}</h3><p>{saved?.scope==='structuredHost'?t('设置独立主机结构化诊断日志的详细程度；按目标覆盖匹配程序里的日志名称。原始故障记录、运行状态、审批和操作回执照常保留。'):t('设置主机桌面控制台的详细程度；按目标覆盖只影响匹配名称的日志。运行状态、审批和操作回执照常显示。')}</p></div>
    <form className="forge-form" onSubmit={event=>{event.preventDefault();void save()}}><fieldset disabled={busy||!saved}>
      <label>{t('默认级别')}<select className="workspace-field" value={draft.level} onChange={event=>setDraft(value=>({...value,level:event.target.value}))}>{options.map(([value,label])=><option key={value} value={value}>{t(label)}</option>)}</select></label>
      <details><summary>{t('按目标覆盖')}<SettingsDisclosureHint /></summary>{draft.targets.map((row,index)=><div className="portable-provider-actions" key={index}><input className="workspace-field" aria-label={t('日志目标')} value={row.target} onChange={event=>change(index,{target:event.target.value})}/><select className="workspace-field" aria-label={t('目标日志级别')} value={row.level} onChange={event=>change(index,{level:event.target.value})}>{options.map(([value,label])=><option key={value} value={value}>{t(label)}</option>)}</select><button className="quiet-button" type="button" onClick={()=>setDraft(value=>({...value,targets:value.targets.filter((_,rowIndex)=>rowIndex!==index)}))}>{t('移除')}</button></div>)}<button className="quiet-button" type="button" disabled={draft.targets.length>=1024} onClick={()=>setDraft(value=>({...value,targets:[...value.targets,{target:'',level:'info'}]}))}>{t('添加日志目标')}</button></details>
      <div className="portable-provider-actions"><button className="quiet-button" type="button" disabled={busy} onClick={()=>{const observed=generation.current;setBusy(true);setError('');void api({operation:'get'}).then(value=>{if(observed===generation.current){setSaved(value);setDraft(value.settings)}}).catch(reason=>{if(observed===generation.current)setError(readErrorMessage(reason))}).finally(()=>{if(observed===generation.current)setBusy(false)})}}>{t('重新读取已保存设置')}</button><button className="primary-button" type="submit" disabled={!dirty}>{t('保存日志设置')}</button><button className="quiet-button" type="button" disabled={!dirty} onClick={()=>{if(saved)setDraft(saved.settings)}}>{t('放弃未保存修改')}</button></div>
    </fieldset></form>{error&&<p className="inline-error" role="alert">{error}</p>}
  </section>
}
