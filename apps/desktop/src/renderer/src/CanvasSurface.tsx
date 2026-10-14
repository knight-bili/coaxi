import { createContext, useContext, useEffect, useMemo, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Background, BackgroundVariant, Controls, MiniMap, NodeResizer, ReactFlow, ReactFlowProvider,
  SelectionMode, ViewportPortal, applyNodeChanges, useReactFlow, type Node, type NodeProps } from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import type { AgentProfile, CanvasConversationRef, CanvasNode, CanvasNodeMutation, ResolvedTheme, StoredCommandResult } from '@contracts'
import { useCanvasGraph } from './useCanvasGraph'
import { initialCanvasReferenceState, type CanvasReferenceState } from './canvas-reference-controller'
import { canvasReferenceOptions, canvasSavedReference, resolveCanvasRegion, type CanvasFolderResolution } from './canvas-reference-model'
import { browserCanvasStorage, loadCanvasViewState, saveCanvasViewState, CANVAS_MIN_ZOOM, CANVAS_MAX_ZOOM, type CanvasViewScope } from './canvas-view-state'
import { basePathName, BOARD_DOT_GAP, CARD_WIDTH, CARD_HEIGHT, REGION_COLLAPSED_HEIGHT,
  REGION_FOOTER_HEIGHT, computeAlignment, computeDropHint, effectiveColumns, isRegionKind, layoutRegionGrid, noteHoldsProse,
  packLayout, regionHeightForRows, regionWidthForColumns, visibleMemberCap, columnsForRegionWidth, rowsForRegionHeight,
  FILE_CARD_MIN_WIDTH, FILE_CARD_MIN_HEIGHT, TERMINAL_CARD_MIN_WIDTH, TERMINAL_CARD_MIN_HEIGHT, type AlignmentGuide } from './canvas-layout'
import { CanvasNodeEditor, canvasKindNames, canvasColorName, type CanvasEditTarget, type CanvasFolderGroupOption, type CanvasReferenceOption } from './CanvasNodeEditor'
import { AppDialogBody, AppDialogContent, AppDialogFooter, AppDialogHeader } from './AppDialog'
import { useThreadClient } from './camp-client'
import { SafeMarkdown } from './SafeMarkdown'
import { UiText, uiAttribute, useInterfaceLanguage } from './interface-language'
import { readErrorMessage } from './error-message'
import { useCanvasFolders } from './useCanvasFolders'
import './canvas.css'

export interface CanvasLeaveGuard { canLeave(): Promise<boolean>; isBlocked(): boolean }
export interface CanvasSurfaceProps {
  boardId: string
  scope: CanvasViewScope | null
  agents: AgentProfile[]
  groups?: CanvasFolderGroupOption[]
  references?: CanvasReferenceOption[]
  catalog?: CanvasReferenceState
  onRefreshReferences?(): void
  folderResolutions?: ReadonlyMap<string, CanvasFolderResolution>
  resolvedMembers?: ReadonlyMap<string, { references: CanvasReferenceOption[]; complete: boolean; issue?: string }>
  theme: ResolvedTheme
  onOpenReference(reference: CanvasConversationRef): void
  onOpenMember?(agentId: string): void
  onOpenProject?(path: string): void
  onOpenFile?(path: string): void
  onLeaveGuardChange?(guard: CanvasLeaveGuard | null): void
}
type FlowNode = Node<{ row: CanvasNode }, 'canvas-card'>
const EMPTY_ROWS: CanvasNode[] = [], EMPTY_GROUPS: CanvasFolderGroupOption[] = []
const EMPTY_CATALOG = initialCanvasReferenceState()
type Presentation = CanvasSurfaceProps & {
  blocked: boolean
  expandedRegions: ReadonlySet<string>
  title(reference: CanvasConversationRef): string
  available(reference: CanvasConversationRef): boolean | undefined
  collapse(row: CanvasNode): void
  expand(row: CanvasNode): void
  detach(row: CanvasNode, reference: CanvasConversationRef): void
  edit(row: CanvasNode): void
  beginGesture(): number | null
  resize(row: CanvasNode, geometry: { x: number; y: number; width: number; height: number }, revision: number | null): void
}
const PresentationContext = createContext<Presentation | null>(null)

