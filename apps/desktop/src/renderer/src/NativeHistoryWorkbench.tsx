import { useCallback, useEffect, useRef, useState } from 'react'
import { Archive, ArchiveRestore, Download, FolderOpen, Import, Search, Tag } from 'lucide-react'
import { sha256 } from '@noble/hashes/sha2.js'
import { NativeTranscriptBody } from './NativeSessionViewer'
import { nativeTranscript, type NativeTranscript } from './native-child-reader'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { NativeContinuationDialog } from './NativeContinuationDialog'
import { NativeCreationRecords } from './NativeCreationRecords'
import type { NativeContinuationMode } from '@contracts'
type Provider='codex'|'claude'|'qoder'|'grok'|'opencode'|'gemini'|'openclaw'|'cline'|'hermes'|'codebuddy'|'kimi_code'|'pi'|'cursor'|'deepseek'|'antigravity'
type Preview={digest:string;turnCount:number;summary:{title:string|null};notes:{malformed_records:number;incomplete_tail:boolean}}
type Candidate={id:string;name:string;bytes:number;modified:number;snapshot?:boolean;preview?:Preview;selected?:boolean;imported?:boolean;archived?:boolean}
type Plan={planId:string;provider:Provider;entries:Candidate[]}
type Imported={id:string;provider:Provider;sourceName:string;summary:{title:string|null};importedAt:string;tags:string[];revision:string;archived:boolean;sharedSourceSnapshot:boolean;linkedThreadId:string}
// Source import selection + source-native transcript body, without granting
// execution/file access to paths or subagent ids found inside imported logs.
export function NativeHistoryWorkbench({threadId,onOpenConversation}:{threadId:string;onOpenConversation?(reference:import('@contracts').CanvasConversationRef):Promise<boolean>}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),scope={threadId}
  const [provider,setProvider]=useState<Provider>('codex'),[plan,setPlan]=useState<Plan|null>(null),[entries,setEntries]=useState<Imported[]>([])
  const [selected,setSelected]=useState<Imported|null>(null),[data,setData]=useState<NativeTranscript|null>(null),[filter,setFilter]=useState(''),[tags,setTags]=useState('')
  const [showArchived,setShowArchived]=useState(false)
  const [execution,setExecution]=useState<{entry:Imported;data:NativeTranscript;mode:NativeContinuationMode;selectedTurnId?:string}|null>(null)
  const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[progress,setProgress]=useState(''),generation=useRef(0)
  const refresh=useCallback(async()=>{const next=(await client.request<{entries:Imported[]}>('history.library',{scope:{threadId},operation:'list',includeArchived:showArchived})).entries;setEntries(next);setSelected(previous=>previous?next.find(entry=>entry.id===previous.id)??null:null)},[client,threadId,showArchived])
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
      const result=await client.nativeHistory.request<{archived?:boolean}>(threadId,{operation:'import',planId:plan.planId,entryId:candidate.id,expectedDigest:candidate.preview.digest})
      setPlan(previous=>previous?{...previous,entries:previous.entries.map(entry=>entry.id===candidate.id?{...entry,imported:true,archived:Boolean(result.archived)}:entry)}:previous);++done
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
    <div className="workbench-toolbar"><select className="workspace-field" value={provider} disabled={busy} aria-label={t('原生会话来源')} onChange={event=>setProvider(event.target.value as Provider)}>{(['codex','claude','qoder','grok','opencode','gemini','openclaw','cline','hermes','codebuddy','kimi_code','pi','cursor','deepseek','antigravity'] as const).map(value=><option key={value} value={value}>{value}</option>)}</select>
      {client.nativeHistory&&<button className="quiet-button" disabled={busy} onClick={()=>void run(async()=>{const result=await client.nativeHistory!.scan<Plan>(threadId,provider);if(result)setPlan(result)})}><FolderOpen size={14}/>{t('选择记录文件夹')}</button>}
      <label><input className="workspace-check" type="checkbox" checked={showArchived} disabled={busy} onChange={event=>{setShowArchived(event.target.checked);setData(null)}}/>{t('显示归档')}</label><button className="quiet-button" disabled={busy} onClick={()=>void run(refresh)}>{t('刷新')}</button><Search size={14}/><input className="workspace-field" value={filter} onChange={event=>setFilter(event.target.value)} placeholder={t('搜索已导入会话或标签')}/>{progress&&<span role="status">{progress}</span>}
    </div>
    {error&&<p className="workbench-error" role="alert">{error}</p>}
    {plan&&<section className="workbench-import-plan"><div className="workbench-toolbar"><strong>{t('选择需要导入的会话')}</strong><button className="quiet-button" disabled={busy||choices.length===0||choices.some(entry=>!entry.preview)} onClick={()=>void run(importSelected)}><Import size={14}/>{t('导入所选')} ({choices.length})</button><button className="quiet-button" disabled={busy} onClick={()=>setPlan(null)}>{t('关闭')}</button></div><div className="workbench-import-rows">{plan.entries.map(candidate=><div className="workbench-import-row" key={candidate.id}><input className="workspace-check" type="checkbox" aria-label={candidate.name} checked={Boolean(candidate.selected)} disabled={busy||candidate.imported} onChange={event=>setPlan(previous=>previous?{...previous,entries:previous.entries.map(entry=>entry.id===candidate.id?{...entry,selected:event.target.checked}:entry)}:previous)}/><div><strong title={candidate.name}>{candidate.name}</strong><small>{new Date(candidate.modified*1000).toLocaleString()} · {candidate.snapshot?t('已冻结来源资料'):`${(candidate.bytes/1024).toFixed(1)} KB`}</small>{candidate.preview&&<p>{candidate.preview.summary?.title??''} · {candidate.preview.turnCount} {t('轮消息')}{candidate.preview.notes.malformed_records>0?' · '+t('包含无法解析的记录'):''}{candidate.preview.notes.incomplete_tail?' · '+t('结尾记录尚未写完'):''}</p>}</div><button className="quiet-button" disabled={busy||candidate.imported} onClick={()=>void run(()=>preview(candidate))}>{t(candidate.archived?'已在归档中':candidate.imported?'已导入':'预览')}</button></div>)}</div></section>}
    <div className="workbench-history-body"><aside className="workbench-history-list">{entries.filter(entry=>`${entry.summary?.title??''} ${entry.sourceName} ${entry.tags.join(' ')}`.toLowerCase().includes(filter.toLowerCase())).map(entry=><button key={entry.id} disabled={busy} className={entry.id===selected?.id?'is-active':''} onClick={()=>void run(()=>read(entry))}><strong>{entry.summary?.title||entry.sourceName}</strong><small>{entry.provider} · {new Date(entry.importedAt).toLocaleDateString()}{entry.archived?' · '+t('已归档'):''}</small>{entry.tags.length>0&&<small>{entry.tags.join(' · ')}</small>}</button>)}</aside>
      <div className="workbench-history-detail">{selected&&<div className="workbench-toolbar"><Tag size={14}/><input className="workspace-field" value={tags} aria-label={t('会话标签')} onChange={event=>setTags(event.target.value)} placeholder={t('逗号分隔标签')}/><button className="quiet-button" disabled={busy} onClick={()=>void run(async()=>{await client.request('history.library',{scope,operation:'tags',importId:selected.id,expectedRevision:selected.revision,tags:[...new Set(tags.split(/[,，]/).map(tag=>tag.trim()).filter(Boolean))]});await refresh()})}>{t('保存标签')}</button>{client.nativeHistory&&<button className="quiet-button" disabled={busy} onClick={()=>void run(async()=>{if(selected.sharedSourceSnapshot&&!window.confirm(t('原工具把多条会话放在同一个数据库中。此导出包含当次冻结的共享原始资料，可能含其他会话。是否继续？')))return;await client.nativeHistory!.export(threadId,selected.id,selected.revision)})}><Download size={14}/>{t('导出原始资料')}</button>}<button className="quiet-button" disabled={busy} onClick={()=>void run(async()=>{await client.request('history.library',{scope,operation:selected.archived?'restore':'archive',importId:selected.id,expectedRevision:selected.revision});setData(null);await refresh()})}>{selected.archived?<ArchiveRestore size={14}/>:<Archive size={14}/>} {t(selected.archived?'恢复记录':'归档记录')}</button></div>}
        {selected&&<NativeCreationRecords key={selected.id} sourceThreadId={selected.linkedThreadId} importId={selected.id} onOpen={onOpenConversation}/>}
        {selected&&data&&!selected.archived&&<div className="workbench-toolbar"><button className="quiet-button" disabled={busy} onClick={()=>setExecution({entry:selected,data,mode:'continue'})}>{t('从原生会话继续')}</button><button className="quiet-button" disabled={busy} onClick={()=>setExecution({entry:selected,data,mode:'fork_tail'})}>{t('从末尾分叉')}</button><span>{t('先预览和确认，再在新会话发送第一条消息。')}</span></div>}
        <div className="native-session-body">{data?<NativeTranscriptBody data={data} reader={null} target={null} live={false} visible onForkFromTurn={selected&&!selected.archived&&['codex','claude','deepseek'].includes(selected.provider)?selectedTurnId=>setExecution({entry:selected,data,mode:'fork_point',selectedTurnId}):undefined}/>:<div className="workbench-empty">{t(entries.length?'选择会话查看原始消息和工具记录。':'还没有导入的原生会话。')}</div>}</div>
      </div>
    </div>
    {execution&&<NativeContinuationDialog key={`${execution.entry.id}:${execution.mode}:${execution.selectedTurnId??''}`} threadId={execution.entry.linkedThreadId} entry={execution.entry} data={execution.data} mode={execution.mode} selectedTurnId={execution.selectedTurnId} onClose={()=>setExecution(current=>current===execution?null:current)} onOpen={onOpenConversation?receipt=>onOpenConversation({threadId:receipt.threadId,conversationId:receipt.conversationId}):undefined}/>}
  </div>
}
