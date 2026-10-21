import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Files, FolderPlus, GitBranch, Plus, RefreshCw, Save, X } from 'lucide-react'
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels'
import { AppDialogContent, AppDialogHeader } from './AppDialog'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { newCommandId } from '../../shared/command-id'
import { FileTree, FileTreeFile, FileTreeFolder } from './codeg-workbench/file-tree'
import './workspace-workbench.css'

type Entry = { name: string; path: string; directory: boolean; link: boolean; bytes: number }
type Directory = { entries: Entry[]; next: string | null }
type Document = { path: string; text: string; saved: string; digest: string | null; conflict?: boolean }
type GitResult = { success: boolean; exitCode: number | null; output: string; error: string }
const gitActions = {
  stage: '暂存文件', unstage: '取消暂存', commit: '提交暂存内容', stash_save: '保存到 Stash', stash_apply: '应用 Stash', stash_pop: '取出 Stash', stash_drop: '删除 Stash',
  branch_create: '新建分支', checkout: '切换分支', reset: '重置历史', merge: '合并分支', rebase: '变基', merge_continue: '继续合并', merge_abort: '中止合并',
  rebase_continue: '继续变基', rebase_abort: '中止变基', cherry_pick: '拣选提交', cherry_pick_continue: '继续拣选', cherry_pick_abort: '中止拣选', revert: '撤销提交',
  fetch: '获取远程更新', pull: '拉取远程更新', push: '推送', remote_add: '添加远程仓库', remote_remove: '移除远程仓库', clean: '清理指定未跟踪文件', init: '初始化 Git',
} as const