function CanvasCard({ data, selected }: NodeProps<FlowNode>): React.JSX.Element {
  const actions = useContext(PresentationContext)!
  const resizeRevision = useRef<number | null>(null)
  const row = data.row, d = row.data, region = isRegionKind(d.kind)
  const member = actions.agents.find(item => item.agentId === d.agentId)
  const title = d.title || (d.conversation ? actions.title(d.conversation) : member?.displayName)
    || (d.path || d.folderPath ? basePathName(d.path || d.folderPath!) : uiAttribute(canvasKindNames[d.kind]))
  const columns = effectiveColumns(d, d.width), cap = visibleMemberCap(d, columns)
  const resolution = actions.resolvedMembers?.get(row.id)
  const references = resolution?.references.map(item => item.reference) ?? (d.kind === 'custom' ? d.conversations : [])
  const visible = actions.expandedRegions.has(row.id) ? references : references.slice(0, cap)
  return <article className={`canvas-card canvas-card-${d.kind}${d.collapsed ? ' is-collapsed' : ''}`} data-color={d.color ?? undefined}>
    <NodeResizer isVisible={selected && !actions.blocked && !d.collapsed && d.kind !== 'conversation'}
      minWidth={d.kind === 'file' ? FILE_CARD_MIN_WIDTH : d.kind === 'terminal' ? TERMINAL_CARD_MIN_WIDTH : 48}
      minHeight={d.kind === 'file' ? FILE_CARD_MIN_HEIGHT : d.kind === 'terminal' ? TERMINAL_CARD_MIN_HEIGHT : 48} maxWidth={20000} maxHeight={20000}
      onResizeStart={() => { resizeRevision.current = actions.beginGesture() }}
      onResizeEnd={(_event, geometry) => { actions.resize(row, geometry, resizeRevision.current); resizeRevision.current = null }} />
    <header className="canvas-card-handle">
      {d.color && <span className="canvas-color-dot" role="img" aria-label={uiAttribute(canvasColorName(d.color))} />}
      <strong title={title}>{title}</strong>
      <button className="nodrag" type="button" disabled={actions.blocked} onClick={() => actions.edit(row)} aria-label={uiAttribute('编辑节点')}><UiText zh="编辑" /></button>
      {region && <button className="nodrag" type="button" disabled={actions.blocked} onClick={() => actions.collapse(row)}
        aria-label={uiAttribute(d.collapsed ? '展开区域' : '折叠区域')}>{d.collapsed ? '+' : '−'}</button>}
    </header>
    {!d.collapsed && <div className="canvas-card-body nodrag nowheel">
      {d.kind === 'note' && <SafeMarkdown theme={actions.theme} mode="document">{d.content || ''}</SafeMarkdown>}
      {d.kind === 'conversation' && d.conversation && <button type="button" disabled={actions.available(d.conversation) === false} onClick={() => actions.onOpenReference(d.conversation!)}><UiText zh="打开对话" /></button>}
      {d.conversation && actions.available(d.conversation) === false && <p className="canvas-empty-copy"><UiText zh="对话已结束或不可用" /></p>}
      {d.kind === 'folder' && <><code>{d.folderPath}</code>{actions.onOpenProject && <button type="button" onClick={() => actions.onOpenProject?.(d.folderPath!)}><UiText zh="进入工程" /></button>}</>}
      {d.kind === 'agent' && <>{member && !member.removedAt && actions.onOpenMember && <button type="button" onClick={() => actions.onOpenMember?.(member.agentId)}><UiText zh="打开队员" /></button>}
        {d.agentId && (!member || member.removedAt) && <p className="canvas-empty-copy"><UiText zh="队员已移除或不可用" /></p>}</>}
      {region && <>
        <div className="canvas-member-grid" style={{ gridTemplateColumns: `repeat(${columns}, ${CARD_WIDTH}px)` }}>
          {visible.map(reference => <div key={JSON.stringify(reference)} className="canvas-member-card">
            <button type="button" disabled={actions.available(reference) === false} onClick={() => actions.onOpenReference(reference)}>{resolution?.references.find(item => item.reference.threadId === reference.threadId && item.reference.conversationId === reference.conversationId)?.label ?? actions.title(reference)}</button>
            <button type="button" disabled={actions.blocked} onClick={() => actions.detach(row, reference)}><UiText zh={d.kind === 'custom' ? '移出分组' : '固定到画布'} /></button>
          </div>)}
        </div>
        {references.length > cap && <button type="button" onClick={() => actions.expand(row)}>
          {actions.expandedRegions.has(row.id) ? uiAttribute('收起成员') : uiAttribute('显示全部 {0} 个成员', references.length)}
        </button>}
        {d.kind !== 'custom' && !resolution && <p className="canvas-empty-copy"><UiText zh="关联对话尚未载入" /></p>}
        {resolution && !resolution.complete && <p className="canvas-empty-copy">{resolution.issue || uiAttribute('部分关联对话尚未载入')}</p>}
      </>}
      {(d.kind === 'file' || d.kind === 'terminal') && <code>{d.path}</code>}
      {d.kind === 'file' && actions.onOpenFile && <button type="button" onClick={() => actions.onOpenFile?.(d.path!)}><UiText zh="打开文件" /></button>}
      {d.kind === 'terminal' && <p className="canvas-empty-copy"><UiText zh="终端未连接" /></p>}
    </div>}
  </article>
}
const nodeTypes = { 'canvas-card': CanvasCard }
export function CanvasSurface(props: CanvasSurfaceProps): React.JSX.Element {
  const client = useThreadClient(), owner = useRef({ client, generation: 0 })
  if (owner.current.client !== client) owner.current = { client, generation: owner.current.generation + 1 }
  // A changed owner/board mounts fresh view state; an effect must never save the old viewport under a new key.
  const key = JSON.stringify([owner.current.generation, props.scope?.host, props.scope?.core, props.boardId])
  return <ReactFlowProvider key={key}><CanvasSurfaceContent {...props} /></ReactFlowProvider>
}

