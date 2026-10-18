import { useContext, useMemo, useState } from 'react'
import { PinnedFilePreview } from '../../FilePreviewContext'
import { useUiText } from '../../interface-language'
import { FileCode2, Eye, X } from 'lucide-react'
import { findHtmlFileMentions, hasCodexVisualizeRef, splitCodexVisualizeRefs } from '../lib/codex-visualize'
import { NativeMarkdownScope } from './native-markdown-context'
import { MessageResponse } from '../components/ai-elements/source-message-response'
import { JsonRenderScope } from '../components/ai-elements/json-render-pre'
export { NativeMarkdownScope } from './native-markdown-context'
function HtmlPreview({ path, initiallyOpen = false }: { path: string; initiallyOpen?: boolean }): React.JSX.Element {
  const scope = useContext(NativeMarkdownScope), t = useUiText()
  const [open, setOpen] = useState(initiallyOpen), [actions, setActions] = useState<HTMLDivElement | null>(null)
  const source = useMemo(() => scope?.source ? { kind: 'native_child_file' as const, ...scope.source, rawReference: path } : null, [scope?.source?.threadId, scope?.source?.readId, path])
  const name = path.split(/[\\/]/).at(-1) ?? path, dir = path.slice(0, path.length - name.length)
  return <div className="native-inline-html" data-testid="html-file-preview">
    <div className="flex h-10 min-w-0 items-center gap-2.5 rounded-lg border border-border bg-card px-3">
      <FileCode2 className="h-4 w-4 shrink-0 text-muted-foreground" /><span className="shrink-0 text-xs font-medium text-foreground/85">{name}</span>
      <span className="min-w-0 flex-1 truncate text-2xs text-muted-foreground" title={path}>{dir}</span>
      <div ref={setActions} className="native-html-actions" />
      <button type="button" className="inline-flex h-7 shrink-0 items-center gap-1.5 rounded-full px-2.5 text-xs text-muted-foreground hover:bg-primary/8 hover:text-foreground"
        aria-label={open ? t('收起预览：{0}', name) : t('预览：{0}', name)} onClick={() => setOpen(value => !value)}>{open ? <X size={14} /> : <Eye size={14} />}{t(open ? '收起' : '预览')}</button>
    </div>
    {open && (scope?.visible && scope.api && source ? <div className="native-inline-html-body"><PinnedFilePreview api={scope.api} source={source} resolvedTheme={scope.theme} actionsHost={actions} /></div>
      : <p role="status">{t('当前页面暂时无法读取这份文件。')}</p>)}
  </div>
}
export function NativeMarkdown({ children, streaming = false, assistantReply = false }: { children: string; streaming?: boolean; assistantReply?: boolean }): React.JSX.Element {
  const parts = useMemo(() => hasCodexVisualizeRef(children) ? splitCodexVisualizeRefs(children) : [{ kind: 'markdown' as const, text: children }], [children])
  const mentions = useMemo(() => streaming ? [] : findHtmlFileMentions(children, { exclude: parts.flatMap(item => item.kind === 'visualize' ? [item.ref.path] : []) }), [children, parts, streaming])
  return <>{parts.map((part, index) => {
    if (part.kind === 'visualize') return <HtmlPreview key={`${index}:${part.ref.path}`} path={part.ref.path} initiallyOpen />
    const content = <MessageResponse mode={streaming ? 'streaming' : 'static'} parseIncompleteMarkdown={streaming}>{part.text}</MessageResponse>
    return <div key={index} className='break-words text-sm prose prose-sm dark:prose-invert max-w-none [&_ul]:list-inside [&_ol]:list-inside [&_[data-streamdown="code-block-body"]]:max-h-96 [&_[data-streamdown="code-block-body"]]:overflow-auto'>
      {assistantReply ? <JsonRenderScope>{content}</JsonRenderScope> : content}
    </div>
  })}{mentions.map(path => <HtmlPreview key={path} path={path} />)}</>
}
