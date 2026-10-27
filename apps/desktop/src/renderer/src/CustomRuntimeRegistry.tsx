import {useEffect,useRef,useState} from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import type {CustomRuntimeDefinitionInput,CustomRuntimePlatform} from '@contracts'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'
import {newCustomDefinition,parseCustomDistribution} from './custom-runtime-editor'
import {AppDialogBody,AppDialogContent,AppDialogFooter,AppDialogHeader} from './AppDialog'

export interface CustomRuntimeSeed{definition:CustomRuntimeDefinitionInput;raw:string}
interface Entry{id:string;name:string;description:string;version?:string;icon?:string;website?:string;repository?:string;license?:string;distribution:Record<string,unknown>}
interface Catalog{registryUrl:string;sourceDigest:string;fetchedAt:string;cached:boolean;entries:Entry[];registeredIds:string[];platform:CustomRuntimePlatform|null}
const binaryKeys:Record<CustomRuntimePlatform,string[]>={'windows-x64':['windows-x86_64','x86_64-pc-windows-msvc'],'windows-arm64':['windows-aarch64','aarch64-pc-windows-msvc'],'macos-x64':['darwin-x86_64','x86_64-apple-darwin'],'macos-arm64':['darwin-aarch64','aarch64-apple-darwin'],'linux-x64':['linux-x86_64','x86_64-unknown-linux-gnu'],'linux-arm64':['linux-aarch64','aarch64-unknown-linux-gnu']}
function preferred(entry:Entry,platform:CustomRuntimePlatform|null):CustomRuntimeDefinitionInput['distributionKind']|null{
  const spec=entry.distribution;if(!spec||typeof spec!=='object')return null
  if(platform&&binaryKeys[platform].some(key=>Boolean((spec.binary as Record<string,unknown>|undefined)?.[key])))return 'binary'
  if(spec.npx)return 'npx';if(spec.uvx)return 'uvx';if(spec.binary&&Object.keys(spec.binary).length)return 'binary';return null
}
/** 原 Coaxi 注册表搜索列表：选择只带入表单，保存和安装仍需用户分别操作。 */
export function CustomRuntimeRegistry({onClose,onChoose}:{onClose():void;onChoose(seed:CustomRuntimeSeed):void}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0)
  const [catalog,setCatalog]=useState<Catalog|null>(null),[search,setSearch]=useState(''),[limit,setLimit]=useState(100),[busy,setBusy]=useState(false),[error,setError]=useState('')
  const load=async(refresh=false):Promise<void>=>{const observed=generation.current;setBusy(true);setError('');try{const value=await client.request<Catalog>('runtime.custom.registry',{refresh});if(observed===generation.current)setCatalog(value)}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}}
  useEffect(()=>{generation.current++;void load();return()=>{generation.current++}},[client])
  const entries=(catalog?.entries??[]).filter(entry=>typeof entry.id==='string'&&typeof entry.name==='string'&&[entry.id,entry.name,entry.description??''].some(value=>value.toLowerCase().includes(search.toLowerCase())))
  const choose=(entry:Entry):void=>{if(!catalog)return;try{
    const kind=preferred(entry,catalog.platform);if(!kind)throw new Error(t('这条记录没有可用的分发方式。'))
    const raw=JSON.stringify(entry,null,2),parsed=parseCustomDistribution(raw,kind)
    const definition:CustomRuntimeDefinitionInput={...newCustomDefinition(),registryId:entry.id,name:entry.name,description:entry.description??'',version:entry.version||'1.0.0',iconUrl:entry.icon??null,distributionKind:kind,spec:parsed.spec,source:'registry',sourceRef:`${catalog.registryUrl}#${encodeURIComponent(entry.id)}`}
    onChoose({definition,raw})
  }catch(reason){setError(readErrorMessage(reason))}}
  return <Dialog.Root open onOpenChange={open=>{if(!open)onClose()}}><Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent width="wide" className="custom-runtime-dialog"><AppDialogHeader title={t('从 ACP 注册表添加')} description={t('从公开注册表添加智能体。先选程序和分发方式，再核对表单；这里不会下载或运行程序。')} closeLabel={t('关闭')}/><AppDialogBody><div className="custom-runtime-toolbar"><input aria-label={t('搜索注册表')} value={search} placeholder={t('搜索名称或标识')} onChange={event=>{setSearch(event.target.value);setLimit(100)}}/><button className="quiet-button" disabled={busy} onClick={()=>void load(true)}>{t(busy?'正在读取…':'刷新来源')}</button></div>
    {catalog&&<p className="custom-runtime-source">{catalog.registryUrl}<br/>{new Date(catalog.fetchedAt).toLocaleString()} · {t(catalog.cached?'本机缓存':'本次读取')} · {catalog.platform??t('平台未识别')}</p>}
    <div className="runtime-product-list">{entries.slice(0,limit).map(entry=>{const added=catalog?.registeredIds.includes(entry.id);const kind=preferred(entry,catalog?.platform??null);return <article key={entry.id} className="runtime-product-row custom-runtime-row"><div className="runtime-product-copy"><strong>{entry.name}</strong><small>{entry.description}</small><small>{entry.id} · {entry.version??t('来源未标版本')}{entry.license?' · '+entry.license:''}</small></div><button className="quiet-button" disabled={Boolean(added)||!kind} onClick={()=>choose(entry)}>{t(added?'已添加':'带入表单')}</button></article>})}</div>
    {entries.length>limit&&<button className="quiet-button" onClick={()=>setLimit(value=>value+100)}>{t('更多程序')}</button>}{!busy&&catalog&&entries.length===0&&<p>{t('没有符合搜索条件的程序。')}</p>}{error&&<p className="inline-error" role="alert">{error}</p>}</AppDialogBody><AppDialogFooter><Dialog.Close className="quiet-button">{t('关闭')}</Dialog.Close></AppDialogFooter></AppDialogContent></Dialog.Portal></Dialog.Root>
}
