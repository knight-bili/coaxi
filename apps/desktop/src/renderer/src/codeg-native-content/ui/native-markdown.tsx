import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import type { FilePreviewApi, ResolvedTheme } from '@contracts'
import { SafeMarkdown } from '../../SafeMarkdown'
import { PinnedFilePreview } from '../../FilePreviewContext'
import { readNativeTranscriptImage, type NativeFileSource } from '../../native-transcript-files'
import { useUiText } from '../../interface-language'
import { FileCode2, Eye, X } from 'lucide-react'
import NativeImage from './native-image'
import { ImageActions, useImageActions } from './native-image-actions'
import { ImagePreviewDialog } from '../components/ui/image-preview-dialog'
import { CodeBlock, CodeBlockHeader, CodeBlockTitle, CodeBlockCopyButton } from '../components/ai-elements/code-block'
import { findHtmlFileMentions, hasCodexVisualizeRef, splitCodexVisualizeRefs } from '../lib/codex-visualize'
import type { UserImageDisplay } from '../lib/adapters/ai-elements-adapter'

type Scope = { source: NativeFileSource | null; api: FilePreviewApi | null; theme: ResolvedTheme; visible: boolean; open: ((path: string, line?: number | null) => Promise<void>) | null }
export const NativeMarkdownScope = createContext<Scope | null>(null)
function InlinePicture({ image, alt }: { image: UserImageDisplay; alt: string }): React.JSX.Element {
  const [preview, setPreview] = useState(false), { canCopy, copy, download } = useImageActions(), t = useUiText()
  const src = `data:${image.mime_type};base64,${image.data}`
  return <><ImageActions image={image} as="span" className="inline-block max-w-full align-middle">
    <button type="button" className="inline-block max-w-full cursor-zoom-in" aria-label={alt || image.name} onClick={() => setPreview(true)}>
      <NativeImage src={src} alt={alt || image.name} className="h-auto max-h-96 max-w-full object-contain" />
    </button>
  </ImageActions><ImagePreviewDialog src={src} alt={alt || image.name} open={preview} onOpenChange={setPreview}
    onCopy={canCopy ? () => void copy(image) : undefined} onDownload={() => void download(image)} copyLabel={t('复制图片')} downloadLabel={t('保存图片…')}
    renderImage={picture => <ImageActions image={image}>{picture}</ImageActions>} /></>
}
function InlineImage({ reference, alt }: { reference: string; alt: string }): React.JSX.Element {
  const scope = useContext(NativeMarkdownScope), t = useUiText(), root = useRef<HTMLSpanElement>(null)
  const [visible, setVisible] = useState(false), [state, setState] = useState<{ owner: Scope; reference: string; image: UserImageDisplay | null } | null>(null)
  useEffect(() => {
    if (!root.current) return
    const observer = new IntersectionObserver(entries => { if (entries.some(item => item.isIntersecting)) { setVisible(true); observer.disconnect() } }, { rootMargin: '160px' })
    observer.observe(root.current); return () => observer.disconnect()
  }, [])
  useEffect(() => {
    if (!visible || !scope?.visible || !scope.source || !scope.api) return
    let active = true
    void readNativeTranscriptImage(scope.api, scope.source, reference).then(async content => {
      const data = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader()
        reader.onload = () => resolve(String(reader.result).split(',')[1] ?? '')
        reader.onerror = () => reject(new Error('image_unavailable'))
        reader.readAsDataURL(new Blob([Uint8Array.from(content.bytes)], { type: content.mime }))
      })
      if (active) setState({ owner: scope, reference, image: { name: alt || reference.split(/[\\/]/).at(-1) || 'image', data, mime_type: content.mime, uri: null } })
    }).catch(() => { if (active) setState({ owner: scope, reference, image: null }) })
    return () => { active = false }
  }, [scope, reference, alt, visible])
  const current = state?.owner === scope && state.reference === reference ? state : null
  return <span ref={root} className="native-markdown-image">{current?.image ? <InlinePicture image={current.image} alt={alt} />
    : <span role={current ? 'img' : 'status'} aria-label={alt || reference}>{alt || reference}{current ? t('（暂不可读）') : ''}</span>}</span>
}
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
export function NativeMarkdown({ children, streaming = false }: { children: string; streaming?: boolean }): React.JSX.Element {
  const scope = useContext(NativeMarkdownScope), [error, setError] = useState<string | null>(null), t = useUiText()
  const parts = useMemo(() => hasCodexVisualizeRef(children) ? splitCodexVisualizeRefs(children) : [{ kind: 'markdown' as const, text: children }], [children])
  const mentions = useMemo(() => streaming ? [] : findHtmlFileMentions(children, { exclude: parts.flatMap(item => item.kind === 'visualize' ? [item.ref.path] : []) }), [children, parts, streaming])
  const image = useCallback((reference: string, alt: string): ReactNode => <InlineImage reference={reference} alt={alt} />, [])
  const code = useCallback((value: string, language = 'text'): ReactNode => <CodeBlock code={value} language={language}>
    <CodeBlockHeader><CodeBlockTitle>{language}</CodeBlockTitle><CodeBlockCopyButton aria-label={t('复制代码')} /></CodeBlockHeader>
  </CodeBlock>, [t])
  return <>{parts.map((part, index) => part.kind === 'visualize' ? <HtmlPreview key={`${index}:${part.ref.path}`} path={part.ref.path} initiallyOpen />
    : <SafeMarkdown key={index} mode="document" theme={scope?.theme ?? 'day'} renderLocalImage={image} renderCodeBlock={code}
      onFileReference={scope?.open ? (reference, _origin, target) => { setError(null); void scope.open!(reference, target?.line).catch(() => setError(t('暂时无法打开这份文件。'))) } : undefined}>{part.text}</SafeMarkdown>)}
    {mentions.map(path => <HtmlPreview key={path} path={path} />)}{error && <p role="alert" className="native-session-note is-error">{error}</p>}
  </>
}
