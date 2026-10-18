// Codeg aff1e6c components/message/subagent-session-dialog.tsx: readonly
// non-modal drawer, 16px body column, role asides and preserved refresh body.
// Core authority + shared Rovai Markdown are the integration seams.
import { useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { Drawer } from '@base-ui/react/drawer'
import { Brain, ChevronDown, Eye, Loader2, RefreshCw, X } from 'lucide-react'
import { NativeChildReader, type NativeTranscript, type NativeTranscriptBlock } from './native-child-reader'
import type { NativeSessionTarget } from './native-session-viewer-context'
import { NativeSessionViewerContext } from './native-session-viewer-context'
import { useThreadClient } from './camp-client'
import { useInterfaceLanguage, useUiText } from './interface-language'
import { CodegRuntimeMark } from './CodegRuntimeMark'
import { DelegationViewerHost } from './DelegationViewer'
import { useEscapeShield, shieldedDismissal } from './session-viewer-dismissal'
import './native-session-viewer.css'
import { nativeToolChildren, nativeToolPairs } from './native-transcript-model'
import { adaptNativeMessages, nativeTurnStats } from './native-content-adapter'
import type { AdaptedContentPart, AdaptedToolCallPart, AdapterMessageText } from './codeg-native-content/lib/adapters/ai-elements-adapter'
import { normalizeToolName } from './codeg-native-content/lib/tool-call-normalization'
import { isPlanModeToolName } from './codeg-native-content/lib/plan-parse'
import { PlanCard } from './codeg-native-content/components/message/plan-card'
import { PlanMarkdownCard, PlanModeCard } from './codeg-native-content/components/message/plan-mode-card'
import { GoalRunPart, GoalToolCallPart } from './codeg-native-content/components/message/goal-tool-call'
import { SourceMarkdownRenderer, useTranslations } from './codeg-native-content/ui/source-primitives'
import { Tool, ToolHeader, ToolContent } from './codeg-native-content/components/ai-elements/tool'
import { CodeBlock, CodeBlockHeader, CodeBlockTitle, CodeBlockCopyButton } from './codeg-native-content/components/ai-elements/code-block'
import { JsonTreeView, parseJsonForTree } from './codeg-native-content/components/ai-elements/json-tree'
import { Terminal } from './codeg-native-content/components/ai-elements/terminal'
import { parseCodexCommandEnvelope } from './codeg-native-content/lib/codex-command-action'
import { isShellSessionToolName } from './codeg-native-content/lib/shell-session-tool'
import { ToolCallPart as OrdinaryToolCallPart } from './codeg-native-content/components/message/ordinary-tool-call'
import { NativeFileActionsProvider } from './codeg-native-content/ui/native-file-actions'
import { ReadonlyToolCard, readonlyToolCardKind } from './codeg-native-content/ui/readonly-tool-card'
import { DelegationStatusGroupCard } from './codeg-native-content/components/message/delegation-status-group-card'
import { ToolGroupPart } from './codeg-native-content/components/message/tool-group-part'
import { BackgroundTaskCard } from './codeg-native-content/components/message/background-task-card'
import { AgentToolCallPart } from './codeg-native-content/components/message/agent-tool-call'
import { CollabAgentCard } from './codeg-native-content/components/message/collab-agent-card'
import { SubagentSessionButton } from './codeg-native-content/components/message/subagent-session-button'
import { NativeSessionActionsProvider, type SourceSessionTarget } from './codeg-native-content/ui/native-session-actions'
import { COLLAB_AGENT_TOOL_NAME } from './codeg-collab-tool'
import { useOptionalFilePreview, useOptionalFilePreviewApi } from './FilePreviewContext'
import { NativeMarkdown, NativeMarkdownScope } from './codeg-native-content/ui/native-markdown'
import { NativeImageScope } from './codeg-native-content/ui/native-image-actions'
import { NativeMessageScroll } from './codeg-native-content/ui/native-turn-actions'
import { UserImageAttachments } from './codeg-native-content/components/message/user-image-attachments'
import { GeneratedImagesBlock } from './codeg-native-content/components/message/generated-images-block'
import { TurnStats } from './codeg-native-content/components/message/turn-stats'
import './codeg-native-content/source-rich.css'
import './codeg-native-content/source-media.css'

function readMessage(code: string): string {
  if (/origin_unavailable|source_unavailable/.test(code)) return '暂时找不到这份会话记录。'
  if (/user_required|scope_changed|reference_unavailable|binding_changed/.test(code)) return '会话归属已变化，请从原记录重新打开。'
  if (/reader_busy|reader_limit/.test(code)) return '正在读取其他会话，请稍后重试。'
  if (/child_unavailable|read_missing|read_closed/.test(code)) return '这份读取已失效，请刷新后重试。'
  return '会话暂时无法读取，请稍后重试。'
}
export function NativeSessionViewer({ target, open, visible, onOpenChange, returnFocus, environment }: {
  target: NativeSessionTarget; open: boolean; visible: boolean; onOpenChange(open: boolean): boolean; returnFocus?: HTMLElement
  environment: Omit<React.ComponentProps<typeof DelegationViewerHost>, 'children'>
}): React.JSX.Element {
  useEscapeShield()
  const client = useThreadClient(), t = useUiText()
  const reader = useMemo(() => new NativeChildReader(client, target.source), [client, target])
  const state = useSyncExternalStore(reader.subscribe, reader.getSnapshot, reader.getSnapshot)
  const heading = useRef<HTMLElement | null>(null), body = useRef<HTMLDivElement | null>(null), scroll = useRef(0)
  useEffect(() => open && visible ? reader.retain() : undefined, [reader, open, visible])
  useEffect(() => () => reader.dispose(), [reader])
  useLayoutEffect(() => { if (body.current && state.data) body.current.scrollTop = Math.min(scroll.current, Math.max(0, body.current.scrollHeight - body.current.clientHeight)) }, [open, visible, state.data])
  return <Drawer.Root open={open} modal={false} disablePointerDismissal swipeDirection="right" onOpenChange={(next, details) => {
    if ((!visible && ['outside-press', 'escape-key', 'focus-out'].includes(details.reason)) || !next && shieldedDismissal(details)) { details.cancel(); return }
    if (!onOpenChange(next)) details.cancel()
  }}>
    <Drawer.Portal><Drawer.Viewport className="delegation-drawer-viewport" data-host-hidden={!visible || undefined} inert={!visible || undefined}>
      <Drawer.Popup className="delegation-drawer native-session-viewer" initialFocus={heading} finalFocus={() => returnFocus?.isConnected ? returnFocus : false}>
        <Drawer.Content className="delegation-drawer-content">
          <Drawer.Title className="sr-only">{t('原生帮手会话')}</Drawer.Title>
          <Drawer.Description className="sr-only">{t('只读查看帮手已经保存的消息和工具记录。')}</Drawer.Description>
          <header className="native-session-header" tabIndex={-1} ref={heading}>
            <CodegRuntimeMark target={target.runtime} size={16} />
            <div><strong>{t('原生帮手会话')}</strong>{target.subtitle && <small>{target.subtitle}</small>}</div>
            {state.loading && <Loader2 size={14} className="native-session-spinner" aria-label={t('正在读取会话')} />}
            <button type="button" className="delegation-icon-button" aria-label={t('刷新原生帮手会话')} onClick={() => void reader.refresh()}><RefreshCw size={15} /></button>
            <Drawer.Close className="delegation-icon-button" aria-label={t('关闭原生帮手会话')}><X size={16} /></Drawer.Close>
          </header>
          {state.error && <div className="native-session-notice"><p className="delegation-read-error" role="alert">{t(readMessage(state.error))}<button type="button" onClick={() => void reader.refresh()}>{t('重试')}</button></p></div>}
          <div className="native-session-body" ref={body} onScroll={event => { scroll.current = event.currentTarget.scrollTop }}>
            {!state.data && state.loading && <p className="delegation-empty"><Loader2 size={14} className="native-session-spinner" />{t('正在读取会话…')}</p>}
            {state.data && <DelegationViewerHost {...environment} visible={visible}>
              <NativeTranscriptBody data={state.data} reader={reader} target={target} live={open && visible && state.live} visible={open && visible} />
              {state.data.turns.length === 0 && <p className="delegation-empty">{t('还没有可显示的会话内容。')}</p>}
              {(state.data.notes.malformed_records > 0 || state.data.notes.incomplete_tail || state.data.notes.unavailable_children > 0)
                && <p className="native-session-note" role="status">{t('部分记录暂时无法读取，可刷新后再核对。')}</p>}
            </DelegationViewerHost>}
          </div>
        </Drawer.Content>
      </Drawer.Popup>
    </Drawer.Viewport></Drawer.Portal>
  </Drawer.Root>
}

type ToolUse = Extract<NativeTranscriptBlock, { type: 'tool_use' }>
type ToolResult = Extract<NativeTranscriptBlock, { type: 'tool_result' }>
function NativeTranscriptBody({ data, reader, target, live, visible }: { data: NativeTranscript; reader: NativeChildReader; target: NativeSessionTarget; live: boolean; visible: boolean }): React.JSX.Element {
  const turns = data.turns
  const language = useInterfaceLanguage(), t = useUiText(), planText = useTranslations('Folder.chat.proposedPlan')
  const filePreview = useOptionalFilePreview()
  const previewOpen = filePreview?.open
  const fileApi = useOptionalFilePreviewApi(), transcript = useRef<HTMLDivElement>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const openFile = useCallback(async (rawReference: string, line?: number | null): Promise<void> => {
    const source = reader.fileSource()
    if (!source || !previewOpen) throw new Error('native_child.source_unavailable')
    const outcome = await previewOpen({ kind: 'native_child_file', ...source, rawReference },
      line != null && Number.isSafeInteger(line) && line > 0 ? { line } : undefined,
      undefined, { commitOnSuccess: true, revalidateSource: true })
    if (outcome.kind === 'error') throw new Error('native_child.file_unavailable')
  }, [reader, previewOpen])
  const source = reader.fileSource()
  const scope = useMemo(() => ({ source, api: fileApi, theme: filePreview?.resolvedTheme ?? 'day' as const, visible, open: filePreview ? openFile : null }),
    [source?.threadId, source?.readId, fileApi, filePreview?.resolvedTheme, visible, Boolean(previewOpen), openFile])
  const imageScope = useMemo(() => ({ visible, notice: setNotice }), [visible, source?.threadId, source?.readId])
  const renderMarkdown = useCallback((text: string, streaming: boolean) => <NativeMarkdown streaming={streaming}>{text}</NativeMarkdown>, [])
  const scrolling = useMemo(() => ({ scrollToIndex(index: number, options: { smooth: boolean }) {
    transcript.current?.querySelector<HTMLElement>(`[data-native-turn-index="${index}"]`)?.scrollIntoView({ block: 'start', behavior: options.smooth ? 'smooth' : 'instant' })
  } }), [])
  const text = useMemo<AdapterMessageText>(() => ({
    attachedResources: language === 'en' ? 'Attached resources' : '附加资源',
    toolCallFailed: language === 'en' ? 'Tool call failed' : '工具调用失败',
    pageHandoffName: handoff => handoff.kind === 'element' ? handoff.label || (language === 'en' ? 'Page element' : '页面元素')
      : handoff.kind === 'screenshot' ? language === 'en' ? 'Page screenshot' : '页面截图'
        : language === 'en' ? `${handoff.count} console errors` : `${handoff.count} 条控制台错误`
  }), [language])
  const messages = useMemo(() => adaptNativeMessages(turns, live, text), [turns, live, text])
  const { results } = useMemo(() => nativeToolPairs(turns), [turns])
  const tool = (part: AdaptedToolCallPart, key: string): React.JSX.Element => {
    if (readonlyToolCardKind(part)) return <NativeSourceTool key={key} part={part}><ReadonlyToolCard part={part} /></NativeSourceTool>
    const name = normalizeToolName(part.toolName)
    if (isPlanModeToolName(name)) return <PlanModeCard key={key} toolName={name} input={part.input} output={part.output} errorText={part.errorText ?? null} state={part.state} />
    if (name === 'create_goal' || name === 'update_goal') return <GoalToolCallPart key={key} part={part} />
    const origin = /^native-view:(\d+):(\d+)$/.exec(part.toolCallId)
    const raw = origin ? turns[Number(origin[1])]?.blocks[Number(origin[2])] : undefined
    const use: ToolUse = raw?.type === 'tool_use' ? raw : { type: 'tool_use', tool_use_id: part.toolCallId, tool_name: part.toolName, input_preview: part.input, meta: part.meta, status: part.toolStatus }
    const original = raw?.type === 'tool_use' ? results.get(raw) : undefined
    const result: ToolResult | undefined = original ? { ...original, images: undefined } : part.output !== undefined || part.errorText !== undefined
      ? { type: 'tool_result', tool_use_id: part.toolCallId, output_preview: part.errorText ?? part.output ?? null, is_error: part.state === 'output-error', agent_stats: part.agentStats ? { ...part.agentStats } : null } : undefined
    if (name.toLowerCase() === 'agent' || name.toLowerCase() === COLLAB_AGENT_TOOL_NAME) return <NativeAgentSourceTool key={key} part={part} reader={reader} target={target} childIds={nativeToolChildren(use, result)}>
      {name.toLowerCase() === 'agent'
        ? <AgentToolCallPart part={part} renderToolCall={(child, childKey) => tool({ ...child, agentStats: undefined, agentTranscript: undefined }, childKey)} />
        : <CollabAgentCard input={part.input ?? null} errorText={part.errorText ?? null} state={part.state} />}
    </NativeAgentSourceTool>
    // Dedicated agent/interaction cards retain their existing child-session route
    // until their fixed source components are ported in the next finite batch.
    const specialized = /^(agent|task|collab_agent|delegate_to_agent|check_delegation|cancel_delegation|resume_delegation|ask_user_question|check_user_feedback)$/.test(name.toLowerCase())
      || name.startsWith('mcp__') || nativeToolChildren(use, result).length > 0
    if (!specialized) return <NativeSourceTool key={key} part={part}><OrdinaryToolCallPart part={part} /></NativeSourceTool>
    return <NativeTool key={key} use={use} result={result} reader={reader} target={target} live={live} />
  }
  const renderPart = (part: AdaptedContentPart, key: string, user = false, streaming = false): React.ReactNode => {
    if (part.type === 'text') return user ? <div className="native-session-plain-text" key={key}>{part.text}</div> : <NativeMarkdown key={key} streaming={streaming}>{part.text}</NativeMarkdown>
    if (part.type === 'reasoning') return <NativeReasoning key={key} text={part.content} />
    if (part.type === 'turn-error') return <p key={key} className="native-session-note is-error" role="status">{part.message}</p>
    if (part.type === 'plan') return <PlanCard key={key} entries={part.entries} isStreaming={live && part.isStreaming} />
    if (part.type === 'proposed-plan') return <PlanMarkdownCard key={key} markdown={part.markdown.trim()} label={planText('title')} emptyLabel={planText('planning')} />
    if (part.type === 'goal-run') return <GoalRunPart key={key} part={live ? part : { ...part, isRunning: false }} renderPart={(child, childKey) => renderPart(child, childKey, user, streaming)} />
    if (part.type === 'tool-call') return tool(part, key)
    if (part.type === 'tool-group') return <ToolGroupPart key={key} part={part} renderItem={tool} />
    if (part.type === 'delegation-status-group') return <div className="native-session-source-tool" key={key}><DelegationStatusGroupCard polls={part.polls} />{part.polls.map(item => <NativeRawTool key={item.toolCallId} part={item} />)}</div>
    if (part.type === 'background-task-group') return <div className="native-session-source-tool" key={key}><BackgroundTaskCard polls={part.polls} />{part.polls.map(item => <NativeRawTool key={item.toolCallId} part={item} />)}</div>
    if (part.type === 'tool-result') return <NativeTool key={key} result={{ type: 'tool_result', tool_use_id: part.toolCallId, output_preview: part.errorText ?? part.output, is_error: part.state === 'output-error', agent_stats: part.agentStats ? { ...part.agentStats } : null }} reader={reader} target={target} live={false} />
    if (part.type === 'generated-image') return <GeneratedImagesBlock key={key} label={part.label} image={part.image} revisedPrompt={part.revisedPrompt}
      status={!live && !part.image && part.status !== 'failed' ? 'completed' : part.status} />
    return null
  }
  return <NativeFileActionsProvider open={filePreview ? openFile : null}><NativeMarkdownScope.Provider value={scope}><SourceMarkdownRenderer.Provider value={renderMarkdown}><NativeImageScope.Provider value={imageScope}><NativeMessageScroll.Provider value={scrolling}>
    {notice && <p className="native-session-note" role="status">{notice}</p>}
    <div ref={transcript} className="native-session-transcript codeg-native-rich">{messages.map((message, turnIndex) => {
      const previousUserIndex = turns.slice(0, turnIndex).findLastIndex(turn => turn.role === 'user')
      return <section data-native-turn-index={turnIndex} key={`${message.id}:${turnIndex}`} className={message.role === 'user' ? 'native-session-input-turn' : 'native-session-turn'}>
    {message.content.map((part, index) => renderPart(part, `${message.id}:${index}`, message.role === 'user', live && turnIndex === messages.length - 1))}
    {message.userResources?.map((resource, index) => <button type="button" className="native-session-resource" title={resource.uri} key={`resource:${index}`}
      onClick={() => { void openFile(resource.uri).catch(() => setNotice(t('暂时无法打开这份文件。'))) }}>{resource.name}</button>)}
    {message.userImages && <UserImageAttachments images={message.userImages} />}
    {message.role === 'assistant' && <TurnStats {...nativeTurnStats(turns[turnIndex])} copyText={turns[turnIndex].blocks.flatMap(block => block.type === 'text' ? [block.text] : []).join('\n\n')}
      isResponseComplete={!live || turnIndex < turns.length - 1} previousUserIndex={previousUserIndex < 0 ? undefined : previousUserIndex} />}
  </section>})}<details className="native-session-reasoning"><summary>{t('会话统计与原始信息')}</summary><JsonTreeView value={{ context_tokens: data.context_tokens, session_stats: data.session_stats, summary: data.summary,
    turns: turns.map(({ blocks: _blocks, ...metadata }) => metadata) }} /></details></div>
  </NativeMessageScroll.Provider></NativeImageScope.Provider></SourceMarkdownRenderer.Provider></NativeMarkdownScope.Provider></NativeFileActionsProvider>
}
function NativeSourceTool({ part, children }: { part: AdaptedToolCallPart; children: React.ReactNode }): React.JSX.Element {
  return <div className="native-session-source-tool">{children}<NativeRawTool part={part} /></div>
}
function NativeAgentSourceTool({ part, reader, target, childIds, children }: { part: AdaptedToolCallPart; reader: NativeChildReader; target: NativeSessionTarget; childIds: string[]; children: React.ReactNode }): React.JSX.Element {
  const t = useUiText(), host = useContext(NativeSessionViewerContext)
  const [unavailable, setUnavailable] = useState(false)
  const open = useCallback((request: SourceSessionTarget): void => {
    const source = reader.child(request.sessionId)
    if (!host?.visible || request.agentType !== target.runtime || !source) { setUnavailable(true); return }
    setUnavailable(false)
    host.open({ ...target, identity: `${target.identity}:${request.sessionId}`, parentConversationId: `native-view:${target.identity}`,
      source, subtitle: request.description || `#${request.sessionId.slice(-8)}` }, document.activeElement instanceof HTMLElement ? document.activeElement : undefined)
  }, [reader, target, host])
  // A multi-agent source capsule deliberately has no one-child header action.
  // Keep every verified destination reachable instead of choosing the first.
  const destinations = childIds.length > 1 ? childIds.filter(id => reader.child(id)) : []
  return <NativeSourceTool part={part}><NativeSessionActionsProvider open={open}>{children}
    {destinations.map(id => <div className="native-session-open-child" key={id}><span>#{id.slice(-8)}</span><SubagentSessionButton onClick={() => open({ kind: 'agentSession', sessionId: id, agentType: target.runtime, subagentType: null, description: null })} /></div>)}
    {unavailable && <p className="native-session-note is-error" role="alert">{t(readMessage('native_child.child_unavailable'))}</p>}
  </NativeSessionActionsProvider></NativeSourceTool>
}
function NativeRawTool({ part }: { part: AdaptedToolCallPart }): React.JSX.Element {
  const t = useUiText()
  return <details className="native-session-reasoning"><summary><span>{t('原始工具记录')}</span><ChevronDown size={14} /></summary>
      {part.input != null && <NativeToolValue label={t('输入')} value={part.input} />}
      {part.output != null && <NativeToolValue label={t('原始结果')} value={part.output} />}
      {part.errorText != null && <NativeToolValue label={t('错误')} value={part.errorText} />}
      {part.meta != null && <NativeToolValue label={t('工具信息')} value={JSON.stringify(part.meta, null, 2)} />}
      {part.agentStats != null && <NativeToolValue label={t('帮手执行记录')} value={JSON.stringify(part.agentStats, null, 2)} />}
    </details>
}
function NativeReasoning({ text }: { text: string }): React.JSX.Element {
  const t = useUiText()
  return <details className="native-session-reasoning"><summary><Brain size={14} /><span>{t('思考')}</span><ChevronDown size={14} /></summary><NativeMarkdown>{text}</NativeMarkdown></details>
}
function NativeTool({ use, result, reader, target, live }: { use?: ToolUse; result?: ToolResult; reader: NativeChildReader; target: NativeSessionTarget; live: boolean }): React.JSX.Element {
  const t = useUiText(), host = useContext(NativeSessionViewerContext)
  const [expanded, setExpanded] = useState(false)
  const pending = live && !result && (use?.status === 'pending' || use?.status === 'in_progress')
  const failed = result?.is_error === true || use?.status === 'failed'
  const command = use ? ['bash', 'exec_command'].includes(normalizeToolName(use.tool_name).toLowerCase()) || isShellSessionToolName(normalizeToolName(use.tool_name)) : false
  const output = result?.output_preview
  const commandOutput = command && output != null ? parseCodexCommandEnvelope(output)?.output ?? output : ''
  const children = nativeToolChildren(use, result).flatMap(id => { const source = reader.child(id); return source ? [{ id, source }] : [] })
  const openChild = (child: typeof children[number], trigger: HTMLElement): void => host?.open({ ...target,
    identity: `${target.identity}:${child.id}`, parentConversationId: `native-view:${target.identity}`, source: child.source, subtitle: `#${child.id.slice(-8)}` }, trigger)
  return <Tool className="native-session-source-tool" open={expanded || pending && command} onOpenChange={setExpanded}>
    <ToolHeader type="dynamic-tool" toolName={use?.tool_name ?? t('工具结果')} state={failed ? 'output-error' : pending ? 'input-available' : result || use?.status === 'completed' ? 'output-available' : 'input-streaming'} />
    {children.length === 1 && host && <button className="native-session-open-child" type="button" aria-label={t('查看下级帮手会话')} onClick={event => openChild(children[0]!, event.currentTarget)}><Eye size={14} /><span>{t('查看会话')}</span></button>}
    <ToolContent>
      {children.length > 1 && children.map(child => <button key={child.id} className="quiet-button" type="button" onClick={event => openChild(child, event.currentTarget)}><Eye size={14} />#{child.id.slice(-8)} {t('查看会话')}</button>)}
      {use?.input_preview != null && <NativeToolValue label={t('输入')} value={use.input_preview} />}
      {output != null && (command ? <Terminal output={commandOutput} isStreaming={pending} /> : <NativeToolValue label={t('结果')} value={output} />)}
      {command && output != null && commandOutput !== output && <details><summary>{t('原始结果')}</summary><NativeToolValue label={t('原始结果')} value={output} /></details>}
      {use?.meta != null && <details><summary>{t('工具信息')}</summary><NativeToolValue label={t('工具信息')} value={JSON.stringify(use.meta, null, 2)} /></details>}
      {result?.agent_stats && <details><summary>{t('帮手执行记录')}</summary><NativeToolValue label={t('帮手执行记录')} value={JSON.stringify(result.agent_stats, null, 2)} /></details>}
    </ToolContent>
    {result?.images && <UserImageAttachments images={result.images.map((image, index) => ({ ...image, name: `${t('会话图片')}-${index + 1}` }))} />}
  </Tool>
}

function NativeToolValue({ label, value }: { label: string; value: string }): React.JSX.Element {
  const t = useUiText()
  const parsed = useMemo(() => parseJsonForTree(value), [value])
  return <div className="native-session-tool-value"><small>{label}</small>{parsed !== undefined
    ? <JsonTreeView value={parsed} rawText={value} />
    : <CodeBlock code={value} language="text"><CodeBlockHeader><CodeBlockTitle>{label}</CodeBlockTitle><CodeBlockCopyButton aria-label={t('复制{0}', label)} /></CodeBlockHeader></CodeBlock>}
  </div>
}
