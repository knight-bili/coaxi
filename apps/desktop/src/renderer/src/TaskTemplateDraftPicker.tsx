import {useEffect,useRef,useState} from 'react'
import * as Popover from '@radix-ui/react-popover'
import type {PortableTaskTemplate} from '@contracts'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'
import './portable-configuration.css'

type Row=Omit<PortableTaskTemplate,'config'>
type Selection={row:PortableTaskTemplate;config:Record<string,unknown>;materials:Array<{key:string;name:string}>;settings:boolean}
/** 沿 Codeg task-editor-dialog 模板弹层；跨主机材料和执行设置由 User 明确重映射。 */
export function TaskTemplateDraftPicker({disabled,memberCount,materials,onChooseMaterials,onApply}:{
  disabled:boolean;memberCount:number;materials:Array<{id:string;name:string}>;onChooseMaterials():void;onApply(value:{title:string;description:string}):void
}):React.JSX.Element|null{
  const client=useThreadClient(),api=client.portableConfiguration?.templates,t=useUiText(),generation=useRef(0)
  const [open,setOpen]=useState(false),[rows,setRows]=useState<Row[]>([]),[next,setNext]=useState<string|null>(null),[selection,setSelection]=useState<Selection|null>(null),[mapped,setMapped]=useState<Record<string,string>>({}),[settingsConfirmed,setSettingsConfirmed]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState('')
  useEffect(()=>{generation.current++;setOpen(false);setSelection(null);setRows([]);setMapped({});setSettingsConfirmed(false);setBusy(false);setError('');return()=>{generation.current++}},[client,client.presentationScope])
  const list=async(more=false):Promise<void>=>{
    if(!api)return;const observed=generation.current;setBusy(true);setError('')
    try{const result=await api<{items:Row[];next:string|null}>({operation:'list',after:more?next:null,limit:100});if(observed===generation.current){setRows(old=>more?[...old,...result.items.filter(row=>!old.some(item=>item.id===row.id))]:result.items);setNext(result.next)}}
    catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  const choose=async(id:string):Promise<void>=>{
    if(!api)return;const observed=generation.current;setBusy(true);setError('')
    try{
      const row=await api<PortableTaskTemplate|null>({operation:'get',id});if(observed!==generation.current)return
      if(!row)throw new Error(t('这份模板已经被删除。'))
      const config:unknown=JSON.parse(row.config);if(!config||typeof config!=='object'||Array.isArray(config))throw new Error(t('原模板的内容需要先补齐，请在模板设置中处理。'))
      const fields=config as Record<string,unknown>,blocks=fields.prompt_blocks
      if(blocks!==undefined&&!Array.isArray(blocks))throw new Error(t('原模板材料无法读取，完整原文仍保留。'))
      const required=(Array.isArray(blocks)?blocks:[]).flatMap((block,index)=>{
        if(!block||typeof block!=='object'||Array.isArray(block))throw new Error(t('原模板材料无法读取，完整原文仍保留。'))
        const item=block as Record<string,unknown>;if(item.type==='text')return []
        return [{key:String(index),name:typeof item.name==='string'?item.name.slice(0,512):t('原材料 {0}',String(index+1))}]
      })
      if(required.length>128)throw new Error(t('这份模板材料较多，请先在模板设置中拆分。'))
      const settings=fields.agent_type!=null||fields.mode_id!=null||fields.base_branch!=null||Boolean(fields.config_values&&typeof fields.config_values==='object'&&Object.keys(fields.config_values).length)
      setSelection({row,config:fields,materials:required,settings});setMapped({});setSettingsConfirmed(false)
    }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  if(!api)return null
  const materialReady=selection?.materials.every(row=>materials.some(material=>material.id===mapped[row.key]))??false
  const canApply=Boolean(selection)&&!disabled&&!busy&&materialReady&&(!selection!.settings||(settingsConfirmed&&memberCount>0))
  const apply=():void=>{
    if(!selection||!canApply)return
    const fields=selection.config
    const description=typeof fields.display_text==='string'?fields.display_text:(Array.isArray(fields.prompt_blocks)?fields.prompt_blocks.filter(block=>block&&typeof block==='object'&&(block as Record<string,unknown>).type==='text').map(block=>typeof (block as Record<string,unknown>).text==='string'?(block as Record<string,unknown>).text:'').join('\n'):'')
    onApply({title:selection.row.title,description});setOpen(false);setSelection(null)
  }
  return <Popover.Root open={open} onOpenChange={value=>{setOpen(value);if(value){setSelection(null);void list()}}}>
    <Popover.Trigger asChild><button type="button" className="quiet-button" disabled={disabled}>{t('使用任务模板')}</button></Popover.Trigger>
    <Popover.Portal><Popover.Content className="compact-menu mission-template-menu" align="start" sideOffset={8} collisionPadding={16}>
      <div className="section-heading"><h3>{t(selection?'核对这份模板':'任务模板')}</h3></div>
      {!selection&&<>{rows.map(row=><button type="button" className="compact-option" key={row.id} disabled={busy} onClick={()=>void choose(row.id)}>{row.name}</button>)}{!busy&&rows.length===0&&<p className="muted">{t('还没有模板，可在配置迁移与模板设置中创建或搬入。')}</p>}{next&&<button type="button" className="quiet-button" disabled={busy} onClick={()=>void list(true)}>{t('更多模板')}</button>}</>}
      {selection&&<><p><strong>{selection.row.title||selection.row.name}</strong></p>
        {selection.settings&&<label className="portable-template-confirm"><input className="workspace-check" type="checkbox" checked={settingsConfirmed} onChange={event=>setSettingsConfirmed(event.target.checked)}/><span>{t('用当前工程、分支和已选择队员的运行设置代替模板的原设置。原模板继续保留。')}</span></label>}
        {selection.settings&&memberCount===0&&<p className="muted">{t('请先在使命里选择执行队员，再使用此模板。')}</p>}
        {selection.materials.length>0&&<><p className="muted">{t('原材料不会按旧路径读取。请把本次要使用的材料加到使命，再逐项对应。')}</p><button type="button" className="quiet-button" disabled={disabled} onClick={onChooseMaterials}>{t('选择本次材料')}</button>{selection.materials.map(row=><label className="portable-template-material" key={row.key}><span>{row.name}</span><select className="workspace-field" value={mapped[row.key]??''} onChange={event=>setMapped({...mapped,[row.key]:event.target.value})}><option value="">{t('选择已经加入使命的材料')}</option>{materials.map(material=><option value={material.id} key={material.id}>{material.name}</option>)}</select></label>)}</>}
        <p className="muted">{t('填入后可继续编辑，确认创建使命前不会启动。当前标题和说明会被替换。')}</p>
        <div className="portable-provider-actions"><button className="quiet-button" type="button" disabled={!canApply} onClick={apply}>{t('填入标题和说明')}</button><button type="button" className="quiet-button" disabled={busy} onClick={()=>setSelection(null)}>{t('返回模板列表')}</button></div>
      </>}
      {busy&&<p role="status">{t('正在读取模板…')}</p>}{error&&<p role="alert" className="inline-error">{error}</p>}
    </Popover.Content></Popover.Portal>
  </Popover.Root>
}
