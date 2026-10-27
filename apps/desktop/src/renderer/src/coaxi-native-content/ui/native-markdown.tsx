import { useMemo } from 'react'
import { findHtmlFileMentions, hasCodexVisualizeRef, splitCodexVisualizeRefs } from '../lib/codex-visualize'
import { MessageResponse } from '../components/ai-elements/source-message-response'
import { JsonRenderScope } from '../components/ai-elements/json-render-pre'
import { CodexVisualizeCard } from '../components/message/codex-visualize-card'
import { HtmlFilePreviews } from '../components/message/html-file-previews'
export { NativeMarkdownScope } from './native-markdown-context'
export function NativeMarkdown({ children, streaming = false, assistantReply = false }: { children: string; streaming?: boolean; assistantReply?: boolean }): React.JSX.Element {
  const parts = useMemo(() => hasCodexVisualizeRef(children) ? splitCodexVisualizeRefs(children) : [{ kind: 'markdown' as const, text: children }], [children])
  const mentions = useMemo(() => streaming ? [] : findHtmlFileMentions(children, { exclude: parts.flatMap(item => item.kind === 'visualize' ? [item.ref.path] : []) }), [children, parts, streaming])
  return <>{parts.map((part, index) => {
    if (part.kind === 'visualize') return <CodexVisualizeCard key={`${index}:${part.ref.path}`} path={part.ref.path} mode={part.ref.mode} />
    const content = <MessageResponse mode={streaming ? 'streaming' : 'static'} parseIncompleteMarkdown={streaming}>{part.text}</MessageResponse>
    return <div key={index} className='break-words text-sm prose prose-sm dark:prose-invert max-w-none [&_ul]:list-inside [&_ol]:list-inside [&_[data-streamdown="code-block-body"]]:max-h-96 [&_[data-streamdown="code-block-body"]]:overflow-auto'>
      {assistantReply ? <JsonRenderScope>{content}</JsonRenderScope> : content}
    </div>
  })}<HtmlFilePreviews paths={mentions} /></>
}
