import { useState, type ImgHTMLAttributes } from 'react'
import { useUiText } from '../../interface-language'

/** Next's image optimization is not a transport. Only snapshot-owned bytes render. */
export function nativeImageSource(image: { mime_type: string; data: string }): string | null {
  return /^image\/(?:png|jpeg|gif|webp|avif|bmp|svg\+xml)$/i.test(image.mime_type) && image.data.length > 0 && /^[A-Za-z0-9+/]*={0,2}$/.test(image.data)
    ? `data:${image.mime_type};base64,${image.data}` : null
}
export default function NativeImage({ unoptimized: _unoptimized, src, ...props }: ImgHTMLAttributes<HTMLImageElement> & { unoptimized?: boolean }): React.JSX.Element {
  const t = useUiText(), [failedSource, setFailedSource] = useState<string | null>(null)
  const valid = typeof src === 'string' && /^data:image\/(?:png|jpeg|gif|webp|avif|bmp|svg\+xml);base64,[A-Za-z0-9+/]+={0,2}$/i.test(src)
  return valid && failedSource !== src ? <img {...props} src={src} loading="lazy" onError={event => { setFailedSource(src); props.onError?.(event) }} />
    : <span role="img" aria-label={props.alt}>{t('该图片暂不可读。')}</span>
}
