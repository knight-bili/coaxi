import { uiAttribute } from './interface-language'
import { memo, createContext, useCallback, useContext, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore, type CSSProperties, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import type { FilePreviewApi, ResolvedTheme } from '@contracts'
import { desktopFilePreviewApi } from './desktop-file-preview-api'
import { FilePreviewLayoutProvider, useOptionalFilePreviewLayout } from './FilePreviewLayout'
import { FileFindProvider } from './FilePreviewFind'
import { FilePreviewResources, windowFilePreviewResources } from './file-preview-resources'
import { FilePreviewPaneContent } from './FilePreviewPane'
import type { FilePreviewContextValue, FilePreviewSession } from './file-preview-controller'
export * from './file-preview-controller'

const FilePreviewApiContext = createContext<FilePreviewApi | null>(null)
const FilePreviewContext = createContext<FilePreviewContextValue | null>(null)
const PreviewHostContext = createContext<((element: HTMLDivElement | null) => void) | null>(null)
const ExecutionPreviewHostContext = createContext<ReadonlyMap<string, HTMLDivElement>>(new Map())

function sessionValue(session: FilePreviewSession, resolvedTheme: ResolvedTheme): FilePreviewContextValue {
  const state = session.getSnapshot()
  return { ...state, ...session.actions, resolvedTheme, activeTab: state.tabs.find(tab => tab.id === state.activeTabId) ?? null }
}

const RetainedPane = memo(FilePreviewPaneContent)
function RetainedPreview({ session, resolvedTheme, current, bounds, resizing, missionActivity, registerExecutionHost, domScope, isFocusedSurface }: {
  session: FilePreviewSession; resolvedTheme: ResolvedTheme; current: boolean; bounds: CSSProperties | null; resizing: boolean; missionActivity?: ReactNode
  registerExecutionHost(threadId: string, element: HTMLDivElement | null): void
  domScope?: string; isFocusedSurface: boolean
}): React.JSX.Element {
  const state = session.getSnapshot()
  const value = useMemo(() => ({ ...sessionValue(session, resolvedTheme), isCurrentThread: current, domScope, isFocusedSurface }), [session, state, resolvedTheme, current, domScope, isFocusedSurface])
  const visible = current && value.paneVisible && !!bounds
  const executionHostRef = useCallback(
    (element: HTMLDivElement | null) => registerExecutionHost(session.threadId, element),
    [registerExecutionHost, session.threadId]
  )
  return <FilePreviewContext.Provider value={value}>
    <div className={`file-preview-retained-host${resizing ? ' is-resizing' : ''}`} hidden={!visible} inert={!visible}
      style={bounds ?? undefined} data-preview-camp={session.threadId}>
      <RetainedPane
        visible={visible}
        missionActivity={missionActivity}
        executionHostRef={executionHostRef}
      />
    </div>
  </FilePreviewContext.Provider>
}

function PreviewDeck({ resources, anchor, resolvedTheme, missionActivity, registerExecutionHost, contained, host, threadId, domScope, isFocusedSurface }: {
  resources: FilePreviewResources; anchor: HTMLDivElement | null; resolvedTheme: ResolvedTheme; missionActivity?: ReactNode
  registerExecutionHost(threadId: string, element: HTMLDivElement | null): void
  contained?: boolean; host: HTMLElement; threadId: string | null
  domScope?: string; isFocusedSurface: boolean
}): React.JSX.Element {
  const layout = useOptionalFilePreviewLayout()
  const [bounds, setBounds] = useState<CSSProperties | null>(null)
  useLayoutEffect(() => {
    if (!anchor) return
    const measure = (): void => {
      const bounds = anchor.getBoundingClientRect()
      if (!bounds.width || !bounds.height || !anchor.clientWidth) return
      const hostBounds = host.getBoundingClientRect()
      const scale = contained ? hostBounds.width / host.clientWidth : bounds.width / anchor.clientWidth
      if (!Number.isFinite(scale) || scale <= 0) return
      const rect = { left: (bounds.left - (contained ? hostBounds.left : 0)) / scale,
        top: (bounds.top - (contained ? hostBounds.top : 0)) / scale, width: bounds.width / scale, height: bounds.height / scale }
      setBounds(previous => previous?.left === rect.left && previous.top === rect.top
        && previous.width === rect.width && previous.height === rect.height ? previous
        : { left: rect.left, top: rect.top, width: rect.width, height: rect.height })
    }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(anchor)
    if (contained) observer.observe(host)
    window.addEventListener('resize', measure)
    window.addEventListener('scroll', measure, true)
    return () => { observer.disconnect(); window.removeEventListener('resize', measure); window.removeEventListener('scroll', measure, true) }
  }, [anchor, layout?.width, layout?.availableWidth, layout?.compact, contained, host])
  // The split context updates before a ResizeObserver delivery on a window resize.
  const width = layout?.workspace ? (layout.compact ? layout.availableWidth : Math.round(layout.width)) : undefined
  const positioned = bounds && {
    ...bounds, ...(contained ? { position: 'absolute' as const } : {}), width: width ?? bounds.width,
    left: typeof bounds.left === 'number' && typeof bounds.width === 'number' && width !== undefined
      ? bounds.left + bounds.width - width : bounds.left
  }
  return <>{[...resources.sessions.values()].filter(session => !contained || session.threadId === threadId).map(session => <RetainedPreview key={session.id} session={session}
    resolvedTheme={resolvedTheme} current={session.threadId === threadId} bounds={positioned} resizing={!!layout?.resizing}
    missionActivity={session.threadId === threadId ? missionActivity : null}
    registerExecutionHost={registerExecutionHost} domScope={domScope} isFocusedSurface={isFocusedSurface && session.threadId === threadId} />)}</>

}

export function FilePreviewProvider({ threadId, resolvedTheme, api: providedApi, children, missionActivity, contained = false }: {
  threadId: string | null; resolvedTheme: ResolvedTheme; api?: FilePreviewApi; children: ReactNode; missionActivity?: ReactNode; contained?: boolean
}): React.JSX.Element {
  const api = providedApi ?? (typeof window === 'undefined' || window.rovai ? desktopFilePreviewApi : null)
  if (!api) throw new Error(uiAttribute('共享文件页面缺少显式资源适配。'))
  const resources = useMemo(() => windowFilePreviewResources(api), [api])
  const owner = useRef({}).current
  const surfaceId = useId(), domScope = contained ? `canvas-${surfaceId}` : undefined
  const isFocusedSurface = resources.isFocused(owner)
  useSyncExternalStore(resources.subscribe, resources.getSnapshot, resources.getSnapshot)
  const [anchor, setAnchor] = useState<HTMLDivElement | null>(null)
  const [deckHost, setDeckHost] = useState<HTMLElement>(document.body)
  const [executionHosts, setExecutionHosts] = useState<ReadonlyMap<string, HTMLDivElement>>(new Map())
  const registerAnchor = useCallback((element: HTMLDivElement | null): void => {
    setAnchor(element)
    if (element) setDeckHost(element.closest<HTMLElement>(contained ? '.file-preview-contained' : '.app-shell') ?? document.body)
  }, [contained])
  const registerExecutionHost = useCallback((targetThreadId: string, element: HTMLDivElement | null): void => {
    setExecutionHosts((current) => {
      if (current.get(targetThreadId) === element || (!element && !current.has(targetThreadId))) return current
      const next = new Map(current)
      if (element) next.set(targetThreadId, element)
      else next.delete(targetThreadId)
      return next
    })
  }, [])
  const session = resources.session(threadId ?? '')
  const state = session.getSnapshot()
  const value = useMemo(() => ({ ...sessionValue(session, resolvedTheme), domScope, isFocusedSurface }), [session, state, resolvedTheme, domScope, isFocusedSurface])
  useLayoutEffect(() => { resources.activate(threadId, owner); return () => resources.deactivate(owner) }, [resources, threadId, owner])
  useEffect(() => {
    const release = resources.retainOwner()
    const dispose = (): void => resources.dispose()
    window.addEventListener('beforeunload', dispose)
    return () => { window.removeEventListener('beforeunload', dispose); release() }
  }, [resources])
  const deck = createPortal(<PreviewDeck resources={resources} anchor={anchor} resolvedTheme={resolvedTheme} missionActivity={missionActivity}
    registerExecutionHost={registerExecutionHost} contained={contained} host={deckHost} threadId={threadId} domScope={domScope} isFocusedSurface={isFocusedSurface} />, deckHost)
  return <FilePreviewApiContext.Provider value={api}><FilePreviewContext.Provider value={value}>
    <FilePreviewLayoutProvider threadId={threadId} visible={!!threadId && value.paneVisible} activityMode={value.activeTab?.kind === 'mission_activity'}>
      <FileFindProvider activeTabId={value.activeTabId} visible={!!threadId && value.paneVisible && isFocusedSurface}>
        <ExecutionPreviewHostContext.Provider value={executionHosts}>
          <PreviewHostContext.Provider value={registerAnchor}>{contained ? <div className="file-preview-contained"
            onPointerDownCapture={() => resources.focus(threadId, owner)} onFocusCapture={() => resources.focus(threadId, owner)}>{children}{deck}</div> : <>{children}{deck}</>}</PreviewHostContext.Provider>
        </ExecutionPreviewHostContext.Provider>
      </FileFindProvider>
    </FilePreviewLayoutProvider>
  </FilePreviewContext.Provider></FilePreviewApiContext.Provider>
}

export function useFilePreview(): FilePreviewContextValue {
  const value = useContext(FilePreviewContext)
  if (!value) throw new Error('FilePreviewProvider is unavailable')
  return value
}
export function useOptionalFilePreview(): FilePreviewContextValue | null { return useContext(FilePreviewContext) }
export function useFilePreviewApi(): FilePreviewApi {
  const api = useContext(FilePreviewApiContext)
  if (!api) throw new Error('FilePreviewApi is unavailable')
  return api
}
export function usePreviewHost(): (element: HTMLDivElement | null) => void {
  const value = useContext(PreviewHostContext)
  if (!value) throw new Error('FilePreviewProvider is unavailable')
  return value
}
export function useExecutionPreviewHost(threadId: string): HTMLDivElement | null {
  return useContext(ExecutionPreviewHostContext).get(threadId) ?? null
}
