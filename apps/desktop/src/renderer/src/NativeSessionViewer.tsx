// Codeg aff1e6c components/message/subagent-session-dialog.tsx: readonly
// non-modal drawer, 16px body column, role asides and preserved refresh body.
// Core authority + shared Rovai Markdown are the integration seams.
import { useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { Drawer } from '@base-ui/react/drawer'
import { Brain, Check, ChevronDown, Eye, Loader2, RefreshCw, Wrench, X, XCircle } from 'lucide-react'
import { NativeChildReader, type NativeTranscriptBlock, type NativeTranscriptTurn } from './native-child-reader'
import type { NativeSessionTarget } from './native-session-viewer-context'
import { NativeSessionViewerContext } from './native-session-viewer-context'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { CodegRuntimeMark } from './CodegRuntimeMark'
import { SafeMarkdown } from './SafeMarkdown'
import { DelegationViewerHost } from './DelegationViewer'
import { useEscapeShield, shieldedDismissal } from './session-viewer-dismissal'
import './native-session-viewer.css'
import { nativeToolChildren, nativeToolPairs } from './native-transcript-model'

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
              <NativeTranscriptBody turns={state.data.turns} reader={reader} target={target} live={open && visible && state.live} />
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
function NativeTranscriptBody({ turns, reader, target, live }: { turns: NativeTranscriptTurn[]; reader: NativeChildReader; target: NativeSessionTarget; live: boolean }): React.JSX.Element {
  const { results, paired } = nativeToolPairs(turns)
  return <div className="native-session-transcript">{turns.map((turn, turnIndex) => <section key={`${turn.id}:${turnIndex}`} className={turn.role === 'user' ? 'native-session-input-turn' : 'native-session-turn'}>
    {turn.blocks.map((block, index) => {
      const key = `${turn.id}:${index}`
      if (block.type === 'text') return <SafeMarkdown key={key}>{block.text}</SafeMarkdown>
      if (block.type === 'thinking') return <NativeReasoning key={key} text={block.text} />
      if (block.type === 'turn_error') return <p key={key} className="native-session-note is-error" role="status">{block.message}</p>
      if (block.type === 'image') return <NativeImage key={key} image={block} />
      if (block.type === 'image_generation') return <div key={key}>{block.image && <NativeImage image={block.image} />}{block.revised_prompt && <NativeReasoning text={block.revised_prompt} imagePrompt />}</div>
      if (block.type === 'tool_use') return <NativeTool key={key} use={block} result={results.get(block)} reader={reader} target={target} live={live} />
      if (block.type === 'tool_result' && !paired.has(block)) return <NativeTool key={key} result={block} reader={reader} target={target} live={false} />
      return null
    })}
  </section>)}</div>
}
function NativeReasoning({ text, imagePrompt = false }: { text: string; imagePrompt?: boolean }): React.JSX.Element {
  const t = useUiText()
  return <details className="native-session-reasoning"><summary><Brain size={14} /><span>{t(imagePrompt ? '生成说明' : '思考')}</span><ChevronDown size={14} /></summary><SafeMarkdown>{text}</SafeMarkdown></details>
}
function NativeImage({ image }: { image: { data: string; mime_type: string } }): React.JSX.Element {
  const t = useUiText()
  const valid = /^image\/(png|jpeg|gif|webp|avif|bmp)$/i.test(image.mime_type)
  const src = valid ? `data:${image.mime_type};base64,${image.data}` : null
  return src ? <img className="native-session-image" alt={t('会话图片')} src={src} loading="lazy" /> : <p className="native-session-note">{t('该图片格式暂不支持预览。')}</p>
}
function NativeTool({ use, result, reader, target, live }: { use?: ToolUse; result?: ToolResult; reader: NativeChildReader; target: NativeSessionTarget; live: boolean }): React.JSX.Element {
  const t = useUiText(), host = useContext(NativeSessionViewerContext)
  const [expanded, setExpanded] = useState(false)
  const pending = live && !result && (use?.status === 'pending' || use?.status === 'in_progress')
  const failed = result?.is_error === true || use?.status === 'failed'
  const children = nativeToolChildren(use, result).flatMap(id => { const source = reader.child(id); return source ? [{ id, source }] : [] })
  const openChild = (child: typeof children[number], trigger: HTMLElement): void => host?.open({ ...target,
    identity: `${target.identity}:${child.id}`, parentConversationId: `native-view:${target.identity}`, source: child.source, subtitle: `#${child.id.slice(-8)}` }, trigger)
  return <section className={`native-session-tool${failed ? ' is-error' : ''}`}>
    <div className="native-session-tool-heading"><button type="button" aria-expanded={expanded} onClick={() => setExpanded(value => !value)}>
      <Wrench size={14} /><span>{use?.tool_name ?? t('工具结果')}</span>{pending ? <Loader2 size={13} className="native-session-spinner" /> : failed ? <XCircle size={13} /> : result || use?.status === 'completed' ? <Check size={13} /> : null}<ChevronDown size={13} /></button>
      {children.length === 1 && host && <button className="native-session-open-child" type="button" aria-label={t('查看下级帮手会话')} onClick={event => openChild(children[0]!, event.currentTarget)}><Eye size={14} /><span>{t('查看会话')}</span></button>}
    </div>
    {expanded && <div className="native-session-tool-content">
      {children.length > 1 && children.map(child => <button key={child.id} className="quiet-button" type="button" onClick={event => openChild(child, event.currentTarget)}><Eye size={14} />#{child.id.slice(-8)} {t('查看会话')}</button>)}
      {use?.input_preview != null && <><small>{t('输入')}</small><pre>{use.input_preview}</pre></>}
      {result?.output_preview != null && <><small>{t('结果')}</small><pre>{result.output_preview}</pre></>}
      {use?.meta != null && <details><summary>{t('工具信息')}</summary><pre>{JSON.stringify(use.meta, null, 2)}</pre></details>}
      {result?.agent_stats && <details><summary>{t('帮手执行记录')}</summary><pre>{JSON.stringify(result.agent_stats, null, 2)}</pre></details>}
    </div>}
    {result?.images?.map((image, index) => <NativeImage image={image} key={index} />)}
  </section>
}
