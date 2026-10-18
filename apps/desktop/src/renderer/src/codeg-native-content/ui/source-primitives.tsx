// Codeg aff1e6c UI seams for readonly native transcripts, Apache-2.0.
// Card JSX stays upstream. Core ownership, Rovai Markdown and native scrolling
// stay here; this module never calls the donor's store, Tauri or model tools.
import { createContext, useContext, type ElementType, type ReactNode } from 'react'
import { SafeMarkdown } from '../../SafeMarkdown'
import { useInterfaceLanguage } from '../../interface-language'
import { twMerge } from 'tailwind-merge'
import { cx } from 'class-variance-authority'
import { formatSourceCopy } from './source-translations'
export { Collapsible, CollapsibleTrigger, CollapsibleContent } from '../components/ui/instant-collapsible'
export { copyTextToClipboard } from './source-clipboard'
export { Badge } from '../components/ui/badge'
export { ScrollArea } from '../components/ui/scroll-area'
export const SourceMarkdownRenderer = createContext<((text: string, streaming: boolean) => ReactNode) | null>(null)

export function cn(...values: Parameters<typeof cx>): string { return twMerge(cx(...values)) }
export function useTranslations(scope: string): (key: string, values?: Record<string, unknown>) => string {
  const language = useInterfaceLanguage()
  return (key, values) => formatSourceCopy(language, scope, key, values)
}
export function useLocale(): string { return useInterfaceLanguage() }
// Source streaming flags are presentation hints. Rovai's safe Markdown renderer
// receives each updated body; these flags never launch a Runtime or execute HTML.
export function MessageResponse({ children, mode }: { children: string; mode?: 'streaming' | 'static'; parseIncompleteMarkdown?: boolean }): React.JSX.Element {
  const render = useContext(SourceMarkdownRenderer)
  return <>{render ? render(children, mode === 'streaming') : <SafeMarkdown>{children}</SafeMarkdown>}</>
}
export function Shimmer({ children, as: Tag = 'p', className, duration = 2, shineColor = 'var(--color-background)' }: { children: string; as?: ElementType; duration?: number; shineColor?: string; className?: string }): React.JSX.Element {
  return <Tag className={cn('codeg-source-shimmer', className)} style={{ '--spread': `${children.length * 2}px`, '--shine': shineColor, animationDuration: `${duration}s` }}>{children}</Tag>
}
