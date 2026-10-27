import { useEffect, useMemo, useState } from 'react'
import { ChevronUp, ChevronDown, Plus, Save, Tag, Trash2 } from 'lucide-react'
import { newCommandId } from '../../shared/command-id'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { WorkspaceTagChoices } from './WorkspaceTagChoices'

type Organizer = { tags:string[]; directoryAliases:Array<{path:string;name:string;group:string}>; quickMessages:Array<{id:string;title:string;text:string}> }
const empty = ():Organizer=>({tags:[],directoryAliases:[],quickMessages:[]})

// Coaxi conversation-tag-list-editor / folder aliases and quick-message rows;
// shared Coaxi settings controls, with Core digest-CAS persistence.
export function WorkspaceOrganizer({threadId,onDirtyChange,onUseMessage,onDirectoryAliasesApplied}:{threadId:string;onDirtyChange(dirty:boolean):void;onUseMessage?(text:string):boolean;onDirectoryAliasesApplied?():void}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),scope=useMemo(()=>({threadId}),[threadId])
  const [value,setValue]=useState<Organizer>(empty),[saved,setSaved]=useState<Organizer>(empty),[digest,setDigest]=useState<string|null>(null)
  const [level,setLevel]=useState<'global'|'project'|'thread'>('thread')
  const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[tag,setTag]=useState('')
  const [renaming,setRenaming]=useState<{from:string;value:string}|null>(null)
  const dirty=JSON.stringify(value)!==JSON.stringify(saved)
  useEffect(()=>{onDirtyChange(dirty)},[dirty,onDirtyChange])
  useEffect(()=>{
    let active=true;setBusy(true)
    void client.request<{value:Organizer;digest:string}>('workbench.organizer.get',{scope,level}).then(result=>{if(active){setValue(result.value);setSaved(result.value);setDigest(result.digest)}}).catch(failure=>{if(active)setError(String(failure))}).finally(()=>{if(active)setBusy(false)})
    return()=>{active=false}
  },[client,scope,level])
  const save=async():Promise<void>=>{
    if(busy||!digest)return
    const sent=value;setBusy(true);setError(null)
    try{const result=await client.request<{value:Organizer;digest:string}>('workbench.organizer.set',{scope,level,expectedDigest:digest,value:sent});setSaved(result.value);setDigest(result.digest);setValue(current=>JSON.stringify(current)===JSON.stringify(sent)?result.value:current);onDirectoryAliasesApplied?.()}catch(failure){setError(String(failure))}finally{setBusy(false)}
  }
  const applyDirectoryAliases=async():Promise<void>=>{
    if(busy||dirty||!digest)return
    const sent=saved;setBusy(true);setError(null)
    try{const result=await client.request<{value:Organizer;digest:string}>('workbench.organizer.set',{scope,level,expectedDigest:digest,value:sent});setSaved(result.value);setDigest(result.digest);setValue(current=>JSON.stringify(current)===JSON.stringify(sent)?result.value:current);onDirectoryAliasesApplied?.()}catch(failure){setError(String(failure))}finally{setBusy(false)}
  }
  return <section className="workbench-organizer">
    <div className="workbench-toolbar"><strong>{t('会话整理')}</strong><label>{t('适用范围')}<select className="workspace-field" aria-label={t('适用范围')} disabled={busy||dirty} value={level} onChange={event=>setLevel(event.target.value as typeof level)}><option value="global">{t('全局')}</option><option value="project">{t('当前工程')}</option><option value="thread">{t('当前对话与原有内容')}</option></select></label><button className="quiet-button" type="button" disabled={busy||!dirty||!digest} onClick={()=>void save()}><Save size={14}/>{t('保存')}</button><button className="quiet-button" type="button" disabled={busy||!dirty} onClick={()=>{setValue(saved);setError(null)}}>{t('放弃更改')}</button></div>
    {error&&<p className="workbench-error" role="alert">{error}</p>}
    <fieldset disabled={busy}><legend>{t(level==='thread'?'当前对话标签':'可供对话选择的标签')}</legend><div className="workbench-tags">{value.tags.map(label=><span key={label}><Tag size={12}/><button className="quiet-button" type="button" aria-label={t('重命名标签')} onClick={()=>setRenaming({from:label,value:label})}>{label}</button><button className="quiet-button" type="button" aria-label={t('移除标签')} onClick={()=>{setValue(current=>({...current,tags:current.tags.filter(item=>item!==label)}));if(renaming?.from===label)setRenaming(null)}}>×</button></span>)}</div>
      {renaming&&<form className="workbench-toolbar" onSubmit={event=>{event.preventDefault();const next=renaming.value.trim();if(!next||next!==renaming.from&&value.tags.includes(next)){setError(t('标签名称不能为空或重复。'));return}setValue(current=>({...current,tags:current.tags.map(tag=>tag===renaming.from?next:tag)}));setRenaming(null);setError(null)}}><input className="workspace-field" aria-label={t('新的标签名称')} value={renaming.value} onChange={event=>setRenaming(current=>current?{...current,value:event.target.value}:null)}/><button className="primary-button" type="submit">{t('确认重命名')}</button><button className="quiet-button" type="button" onClick={()=>setRenaming(null)}>{t('取消')}</button></form>}
      {level==='thread'&&<WorkspaceTagChoices threadId={threadId} selected={value.tags} disabled={busy} onChoose={tag=>setValue(current=>current.tags.includes(tag)?current:{...current,tags:[...current.tags,tag]})}/>}
      <form className="workbench-toolbar" onSubmit={event=>{event.preventDefault();const next=tag.trim();if(next&&!value.tags.includes(next))setValue(current=>({...current,tags:[...current.tags,next]}));setTag('')}}><input className="workspace-field" aria-label={t('标签名称')} value={tag} onChange={event=>setTag(event.target.value)}/><button className="primary-button" type="submit" disabled={!tag.trim()}><Plus size={14}/>{t('添加')}</button></form>
    </fieldset>
    <fieldset disabled={busy}><legend>{t('关联目录、别名与分组')}</legend>
      <p>{t('保存后，关联目录会应用到当前工程；全局列表不会自动修改其他工程。')}</p>
      <button className="quiet-button" type="button" disabled={busy||dirty||!digest} onClick={()=>void applyDirectoryAliases()}>{t('应用关联目录到当前工程')}</button>
      {value.directoryAliases.map((alias,index)=><div className="workbench-organizer-row" key={index}><input className="workspace-field" aria-label={t('目录')} value={alias.path} onChange={event=>setValue(current=>({...current,directoryAliases:current.directoryAliases.map((row,i)=>i===index?{...row,path:event.target.value}:row)}))}/><input className="workspace-field" aria-label={t('别名')} value={alias.name} onChange={event=>setValue(current=>({...current,directoryAliases:current.directoryAliases.map((row,i)=>i===index?{...row,name:event.target.value}:row)}))}/><input className="workspace-field" aria-label={t('分组')} value={alias.group} onChange={event=>setValue(current=>({...current,directoryAliases:current.directoryAliases.map((row,i)=>i===index?{...row,group:event.target.value}:row)}))}/><button className="quiet-button" type="button" aria-label={t('移除目录')} onClick={()=>setValue(current=>({...current,directoryAliases:current.directoryAliases.filter((_,i)=>i!==index)}))}><Trash2 size={14}/></button></div>)}
      <button className="quiet-button" type="button" onClick={()=>setValue(current=>({...current,directoryAliases:[...current.directoryAliases,{path:'',name:'',group:''}]}))}><Plus size={14}/>{t('添加目录')}</button>
    </fieldset>
    <fieldset disabled={busy}><legend>{t('快捷消息')}</legend>
      {value.quickMessages.map((message,index)=><div className="workbench-quick-message" key={message.id}><input className="workspace-field" aria-label={t('消息名称')} value={message.title} onChange={event=>setValue(current=>({...current,quickMessages:current.quickMessages.map(row=>row.id===message.id?{...row,title:event.target.value}:row)}))}/><textarea className="workspace-field" aria-label={t('消息正文')} value={message.text} onChange={event=>setValue(current=>({...current,quickMessages:current.quickMessages.map(row=>row.id===message.id?{...row,text:event.target.value}:row)}))}/><div className="workbench-toolbar"><button className="quiet-button" type="button" aria-label={t('上移快捷消息')} disabled={index===0||busy} onClick={()=>setValue(current=>{const rows=[...current.quickMessages];[rows[index-1],rows[index]]=[rows[index],rows[index-1]];return {...current,quickMessages:rows}})}><ChevronUp size={14}/></button><button className="quiet-button" type="button" aria-label={t('下移快捷消息')} disabled={index===value.quickMessages.length-1||busy} onClick={()=>setValue(current=>{const rows=[...current.quickMessages];[rows[index],rows[index+1]]=[rows[index+1],rows[index]];return {...current,quickMessages:rows}})}><ChevronDown size={14}/></button><button className="quiet-button" type="button" disabled={dirty} onClick={()=>void navigator.clipboard.writeText(message.text).catch(failure=>setError(String(failure)))}>{t('复制')}</button>{onUseMessage&&<button className="quiet-button" type="button" disabled={dirty} onClick={()=>{if(!onUseMessage(message.text))setError(t('当前输入暂时无法接收快捷消息。'))}}>{t('填入对话')}</button>}<button className="quiet-button" type="button" aria-label={t('移除快捷消息')} onClick={()=>setValue(current=>({...current,quickMessages:current.quickMessages.filter(row=>row.id!==message.id)}))}><Trash2 size={14}/></button></div></div>)}
      <button className="quiet-button" type="button" onClick={()=>setValue(current=>({...current,quickMessages:[...current.quickMessages,{id:newCommandId(),title:'',text:''}]}))}><Plus size={14}/>{t('添加快捷消息')}</button>
    </fieldset>
  </section>
}
