import {useEffect,useRef,useState} from 'react'
import type {NativeSteerPolicy,StoredCommandResult} from '@contracts'
import {newCommandId} from '../../shared/command-id'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'
import {assertApplied} from './member-runtime-commands'

type Original={commandId:string;schemaVersion:1;expectedRevision:number;members:NativeSteerPolicy['members']}
function decode(value:unknown):Original|null{
  if(!value||typeof value!=='object')return null
  const row=value as Partial<Original>
  if(typeof row.commandId!=='string'||!/^[0-9a-f-]{36}$/.test(row.commandId)||row.schemaVersion!==1||!Number.isSafeInteger(row.expectedRevision)||!Array.isArray(row.members)||row.members.length>4096||row.members.some(member=>!member||typeof member.agentId!=='string'||typeof member.enabled!=='boolean'))return null
  return row as Original
}
/** 复用原 Rovai 队员设置行及私有编辑器恢复目录，不另造设置外观。 */
export function MemberNativeSteerSettings({agentId,disabled=false}:{agentId:string;disabled?:boolean}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0),busyRef=useRef(false)
  const [base,setBase]=useState<NativeSteerPolicy|null>(null),[enabled,setEnabled]=useState(false),[pending,setPending]=useState<Original|null>(null),[ready,setReady]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState('')
  const key=client.presentationScope?`native-steer-policy:${client.presentationScope}:${agentId}`:null
  const retain=async(value:Original|null):Promise<void>=>{
    if(!key)throw new Error(t('工作区身份尚未确定，本次未发送。'))
    if(client.workbenchEditorRecovery){await client.workbenchEditorRecovery.put({schemaVersion:1,key,editorId:'native-lifecycle',pending:value});return}
    if(!client.editingRecovery)throw new Error(t('原请求恢复存储尚不可用，本次未发送。'))
    client.editingRecovery.set(key,value)
  }
  const load=async():Promise<void>=>{
    const observed=generation.current;setReady(false);setError('')
    try{
      if(!key)throw new Error(t('工作区身份尚未确定，本次未发送。'))
      let raw:unknown
      if(client.workbenchEditorRecovery){const record=await client.workbenchEditorRecovery.get(key);if(record!==null){const item=record as {key?:string;editorId?:string;pending?:unknown};if(item.key!==key||item.editorId!=='native-lifecycle')throw new Error(t('原请求记录归属不同，保留原记录。'));raw=item.pending}}
      else{if(!client.editingRecovery)throw new Error(t('原请求恢复存储尚不可用，本次未发送。'));raw=client.editingRecovery.get(key)}
      const original=raw==null?null:decode(raw);if(raw!=null&&!original)throw new Error(t('原请求记录损坏，保留原记录。'))
      const value=await client.request<NativeSteerPolicy>('runtime.steer.policy.get',{})
      if(observed!==generation.current)return
      setBase(value);setEnabled(value.members.find(member=>member.agentId===agentId)?.enabled??false);setPending(original);setReady(true)
    }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}
  }
  useEffect(()=>{generation.current++;busyRef.current=false;setBusy(false);setPending(null);setBase(null);void load();return()=>{generation.current++}},[client,agentId,key])
  const accept=async(result:StoredCommandResult,original:Original,observed:number):Promise<void>=>{
    if(result.commandId!==original.commandId||result.commandType!=='runtime.steer.policy.set')throw new Error(t('原回执的操作归属不同，请保留编号并核对。'))
    if(result.status==='rejected'){await retain(null);if(observed===generation.current){setPending(null);setBase(null)}assertApplied(result);return}
    const value=result.payload as unknown as NativeSteerPolicy
    if(value.schemaVersion!==1||value.revision!==original.expectedRevision+1||!Array.isArray(value.members))throw new Error(t('原请求尚无确定回执，未重复发送。'))
    await retain(null);if(observed===generation.current){setPending(null);setBase(value);setEnabled(value.members.find(member=>member.agentId===agentId)?.enabled??false)}
  }
  const save=async():Promise<void>=>{
    if(busyRef.current||!ready||!base||pending)return
    busyRef.current=true;setBusy(true);setError('');const observed=generation.current
    const original:Original={commandId:newCommandId(),schemaVersion:1,expectedRevision:base.revision,members:[...base.members.filter(member=>member.agentId!==agentId),{agentId,enabled}]}
    try{await retain(original);if(observed!==generation.current)return;setPending(original);const result=await client.request<StoredCommandResult>('runtime.steer.policy.set',original);await accept(result,original,observed)}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current){busyRef.current=false;setBusy(false)}}
  }
  const reconcile=async():Promise<void>=>{
    if(busyRef.current||!pending)return
    busyRef.current=true;setBusy(true);setError('');const original=pending,observed=generation.current
    try{const value=await client.request<{state:'unknown'|'recorded';result?:StoredCommandResult}>('commands.reconcile',{operation:'runtime.steer.policy.set',params:{commandId:original.commandId}});if(observed!==generation.current)return;if(value.state!=='recorded'||!value.result)throw new Error(t('原请求尚无确定回执，未重复发送。'));await accept(value.result,original,observed)}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current){busyRef.current=false;setBusy(false)}}
  }
  return <section className="member-editor-section interaction-member-settings" aria-label={t('原生插话')}><div className="member-editor-section-heading"><h2>{t('原生插话')}</h2><span>{t('用于之后开始的新执行')}</span></div><label className="interaction-setting-row"><span><strong>{t('工作中直接接收完整补充')}</strong><small>{t('确认后将文字和图片送给对应队员的当前轮次；需要该次原生连接实际支持。')}</small></span><input type="checkbox" role="switch" checked={enabled} disabled={disabled||busy||!ready||!!pending} onChange={event=>setEnabled(event.target.checked)}/></label><p className="interaction-help">{t('默认关闭。已有任务不新增通道；不支持直接插话时，仍可使用已开启的反馈工具或普通下一轮消息。')}</p>{error&&<p role="alert" className="environment-error">{error}</p>}<div className="interaction-actions">{pending?<button type="button" disabled={busy} onClick={()=>void reconcile()}>{t('核对原保存请求')}</button>:<><button className="quiet-button" type="button" disabled={busy} onClick={()=>void load()}>{t('重新读取')}</button><button type="button" disabled={disabled||busy||!ready||!base||enabled===(base.members.find(member=>member.agentId===agentId)?.enabled??false)} onClick={()=>void save()}>{t(busy?'正在保存…':'保存原生插话设置')}</button></>}</div></section>
}
