import {useEffect,useRef,useState} from 'react'
import {customRuntimeReference,type AgentProfile,type CustomRuntimeDefinition,type CustomRuntimeDefinitionPage,type SetMemberRuntimeConfigurationCommand,type StoredCommandResult} from '@contracts'
import {newCommandId} from '../../shared/command-id'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'
import {assertApplied} from './member-runtime-commands'

type Original={commandId:string;command:SetMemberRuntimeConfigurationCommand}
/** 保留原队员配置区；自定义程序使用具体定义引用，不借用任一内置安装。 */
export function MemberCustomRuntime({agent,disabled,onSaved}:{agent:AgentProfile;disabled:boolean;onSaved():Promise<void>}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0)
  const [page,setPage]=useState<CustomRuntimeDefinitionPage>({items:[],next:null}),[selected,setSelected]=useState(agent.runtimeConfiguration?.customRuntimeRef?.id??''),[busy,setBusy]=useState(false),[error,setError]=useState(''),[original,setOriginal]=useState<Original|null>(null)
  const key=client.presentationScope?`custom-member-runtime:${client.presentationScope}:${agent.agentId}`:null
  const retain=(value:Original|null):void=>{setOriginal(value);if(key)client.editingRecovery?.set(key,value)}
  const load=async(more=false):Promise<void>=>{const observed=generation.current;try{const next=await client.request<CustomRuntimeDefinitionPage>('runtime.custom.list',{after:more?page.next:null,limit:100});if(observed===generation.current)setPage(old=>more?{...next,items:[...old.items,...next.items]}:next)}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}}
  useEffect(()=>{generation.current++;setPage({items:[],next:null});setSelected(agent.runtimeConfiguration?.customRuntimeRef?.id??'');setError('');const saved=key?client.editingRecovery?.get(key) as Original|null:null;setOriginal(saved?.command?.agentId===agent.agentId&&saved.command.adapterKind==='custom-acp'?saved:null);void load();return()=>{generation.current++}},[client,agent.agentId,key])
  const current=page.items.find(row=>row.id===selected),saved=agent.runtimeConfiguration?.customRuntimeRef
  const apply=async(reconcile=false):Promise<void>=>{
    if(busy||disabled||!current&&!original)return;const observed=generation.current;setBusy(true);setError('')
    const request=original??{commandId:newCommandId(),command:{agentId:agent.agentId,expectedVersion:agent.version,adapterKind:'custom-acp' as const,model:{mode:'runtime_default' as const},permissions:{adapterKind:'custom-acp' as const,schemaVersion:1,values:{}},customRuntimeRef:customRuntimeReference(current as CustomRuntimeDefinition)}}
    retain(request)
    try{let result:StoredCommandResult
      if(reconcile){const value=await client.request<{state:'unknown'|'recorded';result?:StoredCommandResult}>('commands.reconcile',{operation:'members.runtime.set',params:request});if(observed!==generation.current)return;if(value.state==='unknown'||!value.result){setError(t('原配置尚无确定回执，没有重复保存。'));return}result=value.result}
      else result=await client.request<StoredCommandResult>('members.runtime.set',request)
      if(observed!==generation.current)return;retain(null);assertApplied(result);await onSaved()
    }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  return <section className="member-editor-section"><div className="member-editor-section-heading"><h2>{t('自定义智能体程序')}</h2><span>{t('从已添加的具体版本中选择')}</span></div><label className="interaction-setting-row"><span>{t('程序')}</span><select value={selected} disabled={disabled||busy||Boolean(original)} onChange={event=>setSelected(event.target.value)}><option value="">{t('选择自定义智能体')}</option>{page.items.map(row=><option value={row.id} key={row.id}>{row.name} · {row.version} · r{row.revision}{!row.enabled?' · '+t('已停用'):''}</option>)}{saved&&!page.items.some(row=>row.id===saved.id)&&<option value={saved.id}>{saved.id} · r{saved.definitionRevision}</option>}</select></label>{page.next&&<button className="quiet-button" type="button" onClick={()=>void load(true)}>{t('更多程序')}</button>}
    <p className="interaction-help">{t('保存只影响之后的任务。原任务仍使用原安装和秘密版本；先在智能体设置中安装所选版本。')}</p>{saved&&<p className="interaction-help">{t('当前保存的定义：{0}，修订 {1}',saved.id,saved.definitionRevision)}</p>}{current&&<p className="interaction-help">{t('本次选择：{0}，修订 {1}',current.name,current.revision)}</p>}
    {original?<div className="interaction-actions"><code>{original.commandId}</code><button className="quiet-button" disabled={busy} onClick={()=>void apply(true)}>{t('核对原配置请求')}</button></div>:<button className="quiet-button" disabled={disabled||busy||!current?.enabled||Boolean(current&&saved?.definitionDigest===current.definitionDigest&&agent.runtimeConfiguration?.adapterKind==='custom-acp')} onClick={()=>void apply()}>{t(busy?'正在保存…':'使用所选程序')}</button>}{error&&<p role="alert" className="inline-error">{error}</p>}
  </section>
}
