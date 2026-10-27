import { useEffect,useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Cat,Download,Heart,Search,Store } from 'lucide-react'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { AppDialogContent,AppDialogHeader } from './AppDialog'
import { readPetSprite } from './pet-sprite-reader'
import { publishPetsChanged } from './PetSettings'
type Pet={id:string;displayName:string;description:string;kind?:string;ownerName?:string;tags:string[];downloadCount:number;likeCount:number;posterUrl?:string;previewUrl?:string;downloadUrl:string;alreadyInstalled:boolean}
function MarketImage({url,name}:{url:string|undefined;name:string}):React.JSX.Element{
  const client=useThreadClient(),[source,setSource]=useState<string|null>(null)
  useEffect(()=>{if(!url)return;const controller=new AbortController();let asset:string|null=null;void readPetSprite(input=>client.request('pets.operation',{operation:'market_asset',url,offset:input.offset}),url,controller.signal).then(value=>{if(controller.signal.aborted)URL.revokeObjectURL(value);else{asset=value;setSource(value)}},()=>undefined);return()=>{controller.abort();if(asset)URL.revokeObjectURL(asset)}},[client,url])
  return source?<img className="pet-market-poster" src={source} alt={name}/>:<div className="pet-market-poster"><Cat size={34}/></div>
}
// Coaxi PetMarketplaceDialog: explicit search/filter, original stats and
// reinstall choice; all network/package rules are the fixed source backend.
export function PetMarketplace({onInstalled}:{onInstalled():Promise<void>}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),[open,setOpen]=useState(false),[query,setQuery]=useState(''),[search,setSearch]=useState(''),[sort,setSort]=useState('latest'),[kind,setKind]=useState('all'),[page,setPage]=useState(1)
  const [pets,setPets]=useState<Pet[]>([]),[pages,setPages]=useState(1),[total,setTotal]=useState(0),[busy,setBusy]=useState(false),[installing,setInstalling]=useState<string|null>(null),[error,setError]=useState<string|null>(null)
  useEffect(()=>{if(!open)return;let active=true;setBusy(true);setError(null);void client.request<{pets:Pet[];total:number;totalPages:number}>('pets.operation',{operation:'market_list',value:{page,pageSize:12,q:query,...(sort==='latest'?{}:{sort}),...(kind==='all'?{}:{kind})}}).then(value=>{if(active){setPets(value.pets);setPages(value.totalPages);setTotal(value.total)}},failure=>{if(active)setError(String(failure))}).finally(()=>{if(active)setBusy(false)});return()=>{active=false}},[client,open,query,sort,kind,page])
  const install=async(pet:Pet):Promise<void>=>{
    if(installing)return
    if(pet.alreadyInstalled&&!window.confirm(t('重新安装会替换这个宠物的图片和资料，继续吗？')))return
    setInstalling(pet.id);setError(null)
    try{await client.request('pets.operation',{operation:'market_install',value:{id:pet.id,downloadUrl:pet.downloadUrl,overwrite:pet.alreadyInstalled}});setPets(previous=>previous.map(entry=>entry.id===pet.id?{...entry,alreadyInstalled:true}:entry));await onInstalled();publishPetsChanged()}catch(failure){setError(String(failure))}finally{setInstalling(null)}
  }
  return <><button className="quiet-button" type="button" onClick={()=>setOpen(true)}><Store size={14}/>{t('宠物市场')}</button><Dialog.Root open={open} onOpenChange={next=>{if(!installing)setOpen(next)}}><Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent tone="neutral" width="large" className="pet-market-dialog"><AppDialogHeader title={t('宠物市场')} description="codex-pets.net" closeDisabled={installing!==null}/>
    <form className="workbench-toolbar" onSubmit={event=>{event.preventDefault();setPage(1);setQuery(search)}}><Search size={15}/><input className="workspace-field" value={search} placeholder={t('搜索宠物、作者或标签')} aria-label={t('搜索宠物')} onChange={event=>setSearch(event.target.value)}/><button className="primary-button" type="submit" disabled={busy}>{t('搜索')}</button><select className="workspace-field" aria-label={t('排序')} value={sort} onChange={event=>{setSort(event.target.value);setPage(1)}}><option value="latest">{t('最新')}</option><option value="popular">{t('热门')}</option><option value="views">{t('浏览最多')}</option></select><input className="workspace-field" value={kind==='all'?'':kind} aria-label={t('宠物类型')} placeholder={t('全部类型')} onChange={event=>{setKind(event.target.value||'all');setPage(1)}}/></form>
    {error&&<p role="alert" className="workbench-error">{error}</p>}
    <div className="pet-market-scroll"><div className="pet-catalog-grid">{pets.map(pet=><article className="pet-catalog-card" key={pet.id}><MarketImage url={pet.posterUrl??pet.previewUrl} name={pet.displayName}/><strong>{pet.displayName}</strong>{pet.ownerName&&<small>{pet.ownerName}</small>}<p>{pet.description}</p><small>{pet.tags.join(' · ')}</small><div className="workbench-toolbar"><span><Download size={12}/>{pet.downloadCount}</span><span><Heart size={12}/>{pet.likeCount}</span><button className="quiet-button" disabled={installing!==null} onClick={()=>void install(pet)}>{t(installing===pet.id?'正在安装…':pet.alreadyInstalled?'重新安装':'安装')}</button></div></article>)}</div>{busy&&<p role="status">{t('正在读取宠物市场…')}</p>}</div>
    <div className="workbench-toolbar"><span>{total} {t('个宠物')}</span><button className="quiet-button" disabled={busy||page<=1} onClick={()=>setPage(value=>value-1)}>{t('上一页')}</button><span>{page}/{Math.max(pages,1)}</span><button className="quiet-button" disabled={busy||page>=pages} onClick={()=>setPage(value=>value+1)}>{t('下一页')}</button></div>
  </AppDialogContent></Dialog.Portal></Dialog.Root></>
}
