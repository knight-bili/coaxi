import {useUiText} from './interface-language'

type ChoiceField={key:string;label:string;options:ReadonlyArray<string|boolean|null>}
// 字段与闭集来源：Coaxi aff1e6c 的 codex-model-list-editor.tsx。
const protocol:ChoiceField[]=[
  {key:'tool_mode',label:'工具调用方式',options:[null,'direct','code_mode','code_mode_only']},
  {key:'multi_agent_version',label:'多智能体协议',options:[null,'v1','v2']},
  {key:'use_responses_lite',label:'使用精简 Responses 协议',options:[true,false]}
]
const behavior:ChoiceField[]=[
  {key:'default_reasoning_summary',label:'推理摘要',options:['auto','concise','detailed','none']},
  {key:'default_verbosity',label:'回答详细程度',options:[null,'low','medium','high']},
  {key:'shell_type',label:'终端工具类型',options:['default','local','unified_exec','disabled','shell_command']},
  {key:'apply_patch_tool_type',label:'文件补丁工具',options:[null,'freeform']},
  {key:'web_search_tool_type',label:'网页搜索内容',options:['text','text_and_image']},
  ...([
    ['supports_reasoning_summary_parameter','支持推理摘要参数'],['support_verbosity','支持详细程度参数'],
    ['supports_parallel_tool_calls','支持并行工具调用'],['supports_search_tool','支持搜索工具'],
    ['supports_image_detail_original','支持原始图片细节'],['include_apps_usage_instructions','包含应用使用说明'],
    ['include_plugin_usage_instructions','包含插件使用说明'],['include_skills_usage_instructions','包含技能使用说明']
  ] as const).map(([key,label])=>({key,label,options:[true,false]}))
]
const textFields=[['description','模型说明'],['base_instructions','模型基础说明']] as const
const known=new Set([...protocol,...behavior].map(row=>row.key).concat(textFields.map(row=>row[0])))

/** 沿用原版逐模型折叠与协议、行为、说明分组；继承不猜测尚未读取的基准值。 */
export function PortableCodexOverrides({value,disabled,onChange}:{value:Record<string,unknown>;disabled:boolean;onChange(value:Record<string,unknown>):void}):React.JSX.Element{
  const t=useUiText(),owns=(key:string):boolean=>Object.prototype.hasOwnProperty.call(value,key)
  const update=(key:string,next:unknown,inherit=false):void=>{const copy={...value};if(inherit)delete copy[key];else copy[key]=next;onChange(copy)}
  const choices=(rows:ChoiceField[]):React.JSX.Element=><div className="portable-provider-fields">{rows.map(field=>{
    const raw=value[field.key],available=field.options.some(option=>Object.is(option,raw)),current=!owns(field.key)?'inherit':available?JSON.stringify(raw):'stored'
    return <label key={field.key}><span>{t(field.label)}</span><select className="workspace-field" disabled={disabled} value={current} onChange={event=>{if(event.target.value==='stored')return;update(field.key,event.target.value==='inherit'?undefined:JSON.parse(event.target.value),event.target.value==='inherit')}}>
      <option value="inherit">{t('继承基准模型')}</option>{current==='stored'&&<option value="stored">{t('保留原设置（由原程序核对）')}</option>}
      {field.options.map(option=><option key={JSON.stringify(option)} value={JSON.stringify(option)}>{option===null?t('不使用'):typeof option==='boolean'?t(option?'开启':'关闭'):option}</option>)}
    </select></label>
  })}</div>
  const other=Object.keys(value).filter(key=>!known.has(key))
  return <details className="portable-codex-overrides"><summary>{t('高级设置')}</summary>
    <p className="field-help">{t('这些设置只保存到当前模型。继承表示不覆盖基准值；实际程序仍会核对目录和支持范围。')}</p>
    <h4>{t('协议')}</h4>{choices(protocol)}<h4>{t('模型行为')}</h4>{choices(behavior)}
    <h4>{t('模型说明')}</h4><div className="portable-provider-fields">{textFields.map(([key,label])=><label key={key}><span>{t(label)}</span><div className="portable-codex-text-override">
      <label><input type="checkbox" checked={owns(key)} disabled={disabled} onChange={event=>update(key,'',!event.target.checked)}/><span>{t('覆盖基准内容')}</span></label>
      {owns(key)&&<textarea rows={key==='description'?2:6} value={typeof value[key]==='string'?value[key] as string:''} disabled={disabled} aria-label={t(label)} onChange={event=>update(key,event.target.value)}/>}
    </div></label>)}</div>
    {other.length>0&&<p className="field-help">{t('另有 {0} 项原覆盖设置完整保留。未在此表单编辑的字段不会删除。',String(other.length))}</p>}
  </details>
}
