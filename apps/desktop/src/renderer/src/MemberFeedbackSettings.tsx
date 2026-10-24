import { useEffect,useRef,useState } from 'react'
import type { FeedbackPolicy } from '@contracts'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { readErrorMessage } from './error-message'

type Update={schemaVersion:1;expectedRevision:number;members:FeedbackPolicy['members']}
const same=(a:FeedbackPolicy['members'],b:FeedbackPolicy['members'])=>a.length===b.length&&a.every(row=>b.some(other=>other.agentId===row.agentId&&other.enabled===row.enabled))
/** 独立开关沿用原队员工具设置行；不会热改既有 Run 的冻结目录。 */
export function MemberFeedbackSettings({agentId,disabled=false}:{agentId:string;disabled?:boolean}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0)
  const [base,setBase]=useState<FeedbackPolicy|null>(null),[enabled,setEnabled]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState(''),[pending,setPending]=useState<Update|null>(null)
  const request=<T,>(action:string,payload:unknown)=>client.request<T>('feedback.operation',{action,payload})
  useEffect(()=>{const observed=++generation.current;setBase(null);setPending(null);setError('');void request<FeedbackPolicy>('policy.get',{}).then(value=>{if(observed===generation.current){setBase(value);setEnabled(value.members.find(row=>row.agentId===agentId)?.enabled??false)}}).catch(reason=>{if(observed===generation.current)setError(readErrorMessage(reason))});return()=>{generation.current++}},[client,agentId])
  const save=async(original?:Update):Promise<void>=>{
    if(busy||!base&&!original)return
    const payload=original??{schemaVersion:1 as const,expectedRevision:base!.revision,members:[...base!.members.filter(row=>row.agentId!==agentId),{agentId,enabled}]};const observed=generation.current;setPending(payload);setBusy(true);setError('')
    try{const value=await request<FeedbackPolicy>('policy.set',payload);if(observed===generation.current){setBase(value);setEnabled(value.members.find(row=>row.agentId===agentId)?.enabled??false);setPending(null)}}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  const reread=async():Promise<void>=>{
    const observed=generation.current;setBusy(true);setError('')
    try{const value=await request<FeedbackPolicy>('policy.get',{});if(observed!==generation.current)return
      if(pending&&value.revision===pending.expectedRevision+1&&same(value.members,pending.members)){setPending(null);setBase(value);setEnabled(value.members.find(row=>row.agentId===agentId)?.enabled??false)}
      else if(pending)setError(t(value.revision>pending.expectedRevision?'设置已变化，原请求不能再覆盖；放弃本机原计划后重新读取。':'原保存尚未确认，可原样重试。'))
      else{setBase(value);setEnabled(value.members.find(row=>row.agentId===agentId)?.enabled??false)}
    }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  return <section className="member-editor-section interaction-member-settings" aria-label={t('工作中接收补充')}><div className="member-editor-section-heading"><h2>{t('工作中接收补充')}</h2><span>{t('用于之后开始的新执行')}</span></div><label className="interaction-setting-row"><span><strong>{t('文字与卡片反馈工具')}</strong><small>{t('只有你确认提交的文字和卡片选择，才交给对应会话的当前队员。')}</small></span><input className="workspace-check" type="checkbox" role="switch" checked={enabled} disabled={disabled||busy||!base||Boolean(pending)} onChange={event=>setEnabled(event.target.checked)}/></label><p className="interaction-help">{t('默认关闭。开启后从新任务生效；已有任务和其他队员不会自动增加工具。不支持反馈工具时可在普通输入框继续。')}</p>{error&&<p role="alert" className="environment-error">{error}</p>}<div className="interaction-actions"><button className="quiet-button" type="button" disabled={busy} onClick={()=>void reread()}>{t('核对设置')}</button>{pending?<><button className="quiet-button" type="button" disabled={busy} onClick={()=>void save(pending)}>{t('原样重试')}</button><button className="quiet-button" type="button" disabled={busy} onClick={()=>{setPending(null);setBase(null);setError(t('请重新读取后再编辑。'))}}>{t('放弃本机原计划')}</button></>:<button className="quiet-button" type="button" disabled={disabled||busy||!base||enabled===(base.members.find(row=>row.agentId===agentId)?.enabled??false)} onClick={()=>void save()}>{t(busy?'正在保存…':'保存反馈设置')}</button>}</div></section>
}
