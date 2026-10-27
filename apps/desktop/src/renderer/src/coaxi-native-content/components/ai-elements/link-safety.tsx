// Fixed Codeg aff1e6c FilePathLink button JSX; Apache-2.0.
// Native file authority and errors use Core/Coaxi, never the source app store.
import { useCallback, useRef, useState, type ReactNode } from 'react'
import { cn } from '../../ui/source-primitives'
import { useNativeFileAction } from '../../ui/native-file-actions'
import { useUiText } from '../../../interface-language'

export function FilePathLink({ filePath, line, className, title, children }: { filePath: string; line?: number | null; className?: string; title?: string; children: ReactNode }): React.JSX.Element {
  const openFileTarget = useNativeFileAction()
  const t = useUiText(), [failed, setFailed] = useState(false)
  const [opening, setOpening] = useState(false), openingRef = useRef(false)
  const handleOpen = useCallback(() => {
    if (!openFileTarget || openingRef.current) return
    openingRef.current = true; setOpening(true)
    setFailed(false)
    void openFileTarget(filePath, line).catch(() => setFailed(true))
      .finally(() => { openingRef.current = false; setOpening(false) })
  }, [filePath, line, openFileTarget])
  return <span className={cn('block min-w-0', className)}><button type="button" title={openFileTarget ? title ?? filePath : t('文件访问已失效')} aria-busy={opening} disabled={opening || !openFileTarget}
    className="max-w-full cursor-pointer truncate text-left align-bottom hover:underline focus-visible:underline focus-visible:outline-none disabled:cursor-wait disabled:opacity-70 disabled:hover:no-underline"
    onClick={event => { event.stopPropagation(); handleOpen() }}>{children}</button>
    {failed && <span className="block whitespace-normal font-sans text-destructive text-xs" role="alert">{t('暂时无法打开文件，请刷新会话后重试。')}</span>}</span>
}
