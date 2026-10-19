import { createContext, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import type { FilePreviewApi, ResolvedTheme } from '@contracts'
import { readNativeTranscriptImage, type NativeFileSource } from '../../native-transcript-files'
import { useUiText } from '../../interface-language'
import NativeImage from './native-image'
import { ImageActions, useImageActions } from './native-image-actions'
import { ImagePreviewDialog } from '../components/ui/image-preview-dialog'
import type { UserImageDisplay } from '../lib/adapters/ai-elements-adapter'
type Scope = { source: NativeFileSource | null; api: FilePreviewApi | null; theme: ResolvedTheme; visible: boolean; local?: boolean; isCurrent?(): boolean; open: ((path: string, line?: number | null) => Promise<void>) | null }
export const NativeMarkdownScope = createContext<Scope | null>(null)
export function NativeMarkdownProvider({ value, children }: { value: Scope; children: ReactNode }): React.JSX.Element {
  const latest = useRef<Scope | null>(value)
  useLayoutEffect(() => { latest.current = value; return () => { latest.current = null } }, [value])
  const bound = useMemo(() => ({ ...value, isCurrent: () => latest.current === value && value.visible }), [value])
  return <NativeMarkdownScope.Provider value={bound}>{children}</NativeMarkdownScope.Provider>
}
function InlinePicture({ image, alt, linked }: { image: UserImageDisplay; alt: string; linked?: boolean }): React.JSX.Element {
  const [preview, setPreview] = useState(false), { canCopy, copy, download } = useImageActions(), t = useUiText()
  const src = `data:${image.mime_type};base64,${image.data}`
  return <><ImageActions image={image} as="span" className="inline-block max-w-full align-middle">
    {linked ? <NativeImage src={src} alt={alt || image.name} className="h-auto max-h-96 max-w-full object-contain" /> : <button type="button" className="inline-block max-w-full cursor-zoom-in" aria-label={alt || image.name} onClick={() => setPreview(true)}>
      <NativeImage src={src} alt={alt || image.name} className="h-auto max-h-96 max-w-full object-contain" />
    </button>}
  </ImageActions><ImagePreviewDialog src={src} alt={alt || image.name} open={preview} onOpenChange={setPreview}
    onCopy={canCopy ? () => void copy(image) : undefined} onDownload={() => void download(image)} copyLabel={t('复制图片')} downloadLabel={t('保存图片…')}
    renderImage={picture => <ImageActions image={image}>{picture}</ImageActions>} /></>
}
export function InlineImage({ reference, alt, linked }: { reference: string; alt: string; linked?: boolean }): React.JSX.Element {
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
  return <span ref={root} className="native-markdown-image">{current?.image ? <InlinePicture image={current.image} alt={alt} linked={linked} />
    : <span role={current ? 'img' : 'status'} aria-label={alt || reference}>{alt || reference}{current ? t('（暂不可读）') : ''}</span>}</span>
}
