import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, type ComponentProps } from 'react'

/** Web srcDoc inherits the App's script CSP. Its existing preview entry provides
 * a bootstrap frame; the actual Codeg document keeps its original opaque sandbox. */
export const NativeHtmlFrame = forwardRef<HTMLIFrameElement, ComponentProps<'iframe'> & { shellUrl?: string }>(function NativeHtmlFrame(
  { shellUrl, srcDoc, sandbox, ...props }, forwarded
) {
  const ref = useRef<HTMLIFrameElement>(null)
  const id = useMemo(() => crypto.randomUUID(), [srcDoc, sandbox, shellUrl])
  useImperativeHandle(forwarded, () => ref.current!, [id])
  useEffect(() => {
    if (!shellUrl || !srcDoc) return
    let initialized = false
    const receive = (event: MessageEvent) => {
      const data = event.data
      if (event.source !== ref.current?.contentWindow || event.origin !== 'null' || !data || typeof data !== 'object'
        || data.protocol !== 'rovai-codeg-preview-v1' || data.id !== id || data.type !== 'ready' || initialized) return
      initialized = true
      ref.current?.contentWindow?.postMessage({ protocol: 'rovai-codeg-preview-v1', id, type: 'initialize', html: srcDoc,
        scripts: sandbox === 'allow-scripts' }, '*')
    }
    window.addEventListener('message', receive)
    return () => window.removeEventListener('message', receive)
  }, [id, shellUrl, srcDoc, sandbox])
  if (!shellUrl) return <iframe key={id} {...props} ref={ref} srcDoc={srcDoc} sandbox={sandbox} />
  return <iframe key={id} {...props} ref={ref} src={`${shellUrl}#codeg.${id}`} sandbox="allow-scripts" onLoad={event => {
    props.onLoad?.(event)
    ref.current?.contentWindow?.postMessage({ protocol: 'rovai-codeg-preview-v1', id, type: 'connect' }, '*')
  }} />
})