function CanvasSurfaceContent(props: CanvasSurfaceProps): React.JSX.Element {
  const graph = useCanvasGraph(props.boardId), flow = useReactFlow<FlowNode>()
  const language = useInterfaceLanguage(), catalog = props.catalog ?? EMPTY_CATALOG
  const [storage] = useState(browserCanvasStorage)
  const [view, setView] = useState(() => loadCanvasViewState(storage, props.scope, props.boardId))
  const [nodes, setNodes] = useState<FlowNode[]>([]), [selected, setSelected] = useState<string[]>([])
  const [issue, setIssue] = useState<string | null>(null)
  const surface = useRef<HTMLElement | null>(null)
  const [editor, setEditor] = useState<{ target: CanvasEditTarget; revision: number } | null>(null)
  const [deletion, setDeletion] = useState<{ ids: string[]; revision: number; hasProse: boolean } | null>(null)
  const [guides, setGuides] = useState<AlignmentGuide[]>([])
  const panel = useRef(false), panelCommand = useRef(false)
  const dragRevision = useRef<number | null>(null), dragging = useRef(false), currentDerived = useRef<FlowNode[]>([])
  const latestGraph = useRef(graph)
  latestGraph.current = graph
  const graphBlocked = graph.loading || Boolean(graph.readError || graph.pending) || !graph.snapshot?.board
  const blocked = graphBlocked || Boolean(editor || deletion)
  const rows = graph.snapshot?.nodes ?? EMPTY_ROWS
  const folderPaths = useMemo(() => rows.flatMap(row => row.data.kind === 'folder' && row.data.folderPath ? [row.data.folderPath]
    : row.data.kind === 'group' ? props.groups?.find(group => group.id === row.data.folderGroupId)?.projectPaths ?? [] : []), [rows, props.groups])
  const folders = useCanvasFolders(folderPaths)
  const folderResolutions = props.folderResolutions ?? folders.resolutions
  const referenceOptions = useMemo(() => props.references ?? canvasReferenceOptions(catalog.items), [props.references, catalog.items, language])
  const regions = useMemo(() => props.resolvedMembers ?? new Map(rows.filter(row => isRegionKind(row.data.kind)).map(row => [row.id,
    resolveCanvasRegion(row, catalog, props.groups ?? EMPTY_GROUPS, folderResolutions)])),
  [props.resolvedMembers, rows, catalog.items, catalog.complete, catalog.error, props.groups, folderResolutions, language])
  const expanded = useMemo(() => new Set(view.expandedRegions), [view.expandedRegions])
  const derived = useMemo<FlowNode[]>(() => rows.map(row => {
    const d = row.data, region = isRegionKind(d.kind)
    const width = region && d.gridColumns > 0 ? regionWidthForColumns(d.gridColumns) : d.kind === 'conversation' ? CARD_WIDTH : d.width
    let height = d.height
    if (region) {
      if (d.collapsed) height = REGION_COLLAPSED_HEIGHT
      else {
        const memberCount = regions.get(row.id)?.references.length ?? (d.kind === 'custom' ? d.conversations.length : 0)
        const columns = effectiveColumns(d, width), cap = visibleMemberCap(d, columns)
        const count = expanded.has(row.id) ? memberCount : Math.min(cap, memberCount)
        height = Math.max(d.gridRows > 0 ? regionHeightForRows(d.gridRows) : height,
          layoutRegionGrid(count, width, columns).contentHeight + (memberCount > cap ? REGION_FOOTER_HEIGHT : 0))
      }
    } else if (d.kind === 'conversation') height = CARD_HEIGHT
    return { id: row.id, type: 'canvas-card', data: { row }, position: { x: d.x, y: d.y },
      width, height, style: { width, height }, selected: selected.includes(row.id), dragHandle: '.canvas-card-handle',
      zIndex: region ? 0 : 1, connectable: false, deletable: false }
  }), [rows, selected, expanded, regions])
  currentDerived.current = derived
  useEffect(() => { if (!dragging.current) setNodes(derived) }, [derived])
  useEffect(() => { saveCanvasViewState(storage, props.scope, props.boardId, view) }, [storage, view, props.scope, props.boardId])
  useEffect(() => {
    const isBlocked = () => panel.current || dragging.current || Boolean(latestGraph.current.hasUnsettled())
    props.onLeaveGuardChange?.({ isBlocked, canLeave: async () => !isBlocked() })
    return () => props.onLeaveGuardChange?.(null)
  }, [props.onLeaveGuardChange])

  const settlePanel = (receipt: StoredCommandResult) => {
    if (receipt.status === 'rejected') {
      const message = receipt.code === 'canvas.nodes.conflict' ? uiAttribute('画布已有新修改，此次操作未保存。请核对最新布局后重试。') : uiAttribute('此次画布操作未完成，请核对输入后重试。')
      setIssue(message); panelCommand.current = false
      throw new Error(message)
    }
    if (panelCommand.current) { panelCommand.current = false; panel.current = false; setEditor(null); setDeletion(null) }
  }
  const submit = async (mutation: CanvasNodeMutation, revision?: number, fromPanel = false) => {
    setIssue(null)
    panelCommand.current = fromPanel
    try {
      const receipt = await graph.apply(mutation, revision)
      settlePanel(receipt)
    } catch (error) {
      if (!graph.hasUnsettled()) panelCommand.current = false
      setIssue(readErrorMessage(error)); throw error
    } finally {
      dragging.current = false; setGuides([]); setNodes(currentDerived.current)
    }
  }
  const mutate = (mutation: CanvasNodeMutation, revision?: number) => { void submit(mutation, revision).catch(() => undefined) }
  const closePanel = () => { if (!graph.hasUnsettled()) { panel.current = false; setEditor(null); setDeletion(null) } }
  const beginGesture = () => { dragging.current = true; return graph.snapshot?.board?.revision ?? null }
  const recover = async (retry: boolean) => {
    try { const receipt = await (retry ? graph.retry() : graph.reconcile()); if (receipt) settlePanel(receipt) }
    catch (error) { setIssue(readErrorMessage(error)) }
  }
  const title = (reference: CanvasConversationRef) => {
    return referenceOptions.find(item => item.reference.threadId === reference.threadId && item.reference.conversationId === reference.conversationId)?.label
      ?? canvasSavedReference(reference, catalog).label
  }
  const actions: Presentation = { ...props, resolvedMembers: regions, blocked, expandedRegions: expanded, title,
    available: reference => canvasSavedReference(reference, catalog).available,
    collapse: row => { mutate({ type: 'update', nodeId: row.id, patch: { collapsed: !row.data.collapsed } }) },
    expand: row => setView(current => ({ ...current, expandedRegions: expanded.has(row.id)
      ? current.expandedRegions.filter(id => id !== row.id) : [...current.expandedRegions, row.id] })),
    detach: (row, reference) => { mutate({ type: 'detach', regionId: row.id, conversation: reference,
      x: row.data.x + (flow.getNode(row.id)?.width ?? row.data.width) + 24, y: row.data.y }) },
    edit: row => { panel.current = true; setEditor({ target: { type: 'update', row }, revision: graph.snapshot!.board!.revision }) },
    beginGesture,
    resize: (row, geometry, revision) => {
      const region = isRegionKind(row.data.kind)
      const columns = Math.min(12, columnsForRegionWidth(geometry.width)), rows = Math.min(12, rowsForRegionHeight(geometry.height))
      mutate({ type: 'update', nodeId: row.id, patch: { ...geometry,
        ...(region ? { width: regionWidthForColumns(columns), height: regionHeightForRows(rows),
          ...(row.data.gridColumns ? { gridColumns: columns } : {}), ...(row.data.gridRows ? { gridRows: rows } : {}) } : {}) } }, revision ?? undefined)
    }
  }
  const deleteSelected = (ids = selected) => {
    panel.current = true
    setDeletion({ ids: [...ids], revision: graph.snapshot!.board!.revision,
      hasProse: rows.some(row => ids.includes(row.id) && noteHoldsProse(row)) })
  }
  const groupSelected = () => {
    const pins = rows.filter(row => selected.includes(row.id) && row.data.kind === 'conversation' && row.data.conversation)
    if (!pins.length) return
    const anchor = pins[0].data
    mutate({ type: 'group', targetNodeId: null, newRegion: { kind: 'custom', x: anchor.x - 12, y: anchor.y - 52,
      width: regionWidthForColumns(2), height: regionHeightForRows(1) },
    conversations: pins.map(row => row.data.conversation!), consumeNodeIds: pins.map(row => row.id) })
  }
  const error = issue || (graph.readError ? readErrorMessage(graph.readError) : null)
  const recovery = graph.pending && <div className="canvas-notice" role="status"><UiText zh={graph.pending.state === 'unknown' ? '原操作结果待核对，请先核对或重试原操作。' : '正在保存画布…'} />
    {graph.pending.state === 'unknown' && <><button type="button" onClick={() => { void recover(false) }}><UiText zh="核对结果" /></button>
      <button type="button" onClick={() => { void recover(true) }}><UiText zh="重试原操作" /></button></>}
  </div>
  const align = (moved: FlowNode, movedNodes: FlowNode[]) => {
    const moving = movedNodes.length ? movedNodes : [moved], ids = new Set(moving.map(item => item.id))
    const left = Math.min(...moving.map(item => item.position.x)), top = Math.min(...moving.map(item => item.position.y))
    const right = Math.max(...moving.map(item => item.position.x + (item.width ?? item.data.row.data.width)))
    const bottom = Math.max(...moving.map(item => item.position.y + (item.height ?? item.data.row.data.height)))
    const others = nodes.filter(item => !ids.has(item.id)).map(item => ({ ...item.position, width: item.width ?? 0, height: item.height ?? 0 }))
    return computeAlignment({ x: left, y: top, width: right - left, height: bottom - top }, others, 8 / flow.getZoom(), BOARD_DOT_GAP)
  }
  return <section ref={surface} className="canvas-surface" aria-label={uiAttribute('无限画布')} onKeyDownCapture={event => {
    const target = event.target
    if (!(target instanceof HTMLElement) || !target.matches('.react-flow__node, .react-flow__nodesselection-rect') || event.metaKey || event.ctrlKey || event.altKey) return
    const direction = ({ ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] } as Record<string, number[]>)[event.key]
    if (!direction && event.key !== 'Delete' && event.key !== 'Backspace') return
    // ReactFlow's keyboard movement is local only. Route it through the same Core command as pointer gestures.
    event.preventDefault(); event.stopPropagation()
    if (blocked || panel.current || dragging.current || latestGraph.current.hasUnsettled()) return
    const focused = target.dataset.id, ids = focused && !selected.includes(focused) ? [focused] : selected
    if (!ids.length) return
    if (focused && !selected.includes(focused)) setSelected([focused])
    if (!direction) { deleteSelected(ids); return }
    const distance = event.shiftKey ? 20 : 5
    const moves = rows.filter(row => ids.includes(row.id)).map(row => ({ nodeId: row.id, x: row.data.x + direction[0] * distance, y: row.data.y + direction[1] * distance }))
    if (moves.length) mutate({ type: 'move', moves }, graph.snapshot!.board!.revision)
  }}>
    <div className="canvas-toolbar">
      <button type="button" onClick={() => { props.onRefreshReferences?.(); void folders.refresh() }}><UiText zh="刷新关联" /></button>
      <button type="button" disabled={blocked} onClick={() => {
        const bounds = flow.getViewport(), element = surface.current?.getBoundingClientRect()
        const point = element ? flow.screenToFlowPosition({ x: element.left + element.width / 2, y: element.top + element.height / 2 }) : { x: -bounds.x / bounds.zoom, y: -bounds.y / bounds.zoom }
        panel.current = true; setEditor({ target: { type: 'create', position: point }, revision: graph.snapshot!.board!.revision })
      }}><UiText zh="添加节点" /></button>
      <button type="button" disabled={blocked || !rows.some(row => selected.includes(row.id) && row.data.kind === 'conversation')} onClick={groupSelected}><UiText zh="选中对话分组" /></button>
      <button type="button" disabled={blocked || !selected.length} onClick={() => deleteSelected()}><UiText zh="删除所选" /></button>
      <button type="button" disabled={blocked || !rows.length} onClick={() => {
        const sizes = new Map(nodes.map(node => [node.id, { width: node.width ?? node.data.row.data.width, height: node.height ?? node.data.row.data.height }]))
        const moves = packLayout(rows.map(row => ({ id: row.id, ...row.data })), sizes)
        if (moves.length) mutate({ type: 'move', moves })
      }}><UiText zh="自动排列" /></button>
      <button type="button" onClick={() => { void flow.fitView({ padding: 0.15 }) }}><UiText zh="查看全部" /></button>
      <button type="button" aria-pressed={view.minimap} onClick={() => setView(current => ({ ...current, minimap: !current.minimap }))}><UiText zh="小地图" /></button>
    </div>
    {error && <div className="canvas-notice" role="alert">{error}<button type="button" onClick={() => { void graph.refresh().catch(() => undefined) }}><UiText zh="重新读取" /></button></div>}
    {!editor && !deletion && recovery}
    {graph.loading && !graph.snapshot && <p className="canvas-notice" role="status"><UiText zh="正在读取画布…" /></p>}
    {!graph.loading && graph.snapshot?.board && !rows.length && <p className="canvas-empty-hint"><UiText zh="从添加节点开始，也可以把对话整理到同一片画布。" /></p>}
    {!graph.loading && graph.snapshot && !graph.snapshot.board && <p className="canvas-notice"><UiText zh="此画板已被删除，请选择其他画板。" /></p>}
    <PresentationContext.Provider value={actions}>
      <ReactFlow<FlowNode> nodes={nodes} nodeTypes={nodeTypes} onNodesChange={changes => setNodes(current => applyNodeChanges(changes, current))}
        colorMode={props.theme === 'night' ? 'dark' : 'light'} ariaLabelConfig={{ 'controls.zoomIn.ariaLabel': uiAttribute('放大画布'), 'controls.zoomOut.ariaLabel': uiAttribute('缩小画布'), 'controls.fitView.ariaLabel': uiAttribute('查看全部'),
          'controls.ariaLabel': uiAttribute('画布缩放控件'), 'minimap.ariaLabel': uiAttribute('小地图'),
          'node.a11yDescription.default': uiAttribute('按回车或空格选择节点，方向键移动，Shift 加速，Delete 删除。'),
          'node.a11yDescription.keyboardDisabled': uiAttribute('选择节点后可使用工具栏。') }}
        onSelectionChange={({ nodes }) => setSelected(current => {
          const ids = nodes.map(node => node.id); return ids.length === current.length && ids.every((id, i) => id === current[i]) ? current : ids
        })} nodesDraggable={!blocked} elementsSelectable={!blocked} selectionMode={SelectionMode.Partial} selectionOnDrag panOnDrag={[1, 2]} panOnScroll
        nodesConnectable={false} deleteKeyCode={null} minZoom={CANVAS_MIN_ZOOM} maxZoom={CANVAS_MAX_ZOOM}
        defaultViewport={view.viewport ?? { x: 0, y: 0, zoom: 1 }}
        onMoveEnd={(_event, viewport) => setView(current => ({ ...current, viewport }))}
        onNodeDragStart={() => { dragRevision.current = beginGesture() }}
        onNodeDrag={(_event, moved, movedNodes) => setGuides(align(moved, movedNodes).guides)}
        onNodeDragStop={(_event, moved, movedNodes) => {
          const revision = dragRevision.current ?? undefined
          dragRevision.current = null
          const row = moved.data.row, selectedRows = movedNodes.length ? movedNodes : [moved]
          const alignment = align(moved, movedNodes)
          const position = { x: moved.position.x + alignment.dx, y: moved.position.y + alignment.dy }
          if (row.data.conversation && selectedRows.length === 1) {
            const regions = nodes.filter(item => isRegionKind(item.data.row.data.kind)).map(item => ({ nodeId: item.id,
              kind: item.data.row.data.kind, ...item.position, width: item.width ?? 0, height: item.height ?? 0 }))
            const pins = nodes.filter(item => item.data.row.data.conversation).map(item => ({ nodeId: item.id,
              conversation: item.data.row.data.conversation!, ...item.position, width: CARD_WIDTH, height: CARD_HEIGHT }))
            const hint = computeDropHint({ kind: 'pin', nodeId: row.id, conversation: row.data.conversation }, position, regions, pins)
            if (hint.type === 'region') { mutate({ type: 'group', targetNodeId: hint.regionId, newRegion: null,
              conversations: [row.data.conversation], consumeNodeIds: [row.id] }, revision); return }
            if (hint.type === 'merge') { mutate({ type: 'group', targetNodeId: null, newRegion: { kind: 'custom', ...hint.rect },
              conversations: [hint.targetConversation, row.data.conversation], consumeNodeIds: [hint.targetNodeId, row.id] }, revision); return }
          }
          mutate({ type: 'move', moves: selectedRows.map(node => ({ nodeId: node.id, x: node.position.x + alignment.dx, y: node.position.y + alignment.dy })) }, revision)
        }}>
        <Background variant={BackgroundVariant.Dots} gap={BOARD_DOT_GAP} size={1} />
        <Controls showInteractive={false} />
        {view.minimap && <MiniMap pannable zoomable />}
        <ViewportPortal><svg className="canvas-alignment-guides" width={1} height={1} aria-hidden="true">
          {guides.map((guide, i) => <line key={i} x1={guide.axis === 'x' ? guide.at : guide.from} x2={guide.axis === 'x' ? guide.at : guide.to}
            y1={guide.axis === 'y' ? guide.at : guide.from} y2={guide.axis === 'y' ? guide.at : guide.to} vectorEffect="non-scaling-stroke" />)}
        </svg></ViewportPortal>
      </ReactFlow>
    </PresentationContext.Provider>
    {editor && <CanvasNodeEditor target={editor.target} revision={editor.revision} latestRevision={graph.snapshot?.board?.revision ?? null}
      agents={props.agents} groups={props.groups ?? EMPTY_GROUPS}
      references={referenceOptions} referencesComplete={catalog.complete} onRefreshReferences={() => { props.onRefreshReferences?.(); void folders.refresh() }}
      blocked={graphBlocked} pending={Boolean(graph.pending)} recovery={recovery} onClose={closePanel}
      commandError={issue}
      onSave={(mutation, revision) => submit(mutation, revision, true)} />}
    {deletion && <Dialog.Root open onOpenChange={open => { if (!open) closePanel() }}><Dialog.Portal>
      <Dialog.Overlay className="dialog-overlay app-dialog-overlay" />
      <AppDialogContent tone="danger" onEscapeKeyDown={event => { if (graph.hasUnsettled()) event.preventDefault() }} onPointerDownOutside={event => event.preventDefault()}>
        <AppDialogHeader title={uiAttribute('删除所选节点')} description={uiAttribute('只删除画布节点，保留工程、对话、文件及历史。')} closeDisabled={Boolean(graph.pending)} />
        <AppDialogBody><p>{uiAttribute('将删除 {0} 个节点。', deletion.ids.length)}</p>
          {deletion.hasProse && <p><UiText zh="所选便签的文字将一并删除。" /></p>}
          {deletion.revision !== graph.snapshot?.board?.revision && <p><UiText zh="画布已更新，请取消并重新核对所选节点。" /></p>}
          {error && <p role="alert">{error}</p>}{recovery}
        </AppDialogBody>
        <AppDialogFooter><button type="button" className="quiet-button" disabled={Boolean(graph.pending)} onClick={closePanel}><UiText zh="取消" /></button>
          <button type="button" className="danger-button" disabled={graphBlocked || deletion.revision !== graph.snapshot?.board?.revision}
            onClick={() => { void submit({ type: 'delete', nodeIds: deletion.ids }, deletion.revision, true).catch(() => undefined) }}><UiText zh="删除节点" /></button>
        </AppDialogFooter>
      </AppDialogContent>
    </Dialog.Portal></Dialog.Root>}
  </section>
}
