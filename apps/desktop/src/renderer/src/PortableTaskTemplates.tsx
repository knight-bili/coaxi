import {useEffect,useRef,useState} from 'react'
import type {PortableTaskTemplate,PortableTaskTemplateReceipt} from '@contracts'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'
import {newCommandId} from '../../shared/command-id'
import {loadPortablePending,savePortablePending,type PortableConfigurationPending} from './portable-configuration-recovery'

type Row=Omit<PortableTaskTemplate,'config'>
type Draft={id:string;revision:number|null;name:string;title:string;body:string;config:Record<string,unknown>}
function original(row:PortableTaskTemplate):Draft{
  const config:unknown=JSON.parse(row.config)
  if(!config||typeof config!=='object'||Array.isArray(config))throw new Error('这份原模板需要补齐配置，原内容仍保留在库中。')
  const fields=config as Record<string,unknown>
  return {id:row.id,revision:row.revision,name:row.name,title:row.title,body:typeof fields.display_text==='string'?fields.display_text:'',config:fields}
}
function value(draft:Draft):{name:string;title:string;config:string}{
  const blocks=Array.isArray(draft.config.prompt_blocks)?draft.config.prompt_blocks:[]
  let written=false
  const prompt=blocks.flatMap(block=>{
    if(block&&typeof block==='object'&&(block as Record<string,unknown>).type==='text'){
      if(written)return [];written=true;return [{type:'text',text:draft.body}]
    }
    return [block]
  })
  if(!written)prompt.unshift({type:'text',text:draft.body})
  return {name:draft.name,title:draft.title,config:JSON.stringify({...draft.config,display_text:draft.body,prompt_blocks:prompt})}
}

