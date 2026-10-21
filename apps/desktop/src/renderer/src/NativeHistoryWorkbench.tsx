import { useCallback, useEffect, useRef, useState } from 'react'
import { FolderOpen, Import, Search, Tag } from 'lucide-react'
import { sha256 } from '@noble/hashes/sha2.js'
import { NativeTranscriptBody } from './NativeSessionViewer'
import { nativeTranscript, type NativeTranscript } from './native-child-reader'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
type Provider='codex'|'claude'|'qoder'|'grok'|'opencode'|'gemini'|'openclaw'|'cline'|'hermes'|'codebuddy'|'kimi_code'|'pi'|'cursor'|'deepseek'|'antigravity'
type Preview={digest:string;turnCount:number;summary:{title:string|null};notes:{malformed_records:number;incomplete_tail:boolean}}
type Candidate={id:string;name:string;bytes:number;modified:number;snapshot?:boolean;preview?:Preview;selected?:boolean;imported?:boolean}
type Plan={planId:string;provider:Provider;entries:Candidate[]}
type Imported={id:string;provider:Provider;sourceName:string;summary:{title:string|null};importedAt:string;tags:string[]}
// Source import selection + source-native transcript body, without granting
// execution/file access to paths or subagent ids found inside imported logs.
export function NativeHistoryWorkbench({threadId}:{threadId:string}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),scope={threadId}
  const [provider,setProvider]=useState<Provider>('codex'),[plan,setPlan]=useState<Plan|null>(null),[entries,setEntries]=useState<Imported[]>([])
  const [selected,setSelected]=useState<Imported|null>(null),[data,setData]=useState<NativeTranscript|null>(null),[filter,setFilter]=useState(''),[tags,setTags]=useState('')
  const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[progress,setProgress]=useState(''),generation=useRef(0)
  const refresh=useCallback(async()=>setEntries((await client.request<{entries:Imported[]}>('history.library',{scope:{threadId},operation:'list'})).entries),[client,threadId])
  useEffect(()=>{void refresh().catch(failure=>setError(String(failure)));return()=>{++generation.current}},[refresh])
  const run=async(action:()=>Promise<void>):Promise<void>=>{if(busy)return;setBusy(true);setError(null);try{await action()}catch(failure){setError(String(failure))}finally{setBusy(false);setProgress('')}}
  const preview=async(candidate:Candidate):Promise<void>=>{
    if(!plan||!client.nativeHistory)return
    const value=await client.nativeHistory.request<Preview>(threadId,{operation:'preview',planId:plan.planId,entryId:candidate.id})
    setPlan(previous=>previous?{...previous,entries:previous.entries.map(entry=>entry.id===candidate.id?{...entry,preview:value}:entry)}:previous)
  }
  const importSelected=async():Promise<void>=>{
    if(!plan||!client.nativeHistory)return
    const selected=plan.entries.filter(entry=>entry.selected&&!entry.imported);let done=0
    for(const candidate of selected){if(!candidate.preview)throw new Error(t('请先预览每个所选会话。'));setProgress(`${done}/${selected.length}`)
      await client.nativeHistory.request(threadId,{operation:'import',planId:plan.planId,entryId:candidate.id,expectedDigest:candidate.preview.digest})
      setPlan(previous=>previous?{...previous,entries:previous.entries.map(entry=>entry.id===candidate.id?{...entry,imported:true}:entry)}:previous);++done
    }
    await refresh()
  }
  const read=async(entry:Imported):Promise<void>=>{
    const current=++generation.current;setSelected(entry);setData(null);setTags(entry.tags.join(', '))
    let offset=0,total:number|null=null,digest:string|null=null;const chunks:Uint8Array[]=[];const hash=sha256.create()
    for(;;){
      const page=await client.request<{digest:string;totalBytes:number;offset:number;next:number|null;bytes:string}>('history.library',{scope:{threadId},operation:'read',importId:entry.id,offset})
      if(current!==generation.current)return
      if(!Number.isSafeInteger(page.totalBytes)||page.totalBytes<0||page.totalBytes>512*1024*1024||page.offset!==offset||total!==null&&total!==page.totalBytes||digest!==null&&digest!==page.digest)throw new Error(t('导入历史的快照发生变化，请重新打开。'))
      total=page.totalBytes;digest=page.digest;const bytes=Uint8Array.from(atob(page.bytes),char=>char.charCodeAt(0));chunks.push(bytes);hash.update(bytes);offset+=bytes.byteLength
      setProgress(`${offset}/${total}`)
      if(page.next===null){if(offset!==total)throw new Error(t('会话快照尚未读取完整。'));break}
      if(page.next!==offset||!bytes.length)throw new Error(t('会话分页无效。'))
    }
    if(Array.from(hash.digest(),byte=>byte.toString(16).padStart(2,'0')).join('')!==digest)throw new Error(t('会话快照校验不一致。'))
    const bytes=new Uint8Array(offset);let position=0;for(const chunk of chunks){bytes.set(chunk,position);position+=chunk.byteLength}
    if(current===generation.current)setData(nativeTranscript(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes))))
  }
  const choices=plan?.entries.filter(entry=>entry.selected&&!entry.imported)??[]
  return <div className="workbench-history">
    <div className="workbench-toolbar"><select value={provider} disabled={busy} aria-label={t('原生会话来源')} onChange={event=>setProvider(event.target.value as Provider)}>{(['codex','claude','qoder','grok','opencode','gemini','openclaw','cline','hermes','codebuddy','kimi_code','pi','cursor','deepseek','antigravity'] as const).map(value=><option key={value} value={value}>{value}</option>)}</select>
      {client.nativeHistory&&<button disabled={busy} onClick={()=>void run(async()=>{const result=await client.nativeHistory!.scan<Plan>(threadId,provider);if(result)setPlan(result)})}><FolderOpen size={14}/>{t('选择记录文件夹')}</button>}
      <Search size={14}/><input value={filter} onChange={event=>setFilter(event.target.value)} placeholder={t('搜索已导入会话或标签')}/>{progress&&<span role="status">{progress}</span>}
    </div>
    {error&&<p className="workbench-error" role="alert">{error}</p>}
    {plan&&<section className="workbench-import-plan"><div className="workbench-toolbar"><strong>{t('选择需要导入的会话')}</strong><button disabled={busy||choices.length===0||choices.some(entry=>!entry.preview)} onClick={()=>void run(importSelected)}><Import size={14}/>{t('导入所选')} ({choices.length})</button><button disabled={busy} onClick={()=>setPlan(null)}>{t('关闭')}</button></div><div className="workbench-import-rows">{plan.entries.map(candidate=><div className="workbench-import-row" key={candidate.id}><input type="checkbox" aria-label={candidate.name} checked={Boolean(candidate.selected)} disabled={busy||candidate.imported} onChange={event=>setPlan(previous=>previous?{...previous,entries:previous.entries.map(entry=>entry.id===candidate.id?{...entry,selected:event.target.checked}:entry)}:previous)}/><div><strong title={candidate.name}>{candidate.name}</strong><small>{new Date(candidate.modified*1000).toLocaleString()} · {(candidate.bytes/1024).toFixed(1)} KB</small>{candidate.preview&&<p>{candidate.preview.summary?.title??''} · {candidate.preview.turnCount} {t('轮消息')}{candidate.preview.notes.malformed_records>0?' · '+t('包含无法解析的记录'):''}{candidate.preview.notes.incomplete_tail?' · '+t('结尾记录尚未写完'):''}</p>}</div><button disabled={busy||candidate.imported} onClick={()=>void run(()=>preview(candidate))}>{t(candidate.imported?'已导入':'预览')}</button></div>)}</div></section>}
    <div className="workbench-history-body"><aside className="workbench-history-list">{entries.filter(entry=>`${entry.summary?.title??''} ${entry.sourceName} ${entry.tags.join(' ')}`.toLowerCase().includes(filter.toLowerCase())).map(entry=><button key={entry.id} disabled={busy} className={entry.id===selected?.id?'is-active':''} onClick={()=>void run(()=>read(entry))}><strong>{entry.summary?.title||entry.sourceName}</strong><small>{entry.provider} · {new Date(entry.importedAt).toLocaleDateString()}</small>{entry.tags.length>0&&<small>{entry.tags.join(' · ')}</small>}</button>)}</aside>
      <div className="workbench-history-detail">{selected&&<div className="workbench-toolbar"><Tag size={14}/><input value={tags} aria-label={t('会话标签')} onChange={event=>setTags(event.target.value)} placeholder={t('逗号分隔标签')}/><button disabled={busy} onClick={()=>void run(async()=>{await client.request('history.library',{scope,operation:'tags',importId:selected.id,tags:[...new Set(tags.split(/[,，]/).map(tag=>tag.trim()).filter(Boolean))]});await refresh()})}>{t('保存标签')}</button></div>}
        <div className="native-session-body">{data?<NativeTranscriptBody data={data} reader={null} target={null} live={false} visible/>:<div className="workbench-empty">{t(entries.length?'选择会话查看原始消息和工具记录。':'还没有导入的原生会话。')}</div>}</div>
      </div>
    </div>
  </div>
}
