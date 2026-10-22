import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Files, FolderPlus, GitBranch, Plus, RefreshCw, Save, X } from 'lucide-react'
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels'
import { AppDialogContent, AppDialogHeader } from './AppDialog'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { useShortcutActions } from './useShortcutActions'
import { NUMBERED_TAB_ACTION_IDS } from '../../shared/codeg-keyboard-shortcuts'
import type { ShortcutActions } from './shortcut-action-driver'
import { newCommandId } from '../../shared/command-id'
import { FileTree, FileTreeFile, FileTreeFolder } from './codeg-workbench/file-tree'
import {WorkspaceFileMenu,WorkspaceFileActionDialog,WorkspaceFileSearch,WorkspaceTrash,type FileAction} from './WorkspaceFileTools'
import { WorkspaceCodeEditor } from './WorkspaceCodeEditor'
import {WorkspaceTerminals} from './WorkspaceTerminals'
import {ProjectLauncher} from './ProjectLauncher'
import {WorkspaceGitViews} from './WorkspaceGitViews'
import { WorkspaceMergeEditor } from './WorkspaceMergeEditor'
import { WorkspaceOrganizer } from './WorkspaceOrganizer'
import { ForgeWorkbench } from './ForgeWorkbench'
import { BrowserWorkbench } from './BrowserWorkbench'
import { ComputerWorkbench } from './ComputerWorkbench'
import { OfficePreviewWorkbench } from './OfficePreviewWorkbench'
import { NativeHistoryWorkbench } from './NativeHistoryWorkbench'
import { UnifiedDiffPreview } from './codeg-native-content/components/diff/unified-diff-preview'
import './workspace-workbench.css'

type Entry = { name: string; path: string; directory: boolean; link: boolean; bytes: number }
type Directory = { entries: Entry[]; next: string | null }
type Document = { path: string; text: string; saved: string; digest: string | null; conflict?: boolean }
type GitResult = { success: boolean; exitCode: number | null; output: string; error: string }
const gitActions = {
  stage: '暂存文件', unstage: '取消暂存', commit: '提交暂存内容', stash_save: '保存到 Stash', stash_apply: '应用 Stash', stash_pop: '取出 Stash', stash_drop: '删除 Stash',
  clone:'克隆到子目录',worktree_add:'创建独立工作目录',worktree_remove:'移除托管工作目录',branch_delete:'删除已合并分支',branch_rename:'重命名分支',tag_create:'创建标签',tag_delete:'删除标签',discard:'放弃指定工作区改动',branch_create: '新建分支', checkout: '切换分支', reset: '重置历史', merge: '合并分支', rebase: '变基', merge_continue: '继续合并', merge_abort: '中止合并',
  rebase_continue: '继续变基', rebase_abort: '中止变基', cherry_pick: '拣选提交', cherry_pick_continue: '继续拣选', cherry_pick_abort: '中止拣选', revert: '撤销提交',
  fetch: '获取远程更新', pull: '拉取远程更新', push: '推送', remote_add: '添加远程仓库', remote_remove: '移除远程仓库', clean: '清理指定未跟踪文件', init: '初始化 Git',
} as const