/** Codeg file-tree and Git auxiliary-panel structure, hosted in Rovai's dialog. */
export function WorkspaceWorkbench({ threadId, bindLeaveGuard }: {
  threadId: string; bindLeaveGuard(guard: (() => void) | null): void
}): React.JSX.Element {
  const client = useThreadClient(), t = useUiText(), scope = useMemo(() => ({ threadId }), [threadId])
  const [open, setOpen] = useState(false), [tab, setTab] = useState<'files' | 'git'>('files')
  const [directories, setDirectories] = useState(new Map<string, Directory>()), [expanded, setExpanded] = useState(new Set<string>())
  const [documents, setDocuments] = useState<Document[]>([]), [selected, setSelected] = useState<string | null>(null)
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), epoch = useRef(0)
  const [creation, setCreation] = useState<'file' | 'directory' | 'rename' | null>(null), [name, setName] = useState('')
  const [gitView, setGitView] = useState('status'), [gitResult, setGitResult] = useState<GitResult | null>(null)
  const [action, setAction] = useState<keyof typeof gitActions>('stage'), [target, setTarget] = useState(''), [paths, setPaths] = useState('')
  const [remote, setRemote] = useState('origin'), [message, setMessage] = useState(''), [mode, setMode] = useState('mixed')
  const [preview, setPreview] = useState<{ request: Record<string, unknown>; revision: string; arguments: string[]; cleanPreview: string | null } | null>(null)
  const dirty = documents.some(document => document.text !== document.saved)
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
  const openFile = async (path: string): Promise<void> => {
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
  const closeDocument = (path: string): void => {
    const document = documents.find(item => item.path === path)
    if (busy || document && document.text !== document.saved && !window.confirm(t('放弃这个文件尚未保存的修改？'))) return
    setDocuments(previous => previous.filter(item => item.path !== path))
    if (selected === path) setSelected(documents.find(item => item.path !== path)?.path ?? null)
  }
  const renderTree = (path: string, depth = 0): React.ReactNode => <>
    {(directories.get(path)?.entries ?? []).map(entry => entry.directory && !entry.link
      ? <FileTreeFolder key={entry.path} path={entry.path} name={entry.name} depth={depth}>{renderTree(entry.path, depth + 1)}</FileTreeFolder>
      : <FileTreeFile key={entry.path} path={entry.path} name={entry.name} depth={depth} aria-disabled={entry.link} onDoubleClick={() => { if (!entry.link) void openFile(entry.path) }} />)}
    {directories.get(path)?.next && <button type="button" className="quiet-button" disabled={busy} onClick={() => void operation(() => loadDirectory(path, directories.get(path)!.next!))}>{t('加载更多')}</button>}
  </>
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
  return <>
    <button type="button" className="quiet-button conversation-tool-button" title={t('文件与 Git 工程工作区')} onClick={() => setOpen(true)}><Files size={15} /><span>{t('工程')}</span></button>
    <Dialog.Root open={open} onOpenChange={next => { if (!next) { try { guard() } catch (failure) { setError(String(failure)); return } }; setOpen(next) }}>
      <Dialog.Portal><Dialog.Overlay className="dialog-overlay" /><AppDialogContent className="workspace-workbench" width="large">
        <AppDialogHeader title={t('工程工作区')} description={t('文件与 Git')} hideDescription closeDisabled={busy || dirty} />
        <nav className="workbench-toolbar" aria-label={t('工程视图')}>
          <button type="button" className={tab === 'files' ? 'quiet-button is-active' : 'quiet-button'} onClick={() => setTab('files')}><Files size={14} />{t('文件')}</button>
          <button type="button" className={tab === 'git' ? 'quiet-button is-active' : 'quiet-button'} onClick={() => { setTab('git'); void readGit() }}><GitBranch size={14} />Git</button>
          <button type="button" className="quiet-button" disabled={busy} onClick={() => void operation(() => loadDirectory(''))} aria-label={t('刷新工程')}><RefreshCw size={14} /></button>
          <button type="button" className="quiet-button" disabled={busy} onClick={() => {setCreation('file');setName('')}}><Plus size={14} />{t('新文件')}</button>
          <button type="button" className="quiet-button" disabled={busy} onClick={() => {setCreation('directory');setName('')}}><FolderPlus size={14} />{t('新目录')}</button>
        </nav>
        {error && <p className="workbench-error" role="alert">{error}</p>}
        {creation && <form className="workbench-toolbar" onSubmit={event => {event.preventDefault();void create()}}><input aria-label={t('相对路径')} value={name} onChange={event => setName(event.target.value)} placeholder={t('相对工程的路径')} /><button type="submit" disabled={busy || !name.trim()}>{t('确认')}</button><button type="button" onClick={() => setCreation(null)}>{t('取消')}</button></form>}
        <PanelGroup direction="horizontal" className="workbench-panels">
          <Panel defaultSize={23} minSize={15}><div className="workbench-tree"><FileTree expanded={expanded} selectedPath={selected ?? undefined} onSelect={path => { if (![...directories.values()].some(directory => directory.entries.some(entry => entry.path === path && (entry.directory || entry.link)))) void openFile(path) }} onExpandedChange={next => {
            setExpanded(next); void operation(async () => { for (const path of next) if (!directories.has(path)) await loadDirectory(path) })
          }}>{renderTree('')}</FileTree></div></Panel>
          <PanelResizeHandle className="workbench-resize" />
          <Panel minSize={25}>
            {tab === 'files' ? <div className="workbench-editor">
              <div className="workbench-tabs" role="tablist">{documents.map(document => <div className={selected === document.path ? 'workbench-tab is-active' : 'workbench-tab'} key={document.path}><button type="button" role="tab" aria-selected={selected === document.path} onClick={() => setSelected(document.path)}>{document.path}{document.text !== document.saved ? ' •' : ''}</button><button type="button" disabled={busy} onClick={() => closeDocument(document.path)} aria-label={t('关闭文件')}><X size={12} /></button></div>)}</div>
              {current ? <><div className="workbench-toolbar"><span title={current.path}>{current.path}</span><button type="button" className="quiet-button" disabled={busy || current.text === current.saved} onClick={() => void save()}><Save size={14} />{t('保存')}</button><button type="button" className="quiet-button" disabled={busy || dirty} onClick={() => {setCreation('rename');setName(current.path)}}>{t('重命名')}</button></div>
                {current.conflict && <div className="workbench-error" role="alert">{t('文件在外部发生变化，本地修改已保留。请复制修改或重新打开外部版本后合并。')}</div>}
                <textarea className="workbench-source" aria-label={t('文件内容')} value={current.text} spellCheck={false} onKeyDown={event => { if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {event.preventDefault();void save()} }} onChange={event => {const text=event.target.value;setDocuments(previous => previous.map(document => document.path === current.path ? {...document,text} : document))}} />
              </> : <div className="workbench-empty">{t('从文件树打开文件')}</div>}
            </div> : <div className="workbench-git">
              <div className="workbench-toolbar"><select aria-label={t('Git 视图')} value={gitView} onChange={event => {setGitView(event.target.value);void readGit(event.target.value)}}>{Object.entries({status:'变更',diff:'工作区 Diff',diff_staged:'暂存 Diff',log:'历史',branches:'分支',stash:'Stash',remotes:'远程',worktrees:'Worktree',conflicts:'冲突'}).map(([value,label]) => <option key={value} value={value}>{t(label)}</option>)}</select><button type="button" disabled={busy} onClick={() => void readGit()}>{t('刷新')}</button></div>
              <pre className="workbench-git-output">{gitResult ? `${gitResult.output.replaceAll('\0','\n').replaceAll('\x1e','\n')}\n${gitResult.error}` : t('选择 Git 视图')}</pre>
              <div className="workbench-git-actions"><select aria-label={t('Git 操作')} value={action} onChange={event => setAction(event.target.value as keyof typeof gitActions)}>{Object.entries(gitActions).map(([value,label]) => <option key={value} value={value}>{t(label)}</option>)}</select>
                <input aria-label={t('目标分支、提交或 Stash')} placeholder={t('目标分支、提交或 Stash')} value={target} onChange={event => setTarget(event.target.value)} />
                <input aria-label={t('远程名称')} value={remote} onChange={event => setRemote(event.target.value)} />
                {action === 'reset' && <select aria-label={t('重置方式')} value={mode} onChange={event => setMode(event.target.value)}><option value="soft">Soft</option><option value="mixed">Mixed</option><option value="hard">Hard</option></select>}
                <textarea aria-label={t('文件路径，每行一个')} placeholder={t('文件路径，每行一个')} value={paths} onChange={event => setPaths(event.target.value)} />
                {(action === 'commit' || action === 'stash_save') && <textarea aria-label={t('说明')} value={message} onChange={event => setMessage(event.target.value)} />}
                <button type="button" disabled={busy} onClick={() => void operation(async () => { const request = {scope,action,paths:paths.split('\n').map(path=>path.trim()).filter(Boolean),...(target?{target}:{}),remote,message,mode}; const value=await client.request<{revision:string;arguments:string[];cleanPreview:string|null}>('workbench.git.preview',request);setPreview({...value,request:{...request,commandId:newCommandId()}}) })}>{t('预览操作')}</button>
              </div>
            </div>}
          </Panel>
        </PanelGroup>
      </AppDialogContent></Dialog.Portal>
    </Dialog.Root>
    <Dialog.Root open={preview !== null} onOpenChange={next => {if(!next && !busy)setPreview(null)}}><Dialog.Portal><Dialog.Overlay className="dialog-overlay" /><AppDialogContent tone="attention"><AppDialogHeader title={t('确认 Git 操作')} description={t('仅对当前预览的工程状态执行；工程变化时需重新预览。')} closeDisabled={busy} />
      <pre className="workbench-confirm">{preview?.arguments.join(' ')}{preview?.cleanPreview ? `\n${preview.cleanPreview}` : ''}</pre>
      <button type="button" disabled={busy} onClick={() => void operation(async () => { if(!preview)return; const result=await client.request<GitResult>('workbench.git.execute',{...preview.request,expectedRevision:preview.revision,confirmed:true});setGitResult(result);setPreview(null); if(!result.success)setError(result.error) })}>{t('确认执行')}</button>
    </AppDialogContent></Dialog.Portal></Dialog.Root>
  </>
}