/** 参考 Coaxi task-editor-dialog 的模板列表、保存和删除；配置仍由同一 Host 库拥有。 */
export function PortableTaskTemplates():React.JSX.Element|null{
  const client=useThreadClient(),api=client.portableConfiguration?.templates,t=useUiText(),generation=useRef(0)
  const key=client.presentationScope?`portable-task-templates:${client.presentationScope}`:null
  const [rows,setRows]=useState<Row[]>([]),[next,setNext]=useState<string|null>(null),[draft,setDraft]=useState<Draft|null>(null),[baseline,setBaseline]=useState('')
  const [pending,setPending]=useState<PortableConfigurationPending|null>(null),[receipt,setReceipt]=useState<PortableTaskTemplateReceipt|null>(null),[busy,setBusy]=useState(false),[ready,setReady]=useState(false),[error,setError]=useState(''),[message,setMessage]=useState(''),[removing,setRemoving]=useState(false)
  const dirty=draft!==null&&JSON.stringify(draft)!==baseline
  const list=async(more=false):Promise<void>=>{
    if(!api)return;const observed=generation.current
    const result=await api<{items:Row[];next:string|null}>({operation:'list',after:more?next:null,limit:100})
    if(observed===generation.current){setRows(previous=>more?[...previous,...result.items.filter(row=>!previous.some(item=>item.id===row.id))]:result.items);setNext(result.next)}
  }
  const remember=async(value:PortableConfigurationPending|null):Promise<void>=>{if(!key)throw new Error(t('工作区身份尚未确定。'));const observed=generation.current;await savePortablePending(client,key,value);if(observed===generation.current)setPending(value)}
  const settle=async(result:PortableTaskTemplateReceipt,id:string):Promise<void>=>{
    const observed=generation.current
    if(result.commandId!==id)throw new Error(t('原模板操作的回执编号不同。'));setReceipt(result)
    if(result.state==='unknown'){setMessage(t('尚未查到原模板操作的结果，保留原请求并继续核对。'));return}
    if(result.result?.status==='rejected'){await remember(null);throw new Error(t('模板已在其他位置变化，请先重新读取。当前编辑内容仍保留。'))}
    if(result.result?.status==='applied'&&result.files.length>0&&result.files.every(file=>file.state==='applied')){await remember(null);if(observed!==generation.current)return;setDraft(null);setBaseline('');setRemoving(false);setMessage(t('模板库已保存。'));await list()}
    else setMessage(t('原请求已有记录，文件尚未全部保存；请核对文件状态后再继续。'))
  }
  useEffect(()=>{
    const observed=++generation.current;setReady(false);setDraft(null);setRows([]);setPending(null);setReceipt(null);setBusy(false);setError('')
    if(api&&key)void(async()=>{
      try{
        const previous=await loadPortablePending(client,key);if(observed!==generation.current)return
        if(previous&&previous.operation!=='configuration.taskTemplates.mutate')throw new Error(t('原模板记录的操作类型不同，已保留。'))
        setPending(previous);await list();if(observed!==generation.current)return
        if(previous){const result=await api<PortableTaskTemplateReceipt>({operation:'receipt',commandId:previous.commandId});if(observed!==generation.current)return;await settle(result,previous.commandId)}
        if(observed===generation.current)setReady(true)
      }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}
    })()
    return()=>{generation.current++}
  },[client,key])
  useEffect(()=>{const before=(event:BeforeUnloadEvent):void=>{if(dirty||busy){event.preventDefault();event.returnValue=''}};window.addEventListener('beforeunload',before);return()=>window.removeEventListener('beforeunload',before)},[dirty,busy])
  if(!api)return null
  const work=async(action:()=>Promise<void>):Promise<void>=>{if(busy)return;const observed=generation.current;setBusy(true);setError('');setMessage('');try{await action()}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}}
  const open=async(id:string):Promise<void>=>{
    const observed=generation.current;const row=await api<PortableTaskTemplate|null>({operation:'get',id});if(observed!==generation.current)return
    if(!row)throw new Error(t('这份模板已被删除。'));const value=original(row);setDraft(value);setBaseline(JSON.stringify(value));setRemoving(false)
  }
  const mutate=async(operation:'put'|'remove'):Promise<void>=>{
    if(!draft||!key||pending)return
    if(!draft.name.trim()||draft.name.length>512||draft.title.length>4096)throw new Error(t('请填写模板名称，名称或标题不能超过限制。'))
    const candidate=operation==='put'?value(draft):undefined
    if(candidate&&new TextEncoder().encode(candidate.config).length>1024*1024)throw new Error(t('这份任务模板超过大小限制，请缩短说明或拆分材料。'))
    const observed=generation.current,id=newCommandId();await remember({operation:'configuration.taskTemplates.mutate',commandId:id,targetId:draft.id});if(observed!==generation.current)return
    const result=await api<PortableTaskTemplateReceipt>({operation,commandId:id,id:draft.id,expectedRevision:draft.revision,...(candidate?{value:candidate}:{})});if(observed!==generation.current)return;await settle(result,id)
  }
  return <section className="portable-task-templates">
    <div className="section-heading"><h3>{t('任务模板')}</h3><p>{t('保存可重复使用的标题和任务说明。原程序、材料和高级设置随模板保留，使用时在当前工程核对。保存模板本身不会启动任务。')}</p></div>
    <div className="portable-provider-actions"><button className="quiet-button" type="button" disabled={!ready||busy||!!pending||dirty} onClick={()=>{const value:Draft={id:newCommandId(),revision:null,name:'',title:'',body:'',config:{config_values:{}}};setDraft(value);setBaseline('');setRemoving(false)}}>{t('新建模板')}</button><button className="quiet-button" type="button" disabled={busy||dirty} onClick={()=>void work(()=>list())}>{t('刷新模板')}</button></div>
    <div className="portable-provider-list">{rows.map(row=><button type="button" className="portable-provider-row" key={row.id} disabled={busy||!!pending||dirty} onClick={()=>void work(()=>open(row.id))}><span>{row.name}</span><span className="muted">{row.title}</span></button>)}</div>
    {next&&<button className="quiet-button" disabled={busy} onClick={()=>void work(()=>list(true))}>{t('更多模板')}</button>}
    {ready&&rows.length===0&&<p className="muted">{t('还没有模板。你可以新建，也可以从原应用配置搬入。')}</p>}
    {draft&&<form className="forge-form" onSubmit={event=>{event.preventDefault();void work(()=>mutate('put'))}}><fieldset className="portable-provider-fields" disabled={busy||!!pending}>
      <label><span>{t('模板名称')}</span><input className="workspace-field" value={draft.name} maxLength={512} onChange={event=>setDraft({...draft,name:event.target.value})}/></label>
      <label><span>{t('任务标题')}</span><input className="workspace-field" value={draft.title} maxLength={4096} onChange={event=>setDraft({...draft,title:event.target.value})}/></label>
      <label><span>{t('任务说明')}</span><textarea className="workspace-field" rows={6} value={draft.body} onChange={event=>setDraft({...draft,body:event.target.value})}/></label>
      <details><summary>{t('原程序和材料')}</summary><p className="muted">{t('这里只改名称、标题和说明；其余原配置完整保留。任务创建时需要明确选择本机队员和材料。')}</p><dl><dt>{t('原智能体')}</dt><dd>{typeof draft.config.agent_type==='string'?draft.config.agent_type:t('创建任务时选择')}</dd><dt>{t('原工作模式')}</dt><dd>{typeof draft.config.mode_id==='string'?draft.config.mode_id:t('沿用所选队员')}</dd><dt>{t('保留的材料')}</dt><dd>{Array.isArray(draft.config.prompt_blocks)?draft.config.prompt_blocks.filter(block=>block&&typeof block==='object'&&(block as Record<string,unknown>).type!=='text').length:0}</dd></dl></details>
      <div className="portable-provider-actions"><button className="primary-button" type="submit" disabled={!dirty}>{t('保存模板')}</button><button className="quiet-button" type="button" onClick={()=>{setDraft(null);setBaseline('');setRemoving(false)}}>{t('放弃编辑')}</button>{draft.revision!==null&&<button className="danger-button" type="button" onClick={()=>setRemoving(true)}>{t('删除模板')}</button>}</div>
      {removing&&<div><p>{t('删除这份模板？已创建的任务、历史消息和原导入归档继续保留。')}</p><button className="quiet-button" type="button" onClick={()=>void work(()=>mutate('remove'))}>{t('确认删除模板')}</button><button className="quiet-button" type="button" onClick={()=>setRemoving(false)}>{t('取消')}</button></div>}
    </fieldset></form>}
    {pending&&<button className="quiet-button" type="button" disabled={busy} onClick={()=>void work(async()=>{const observed=generation.current;const result=await api<PortableTaskTemplateReceipt>({operation:'receipt',commandId:pending.commandId});if(observed===generation.current)await settle(result,pending.commandId)})}>{t('核对原模板操作')}</button>}
    {pending&&receipt?.state==='recorded'&&receipt.files.some(file=>file.state==='pending')&&<button className="quiet-button" type="button" disabled={busy} onClick={()=>void work(async()=>{const observed=generation.current;const result=await api<PortableTaskTemplateReceipt>({operation:'finish',commandId:pending.commandId});if(observed===generation.current)await settle(result,pending.commandId)})}>{t('继续保存原模板文件')}</button>}
    {pending&&receipt?.state==='recorded'&&receipt.files.some(file=>file.state==='conflict'||file.state==='failed')&&<button className="quiet-button" type="button" disabled={busy} onClick={()=>void work(async()=>{await remember(null)})}>{t('保留原结果并重新编辑')}</button>}
    {receipt?.files.map(file=><p className="muted" key={file.targetId}>{t('模板文件')} · {file.state}</p>)}
    {message&&<p role="status">{message}</p>}{error&&<p role="alert" className="inline-error">{error}</p>}
  </section>
}
