import { useContext, useEffect, useRef, useState, type ComponentProps } from 'react'
import * as Primitive from '@radix-ui/react-context-menu'
import { parseFileReference } from '../../../../file-preview-reference'
import { useUiText } from '../../interface-language'
import { NativeMarkdownScope } from './native-markdown-context'
import { NativeImageScope } from './native-image-actions'
import { inspectNativeFileActions, performNativeFileAction, type NativeFileAction, type NativeFileActionMetadata } from './native-reference-operations'
import { copyTextToClipboard } from './source-clipboard'
import { useTranslations } from './source-primitives'

function Root(props: ComponentProps<typeof Primitive.Root>): React.JSX.Element {
  const scope = useContext(NativeImageScope), owner = useRef({ scope, key: 0 })
  if (owner.current.scope !== scope) owner.current = { scope, key: owner.current.key + 1 }
  return <Primitive.Root key={owner.current.key} {...props} />
}
function Portal({ children, ...props }: ComponentProps<typeof Primitive.Portal>): React.JSX.Element | null {
  const scope = useContext(NativeImageScope)
  return scope.visible ? <Primitive.Portal {...props}><div className="codeg-native-rich native-reference-overlay">{children}</div></Primitive.Portal> : null
}
function Trigger(props: ComponentProps<typeof Primitive.Trigger>): React.JSX.Element {
  const scope = useContext(NativeImageScope)
  return <Primitive.Trigger {...props} disabled={props.disabled || !scope.visible} />
}
export const NativeContextMenu = { ...Primitive, Root, Portal, Trigger }
export function parseLocalFileTarget(target: string): { path: string; line?: number } | null {
  const parsed = parseFileReference(target)
  return parsed ? { path: parsed.pathPart, ...(parsed.target?.line ? { line: parsed.target.line } : {}) } : null
}
export function useNativeFileHost(): { local: boolean; download: boolean; loadingLabel: string; retryLabel: string } {
  const scope = useContext(NativeMarkdownScope), t = useUiText()
  return { local: Boolean(scope?.local), download: typeof scope?.api?.download === 'function', loadingLabel: t('正在读取原文件…'), retryLabel: t('重试') }
}
export function useNativeFileActions(target: string) {
  const scope = useContext(NativeMarkdownScope), imageScope = useContext(NativeImageScope), t = useTranslations('Folder.chat.fileActions')
  const [attempt, setAttempt] = useState(0)
  const [state, setState] = useState<{ scope: typeof scope; target: string; metadata?: NativeFileActionMetadata; error: boolean } | null>(null)
  const current = () => Boolean(scope?.visible && scope.isCurrent?.())
  const owner = scope?.source && scope.api ? { source: scope.source, api: scope.api, current } : null
  useEffect(() => {
    let active = true
    setState(null)
    if (!owner) { setState({ scope, target, error: true }); return }
    void inspectNativeFileActions(owner, target).then(metadata => {
      if (active && current()) setState({ scope, target, metadata, error: false })
    }).catch(() => { if (active && current()) setState({ scope, target, error: true }) })
    return () => { active = false }
  }, [scope, target, attempt])
  const value = state?.scope === scope && state.target === target ? state : null
  const metadata = value?.metadata
  const paths = metadata ? { relative: metadata.pathPresentation !== 'file_name_only' && metadata.sourceRelativePath ? 'relative' : null,
    absolute: metadata.pathPresentation !== 'file_name_only' && (scope?.local || metadata.absolutePath) ? 'absolute' : null } : null
  const run = (action: NativeFileAction): void => {
    if (!owner || !current()) return
    void performNativeFileAction(owner, target, action, copyTextToClipboard).then(() => {
      if (current() && (action === 'relative' || action === 'absolute')) imageScope.notice(t('pathCopied'))
    }).catch(() => {
      if (current()) imageScope.notice(t(action === 'reveal' ? 'openFailed' : action === 'download' ? 'downloadFailed' : 'copyPathFailed', { name: metadata?.fileName ?? target }))
    })
  }
  return { paths, downloadAvailable: Boolean(metadata?.capabilities.includes('download')), loading: !value, error: Boolean(value?.error), retry: () => setAttempt(value => value + 1),
    reveal: () => run('reveal'), copy: (value: string | null) => { if (value === 'relative' || value === 'absolute') run(value) }, download: () => run('download') }
}
