// Codeg aff1e6c UI seams for readonly native transcripts, Apache-2.0.
// Card JSX stays upstream. Core ownership, Rovai Markdown and native scrolling
// stay here; this module never calls the donor's store, Tauri or model tools.
import { type ComponentProps, type ElementType, type ReactNode } from 'react'
import { SafeMarkdown } from '../../SafeMarkdown'
import { useInterfaceLanguage } from '../../interface-language'
import { twMerge } from 'tailwind-merge'
import { formatSourceCopy } from './source-translations'
export { Collapsible, CollapsibleTrigger, CollapsibleContent } from '../components/ui/instant-collapsible'
export { copyTextToClipboard } from './source-clipboard'

export function cn(...values: Array<string | false | null | undefined>): string { return twMerge(values.filter(Boolean).join(' ')) }
export function useTranslations(scope: string): (key: string, values?: Record<string, unknown>) => string {
  const language = useInterfaceLanguage()
  return (key, values) => formatSourceCopy(language, scope, key, values)
}
export function useLocale(): string { return useInterfaceLanguage() }
// Source streaming flags are presentation hints. Rovai's safe Markdown renderer
// receives each updated body; these flags never launch a Runtime or execute HTML.
export function MessageResponse({ children }: { children: string; mode?: 'streaming' | 'static'; parseIncompleteMarkdown?: boolean }): React.JSX.Element { return <SafeMarkdown>{children}</SafeMarkdown> }
export function ScrollArea({ children, className, x, y, onViewportRef, ...props }: ComponentProps<'div'> & { x?: 'scroll' | 'hidden'; y?: 'scroll' | 'hidden'; onViewportRef?: React.Ref<HTMLDivElement> }): React.JSX.Element {
  return <div {...props} ref={onViewportRef} className={cn('codeg-source-scroll', className)} style={{ ...props.style, ...(x ? { overflowX: x === 'hidden' ? 'hidden' : 'auto' } : {}), ...(y ? { overflowY: y === 'hidden' ? 'hidden' : 'auto' } : {}) }}>{children}</div>
}
// Original Badge's outline/secondary variants used by PlanCard. No link or
// mutable badge action exists in this readonly surface.
export function Badge({ variant = 'secondary', className, ...props }: ComponentProps<'span'> & { variant?: 'outline' | 'secondary' }): React.JSX.Element {
  return <span {...props} data-slot="badge" data-variant={variant} className={cn('h-5 gap-1 rounded-4xl border border-transparent px-2 py-0.5 text-xs font-medium transition-all has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&>svg]:size-3! inline-flex items-center justify-center w-fit whitespace-nowrap shrink-0 [&>svg]:pointer-events-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive overflow-hidden group/badge', variant === 'outline' ? 'border-border text-foreground bg-input/30' : 'bg-secondary text-secondary-foreground', className)} />
}
export function Shimmer({ children, as: Tag = 'p', className, duration = 2, shineColor = 'var(--color-background)' }: { children: string; as?: ElementType; duration?: number; shineColor?: string; className?: string }): React.JSX.Element {
  return <Tag className={cn('codeg-source-shimmer', className)} style={{ '--spread': `${children.length * 2}px`, '--shine': shineColor, animationDuration: `${duration}s` }}>{children}</Tag>
}
