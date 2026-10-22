import { useEffect, useRef, useState } from 'react'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import './interaction-workspace.css'

export interface InteractionFeatures {browser:boolean;browserEval:boolean;computer:boolean;computerLaunch:boolean;computerClipboard:boolean}
interface Policy {schemaVersion:1;revision:number;members:Array<{agentId:string;features:InteractionFeatures}>}
const empty=():InteractionFeatures=>({browser:false,browserEval:false,computer:false,computerLaunch:false,computerClipboard:false})
const rows:Array<{key:keyof InteractionFeatures;title:string;description:string;parent?:keyof InteractionFeatures}>=[
  {key:'browser',title:'浏览器',description:'提供读页面、截图、点击、输入等工具；每张页面仍需单独共享。'},
  {key:'browserEval',title:'网页代码',description:'在已共享页面运行代码，每段代码都要由你确认。',parent:'browser'},
  {key:'computer',title:'电脑操作',description:'提供窗口内容和操作工具；只看或操作由你按会话选择。'},
  {key:'computerLaunch',title:'启动应用与调整窗口',description:'在电脑操作之外，单独允许请求启动应用或调整窗口。',parent:'computer'},
  {key:'computerClipboard',title:'剪贴板',description:'单独配备读取、写入剪贴板的工具；共享与本地确认仍分别生效。',parent:'computer'},
]

/** 以 Codeg AgentToolsSettings/SettingRow 的五组开关和 Rovai 队员详情节布局呈现。 */
export function MemberInteractionSettings({agentId,disabled=false}:{agentId:string;disabled?:boolean}):React.JSX.Element|null{
  const api=useThreadClient().interactionWorkspace,t=useUiText()
  const [base,setBase]=useState<Policy|null>(null),[features,setFeatures]=useState(empty),[busy,setBusy]=useState(false),[error,setError]=useState(''),[pending,setPending]=useState<{expectedRevision:number;value:Policy}|null>(null)
  const lifetime=useRef(0)
  useEffect(()=>{let active=true;lifetime.current++;setBase(null);setFeatures(empty());setError('');setPending(null);void api?.request<Policy>('policy.get',{}).then(value=>{if(active){setBase(value);setFeatures(value.members.find(member=>member.agentId===agentId)?.features??empty())}}).catch(error=>{if(active)setError(String(error))});return()=>{active=false;lifetime.current++}},[api,agentId])
  if(!api)return null
  const save=async(original?:NonNullable<typeof pending>):Promise<void>=>{
    if(!base||busy)return
    const request=original??{expectedRevision:base.revision,value:{...base,revision:base.revision+1,members:[...base.members.filter(member=>member.agentId!==agentId),{agentId,features:{...features}}]}}
    const generation=lifetime.current;setBusy(true);setError('');setPending(request)
    try{const value=await api.request<Policy>('policy.replace',request);if(lifetime.current===generation){setBase(value);setFeatures(value.members.find(member=>member.agentId===agentId)?.features??empty());setPending(null)}}
    catch(error){if(lifetime.current===generation)setError(String(error))}
    finally{if(lifetime.current===generation)setBusy(false)}
  }
  const reread=async():Promise<void>=>{
    if(busy)return
    const generation=lifetime.current;setBusy(true);setError('')
    try{const value=await api.request<Policy>('policy.get',{});if(lifetime.current!==generation)return
      if(pending&&JSON.stringify(value)===JSON.stringify(pending.value)){setPending(null);setBase(value);setFeatures(value.members.find(member=>member.agentId===agentId)?.features??empty())}
      else if(pending){setError(t(value.revision>pending.expectedRevision?'设置已变化，原请求不能再覆盖。可放弃本机原计划后重新读取。':'原请求尚未确认，可原样重试。'))}
      else{setBase(value);setFeatures(value.members.find(member=>member.agentId===agentId)?.features??empty())}
    }catch(error){if(lifetime.current===generation)setError(String(error))}finally{if(lifetime.current===generation)setBusy(false)}
  }
  return <section className="member-editor-section interaction-member-settings" aria-label={t('浏览器与电脑工具')}>
    <div className="member-editor-section-heading"><h2>{t('浏览器与电脑工具')}</h2><span>{t('用于之后开始的新执行')}</span></div>
    <p className="interaction-help">{t('配备工具后，在页面或窗口中选择具体会话和队员。原有历史会保留；必要时会重新准备会话。')}</p>
    <div className="interaction-setting-rows">{rows.map(row=><label className="interaction-setting-row" key={row.key}><span><strong>{t(row.title)}</strong><small>{t(row.description)}</small></span><input type="checkbox" role="switch" checked={features[row.key]} disabled={disabled||busy||!base||Boolean(pending)||Boolean(row.parent&&!features[row.parent])} onChange={event=>{const checked=event.target.checked;setFeatures(value=>({...value,[row.key]:checked,...(row.key==='browser'&&!checked?{browserEval:false}:{}),...(row.key==='computer'&&!checked?{computerLaunch:false,computerClipboard:false}:{})}))}}/></label>)}</div>
    {error&&<p role="alert" className="environment-error">{error}</p>}
    <div className="interaction-actions"><button type="button" className="quiet-button" disabled={busy} onClick={()=>void reread()}>{t('重新读取')}</button>{pending?<><button type="button" disabled={busy} onClick={()=>void save(pending)}>{t('原样重试')}</button><button type="button" disabled={busy} onClick={()=>{setPending(null);setError(t('原计划已留在当前显示中。请重新读取后再编辑。'));setBase(null)}}>{t('放弃本机原计划')}</button></>:<button type="button" disabled={disabled||busy||!base||JSON.stringify(features)===JSON.stringify(base.members.find(member=>member.agentId===agentId)?.features??empty())} onClick={()=>void save()}>{t(busy?'正在保存…':'保存工具配置')}</button>}</div>
  </section>
}
