import { useContext, useMemo, useRef, type ComponentProps } from 'react'
import { useHtmlPreviewPresentation } from '../../FilePreviewContext'
import { useInterfaceLanguage, useUiText } from '../../interface-language'
import { NativeMarkdownScope } from './native-markdown-context'
import { NativeImageScope } from './native-image-actions'
import { withNativeHtmlDocument, type NativeHtmlDocument } from './native-html-source'

const themeProperties = { background:'--surface',foreground:'--ink',card:'--surface','card-foreground':'--ink',popover:'--surface','popover-foreground':'--ink',
  primary:'--brand','primary-foreground':'--brand-contrast',secondary:'--surface-hover','secondary-foreground':'--ink',muted:'--surface-hover','muted-foreground':'--muted',
  accent:'--surface-hover','accent-foreground':'--ink',destructive:'--danger',border:'--line',input:'--line',ring:'--brand' }

export function useNativeHtmlHost() {
  const scope = useContext(NativeMarkdownScope), ref = useRef<HTMLElement>(null), t = useUiText(), language = useInterfaceLanguage()
  const { presented, limited } = useHtmlPreviewPresentation(ref)
  const identity = useRef({ api: scope?.api, key: JSON.stringify(scope?.source), revision: 0 })
  const key = JSON.stringify(scope?.source)
  if (identity.current.api !== scope?.api || identity.current.key !== key) identity.current = { api: scope?.api, key, revision: identity.current.revision + 1 }
  return useMemo(() => ({ ref, themeProperties, targetKey: `${key}:${identity.current.revision}`, available: Boolean(scope?.visible && scope.source && scope.api && presented),
    shellUrl: scope?.api?.nativeHtmlFrameUrl,
    limited, limitedLabel: t('页面预览已达窗口上限，请先收起其他页面预览。'),
    theme: scope?.theme === 'night' ? 'dark' : 'light',
    async read<T>(path: string, maximum: number, use: (document: NativeHtmlDocument) => Promise<T>): Promise<T> {
      if (!scope?.source || !scope.api || !scope.isCurrent?.()) throw new Error(t('当前页面暂时无法读取这份文件。'))
      return withNativeHtmlDocument({ source: scope.source, api: scope.api, current: () => Boolean(scope.isCurrent?.()) }, path, maximum, use)
    }
  }), [scope, presented, limited, key, identity.current.revision, language])
}
export function NativeHtmlFileLink({ filePath, children, ...props }: ComponentProps<'button'> & { filePath: string }): React.JSX.Element {
  const scope = useContext(NativeMarkdownScope), image = useContext(NativeImageScope), t = useUiText()
  return <button {...props} type="button" onClick={event => {
    event.preventDefault(); event.stopPropagation()
    if (scope?.open && scope.isCurrent?.()) void scope.open(filePath).catch(() => { if (scope.isCurrent?.()) image.notice(t('暂时无法打开这份文件。')) })
  }}>{children}</button>
}
