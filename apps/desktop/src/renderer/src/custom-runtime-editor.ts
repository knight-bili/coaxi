import type {CustomRuntimeDefinition,CustomRuntimeDefinitionInput} from '@contracts'
import {newCommandId} from '../../shared/command-id'

type ObjectValue=Record<string,unknown>
function object(value:unknown):ObjectValue{if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('配置必须是 JSON 对象。');return value as ObjectValue}
function known(value:ObjectValue,names:readonly string[]):void{if(Object.keys(value).some(key=>!names.includes(key)))throw new Error('配置含有未知字段，请按原始格式核对后再导入。')}
function packageSpec(value:unknown,kind:'npx'|'uvx'):NonNullable<CustomRuntimeDefinitionInput['spec']['npx'|'uvx']>{
  const raw=object(value);known(raw,['package','args','env','cmd',...(kind==='npx'?['nodeRequired','node_required']:['uvRequired','uv_required','python'])])
  if('nodeRequired'in raw&&'node_required'in raw||'uvRequired'in raw&&'uv_required'in raw)throw new Error('同一版本要求不能同时填写两种字段名。')
  return {package:raw.package,args:raw.args??[],env:raw.env??{},cmd:raw.cmd??null,...(kind==='npx'?{nodeRequired:raw.nodeRequired??raw.node_required??null}:{uvRequired:raw.uvRequired??raw.uv_required??null,python:raw.python??null})} as NonNullable<CustomRuntimeDefinitionInput['spec']['npx'|'uvx']>
}
/** 沿用原版手动表单：接受 distribution 或整条注册表记录，先展示规范化结果。 */
export function parseCustomDistribution(text:string,kind:CustomRuntimeDefinitionInput['distributionKind']):{spec:CustomRuntimeDefinitionInput['spec'];metadata:ObjectValue;kinds:CustomRuntimeDefinitionInput['distributionKind'][]}{
  let parsed:unknown;try{parsed=JSON.parse(text)}catch{throw new Error('配置 JSON 格式不正确。')}
  const raw=object(parsed),metadata='distribution'in raw?raw:{}
  if('distribution'in raw)known(raw,['id','name','description','version','website','repository','license','icon','distribution'])
  const channels=object(raw.distribution??raw);known(channels,['npx','uvx','binary'])
  const kinds:CustomRuntimeDefinitionInput['distributionKind'][]=[]
  if(channels.binary&&Object.keys(object(channels.binary)).length)kinds.push('binary');if(channels.npx)kinds.push('npx');if(channels.uvx)kinds.push('uvx')
  if(!kinds.includes(kind))throw new Error('所选分发方式不在这份配置中。')
  const spec:CustomRuntimeDefinitionInput['spec']={npx:null,uvx:null,binary:{}}
  if(channels.npx)spec.npx=packageSpec(channels.npx,'npx') as NonNullable<typeof spec.npx>
  if(channels.uvx)spec.uvx=packageSpec(channels.uvx,'uvx') as NonNullable<typeof spec.uvx>
  if(channels.binary)for(const [platform,entry]of Object.entries(object(channels.binary))){const item=object(entry);known(item,['archive','url','cmd','args','env','sha256']);if('archive'in item&&'url'in item)throw new Error('同一下载地址不能同时填写 archive 和 url。');(spec.binary as ObjectValue)[platform]={archive:item.archive??item.url,cmd:item.cmd??'',args:item.args??[],env:item.env??{},sha256:item.sha256??null}}
  return {spec,metadata,kinds}
}
export function newCustomDefinition():CustomRuntimeDefinitionInput{return {schemaVersion:1,id:newCommandId(),registryId:'',name:'',description:'',version:'1.0.0',distributionKind:'npx',spec:{npx:{package:'',args:['--acp'],env:{},cmd:null,nodeRequired:null},uvx:null,binary:{}},iconUrl:null,skillsSharedStore:false,skillsDir:null,source:'manual',sourceRef:null,versionProbe:null,supportsMcp:true,enabled:true}}
export function editCustomDefinition(value:CustomRuntimeDefinition):CustomRuntimeDefinitionInput{const {revision:_revision,definitionDigest:_digest,createdAt:_created,updatedAt:_updated,...definition}=structuredClone(value);return definition}
/** 预览显示秘密的新输入状态，绝不把输入值当普通配置回显或持久化。 */
export function customDefinitionPreview(value:CustomRuntimeDefinitionInput):unknown{
  const copy=structuredClone(value);const specs=[copy.spec.npx,copy.spec.uvx,...Object.values(copy.spec.binary)]
  for(const spec of specs)if(spec)for(const [name,entry]of Object.entries(spec.env)){if(typeof entry==='string')spec.env[name]='<本次新输入，保存后仅保留秘密引用>'}
  return copy
}
