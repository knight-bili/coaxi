// Original Coaxi tooltip markup/styles with the existing Coaxi Base UI primitive.
import { Tooltip as Primitive } from '@base-ui/react/tooltip'
import { useContext, useEffect, useState, type ComponentProps, type ReactElement } from 'react'
import { cn } from './source-primitives'
import { NativeImageScope } from './native-image-actions'
import { shieldSessionViewerEscape } from '../../session-viewer-dismissal'
export function TooltipProvider({ delayDuration = 0, children }: { delayDuration?: number; children: React.ReactNode }): React.JSX.Element {
  return <Primitive.Provider delay={delayDuration}>{children}</Primitive.Provider>
}
export function Tooltip(props: ComponentProps<typeof Primitive.Root>): React.JSX.Element {
  const { visible } = useContext(NativeImageScope), [open, setOpen] = useState(props.defaultOpen ?? false)
  useEffect(() => { if (!visible) setOpen(false) }, [visible])
  return <Primitive.Root {...props} open={visible && (props.open ?? open)} onOpenChange={(next, details) => {
    if (!next && details.reason === 'escape-key') shieldSessionViewerEscape(details.event)
    setOpen(next); props.onOpenChange?.(next, details)
  }} />
}
export function TooltipTrigger({ asChild: _asChild, children }: { asChild?: boolean; children: ReactElement }): React.JSX.Element {
  return <Primitive.Trigger render={children} />
}
export function TooltipContent({ className, side = 'top', children }: ComponentProps<'div'> & { side?: 'top' | 'bottom' | 'left' | 'right' }): React.JSX.Element {
  return <Primitive.Portal><div className="coaxi-native-rich native-tooltip-overlay"><Primitive.Positioner side={side} sideOffset={4}>
    <Primitive.Popup data-slot="tooltip-content" className={cn('rounded-2xl px-3 py-1.5 text-xs bg-foreground text-background w-fit max-w-xs', className)}>{children}</Primitive.Popup>
  </Primitive.Positioner></div></Primitive.Portal>
}