/** Codeg file-tree and Git auxiliary-panel structure, hosted in Rovai's dialog. */
export function WorkspaceWorkbench({ threadId, projectPath, members, bindLeaveGuard, onQuickMessage }: {
  threadId: string; projectPath:string; members:Array<{id:string;name:string}>; bindLeaveGuard(guard: (() => void) | null): void; onQuickMessage?(text:string):boolean
}): React.JSX.Element {
  const client = useThreadClient(), t = useUiText(), scope = useMemo(() => ({ threadId }), [threadId])
  const [open, setOpen] = useState(false), [tab, setTab] = useState<'files' | 'git' | 'organizer' | 'forge' | 'browser' | 'computer' | 'office' | 'history' | 'trash' | 'launcher' | 'terminals'>('files')
  const [fileAction,setFileAction]=useState<FileAction|null>(null),[searchVisible,setSearchVisible]=useState(false),[cursorLine,setCursorLine]=useState<number|undefined>(undefined)
  const [organizerDirty,setOrganizerDirty]=useState(false)
  const [forgeDirty,setForgeDirty]=useState(false)
  const [officePath,setOfficePath]=useState<string|null>(null)
  const [directories, setDirectories] = useState(new Map<string, Directory>()), [expanded, setExpanded] = useState(new Set<string>())
  const [documents, setDocuments] = useState<Document[]>([]), [selected, setSelected] = useState<string | null>(null)
  const closedFiles=useRef<string[]>([])
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), epoch = useRef(0)
  const [creation, setCreation] = useState<'file' | 'directory' | 'rename' | null>(null), [name, setName] = useState('')
  const [terminalUnsettled,setTerminalUnsettled]=useState(false)
  const [gitDirty,setGitDirty]=useState(false),[gitVersion,setGitVersion]=useState(0)
  const [gitView, setGitView] = useState('status'), [gitResult, setGitResult] = useState<GitResult | null>(null)
  const [action, setAction] = useState<keyof typeof gitActions>('stage'), [target, setTarget] = useState(''), [paths, setPaths] = useState('')
  const [remote, setRemote] = useState('origin'), [message, setMessage] = useState(''), [mode, setMode] = useState('mixed')
  const [preview, setPreview] = useState<{ request: Record<string, unknown>; revision: string; arguments: string[]; cleanPreview: string | null } | null>(null)
  const [merge, setMerge] = useState<{path:string;base:string;ours:string;theirs:string;merged:string}|null>(null)
  const dirty = terminalUnsettled || organizerDirty || forgeDirty || gitDirty || documents.some(document => document.text !== document.saved)
  const current = documents.find(document => document.path === selected)
  const guard = useCallback(() => { if (busy) throw new Error(t('工程操作仍在进行，请稍后离开。')); if (dirty) throw new Error(t('工程文件尚未保存，请先保存或关闭对应标签。')) }, [busy, dirty, t])
  useEffect(() => { bindLeaveGuard(guard); return () => bindLeaveGuard(null) }, [bindLeaveGuard, guard])
  useEffect(() => () => { ++epoch.current }, [])
  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent): void => { if (dirty || busy) { event.preventDefault(); event.returnValue = '' } }
    window.addEventListener('beforeunload', beforeUnload)
    return () => window.removeEventListener('beforeunload', beforeUnload)
  }, [dirty, busy])
  const operation = async <T,>(run: () => Promise<T>): Promise<T | undefined> => {
    if (busy) return
    const generation = epoch.current; setBusy(true); setError(null)
    try { return await run() } catch (failure) { if (generation === epoch.current) setError(failure instanceof Error ? failure.message : t('工程操作失败。')); return undefined }
    finally { if (generation === epoch.current) setBusy(false) }
  }
  const loadDirectory = async (path: string, after?: string): Promise<void> => {
    const generation = epoch.current
    const result = await client.request<Directory>('workbench.files.list', { scope, path, ...(after ? { after } : {}) })
    if (generation === epoch.current) setDirectories(previous => new Map(previous).set(path, { entries: after ? [...(previous.get(path)?.entries ?? []), ...result.entries] : result.entries, next: result.next }))
  }
  useEffect(() => { if (open && !directories.has('')) void operation(() => loadDirectory('')) }, [open])
  const openFile = async (path: string,line?:number): Promise<void> => {
    setCursorLine(line);setTab('files')
    if(/\.(docx|xlsx|pptx)$/i.test(path)){setOfficePath(path);setTab('office');return}
    setSelected(path)
    if (documents.some(document => document.path === path)) return
    await operation(async () => {
      const result = await client.request<{ path: string; text: string; digest: string }>('workbench.files.read', { scope, path })
      setDocuments(previous => previous.some(document => document.path === path) ? previous : [...previous, { ...result, saved: result.text }])
    })
  }
  const save = async (): Promise<void> => {
    if (!current) return
    const document = current
    await operation(async () => {
      try {
        const result = await client.request<{ applied: boolean; digest: string }>('workbench.files.mutate', { commandId: newCommandId(), scope, action: 'save', path: document.path, text: document.text, expectedDigest: document.digest })
        setDocuments(previous => previous.map(item => item.path === document.path ? { ...item, saved: document.text, digest: result.digest, conflict: false } : item))
      } catch (failure) {
        if (String(failure).includes('file_changed')) setDocuments(previous => previous.map(item => item.path === document.path ? { ...item, conflict: true } : item))
        throw failure
      }
    })
  }
  useEffect(()=>{
    if(!open||tab!=='files'||!current||busy)return
    let active=true,running=false;const document=current
    const check=async():Promise<void>=>{if(running||globalThis.document.visibilityState==='hidden')return;running=true;try{const value=await client.request<{versions:Array<{path:string;state:string;digest?:string}>}>('workbench.files.tools',{scope,operation:'versions',paths:[document.path]});if(!active)return;const version=value.versions[0];if(version?.state!=='present'){setDocuments(previous=>previous.map(item=>item.path===document.path?{...item,conflict:true}:item));return}if(version.digest===document.digest)return;if(document.text!==document.saved){setDocuments(previous=>previous.map(item=>item.path===document.path?{...item,conflict:true}:item));return}const external=await client.request<{text:string;digest:string}>('workbench.files.read',{scope,path:document.path});if(active)setDocuments(previous=>previous.map(item=>item.path===document.path&&item.text===document.saved?{...item,...external,saved:external.text,conflict:false}:item))}catch{if(active)setDocuments(previous=>previous.map(item=>item.path===document.path?{...item,conflict:true}:item))}finally{running=false}}
    const timer=setInterval(()=>void check(),4000);window.addEventListener('focus',check);return()=>{active=false;clearInterval(timer);window.removeEventListener('focus',check)}
  },[client,scope,open,tab,current?.path,current?.digest,current?.text,current?.saved,busy])
  const mergeExternal=async():Promise<void>=>{if(!current)return;const document=current;await operation(async()=>{const external=await client.request<{text:string;digest:string}>('workbench.files.read',{scope,path:document.path});setDocuments(previous=>previous.map(item=>item.path===document.path?{...item,saved:external.text,digest:external.digest,conflict:false}:item));setMerge({path:document.path,base:document.saved,ours:document.text,theirs:external.text,merged:document.text})})}
  const closeDocument = (path: string): void => {
    const document = documents.find(item => item.path === path)
    if (busy || document && document.text !== document.saved && !window.confirm(t('放弃这个文件尚未保存的修改？'))) return
    closedFiles.current=[path,...closedFiles.current.filter(value=>value!==path)].slice(0,100)
    setDocuments(previous => previous.filter(item => item.path !== path))
    if (selected === path) setSelected(documents.find(item => item.path !== path)?.path ?? null)
  }
  const renderTree = (path: string, depth = 0): React.ReactNode => <>
    {(directories.get(path)?.entries ?? []).map(entry => entry.directory && !entry.link
      ? <FileTreeFolder key={entry.path} path={entry.path} name={entry.name} depth={depth} actions={<WorkspaceFileMenu path={entry.path} disabled={busy||dirty||entry.link} onAction={setFileAction}/>}>{renderTree(entry.path, depth + 1)}</FileTreeFolder>
      : <FileTreeFile key={entry.path} path={entry.path} name={entry.name} depth={depth} actions={<WorkspaceFileMenu path={entry.path} disabled={busy||dirty||entry.link} onAction={setFileAction}/>} aria-disabled={entry.link} onDoubleClick={() => { if (!entry.link) void openFile(entry.path) }} />)}
    {directories.get(path)?.next && <button type="button" className="quiet-button" disabled={busy} onClick={() => void operation(() => loadDirectory(path, directories.get(path)!.next!))}>{t('加载更多')}</button>}
  </>
  const activeShortcutSurface=():boolean=>{const dialogs=[...document.querySelectorAll('[role="dialog"][data-state="open"]')];return open&&dialogs.at(-1)?.classList.contains('workspace-workbench')===true}
  const moveTab=(delta:number):void=>{const index=documents.findIndex(row=>row.path===selected);if(documents.length)setSelected(documents[(Math.max(0,index)+delta+documents.length)%documents.length].path)}
  useShortcutActions({
    toggle_terminal:()=>{if(organizerDirty||forgeDirty||terminalUnsettled)return true;setOpen(true);setTab(value=>value==='terminals'?'files':'terminals')},
    toggle_aux_panel:()=>{try{if(open)guard();setOpen(value=>!value)}catch(error){setError(String(error))}},
  },{priority:25,active:()=>!document.querySelector('[role="dialog"][aria-modal="true"]')||activeShortcutSurface()})
  useShortcutActions({
    close_current_tab:()=>{if(tab==='files'&&selected)closeDocument(selected)},
    close_all_file_tabs:()=>{if(busy)return true;if(documents.some(row=>row.text!==row.saved)&&!window.confirm(t('放弃所有文件尚未保存的修改？')))return true;closedFiles.current=[...documents.map(row=>row.path).reverse(),...closedFiles.current].slice(0,100);setDocuments([]);setSelected(null)},
    reopen_last_closed_tab:()=>{if(busy)return true;const path=closedFiles.current[0];if(path){closedFiles.current.shift();void openFile(path)}},
    next_tab:()=>{if(tab!=='files')return false;moveTab(1)},prev_tab:()=>{if(tab!=='files')return false;moveTab(-1)},
    ...Object.fromEntries(NUMBERED_TAB_ACTION_IDS.map((id,index)=>[id,()=>{if(tab!=='files')return false;const row=documents[index];if(row)setSelected(row.path)}])) as ShortcutActions,
  },{priority:50,active:activeShortcutSurface})
  const readGit = async (view = gitView): Promise<void> => {
    await operation(async () => setGitResult(await client.request<GitResult>('workbench.git.read', { scope, action: view, paths: paths.split('\n').map(path => path.trim()).filter(Boolean), ...(target ? { target } : {}) })))
  }
  const create = async (): Promise<void> => {
    if (!name.trim() || !creation) return
    await operation(async () => {
      await client.request('workbench.files.mutate', { commandId: newCommandId(), scope, action: creation === 'directory' ? 'create_directory' : creation === 'rename' ? 'rename' : 'save', path: creation === 'rename' ? selected : name.trim(),
        ...(creation === 'rename' ? { destination: name.trim(), expectedDigest: current?.digest } : creation === 'file' ? { text: '', expectedDigest: null } : {}) })
      if (creation === 'rename' && selected) { const old = selected, next = name.trim(); setDocuments(previous => previous.map(item => item.path === old ? { ...item, path: next } : item)); setSelected(next) }
      setCreation(null); await loadDirectory('')
    })
  }
  const prepareGitAction=(action:string,options:Record<string,unknown>):void=>{void operation(async()=>{const request={scope,action,paths:[],remote:'origin',...options};const value=await client.request<{revision:string;arguments:string[];cleanPreview:string|null}>('workbench.git.preview',request);setPreview({...value,request:{...request,commandId:newCommandId()}})})}
  const openMerge = async (): Promise<void> => {
    if(!current)return
    const document=current
    await operation(async()=>{
      const conflicts=await client.request<GitResult>('workbench.git.read',{scope,action:'conflicts'})
      if(!conflicts.success)throw new Error(conflicts.error)
      const stages=new Set(conflicts.output.split('\0').filter(Boolean).filter(row=>row.slice(row.indexOf('\t')+1)===document.path).map(row=>row.slice(0,row.indexOf('\t')).split(' ').at(-1)))
      if(!stages.size)throw new Error(t('这个文件没有未合并的 Git 记录。'))
      const parts=await Promise.all(['1','2','3'].map(async stage=>{
        if(!stages.has(stage))return ''
        const result=await client.request<GitResult>('workbench.git.read',{scope,action:'conflict',mode:stage,paths:[document.path]})
        if(!result.success)throw new Error(result.error);return result.output
      }))
      setMerge({path:document.path,base:parts[0]!,ours:parts[1]!,theirs:parts[2]!,merged:document.text})
    })
  }
  return <>
    <button type="button" className="quiet-button conversation-tool-button" title={t('文件与 Git 工程工作区')} onClick={() => setOpen(true)}><Files size={15} /><span>{t('工程')}</span></button>
    <Dialog.Root open={open} onOpenChange={next => { if (!next) { try { guard() } catch (failure) { setError(String(failure)); return } }; setOpen(next) }}>
      <Dialog.Portal><Dialog.Overlay className="dialog-overlay" /><AppDialogContent className="workspace-workbench" width="large">
        <AppDialogHeader title={t('工程工作区')} description={t('文件与 Git')} hideDescription closeDisabled={busy || dirty} />
        <nav className="workbench-toolbar" aria-label={t('工程视图')}>
          <button disabled={organizerDirty||forgeDirty||terminalUnsettled} className={tab==='terminals'?'quiet-button is-active':'quiet-button'} onClick={()=>setTab('terminals')}>{t('终端')}</button>
          <button disabled={organizerDirty||forgeDirty||terminalUnsettled} className={tab==='launcher'?'quiet-button is-active':'quiet-button'} onClick={()=>setTab('launcher')}>{t('项目启动器')}</button>
          <button type="button" className={tab === 'files' ? 'quiet-button is-active' : 'quiet-button'} disabled={organizerDirty||forgeDirty} onClick={() => setTab('files')}><Files size={14} />{t('文件')}</button>
          <button type="button" className={tab === 'git' ? 'quiet-button is-active' : 'quiet-button'} disabled={organizerDirty||forgeDirty} onClick={() => { setTab('git'); void readGit() }}><GitBranch size={14} />Git</button>
          <button type="button" className={tab==='organizer'?'quiet-button is-active':'quiet-button'} disabled={forgeDirty} onClick={()=>setTab('organizer')}>{t('整理')}</button>
          <button type="button" className={tab==='history'?'quiet-button is-active':'quiet-button'} disabled={organizerDirty||forgeDirty} onClick={()=>setTab('history')}>{t('会话历史')}</button>
          <button type="button" className={tab==='computer'?'quiet-button is-active':'quiet-button'} disabled={organizerDirty||forgeDirty} onClick={()=>setTab('computer')}>{t('电脑')}</button>
          <button type="button" className={tab==='browser'?'quiet-button is-active':'quiet-button'} disabled={organizerDirty||forgeDirty} onClick={()=>setTab('browser')}>{t('浏览器')}</button>
          <button type="button" className={tab==='forge'?'quiet-button is-active':'quiet-button'} disabled={organizerDirty} onClick={()=>setTab('forge')}>{t('仓库面板')}</button>
          <button type="button" className="quiet-button" disabled={busy} onClick={() => void operation(() => loadDirectory(''))} aria-label={t('刷新工程')}><RefreshCw size={14} /></button>
          <button type="button" className="quiet-button" disabled={busy} onClick={() => {setCreation('file');setName('')}}><Plus size={14} />{t('新文件')}</button>
          <button type="button" className="quiet-button" disabled={busy} onClick={() => {setCreation('directory');setName('')}}><FolderPlus size={14} />{t('新目录')}</button>
        </nav>
        {error && <p className="workbench-error" role="alert">{error}</p>}
        {creation && <form className="workbench-toolbar" onSubmit={event => {event.preventDefault();void create()}}><input aria-label={t('相对路径')} value={name} onChange={event => setName(event.target.value)} placeholder={t('相对工程的路径')} /><button type="submit" disabled={busy || !name.trim()}>{t('确认')}</button><button type="button" onClick={() => setCreation(null)}>{t('取消')}</button></form>}
        <div className="workbench-toolbar"><button className={searchVisible?'quiet-button is-active':'quiet-button'} onClick={()=>setSearchVisible(value=>!value)}>{t(searchVisible?'文件树':'搜索文件')}</button><button className="quiet-button" onClick={()=>setTab('trash')}>{t('回收站')}</button></div>
        <PanelGroup direction="horizontal" className="workbench-panels">
          <Panel defaultSize={23} minSize={15}><div className="workbench-tree">{searchVisible?<WorkspaceFileSearch threadId={threadId} onOpen={(path,line)=>void openFile(path,line)}/>:<FileTree expanded={expanded} selectedPath={selected ?? undefined} onSelect={path => { if (![...directories.values()].some(directory => directory.entries.some(entry => entry.path === path && (entry.directory || entry.link)))) void openFile(path) }} onExpandedChange={next => {
            setExpanded(next); void operation(async () => { for (const path of next) if (!directories.has(path)) await loadDirectory(path) })
          }}>{renderTree('')}</FileTree>}</div></Panel>
          <PanelResizeHandle className="workbench-resize" />
          <Panel minSize={25}>
            {merge ? <WorkspaceMergeEditor {...merge} onClose={()=>setMerge(null)} onApply={text=>{setDocuments(previous=>previous.map(document=>document.path===merge.path?{...document,text}:document));setMerge(null)}} /> : tab==='terminals'?<WorkspaceTerminals projectPath={projectPath} onUnsettledChange={setTerminalUnsettled}/> : tab==='launcher'?<ProjectLauncher threadId={threadId} onCreated={async()=>{await loadDirectory('')}}/> : tab==='trash'?<WorkspaceTrash threadId={threadId} onRestored={async()=>{for(const path of ['',...expanded])await loadDirectory(path).catch(()=>{})}}/> : tab==='history'?<NativeHistoryWorkbench threadId={threadId}/> : tab==='office'&&officePath?<OfficePreviewWorkbench key={officePath} threadId={threadId} path={officePath}/> : tab==='computer'?<ComputerWorkbench threadId={threadId}/> : tab==='browser'?<BrowserWorkbench threadId={threadId}/> : tab==='forge'?<ForgeWorkbench threadId={threadId} projectPath={projectPath} members={members} onDirtyChange={setForgeDirty}/> : tab==='organizer'?<WorkspaceOrganizer threadId={threadId} onDirtyChange={setOrganizerDirty} onUseMessage={onQuickMessage?text=>{if(dirty||!onQuickMessage(text))return false;setOpen(false);return true}:undefined}/> : tab === 'files' ? <div className="workbench-editor">
              <div className="workbench-tabs" role="tablist">{documents.map(document => <div className={selected === document.path ? 'workbench-tab is-active' : 'workbench-tab'} key={document.path}><button type="button" role="tab" aria-selected={selected === document.path} onClick={() => setSelected(document.path)}>{document.path}{document.text !== document.saved ? ' •' : ''}</button><button type="button" disabled={busy} onClick={() => closeDocument(document.path)} aria-label={t('关闭文件')}><X size={12} /></button></div>)}</div>
              {current ? <><div className="workbench-toolbar"><span title={current.path}>{current.path}</span><button type="button" className="quiet-button" disabled={busy || current.text === current.saved} onClick={() => void save()}><Save size={14} />{t('保存')}</button><button type="button" className="quiet-button" disabled={busy || dirty} onClick={() => {setCreation('rename');setName(current.path)}}>{t('重命名')}</button><button type="button" className="quiet-button" disabled={busy} onClick={()=>void openMerge()}>{t('解决冲突')}</button></div>
                {current.conflict && <div className="workbench-error" role="alert">{t('文件在外部发生变化，本地修改已保留。')}<button disabled={busy} onClick={()=>void mergeExternal()}>{t('比较并合并外部版本')}</button></div>}
                <div className="workbench-editors">{documents.map(document=><div key={document.path} className="workbench-document" hidden={document.path!==selected}><WorkspaceCodeEditor path={document.path} text={document.text} visible={document.path===selected} line={document.path===selected?cursorLine:undefined} onSave={()=>void save()} onChange={text=>setDocuments(previous=>previous.map(item=>item.path===document.path?{...item,text}:item))} /></div>)}</div>
              </> : <div className="workbench-empty">{t('从文件树打开文件')}</div>}
            </div> : <div className="workbench-git">
              <div className="workbench-toolbar"><select aria-label={t('Git 视图')} value={gitView} onChange={event => {setGitView(event.target.value);void readGit(event.target.value)}}>{Object.entries({status:'变更',diff:'工作区 Diff',diff_staged:'暂存 Diff',log:'历史',branches:'分支',stash:'Stash',remotes:'远程',worktrees:'Worktree',conflicts:'冲突',tags:'标签',reflog:'引用历史'}).map(([value,label]) => <option key={value} value={value}>{t(label)}</option>)}</select><button type="button" disabled={busy} onClick={() => void readGit()}>{t('刷新')}</button></div>
              {gitResult && (gitView==='diff'||gitView==='diff_staged') && gitResult.success ? <div className="workbench-git-output"><UnifiedDiffPreview diffText={gitResult.output} clickableFilePath={false} /></div> : gitResult?<WorkspaceGitViews key={gitVersion} threadId={threadId} view={gitView} result={gitResult} disabled={busy} onAction={prepareGitAction} onDirtyChange={setGitDirty} onOpenFile={path=>{if(!gitDirty)void openFile(path)}}/>:<div className="workbench-empty">{t('选择 Git 视图')}</div>}
              <details className="workbench-git-advanced"><summary>{t('更多 Git 操作')}</summary><div className="workbench-git-actions"><select aria-label={t('Git 操作')} value={action} onChange={event => setAction(event.target.value as keyof typeof gitActions)}>{Object.entries(gitActions).map(([value,label]) => <option key={value} value={value}>{t(label)}</option>)}</select>
                <input aria-label={t('目标分支、提交或 Stash')} placeholder={t('目标分支、提交或 Stash')} value={target} onChange={event => setTarget(event.target.value)} />
                <input aria-label={t('远程名称')} value={remote} onChange={event => setRemote(event.target.value)} />
                {action === 'reset' && <select aria-label={t('重置方式')} value={mode} onChange={event => setMode(event.target.value)}><option value="soft">Soft</option><option value="mixed">Mixed</option><option value="hard">Hard</option></select>}
                <textarea aria-label={t('文件路径，每行一个')} placeholder={t('文件路径，每行一个')} value={paths} onChange={event => setPaths(event.target.value)} />
                {(action === 'commit' || action === 'stash_save' || action==='branch_rename') && <textarea aria-label={t('说明')} value={message} onChange={event => setMessage(event.target.value)} />}
                <button type="button" disabled={busy} onClick={() => void operation(async () => { const request = {scope,action,paths:paths.split('\n').map(path=>path.trim()).filter(Boolean),...(target?{target}:{}),remote,message,mode}; const value=await client.request<{revision:string;arguments:string[];cleanPreview:string|null}>('workbench.git.preview',request);setPreview({...value,request:{...request,commandId:newCommandId()}}) })}>{t('预览操作')}</button>
              </div></details>
            </div>}
          </Panel>
        </PanelGroup>
      </AppDialogContent></Dialog.Portal>
    </Dialog.Root>
    {fileAction&&<WorkspaceFileActionDialog key={`${fileAction.operation}:${fileAction.path}`} threadId={threadId} action={fileAction} onClose={()=>setFileAction(null)} onApplied={async(action,destination)=>{
      const moved=(path:string):string=>destination?destination+path.slice(action.path.length):path
      if(action.operation==='move'){setDocuments(previous=>previous.map(document=>document.path===action.path||document.path.startsWith(`${action.path}/`)?{...document,path:moved(document.path)}:document));setSelected(previous=>previous&&(previous===action.path||previous.startsWith(`${action.path}/`))?moved(previous):previous)}
      if(action.operation==='trash'){setDocuments(previous=>previous.filter(document=>document.path!==action.path&&!document.path.startsWith(`${action.path}/`)));setSelected(previous=>previous&&(previous===action.path||previous.startsWith(`${action.path}/`))?null:previous)}
      setDirectories(new Map());await loadDirectory('');for(const path of expanded)await loadDirectory(path).catch(()=>{})
    }}/>}
    <Dialog.Root open={preview !== null} onOpenChange={next => {if(!next && !busy)setPreview(null)}}><Dialog.Portal><Dialog.Overlay className="dialog-overlay" /><AppDialogContent tone="attention"><AppDialogHeader title={t('确认 Git 操作')} description={t('仅对当前预览的工程状态执行；工程变化时需重新预览。')} closeDisabled={busy} />
      <pre className="workbench-confirm">{preview?.arguments.join(' ')}{preview?.cleanPreview ? `\n${preview.cleanPreview}` : ''}</pre>
      <button type="button" disabled={busy} onClick={() => void operation(async () => { if(!preview)return; const result=await client.request<GitResult>('workbench.git.execute',{...preview.request,expectedRevision:preview.revision,confirmed:true});setGitResult(result);setPreview(null);if(result.success){if(preview.request.action==='commit'){setGitVersion(value=>value+1);setGitDirty(false);setMessage('')}const fresh=await client.request<GitResult>('workbench.git.read',{scope,action:gitView});setGitResult(fresh)}else setError(result.error) })}>{t('确认执行')}</button>
    </AppDialogContent></Dialog.Portal></Dialog.Root>
  </>
}
