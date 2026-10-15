import { useCallback, useEffect, useRef, useState } from 'react'
import type { CanvasBoard, StoredCommandResult, NavigationPreferencesSnapshot, NavigationProjectLayoutMutation } from '@contracts'
import { CanvasSurface, type CanvasLeaveGuard, type CanvasSurfaceProps } from './CanvasSurface'
import { CanvasBoardDialog } from './CanvasBoardDialog'
import { CanvasBoardsController, initialCanvasBoardsState, type CanvasBoardIntent } from './canvas-boards-controller'
import { browserCanvasStorage, canvasViewScope, loadCanvasBoardSelection, saveCanvasBoardSelection, type CanvasViewScope } from './canvas-view-state'
import { useThreadClient } from './camp-client'
import { useCanvasReferences } from './useCanvasReferences'
import { UiText, uiAttribute } from './interface-language'
import { readErrorMessage } from './error-message'
import { ProjectGroupsDialog } from './ProjectGroupsDialog'

export type CanvasWorkspaceProps = Omit<CanvasSurfaceProps, 'boardId' | 'scope' | 'catalog' | 'onRefreshReferences' | 'groups'> & {
  navigationPreferences?: NavigationPreferencesSnapshot | null
  onProjectLayout?(revision: number, mutation: NavigationProjectLayoutMutation): Promise<void>
  onRefreshProjectGroups?(): Promise<void>
}
const NO_BOARDS: CanvasBoard[] = []

function useCanvasBoards() {
  const client = useThreadClient(), controller = useRef<CanvasBoardsController | null>(null)
  const [owned, setOwned] = useState(() => ({ client, state: initialCanvasBoardsState() }))
  useEffect(() => {
    const current = new CanvasBoardsController(client)
    controller.current = current
    const unsubscribe = current.subscribe(() => setOwned({ client, state: current.getSnapshot() }))
    setOwned({ client, state: current.getSnapshot() }); current.start()
    const foreground = () => current.setVisible(document.visibilityState === 'visible')
    const poll = setInterval(() => { if (document.visibilityState === 'visible') void current.refresh().catch(() => undefined) }, 20_000)
    window.addEventListener('focus', foreground); document.addEventListener('visibilitychange', foreground)
    return () => {
      unsubscribe(); current.dispose(); if (controller.current === current) controller.current = null
      clearInterval(poll); window.removeEventListener('focus', foreground); document.removeEventListener('visibilitychange', foreground)
    }
  }, [client])
  return { controller, state: owned.client === client ? owned.state : initialCanvasBoardsState() }
}

export function CanvasWorkspace(props: CanvasWorkspaceProps): React.JSX.Element {
  const client = useThreadClient(), owner = useRef({ client, generation: 0 })
  if (owner.current.client !== client) owner.current = { client, generation: owner.current.generation + 1 }
  const [identity, setIdentity] = useState<{ client: typeof client; scope: CanvasViewScope | null; issue: string | null } | null>(null)
  const [identityAttempt, setIdentityAttempt] = useState(0)
  useEffect(() => {
    let disposed = false
    void client.request<{ dataDir: string }>('app.info').then(info => {
      const scope = canvasViewScope(client.presentationScope, info.dataDir)
      if (!disposed) setIdentity({ client, scope, issue: scope ? null : uiAttribute('画布输入的所属信息不完整，请重新读取画板。') })
    }, error => { if (!disposed) setIdentity({ client, scope: null, issue: readErrorMessage(error) }) })
    return () => { disposed = true }
  }, [client, identityAttempt])
  if (identity?.client !== client) return <section className="canvas-workspace"><p role="status"><UiText zh="正在读取画板…" /></p></section>
  return <CanvasWorkspaceContent key={owner.current.generation} {...props} scope={identity.scope} identityIssue={identity.issue}
    onRetryIdentity={() => { setIdentity(null); setIdentityAttempt(current => current + 1) }} />
}

