import { createContext, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, type ComponentProps, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { LoaderCircle } from 'lucide-react'
import { PanelToggleIcon } from './PanelToggleIcon'
import { useOptionalFilePreview, useWorkbenchPreviewHost } from './FilePreviewContext'
import { readErrorMessage } from './error-message'
import { useThreadClient } from './camp-client'
import { WorkspaceWorkbench } from './WorkspaceWorkbench'
import { useUiText } from './interface-language'
import { useShortcutActions } from './useShortcutActions'
import type { WorkspaceEntryTool } from './WorkspaceToolMenu'
import './engineering-dock.css'

type Props = Omit<ComponentProps<typeof WorkspaceWorkbench>, 'initialTab' | 'initiallyOpen' | 'onClosed' | 'onMaximize' | 'maximized' | 'visible' | 'terminalRequest' | 'terminalHost'> & { missionId?: string | null; children?: ReactNode; headerEntry?: boolean; enabled?: boolean }
const EngineeringDockContext = createContext({ opened: 0, collapse: (_restoreFocus?: boolean) => {} })
export const useEngineeringDock = () => useContext(EngineeringDockContext)

/** Files and tools use the existing retained preview deck; there is no second pane. */
export function WorkspaceWorkbenchEntry({ children, headerEntry = true, enabled = true, ...props }: Props): React.JSX.Element {
  const client = useThreadClient(), t = useUiText(), preview = useOptionalFilePreview()
  const workbenchHost = useWorkbenchPreviewHost(props.threadId)
  const owner = useMemo(() => ({ client, threadId: props.threadId, missionId: props.missionId ?? null }), [client, props.threadId, props.missionId])
  const current = useRef(owner); current.current = owner
  const generation = useRef(0), previousOwner = useRef(owner)
  if (previousOwner.current !== owner) { previousOwner.current = owner; generation.current += 1 }
  const [ready, setReady] = useState<{ root: string; owner: object } | null>(null)
  const [issue, setIssue] = useState<{ owner: object; message: string } | null>(null)
  const [busy, setBusy] = useState<object | null>(null), pending = useRef<object | null>(null)
  const [host, setHost] = useState<Element | null>(null), trigger = useRef<HTMLButtonElement>(null)
  const [terminalHost, setTerminalHost] = useState<HTMLDivElement | null>(null)
  const [opened, setOpened] = useState(0), [terminalCommand, setTerminalCommand] = useState({ owner, value: 0 })
  const active = ready?.owner === owner ? ready : null
  const visible = enabled && !!preview?.paneVisible
  const collapse = (restoreFocus = false): void => { preview?.hidePane(); if (restoreFocus) trigger.current?.focus() }
  useLayoutEffect(() => {
    if (!headerEntry) return
    const resolve = (): void => setHost([...document.querySelectorAll('[data-engineering-entry]')].find(el => (el as HTMLElement).dataset.engineeringEntry === props.threadId) ?? null)
    resolve()
    const observer = new MutationObserver(resolve); observer.observe(document.body, { childList: true, subtree: true })
    return () => observer.disconnect()
  }, [headerEntry, props.threadId])
  const prepare = async (): Promise<boolean> => {
    if (pending.current === owner) return false
    const captured = owner, capturedGeneration = generation.current
    pending.current = captured; setBusy(captured); setIssue(null)
    try {
      const result = await client.request<{ workspaceRoot: string }>('workbench.files.list', { scope: { threadId: props.threadId, missionId: props.missionId ?? null }, path: '' })
      if (current.current !== captured || generation.current !== capturedGeneration) return false
      if (!result.workspaceRoot) throw new Error(t('当前主机尚未提供已核实的工程目录。'))
      if(active&&active.root!==result.workspaceRoot)throw new Error(t('工程目录已变化，请先保存当前草稿，再重新进入工程。'))
      if(!active)setReady({ root: result.workspaceRoot, owner: captured }); setOpened(value => value + 1)
      return true
    } catch (error) {
      if (current.current === captured && generation.current === capturedGeneration) setIssue({ owner: captured, message: readErrorMessage(error) })
      return false
    } finally {
      if (pending.current === captured) { pending.current = null; setBusy(null) }
    }
  }
  const openTool = async (tool: WorkspaceEntryTool): Promise<void> => {
    if (!enabled || !preview || !await prepare()) return
    if (tool === 'terminals') setTerminalCommand(value => ({ owner, value: -(Math.abs(value.owner === owner ? value.value : 0) + 1) }))
    else preview.openWorkspace(tool, undefined, ({ files: '工程文件', git: 'Git', browser: '浏览器', computer: '电脑', history: '会话历史', organizer: '整理', forge: '仓库面板', trash: '回收站', office: '文档' })[tool])
  }
  useEffect(() => {
    if (enabled && preview?.activeTab?.kind === 'workspace' && !active) void prepare()
  }, [enabled, preview?.activeTab?.id, owner, active])
  useEffect(() => {
    const open = (event: Event): void => {
      const detail = (event as CustomEvent<{ threadId: string; tool: WorkspaceEntryTool }>).detail
      if (detail?.threadId === props.threadId) void openTool(detail.tool)
    }
    window.addEventListener('coaxi:workspace-tool', open)
    return () => window.removeEventListener('coaxi:workspace-tool', open)
  })
  useShortcutActions({ toggle_terminal: () => { void openTool('terminals') }, toggle_aux_panel: () => { if (visible) collapse(); else if (preview?.tabs.length) void prepare().then(ok=>{if(ok)preview.showPane()}); else void openTool('files') } },
    { priority: 25, active: () => enabled && !document.querySelector('.launcher-web-page, [role="dialog"][aria-modal="true"], [role="menu"][data-state="open"]') })
  const control = <div className="engineering-controls"><button ref={trigger} type="button" className="file-preview-toggle engineering-unified-toggle" disabled={busy === owner}
    title={t(visible ? '收起工作区' : '展开工作区')} aria-label={t(visible ? '收起工作区' : '展开工作区')} aria-expanded={visible} aria-controls="file-preview-pane"
    onClick={() => {if(visible)collapse();else if(preview?.tabs.length)void prepare().then(ok=>{if(ok)preview.showPane()});else void openTool('files')}}>
    {busy === owner ? <LoaderCircle size={16} /> : <PanelToggleIcon side="right" visible={visible} />}
  </button></div>
  return <EngineeringDockContext.Provider value={{ opened, collapse }}>
    <div className="engineering-layout">
      <div className="engineering-conversation">{children}</div>
      <div className="engineering-terminal-host" ref={setTerminalHost} />
      {headerEntry && host && createPortal(control, host)}
      {issue?.owner === owner && <div className="engineering-entry-error" role="alert">{issue.message}<button type="button" onClick={() => { setIssue(null); void prepare() }}>{t('重试')}</button></div>}
      {active && workbenchHost && createPortal(<WorkspaceWorkbench {...props} projectPath={active.root} initiallyOpen visible={visible}
        terminalHost={terminalHost} terminalRequest={terminalCommand.owner === owner ? terminalCommand.value : 0} onClosed={() => collapse(true)} />, workbenchHost)}
    </div>
  </EngineeringDockContext.Provider>
}
