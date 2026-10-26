import { createPortal } from 'react-dom'
import { useCallback, useEffect, useMemo, useRef, useState,useSyncExternalStore } from 'react'
import type { WorkbenchEditorSnapshot,WorkbenchEditorState } from '@contracts'
import { WorkbenchEditorDrafts } from './workbench-editor-drafts'
import type { EditorRecoveryCopy } from './workbench-editor-recovery'
import * as Dialog from '@radix-ui/react-dialog'
import { AlertCircle, Send, ChevronDown, FileSearch, Files, FolderPlus, Plus, RefreshCw, Save, Trash2 } from 'lucide-react'
import { readErrorMessage } from './error-message'
import { PinnedFilePreview, useOptionalFilePreview, useOptionalFilePreviewApi } from './FilePreviewContext'
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels'
import { AppDialogContent, AppDialogHeader, AppDialogBody, AppDialogFooter } from './AppDialog'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { useShortcutActions } from './useShortcutActions'
import { NUMBERED_TAB_ACTION_IDS } from '../../shared/codeg-keyboard-shortcuts'
import type { ShortcutActions } from './shortcut-action-driver'
import { newCommandId } from '../../shared/command-id'
import { FileTree, FileTreeFile, FileTreeFolder } from './codeg-workbench/file-tree'
import {WorkspaceFileMenu,WorkspaceFileActionDialog,WorkspaceFileSearch,WorkspaceTrash,type FileAction} from './WorkspaceFileTools'
import { WorkspaceCodeEditor } from './WorkspaceCodeEditor'
import { SplitTabDeck } from './SplitTabDeck'
import {WorkspaceTerminals} from './WorkspaceTerminals'
import {WorkspaceGitViews} from './WorkspaceGitViews'
import { WorkspaceMergeEditor } from './WorkspaceMergeEditor'
import { WorkspaceOrganizer } from './WorkspaceOrganizer'
import { ForgeWorkbench } from './ForgeWorkbench'
import { BrowserWorkbench } from './BrowserWorkbench'
import { ComputerWorkbench } from './ComputerWorkbench'
import { OfficePreviewWorkbench } from './OfficePreviewWorkbench'
import { NativeHistoryWorkbench } from './NativeHistoryWorkbench'
import { UnifiedDiffPreview } from './codeg-native-content/components/diff/unified-diff-preview'
import './codeg-native-content/source-rich.css'
import './workspace-workbench.css'

type Entry = { name: string; path: string; directory: boolean; link: boolean; managedAlias?:boolean; bytes: number }
type Directory = { entries: Entry[]; next: string | null }
type Document = { path: string; text: string; saved: string; digest: string | null; conflict?: boolean }
type GitResult = { success: boolean; exitCode: number | null; output: string; error: string }
type WorkbenchTab = 'files' | 'git' | 'organizer' | 'forge' | 'browser' | 'computer' | 'office' | 'history' | 'trash' | 'terminals'
const gitActions = {
  stage: '暂存文件', unstage: '取消暂存', commit: '提交暂存内容', stash_save: '保存到 Stash', stash_apply: '应用 Stash', stash_pop: '取出 Stash', stash_drop: '删除 Stash',
  clone:'克隆到子目录',worktree_add:'创建独立工作目录',worktree_remove:'移除托管工作目录',branch_delete:'删除已合并分支',branch_rename:'重命名分支',tag_create:'创建标签',tag_delete:'删除标签',discard:'放弃指定工作区改动',branch_create: '新建分支', checkout: '切换分支', reset: '重置历史', merge: '合并分支', rebase: '变基', merge_continue: '继续合并', merge_abort: '中止合并',
  rebase_continue: '继续变基', rebase_abort: '中止变基', cherry_pick: '拣选提交', cherry_pick_continue: '继续拣选', cherry_pick_abort: '中止拣选', revert: '撤销提交',
  fetch: '获取远程更新', pull: '拉取远程更新', push: '推送', remote_add: '添加远程仓库', remote_remove: '移除远程仓库', clean: '清理指定未跟踪文件', init: '初始化 Git',
} as const

