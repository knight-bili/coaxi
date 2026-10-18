// Codeg aff1e6c UI seams for readonly native transcripts, Apache-2.0.
// Card JSX stays upstream. Core ownership, Rovai Markdown and native scrolling
// stay here; this module never calls the donor's store, Tauri or model tools.
import { createContext, useContext, useId, type ComponentProps, type ElementType, type ReactNode } from 'react'
import { SafeMarkdown } from '../../SafeMarkdown'
import { useInterfaceLanguage } from '../../interface-language'
import copy from './source-copy.json'
import { twMerge } from 'tailwind-merge'

export function cn(...values: Array<string | false | null | undefined>): string { return twMerge(values.filter(Boolean).join(' ')) }
export function useTranslations(scope: string): (key: string) => string {
  const language = useInterfaceLanguage()
  return key => {
    const catalog: unknown = copy[language === 'en' ? 'en' : 'zh-CN']
    // Source uses both a parent scope and its goal sub-scope.
    const prefix = scope.endsWith('.goal') ? 'goal.' : ''
    const base = scope.endsWith('.goal') ? scope.slice(0, -5) : scope
    let value: unknown = (catalog as Record<string, unknown>)[base]
    for (const part of (prefix + key).split('.')) value = value && typeof value === 'object' ? (value as Record<string, unknown>)[part] : undefined
    return typeof value === 'string' ? value : key
  }
}
export function MessageResponse({ children }: { children: string }): React.JSX.Element { return <SafeMarkdown>{children}</SafeMarkdown> }
export function ScrollArea({ children, className }: { children: ReactNode; className?: string }): React.JSX.Element {
  return <div className={cn('codeg-source-scroll', className)}>{children}</div>
}
// Original Badge's outline/secondary variants used by PlanCard. No link or
// mutable badge action exists in this readonly surface.
export function Badge({ variant = 'secondary', className, ...props }: ComponentProps<'span'> & { variant?: 'outline' | 'secondary' }): React.JSX.Element {
  return <span {...props} data-slot="badge" data-variant={variant} className={cn('h-5 gap-1 rounded-4xl border border-transparent px-2 py-0.5 text-xs font-medium inline-flex items-center justify-center w-fit whitespace-nowrap shrink-0 overflow-hidden', variant === 'outline' ? 'border-border text-foreground bg-input/30' : 'bg-secondary text-secondary-foreground', className)} />
}
const Disclosure = createContext<{ open: boolean; id: string; change(open: boolean): void } | null>(null)
export function Collapsible({ open = false, onOpenChange, children, ...props }: Omit<ComponentProps<'div'>, 'onChange'> & { open?: boolean; onOpenChange?(open: boolean): void }): React.JSX.Element {
  const id = useId()
  return <Disclosure.Provider value={{ open, id, change: next => onOpenChange?.(next) }}><div {...props} data-state={open ? 'open' : 'closed'}>{children}</div></Disclosure.Provider>
}
export function CollapsibleTrigger({ onClick, ...props }: ComponentProps<'button'>): React.JSX.Element {
  const context = useContext(Disclosure)!
  return <button {...props} type="button" data-slot="collapsible-trigger" data-state={context.open ? 'open' : 'closed'} aria-controls={context.id} aria-expanded={context.open} onClick={event => { onClick?.(event); if (!event.defaultPrevented) context.change(!context.open) }} />
}
export function CollapsibleContent(props: ComponentProps<'div'>): React.JSX.Element | null {
  const context = useContext(Disclosure)!
  return context.open ? <div {...props} id={context.id} data-slot="collapsible-content" data-state="open" /> : null
}
export function Shimmer({ children, as: Tag = 'p', className, duration = 2, shineColor = 'var(--color-background)' }: { children: string; as?: ElementType; duration?: number; shineColor?: string; className?: string }): React.JSX.Element {
  return <Tag className={cn('codeg-source-shimmer', className)} style={{ '--spread': `${children.length * 2}px`, '--shine': shineColor, animationDuration: `${duration}s` }}>{children}</Tag>
}