function CanvasWorkspaceContent({ scope, identityIssue, onRetryIdentity, onLeaveGuardChange, navigationPreferences, onProjectLayout, onRefreshProjectGroups, ...props }: CanvasWorkspaceProps & {
  scope: CanvasViewScope | null; identityIssue: string | null; onRetryIdentity(): void
}): React.JSX.Element {
  const { controller, state } = useCanvasBoards(), [storage] = useState(browserCanvasStorage)
  const catalog = useCanvasReferences()
  const [selectedId, setSelectedId] = useState(() => loadCanvasBoardSelection(storage, scope))
  const [dialog, setDialog] = useState<{ board: CanvasBoard | null; remove: boolean } | null>(null)
  const [groupManager, setGroupManager] = useState(false)
  const [issue, setIssue] = useState<string | null>(null), panel = useRef(false), graphGuard = useRef<CanvasLeaveGuard | null>(null)
  const conversationRef = useRef(props.conversation); conversationRef.current = props.conversation
  const boards = state.snapshot?.boards ?? NO_BOARDS
  const selected = boards.find(board => board.id === selectedId) ?? null
  const registerGraphGuard = useCallback((guard: CanvasLeaveGuard | null) => { graphGuard.current = guard }, [])
  const isBlocked = useCallback(() => panel.current || Boolean(conversationRef.current?.opening || controller.current?.getSnapshot().pending || graphGuard.current?.isBlocked()), [controller])
  const canLeave = useCallback(async () => {
    if (isBlocked()) { setIssue(uiAttribute('请先保存或取消编辑，并核对未完成的画布操作。')); return false }
    try { return await conversationRef.current?.close() ?? true }
    catch (error) { setIssue(readErrorMessage(error)); return false }
  }, [isBlocked])
  useEffect(() => {
    const guard = { canLeave, isBlocked }
    onLeaveGuardChange?.(guard)
    const beforeUnload = (event: BeforeUnloadEvent) => { if (guard.isBlocked()) { event.preventDefault(); event.returnValue = '' } }
    window.addEventListener('beforeunload', beforeUnload)
    return () => { onLeaveGuardChange?.(null); window.removeEventListener('beforeunload', beforeUnload) }
  }, [canLeave, isBlocked, onLeaveGuardChange])
  useEffect(() => {
    if (state.loading || !state.snapshot || selected || isBlocked() || props.conversation?.target?.boardId === selectedId) return
    let cancelled = false
    void canLeave().then(allowed => {
      if (allowed && !cancelled) setSelectedId(current => current === selectedId ? boards[0]?.id ?? null : current)
    })
    return () => { cancelled = true }
  }, [state.loading, state.snapshot, selected, selectedId, boards, isBlocked, canLeave, props.conversation?.target])
  useEffect(() => { if (selected) saveCanvasBoardSelection(storage, scope, selected.id) }, [selected, storage, scope])
  const openDialog = async (board: CanvasBoard | null, remove = false) => {
    if (!await canLeave()) return
    setIssue(null); panel.current = true; setDialog({ board: board ? structuredClone(board) : null, remove })
  }
  const closeDialog = () => { if (!controller.current?.getSnapshot().pending) { panel.current = false; setDialog(null) } }
  const acceptReceipt = (result: StoredCommandResult) => {
    if (result.status === 'rejected') {
      const message = result.code === 'canvas.board.conflict' ? uiAttribute('画板已有新修改，此次操作未保存。请重新核对。') : uiAttribute('画板操作未完成，请核对输入后重试。')
      setIssue(message); throw new Error(message)
    }
    const board = result.payload.board as Partial<CanvasBoard> | undefined
    if (result.commandType === 'canvas.boards.create' && typeof board?.id === 'string') setSelectedId(board.id)
    if (result.commandType === 'canvas.boards.delete' && result.payload.boardId === selectedId) setSelectedId(null)
    panel.current = false; setDialog(null); setIssue(null)
  }
  const save = async (intent: CanvasBoardIntent) => {
    if (!controller.current) throw new Error(uiAttribute('画板页面尚未准备好。'))
    try { acceptReceipt(await controller.current.apply(intent)) }
    catch (error) { setIssue(readErrorMessage(error)); throw error }
  }
  const recover = async (retry: boolean) => {
    try {
      const current = controller.current
      if (!current) return
      const receipt = await (retry ? current.retry() : current.reconcile())
      if (receipt) acceptReceipt(receipt)
    } catch (error) { setIssue(readErrorMessage(error)) }
  }
  const recovery = state.pending && <div className="canvas-notice" role="status"><UiText zh={state.pending.state === 'unknown' ? '原画板操作结果待核对。' : '正在保存画板…'} />
    {state.pending.state === 'unknown' && <><button type="button" onClick={() => { void recover(false) }}><UiText zh="核对结果" /></button>
      <button type="button" onClick={() => { void recover(true) }}><UiText zh="重试原操作" /></button></>}
  </div>
  const blocked = state.loading || Boolean(state.readError || state.pending || dialog || groupManager)
  const groups = navigationPreferences && !navigationPreferences.readError ? navigationPreferences.projectGroups.map(group => ({ id: group.id, label: group.name, color: group.color,
    projectPaths: group.projectPaths.filter(path => navigationPreferences.registeredProjects.includes(path) && !navigationPreferences.removedProjects.some(item => item.targetKey === `directory:${path}`)) })) : []
  return <section className="canvas-workspace" aria-label={uiAttribute('画板工作区')}>
    <header className="canvas-boards-toolbar">
      <label><span className="sr-only"><UiText zh="当前画板" /></span><select value={selectedId ?? ''} disabled={blocked || !boards.length}
        onChange={event => { const id = event.target.value; void canLeave().then(allowed => { if (allowed) { setSelectedId(id); setIssue(null) } }) }}>
        {!selected && <option value={selectedId ?? ''}>{uiAttribute('请选择画板')}</option>}
        {boards.map(board => <option key={board.id} value={board.id}>{board.name || uiAttribute('未命名画板')}</option>)}
      </select></label>
      <button type="button" disabled={blocked} onClick={() => { void openDialog(null) }}><UiText zh="创建画板" /></button>
      <button type="button" disabled={blocked || !selected} onClick={() => { void openDialog(selected) }}><UiText zh="编辑画板" /></button>
      <button type="button" disabled={blocked || !selected} onClick={() => { void openDialog(selected, true) }}><UiText zh="删除画板" /></button>
      {navigationPreferences && onProjectLayout && onRefreshProjectGroups && <button type="button" disabled={blocked} onClick={() => { void canLeave().then(allowed => { if (allowed) { panel.current = true; setGroupManager(true) } }) }}><UiText zh="管理项目分组" /></button>}
    </header>
    {identityIssue && <div className="canvas-notice" role="alert"><UiText zh="服务信息未核对，暂时不能展开对话输入。" /> {identityIssue}
      <button type="button" disabled={blocked} onClick={() => { void canLeave().then(allowed => { if (allowed) onRetryIdentity() }) }}><UiText zh="重新核对服务" /></button></div>}
    {(issue || Boolean(state.readError)) && <div className="canvas-notice" role="alert">{issue || readErrorMessage(state.readError)}
      <button type="button" onClick={() => { void controller.current?.refresh().catch(() => undefined) }}><UiText zh="重新读取" /></button></div>}
    {!dialog && recovery}
    {!catalog.complete && <div className="canvas-notice" role={catalog.error ? 'alert' : 'status'}><UiText zh={catalog.error ? '对话清单读取未完成，已保留画布。' : '正在核对完整对话清单…'} />
      {Boolean(catalog.error) && <button type="button" onClick={() => { void catalog.refresh().catch(() => undefined) }}><UiText zh="重新载入对话" /></button>}
    </div>}
    {!state.snapshot && state.loading && <p role="status"><UiText zh="正在读取画板…" /></p>}
    {state.snapshot && !boards.length && !state.loading && <div className="canvas-workspace-empty"><h2><UiText zh="还没有画板" /></h2><p><UiText zh="创建一张画板，整理工程、队员、对话和便签。" /></p></div>}
    {selectedId && <CanvasSurface {...props} boardId={selectedId} scope={scope} catalog={catalog} groups={groups}
      onRefreshReferences={() => { void catalog.refresh().catch(() => undefined) }} onLeaveGuardChange={registerGraphGuard} />}
    {dialog && <CanvasBoardDialog board={dialog.board} remove={dialog.remove}
      latest={dialog.board ? boards.find(board => board.id === dialog.board!.id) ?? null : null}
      blocked={state.loading || Boolean(state.readError)} pending={Boolean(state.pending)} recovery={recovery}
      commandError={issue}
      onClose={closeDialog} onSave={save} />}
    {groupManager && navigationPreferences && onProjectLayout && onRefreshProjectGroups && <ProjectGroupsDialog snapshot={navigationPreferences}
      onSave={onProjectLayout} onRefresh={onRefreshProjectGroups} onClose={() => { panel.current = false; setGroupManager(false) }} />}
  </section>
}