/** Codeg file-tree and Git structure, hosted in the retained workspace deck. */
export function WorkspaceWorkbench({ threadId, projectPath, members, bindLeaveGuard, onQuickMessage, onWorkspaceFiles, onOpenNativeConversation, initiallyOpen=false, initialTab='files', onClosed, onMaximize, maximized=false, visible=true, terminalRequest=0, terminalHost }: {
  threadId: string; projectPath:string; members:Array<{id:string;name:string}>; bindLeaveGuard(guard: (() => void) | null): void; onQuickMessage?(text:string):boolean;onWorkspaceFiles?(files:File[]):Promise<boolean>;onOpenNativeConversation?(reference:import('@contracts').CanvasConversationRef):Promise<boolean>;initiallyOpen?:boolean;initialTab?:'files'|'terminals';onClosed?():void;onMaximize?():void;maximized?:boolean;visible?:boolean;terminalRequest?:number;terminalHost?:HTMLElement|null
}): React.JSX.Element {
  const client = useThreadClient(), t = useUiText(), scope = useMemo(() => ({ threadId }), [threadId])
  const filePreview = useOptionalFilePreview(), filePreviewApi = useOptionalFilePreviewApi()
  const [previewPath, setPreviewPath] = useState<string | null>(null), [previewActions, setPreviewActions] = useState<HTMLDivElement | null>(null)
  const officePreview = previewPath !== null && /\.(docx|xlsx|pptx)$/i.test(previewPath)
  const previewSource = useMemo(() => previewPath && !officePreview ? { kind: 'camp_workspace' as const, threadId, rawReference: previewPath } : null, [threadId, previewPath, officePreview])
  const [treeShown,setTreeShown]=useState(true)
  const workspaceTab=filePreview?.activeTab?.kind==='workspace'?filePreview.activeTab:null
  const open = true, [tab, setTab] = useState<WorkbenchTab>('files')
  const [visited,setVisited]=useState(new Set<WorkbenchTab>(['files']))
  useEffect(()=>{setVisited(previous=>previous.has(tab)?previous:new Set(previous).add(tab))},[tab])
  const closingBrowsers=useRef(new Set<string>()),closedBrowsers=useRef(new Set<string>())
  const [fileAction,setFileAction]=useState<FileAction|null>(null),[searchVisible,setSearchVisible]=useState(false),[cursorLine,setCursorLine]=useState<number|undefined>(undefined)
  const [organizerDirty,setOrganizerDirty]=useState(false)
  const [forgeDirty,setForgeDirty]=useState(false)
  const [officePath,setOfficePath]=useState<string|null>(null)
  const [directories, setDirectories] = useState(new Map<string, Directory>()), [expanded, setExpanded] = useState(new Set<string>())
  const [documents, setDocuments] = useState<Document[]>([]), [selected, setSelected] = useState<string | null>(null)
  const editorDrafts=useMemo(()=>new WorkbenchEditorDrafts(client,threadId,projectPath),[client,threadId,projectPath]),draftStatus=useSyncExternalStore(editorDrafts.subscribe,editorDrafts.snapshot,editorDrafts.snapshot)
  const hydrated=useRef(false),[draftRestoreError,setDraftRestoreError]=useState(''),[draftReadAttempt,setDraftReadAttempt]=useState(0),[hostDraft,setHostDraft]=useState<WorkbenchEditorSnapshot|null>(null)
  const [draftCopies,setDraftCopies]=useState<EditorRecoveryCopy[]|null>(null),[draftCopiesNext,setDraftCopiesNext]=useState<string|null>(null)
  const [splitLayout,setSplitLayout]=useState<unknown|null>(null),[restoreToken,setRestoreToken]=useState(0),restoredSplitLayout=useRef<unknown>(null)
  const acceptRestoredDraft=(value:WorkbenchEditorState|null):void=>{const next=value??{documents:[],selected:null,layout:null};setDocuments(next.documents);setSelected(next.selected);setSplitLayout(next.layout);restoredSplitLayout.current=next.layout;setRestoreToken(value=>value+1)}
  useEffect(()=>{if(!open)return;let active=true;hydrated.current=false;setDraftRestoreError('');void editorDrafts.start().then(value=>{if(active){acceptRestoredDraft(value);hydrated.current=true}}).catch(error=>{if(active)setDraftRestoreError(readErrorMessage(error))});return()=>{active=false;hydrated.current=false;editorDrafts.dispose()}},[editorDrafts,draftReadAttempt,open])
  useEffect(()=>{if(hydrated.current&&draftStatus.ready)editorDrafts.update({documents,selected,layout:splitLayout})},[editorDrafts,documents,selected,splitLayout,draftStatus.ready])
  const closedFiles=useRef<string[]>([])
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), epoch = useRef(0)
  const [terminalShown,setTerminalShown]=useState(initialTab==='terminals'),[terminalMounted,setTerminalMounted]=useState(initialTab==='terminals')
  useEffect(()=>{if(terminalRequest){setTerminalMounted(true);setTerminalShown(value=>terminalRequest>0?true:!value)}},[terminalRequest])
  const [creation, setCreation] = useState<'file' | 'directory' | 'rename' | null>(null), [name, setName] = useState('')
  const [terminalUnsettled,setTerminalUnsettled]=useState(false)
  const [terminalHeight,setTerminalHeight]=useState(260),terminalDrag=useRef<{y:number;height:number}|null>(null)
  useEffect(()=>{if(terminalHost)terminalHost.style.height=terminalShown?terminalHeight+'px':'0px'},[terminalHost,terminalShown,terminalHeight])
  const [gitDirty,setGitDirty]=useState(false),[gitVersion,setGitVersion]=useState(0)
  const [gitView, setGitView] = useState('status'), [gitResult, setGitResult] = useState<GitResult | null>(null)
  const [action, setAction] = useState<keyof typeof gitActions>('stage'), [target, setTarget] = useState(''), [paths, setPaths] = useState('')
  const [remote, setRemote] = useState('origin'), [message, setMessage] = useState(''), [mode, setMode] = useState('mixed')
  const [preview, setPreview] = useState<{ request: Record<string, unknown>; revision: string; arguments: string[]; cleanPreview: string | null } | null>(null)
  const [merge, setMerge] = useState<{path:string;base:string;ours:string;theirs:string;merged:string}|null>(null)
  const dirty = terminalUnsettled || organizerDirty || forgeDirty || gitDirty || documents.some(document => document.text !== document.saved)
  const current = documents.find(document => document.path === selected)
  const guard = useCallback(() => { if (busy) throw new Error(t('工程操作仍在进行，请稍后离开。')); if (dirty) throw new Error(t('工程文件尚未保存，请先保存或关闭对应标签。'));if(editorDrafts.snapshot().pending||editorDrafts.snapshot().saving)throw new Error(t('编辑草稿或布局仍未补存，请在工程内核对后再离开。')) }, [busy, dirty, t,editorDrafts])
  useEffect(() => { bindLeaveGuard(guard); return () => bindLeaveGuard(null) }, [bindLeaveGuard, guard])
  useEffect(() => () => { ++epoch.current }, [])
  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent): void => { if (dirty || busy||editorDrafts.snapshot().pending||editorDrafts.snapshot().saving) { event.preventDefault(); event.returnValue = '' } }
    window.addEventListener('beforeunload', beforeUnload)
    return () => window.removeEventListener('beforeunload', beforeUnload)
  }, [dirty, busy,editorDrafts])
  const operation = async <T,>(run: () => Promise<T>): Promise<T | undefined> => {
    if (busy) return
    const generation = epoch.current; setBusy(true); setError(null)
    try { return await run() } catch (failure) {
      if (generation === epoch.current) {
        const message = readErrorMessage(failure, t('工程操作失败。'))
        setError(message.includes('workbench.text_required') ? t('这个文件不是可编辑的文本。图片、PDF 和网页请使用预览。') : message)
      }
      return undefined
    }
    finally { if (generation === epoch.current) setBusy(false) }
  }
  const loadDirectory = async (path: string, after?: string): Promise<void> => {
    const generation = epoch.current
    const result = await client.request<Directory>('workbench.files.list', { scope, path, ...(after ? { after } : {}) })
    if (generation === epoch.current) setDirectories(previous => new Map(previous).set(path, { entries: after ? [...(previous.get(path)?.entries ?? []), ...result.entries] : result.entries, next: result.next }))
  }
  useEffect(() => { if (open && !directories.has('')) void operation(() => loadDirectory('')) }, [open])
  const openFile = async (path: string,line?:number): Promise<void> => {
    const showFiles = (): void => { setTab('files'); if(workspaceTab?.tool!=='files')filePreview?.openWorkspace('files') }
    if (/\.(png|jpe?g|gif|webp|bmp|ico|avif|svg|pdf|html?|docx|xlsx|pptx)$/i.test(path)) {
      setError(null)
      if (!filePreviewApi && !/\.(docx|xlsx|pptx)$/i.test(path)) { setError(t('文件预览服务暂不可用，请重新打开工程。')); return }
      showFiles(); setPreviewPath(path); return
    }
    if(!draftStatus.ready){setError(t('工程编辑草稿尚未读取成功，请先重新读取。'));return}
    setCursorLine(line);showFiles()
    if (documents.some(document => document.path === path)) { setSelected(path); setPreviewPath(null); return }
    await operation(async () => {
      const result = await client.request<{ path: string; text: string; digest: string }>('workbench.files.read', { scope, path })
      setDocuments(previous => previous.some(document => document.path === path) ? previous : [...previous, { ...result, saved: result.text }])
      setSelected(path); setPreviewPath(null)
    })
  }
  const save = async (path:string|null=selected): Promise<void> => {
    const document = documents.find(item=>item.path===path)
    if (!document) return
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
  const closeDocument = (path: string): boolean => {
    const document = documents.find(item => item.path === path)
    if (busy || document && document.text !== document.saved && !window.confirm(t('放弃这个文件尚未保存的修改？'))) return false
    closedFiles.current=[path,...closedFiles.current.filter(value=>value!==path)].slice(0,100)
    setDocuments(previous => previous.filter(item => item.path !== path))
    if (selected === path) setSelected(documents.find(item => item.path !== path)?.path ?? null)
    return true
  }
  const renderTree = (path: string, depth = 0): React.ReactNode => <>
    {(directories.get(path)?.entries ?? []).map(entry => entry.directory && (!entry.link || entry.managedAlias)
      ? <FileTreeFolder key={entry.path} path={entry.path} name={entry.name} depth={depth} actions={<WorkspaceFileMenu path={entry.path} disabled={busy||dirty||entry.link} onAction={setFileAction}/>}>{renderTree(entry.path, depth + 1)}</FileTreeFolder>
      : <FileTreeFile key={entry.path} path={entry.path} name={entry.name} depth={depth} actions={<WorkspaceFileMenu path={entry.path} disabled={busy||dirty||entry.link} onAction={setFileAction}/>} aria-disabled={entry.link} onDoubleClick={() => { if (!entry.link) void openFile(entry.path) }} />)}
    {directories.get(path)?.next && <button type="button" className="quiet-button" disabled={busy} onClick={() => void operation(() => loadDirectory(path, directories.get(path)!.next!))}>{t('加载更多')}</button>}
  </>
  const activeShortcutSurface=():boolean=>{return open&&visible&&!document.querySelector('[role="dialog"][aria-modal="true"]')&&Boolean(document.activeElement?.closest('.workspace-workbench'))}
  const moveTab=(delta:number):void=>{if(filePreview){const tabs=filePreview.tabs,index=tabs.findIndex(row=>row.id===filePreview.activeTabId);if(tabs.length)filePreview.activate(tabs[(Math.max(0,index)+delta+tabs.length)%tabs.length].id);return}const index=documents.findIndex(row=>row.path===selected);if(documents.length){setPreviewPath(null);setSelected(documents[(Math.max(0,index)+delta+documents.length)%documents.length].path)}}
  useShortcutActions({
    close_current_tab:()=>{if(tab==='files'){if(previewPath)setPreviewPath(null);else if(selected)closeDocument(selected);else if(filePreview?.activeTabId)filePreview.close(filePreview.activeTabId)}},
    close_all_file_tabs:()=>{if(busy)return true;if(documents.some(row=>row.text!==row.saved)&&!window.confirm(t('放弃所有文件尚未保存的修改？')))return true;closedFiles.current=[...documents.map(row=>row.path).reverse(),...closedFiles.current].slice(0,100);setDocuments([]);setSelected(null);setPreviewPath(null)},
    reopen_last_closed_tab:()=>{if(busy)return true;const path=closedFiles.current[0];if(path){closedFiles.current.shift();void openFile(path)}},
    next_tab:()=>{if(tab!=='files')return false;moveTab(1)},prev_tab:()=>{if(tab!=='files')return false;moveTab(-1)},
    ...Object.fromEntries(NUMBERED_TAB_ACTION_IDS.map((id,index)=>[id,()=>{if(tab!=='files')return false;if(filePreview){const row=filePreview.tabs[index];if(row)filePreview.activate(row.id);return}const row=documents[index];if(row)setSelected(row.path)}])) as ShortcutActions,
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
  useEffect(()=>{
    if(!workspaceTab)return
    setTab(workspaceTab.tool)
    if(workspaceTab.tool==='git'&&!gitResult)void readGit()
    if(workspaceTab.resource&&workspaceTab.tool==='office')setOfficePath(workspaceTab.resource)
    if(workspaceTab.resource&&workspaceTab.tool==='files'&&draftStatus.ready){
      if(documents.some(row=>row.path===workspaceTab.resource)){setSelected(workspaceTab.resource);setPreviewPath(null)}
      else void openFile(workspaceTab.resource)
    }
  },[workspaceTab?.id,draftStatus.ready])
  const filesTabId=filePreview?.tabs.find(row=>row.kind==='workspace'&&row.tool==='files'&&!row.resource)?.id
  useEffect(()=>{
    if(filesTabId)filePreview?.updateWorkspaceTab(filesTabId,{dirty:documents.some(row=>row.text!==row.saved)})
  },[filesTabId,filePreview?.updateWorkspaceTab,documents])
  useEffect(()=>filePreview?.bindWorkspaceClose(row=>{
    if(busy)return false
    if(row.tool==='files'&&row.resource)return closeDocument(row.resource)
    if(row.tool==='files'&&documents.some(document=>document.text!==document.saved)){setError(t('请先保存或放弃当前修改，再关闭这个页面。'));return false}
    if(row.tool==='browser'&&row.resource&&!closedBrowsers.current.has(row.id)){
      if(!client.browserWorkspace)return false
      if(!closingBrowsers.current.has(row.id)){
        closingBrowsers.current.add(row.id)
        void client.browserWorkspace.control({id:row.resource,action:'close'}).then(()=>{closedBrowsers.current.add(row.id);filePreview?.close(row.id)}).catch(failure=>setError(readErrorMessage(failure))).finally(()=>closingBrowsers.current.delete(row.id))
      }
      return false
    }
    if(row.tool==='organizer'&&organizerDirty||row.tool==='forge'&&forgeDirty||row.tool==='git'&&gitDirty){setError(t('请先保存或放弃当前修改，再关闭这个页面。'));return false}
    return true
  }),[filePreview?.bindWorkspaceClose,busy,documents,selected,organizerDirty,forgeDirty,gitDirty])
  const terminal=<section className="workbench-bottom-terminal" hidden={!terminalShown}><div className="workspace-terminal-resize" role="separator" aria-label={t('调整终端高度')} aria-orientation="horizontal" aria-valuenow={terminalHeight} aria-valuemin={150} aria-valuemax={Math.max(150,Math.floor((terminalHost?.parentElement?.clientHeight??600)/2))} tabIndex={0} onPointerDown={event=>{terminalDrag.current={y:event.clientY,height:terminalHeight};event.currentTarget.setPointerCapture(event.pointerId)}} onPointerMove={event=>{if(terminalDrag.current)setTerminalHeight(Math.round(Math.max(150,Math.min((terminalHost?.parentElement?.clientHeight??600)/2,terminalDrag.current.height+terminalDrag.current.y-event.clientY))))}} onPointerUp={()=>{terminalDrag.current=null}} onPointerCancel={()=>{terminalDrag.current=null}} onKeyDown={event=>{if(event.key==='ArrowUp'||event.key==='ArrowDown'){event.preventDefault();setTerminalHeight(value=>Math.max(150,Math.min((terminalHost?.parentElement?.clientHeight??600)/2,value+(event.key==='ArrowUp'?24:-24))))}}}/><div className="workspace-terminal-heading"><span>{t('终端')}</span><button className="quiet-button workbench-icon" type="button" title={t('收起终端，保留进程')} aria-label={t('收起终端，保留进程')} onClick={()=>setTerminalShown(false)}><ChevronDown size={15}/></button></div><WorkspaceTerminals projectPath={projectPath} onUnsettledChange={setTerminalUnsettled}/></section>
  const showFiles = merge !== null || tab === 'files' || tab === 'git' || tab === 'office'
  return <>
    <section className="workspace-workbench" aria-label={t('工程工作区')}>
        {showFiles && <div className="workbench-toolbar workbench-file-actions">
          <button type="button" className="quiet-button workbench-icon" title={t(treeShown?'收起目录':'展开目录')} aria-label={t(treeShown?'收起目录':'展开目录')} aria-expanded={treeShown} onClick={()=>setTreeShown(value=>!value)}><Files size={15}/></button>
          <button type="button" className={`workbench-icon ${searchVisible?'quiet-button is-active':'quiet-button'}`} title={t(searchVisible?'文件树':'搜索文件')} aria-label={t(searchVisible?'文件树':'搜索文件')} onClick={()=>setSearchVisible(value=>!value)}><FileSearch size={15}/></button>
          <button type="button" className="quiet-button workbench-icon" title={t('回收站')} aria-label={t('回收站')} onClick={()=>{setTab('trash');filePreview?.openWorkspace('trash')}}><Trash2 size={15}/></button>
          <span className="workbench-toolbar-spacer"/>
          <button type="button" className="quiet-button" disabled={busy} onClick={() => void operation(() => loadDirectory(''))} aria-label={t('刷新工程')}><RefreshCw size={14} /></button>
          <button type="button" className="quiet-button workbench-icon" title={t('新文件')} aria-label={t('新文件')} disabled={busy} onClick={() => {setCreation('file');setName('')}}><Plus size={15} /></button>
          <button type="button" className="quiet-button workbench-icon" title={t('新目录')} aria-label={t('新目录')} disabled={busy} onClick={() => {setCreation('directory');setName('')}}><FolderPlus size={15} /></button>
        </div>}
        {error && <div className="workbench-notice" role="alert"><AlertCircle size={16}/><p>{error}</p><button type="button" className="quiet-button" onClick={()=>setError(null)}>{t('收起')}</button></div>}
        {(showFiles || draftRestoreError || draftStatus.issue || draftStatus.pending) && <details className="workbench-draft-status" open={Boolean(!draftStatus.ready || draftRestoreError || draftStatus.issue || draftStatus.pending)}><summary>{t(!draftStatus.ready?'正在读取编辑草稿':draftStatus.saving?'正在保存编辑草稿':draftStatus.pending?'编辑草稿需要处理':'草稿与恢复')}</summary><div className="workbench-toolbar"><span role="status">{t(!draftStatus.ready?'正在恢复工程编辑草稿':draftStatus.saving?'正在保存编辑草稿':draftStatus.pending?'原编辑草稿等待补存':'编辑草稿已保存')}</span>{(draftRestoreError||draftStatus.issue)&&<span role="alert" className="workbench-error">{draftRestoreError||draftStatus.issue}</span>}{!draftStatus.ready&&<button className="quiet-button" type="button" disabled={busy} onClick={()=>setDraftReadAttempt(value=>value+1)}>{t('重新读取编辑草稿')}</button>}{draftStatus.pending&&<button className="quiet-button" type="button" disabled={draftStatus.saving||busy} onClick={()=>void editorDrafts.flush().catch(error=>setError(readErrorMessage(error)))}>{t('原样补存')}</button>}{draftStatus.issue&&<button className="quiet-button" type="button" disabled={busy||draftStatus.saving} onClick={()=>void editorDrafts.readHost().then(setHostDraft).catch(error=>setError(readErrorMessage(error)))}>{t('查看主机草稿')}</button>}<button className="quiet-button" type="button" disabled={busy||!draftStatus.ready} onClick={()=>void editorDrafts.copies().then(value=>{setDraftCopies(value.items);setDraftCopiesNext(value.next)}).catch(error=>setError(readErrorMessage(error)))}>{t('取回保留的编辑副本')}</button></div></details>}
        {draftCopies&&<details className="workbench-draft-comparison" open><summary>{t('本机保留的编辑副本')}</summary><p>{t('取回只替换编辑器中的草稿，不会写回工程文件。当前输入也会另存为副本。')}</p>{draftCopies.map((copy,index)=><div className="workbench-toolbar" key={copy.key}><span>{t('副本')} {index+1} · {copy.documentCount} {t('个文件')} · {copy.paths.join(' · ')}</span><button className="quiet-button" type="button" disabled={busy||draftStatus.saving} onClick={()=>void editorDrafts.restoreCopy(copy.key).then(value=>{acceptRestoredDraft(value);setDraftCopies(null)}).catch(error=>setError(readErrorMessage(error)))}>{t('把这份副本放回编辑器')}</button></div>)}{!draftCopies.length&&<p>{t('当前没有保留的编辑副本。')}</p>}{draftCopiesNext&&<button className="quiet-button" type="button" onClick={()=>void editorDrafts.copies(draftCopiesNext).then(value=>{setDraftCopies(previous=>[...(previous??[]),...value.items]);setDraftCopiesNext(value.next)}).catch(error=>setError(readErrorMessage(error)))}>{t('加载更多副本')}</button>}<button className="quiet-button" type="button" onClick={()=>setDraftCopies(null)}>{t('收起副本列表')}</button></details>}
        {hostDraft&&<details className="workbench-draft-comparison" open><summary>{t('主机当前草稿')}</summary>{hostDraft.value?.documents.map(item=><details key={item.path}><summary>{item.path}</summary><pre>{item.text}</pre></details>)}<button className="quiet-button" type="button" disabled={busy||draftStatus.saving} onClick={()=>void editorDrafts.useHostVersion().then(value=>{acceptRestoredDraft(value);setHostDraft(null)}).catch(error=>setError(readErrorMessage(error)))}>{t('保留本机副本并使用主机草稿')}</button><button className="quiet-button" type="button" onClick={()=>setHostDraft(null)}>{t('继续保留当前输入')}</button></details>}
        {creation && <form className="workbench-toolbar" onSubmit={event => {event.preventDefault();void create()}}><input className="workspace-field" aria-label={t('相对路径')} value={name} onChange={event => setName(event.target.value)} placeholder={t('相对工程的路径')} /><button className="primary-button" type="submit" disabled={busy || !name.trim()}>{t('确认')}</button><button className="quiet-button" type="button" onClick={() => setCreation(null)}>{t('取消')}</button></form>}
        <PanelGroup direction="horizontal" className="workbench-panels">
          {showFiles && treeShown && <Panel id="workbench-files" order={1} defaultSize={23} minSize={15}><div className="workbench-tree codeg-native-rich">{searchVisible?<WorkspaceFileSearch threadId={threadId} onOpen={(path,line)=>void openFile(path,line)}/>:<FileTree expanded={expanded} selectedPath={previewPath ?? selected ?? undefined} onSelect={path => { if (![...directories.values()].some(directory => directory.entries.some(entry => entry.path === path && (entry.directory || entry.link)))) void openFile(path) }} onExpandedChange={next => {
            setExpanded(next); void operation(async () => { for (const path of next) if (!directories.has(path)) await loadDirectory(path) })
          }}>{renderTree('')}</FileTree>}</div></Panel>}
          {showFiles && treeShown && <PanelResizeHandle className="workbench-resize" />}
          <Panel id="workbench-content" order={2} minSize={25}>
            {merge&&<WorkspaceMergeEditor {...merge} onClose={()=>setMerge(null)} onApply={text=>{setDocuments(previous=>previous.map(document=>document.path===merge.path?{...document,text}:document));setMerge(null)}}/>}
            <div className="workspace-tool-page" hidden={tab!=='files'||!!merge}><div className="workbench-editor">
              {officePreview ? <><div className="workbench-toolbar workbench-preview-heading"><span>{previewPath}</span><button type="button" className="quiet-button" onClick={()=>setPreviewPath(null)}>{t('返回编辑器')}</button></div><OfficePreviewWorkbench key={previewPath} threadId={threadId} path={previewPath!}/></> : previewSource && filePreviewApi ? <><div className="workbench-toolbar workbench-preview-heading"><span title={previewPath!}>{previewPath}</span><div ref={setPreviewActions} className="workbench-preview-actions"/><button type="button" className="quiet-button" onClick={()=>setPreviewPath(null)}>{t('返回编辑器')}</button></div><PinnedFilePreview embedded api={filePreviewApi} source={previewSource} resolvedTheme={filePreview?.resolvedTheme ?? 'night'} actionsHost={previewActions}/></> : current ? <><div className="workbench-toolbar"><span title={current.path}>{current.path}</span><button className="quiet-button workbench-icon" type="button" title={t('把选区或文件加入输入')} aria-label={t('把选区或文件加入输入')} disabled={!onQuickMessage} onClick={()=>{const selection=window.getSelection(),text=selection?.anchorNode?.parentElement?.closest('.workbench-editor')?selection.toString():'';if(!onQuickMessage?.('工程文件：'+current.path+String.fromCharCode(10)+(text||current.text).slice(0,16000)))setError(t('当前输入暂不可用，请稍后重试。'))}}><Send size={14}/></button><button type="button" className="quiet-button" disabled={busy || current.text === current.saved} onClick={() => void save()}><Save size={14} />{t('保存')}</button><button type="button" className="quiet-button" disabled={busy || dirty} onClick={() => {setCreation('rename');setName(current.path)}}>{t('重命名')}</button><button type="button" className="quiet-button" disabled={busy} onClick={()=>void openMerge()}>{t('解决冲突')}</button></div>
                {current.conflict && <div className="workbench-error" role="alert">{t('文件在外部发生变化，本地修改已保留。')}<button className="quiet-button" disabled={busy} onClick={()=>void mergeExternal()}>{t('比较并合并外部版本')}</button></div>}
                <SplitTabDeck key={`${threadId}:${projectPath}`} scopeKey={`${threadId}:${projectPath}`} items={documents.map(item=>({id:item.path,label:item.path,dirty:item.text!==item.saved}))} selected={selected} disabled={busy||!draftStatus.ready} restoredLayout={restoredSplitLayout.current} restoreToken={restoreToken} onLayoutChange={setSplitLayout} externalTabBar onSelect={setSelected} onClose={closeDocument} renderBody={(item,visible)=>{const document=documents.find(row=>row.path===item.id)!;return <WorkspaceCodeEditor path={document.path} text={document.text} visible={visible} line={document.path===selected?cursorLine:undefined} onSave={()=>void save(document.path)} onChange={text=>setDocuments(previous=>previous.map(row=>row.path===document.path?{...row,text}:row))}/>}}/>
              </> : <div className="workbench-empty workbench-files-empty"><Files size={30}/><strong>{t('选择文件，开始查看')}</strong><p>{t('点击左侧文件。图片和网页会显示预览，文本文件可直接编辑。')}</p></div>}
            </div></div>
            <div className="workspace-tool-page" hidden={tab!=='git'||!!merge}><div className="workbench-git">
              <div className="workbench-toolbar"><select className="workspace-field" aria-label={t('Git 视图')} value={gitView} onChange={event => {setGitView(event.target.value);void readGit(event.target.value)}}>{Object.entries({status:'变更',diff:'工作区 Diff',diff_staged:'暂存 Diff',log:'历史',branches:'分支',stash:'Stash',remotes:'远程',worktrees:'Worktree',conflicts:'冲突',tags:'标签',reflog:'引用历史'}).map(([value,label]) => <option key={value} value={value}>{t(label)}</option>)}</select><button className="quiet-button" type="button" disabled={busy} onClick={() => void readGit()}>{t('刷新')}</button></div>
              {gitResult && (gitView==='diff'||gitView==='diff_staged') && gitResult.success ? <div className="workbench-git-output codeg-native-rich"><UnifiedDiffPreview diffText={gitResult.output} clickableFilePath={false} /></div> : gitResult?<WorkspaceGitViews key={gitVersion} threadId={threadId} view={gitView} result={gitResult} disabled={busy} onAction={prepareGitAction} onDirtyChange={setGitDirty} onOpenFile={path=>{if(!gitDirty)void openFile(path)}}/>:<div className="workbench-empty">{t('选择 Git 视图')}</div>}
              <details className="workbench-git-advanced"><summary>{t('更多 Git 操作')}</summary><div className="workbench-git-actions"><select className="workspace-field" aria-label={t('Git 操作')} value={action} onChange={event => setAction(event.target.value as keyof typeof gitActions)}>{Object.entries(gitActions).map(([value,label]) => <option key={value} value={value}>{t(label)}</option>)}</select>
                <input className="workspace-field" aria-label={t('目标分支、提交或 Stash')} placeholder={t('目标分支、提交或 Stash')} value={target} onChange={event => setTarget(event.target.value)} />
                <input className="workspace-field" aria-label={t('远程名称')} value={remote} onChange={event => setRemote(event.target.value)} />
                {action === 'reset' && <select className="workspace-field" aria-label={t('重置方式')} value={mode} onChange={event => setMode(event.target.value)}><option value="soft">Soft</option><option value="mixed">Mixed</option><option value="hard">Hard</option></select>}
                <textarea className="workspace-field" aria-label={t('文件路径，每行一个')} placeholder={t('文件路径，每行一个')} value={paths} onChange={event => setPaths(event.target.value)} />
                {(action === 'commit' || action === 'stash_save' || action==='branch_rename') && <textarea className="workspace-field" aria-label={t('说明')} value={message} onChange={event => setMessage(event.target.value)} />}
                <button className="quiet-button" type="button" disabled={busy} onClick={() => void operation(async () => { const request = {scope,action,paths:paths.split('\n').map(path=>path.trim()).filter(Boolean),...(target?{target}:{}),remote,message,mode}; const value=await client.request<{revision:string;arguments:string[];cleanPreview:string|null}>('workbench.git.preview',request);setPreview({...value,request:{...request,commandId:newCommandId()}}) })}>{t('预览操作')}</button>
              </div></details>
            </div></div>
            {visited.has('trash')&&<div className="workspace-tool-page" hidden={tab!=='trash'}><WorkspaceTrash threadId={threadId} onRestored={async()=>{for(const path of ['',...expanded])await loadDirectory(path).catch(()=>{})}}/></div>}
            {visited.has('history')&&<div className="workspace-tool-page" hidden={tab!=='history'}><NativeHistoryWorkbench threadId={threadId} onOpenConversation={onOpenNativeConversation?async reference=>{guard();return onOpenNativeConversation(reference)}:undefined}/></div>}
            {officePath&&<div className="workspace-tool-page" hidden={tab!=='office'}><OfficePreviewWorkbench key={officePath} threadId={threadId} path={officePath}/></div>}
            {visited.has('computer')&&<div className="workspace-tool-page" hidden={tab!=='computer'}><ComputerWorkbench threadId={threadId}/></div>}
            {visited.has('browser')&&<div className="workspace-tool-page workspace-browser-page" hidden={tab!=='browser'}><BrowserWorkbench threadId={threadId} resource={workspaceTab?.tool==='browser'?workspaceTab.resource:undefined} active={visible&&workspaceTab?.tool==='browser'} onQuickMessage={onQuickMessage} onWorkspaceFiles={onWorkspaceFiles}/></div>}
            {visited.has('forge')&&<div className="workspace-tool-page" hidden={tab!=='forge'}><ForgeWorkbench threadId={threadId} projectPath={projectPath} members={members} onDirtyChange={setForgeDirty}/></div>}
            {visited.has('organizer')&&<div className="workspace-tool-page" hidden={tab!=='organizer'}><WorkspaceOrganizer threadId={threadId} onDirtyChange={setOrganizerDirty} onDirectoryAliasesApplied={()=>{setDirectories(new Map());setExpanded(new Set());void loadDirectory('')}} onUseMessage={onQuickMessage?text=>onQuickMessage(text):undefined}/></div>}
          </Panel>
        </PanelGroup>
      {terminalMounted&&(terminalHost?createPortal(terminal,terminalHost):terminal)}
    </section>
    {fileAction&&<WorkspaceFileActionDialog key={`${fileAction.operation}:${fileAction.path}`} threadId={threadId} action={fileAction} onClose={()=>setFileAction(null)} onApplied={async(action,destination)=>{
      const moved=(path:string):string=>destination?destination+path.slice(action.path.length):path
      if(action.operation==='move'){setDocuments(previous=>previous.map(document=>document.path===action.path||document.path.startsWith(`${action.path}/`)?{...document,path:moved(document.path)}:document));setSelected(previous=>previous&&(previous===action.path||previous.startsWith(`${action.path}/`))?moved(previous):previous)}
      if(action.operation==='trash'){setDocuments(previous=>previous.filter(document=>document.path!==action.path&&!document.path.startsWith(`${action.path}/`)));setSelected(previous=>previous&&(previous===action.path||previous.startsWith(`${action.path}/`))?null:previous)}
      setDirectories(new Map());await loadDirectory('');for(const path of expanded)await loadDirectory(path).catch(()=>{})
    }}/>}
    <Dialog.Root open={preview !== null} onOpenChange={next => {if(!next && !busy)setPreview(null)}}><Dialog.Portal><Dialog.Overlay className="dialog-overlay" /><AppDialogContent tone="attention"><AppDialogHeader title={t('确认 Git 操作')} description={t('仅对当前预览的工程状态执行；工程变化时需重新预览。')} closeDisabled={busy} /><AppDialogBody className="app-dialog-stack"><pre className="workbench-confirm">{preview?.arguments.join(' ')}{preview?.cleanPreview ? `\n${preview.cleanPreview}` : ''}</pre></AppDialogBody><AppDialogFooter><button className="quiet-button" type="button" disabled={busy} onClick={() => void operation(async () => { if(!preview)return; const result=await client.request<GitResult>('workbench.git.execute',{...preview.request,expectedRevision:preview.revision,confirmed:true});setGitResult(result);setPreview(null);if(result.success){if(preview.request.action==='commit'){setGitVersion(value=>value+1);setGitDirty(false);setMessage('')}const fresh=await client.request<GitResult>('workbench.git.read',{scope,action:gitView});setGitResult(fresh)}else setError(result.error) })}>{t('确认执行')}</button></AppDialogFooter></AppDialogContent></Dialog.Portal></Dialog.Root>
  </>
}
