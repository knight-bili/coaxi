// Original Rovai file/web link routing and native Core image grants own this seam.
import { useContext, type ComponentProps } from 'react'
import type { Components } from 'streamdown'
import { FileReferenceLink } from '../../FileReferenceLink'
import { parseFileReference } from '../../../../file-preview-reference'
import { NativeMarkdownScope, InlineImage } from './native-markdown-context'
import { NativeImageScope } from './native-image-actions'
import { useUiText } from '../../interface-language'

export function MarkdownLink({ node: _node, href, children, className }: ComponentProps<'a'> & { node?: unknown }): React.JSX.Element {
  const scope = useContext(NativeMarkdownScope), { notice } = useContext(NativeImageScope), t = useUiText()
  const parsed = href ? parseFileReference(href) : null
  if (href && parsed && scope?.open) return <FileReferenceLink rawReference={href} className={['markdown-file-reference', className].filter(Boolean).join(' ')}
    onActivate={reference => { void scope.open!(reference, parsed.target?.line).catch(() => notice(t('暂时无法打开这份文件。'))) }}>{children}</FileReferenceLink>
  if (href?.startsWith('#')) return <a href={href} onClick={event => {
    event.preventDefault()
    let id: string; try { id = decodeURIComponent(href.slice(1)) } catch { return }
    const root = event.currentTarget.closest('.native-session-transcript')
    const target = Array.from(root?.querySelectorAll<HTMLElement>('[id]') ?? []).find(element => element.id === id)
    target?.scrollIntoView({ block: 'start' })
  }}>{children}</a>
  if (href?.startsWith('https://')) return <a className={['markdown-web-reference', className].filter(Boolean).join(' ')} href={href} target="_blank" rel="noreferrer noopener">{children}</a>
  return <code className="markdown-inert-link">{children}</code>
}
function LocalImageSpan({ node: _node, children, ...props }: ComponentProps<'span'> & { node?: unknown; 'data-codeg-local-image'?: string; 'data-codeg-image-linked'?: string }): React.JSX.Element {
  const source = props['data-codeg-local-image']
  return source ? <InlineImage reference={source} alt={typeof children === 'string' ? children : ''} linked={props['data-codeg-image-linked'] === 'true'} /> : <span {...props}>{children}</span>
}
export const markdownLinkComponents: Components = { a: MarkdownLink as Components['a'] }
export const markdownLocalImageComponents: Components = { span: LocalImageSpan as Components['span'] }
