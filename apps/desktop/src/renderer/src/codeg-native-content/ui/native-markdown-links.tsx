// Original Rovai file/web link routing and native Core image grants own this seam.
import { useContext, type ComponentProps } from 'react'
import type { Components } from 'streamdown'
import { ReferenceBadge } from '../components/chat/composer/badges/reference-badge'
import { parseCodegReferenceUri } from '../components/chat/composer/reference-uri'
import { FileReferenceActions } from '../components/message/file-reference-actions'
import { classifyResourceKind } from '../lib/resource-kind'
import { parseFileReference } from '../../../../file-preview-reference'
import { NativeMarkdownScope, InlineImage } from './native-markdown-context'
import { NativeImageScope } from './native-image-actions'
import { useUiText } from '../../interface-language'
import { activateNativeFileReference } from './native-reference-operations'

export function MarkdownLink({ node: _node, href, children, className }: ComponentProps<'a'> & { node?: unknown }): React.JSX.Element {
  const scope = useContext(NativeMarkdownScope), { notice } = useContext(NativeImageScope), t = useUiText()
  const parsed = href ? parseFileReference(href) : null
  const label = typeof children === 'string' ? children : Array.isArray(children) ? children.map(child => typeof child === 'string' ? child : '').join('') : ''
  if (href?.toLowerCase().startsWith('codeg:')) {
    const reference = parseCodegReferenceUri(href, label)
    if (reference) return <ReferenceBadge data={reference} />
  }
  if (href && classifyResourceKind(href) === 'file' && parsed && scope?.open) {
    const displayPath = `${parsed.pathPart}${parsed.target?.line ? `:${parsed.target.line}` : ''}`
    return <FileReferenceActions key={href} target={href}>
      <button type="button" data-resource-kind="file" title={displayPath}
        onClick={event => { event.preventDefault(); if (scope.source && scope.api && scope.isCurrent?.()) void activateNativeFileReference(
          { source: scope.source, api: scope.api, current: () => Boolean(scope.isCurrent?.()) }, href,
          () => scope.open!(href, parsed.target?.line)
        ).catch(() => { if (scope.isCurrent?.()) notice(t('暂时无法打开这份文件。')) }) }}
        className="inline-flex max-w-full -translate-y-[1.5px] cursor-pointer appearance-none items-center align-middle leading-none hover:opacity-80">
        <ReferenceBadge data={{ refType: 'file', id: href, label: label || href, uri: displayPath, meta: { fileKind: 'file' } }} />
      </button>
    </FileReferenceActions>
  }
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
