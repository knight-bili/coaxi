import { useEffect, useRef, useState } from 'react'

/** Retain the same drawer for its exit animation, including rapid reversals. */
export function useDrawerPresence<T>(value: T | null): { value: T | null; closing: boolean } {
  const lastValue = useRef(value)
  if (value !== null) lastValue.current = value
  const open = value !== null
  const [present, setPresent] = useState(open)
  useEffect(() => {
    if (open) { setPresent(true); return }
    if (!present) return
    const media = window.matchMedia?.('(prefers-reduced-motion: reduce)')
    if (media?.matches) { setPresent(false); return }
    const changed = (): void => { if (media?.matches) setPresent(false) }
    media?.addEventListener?.('change', changed)
    const timer = window.setTimeout(() => setPresent(false), 200)
    return () => { window.clearTimeout(timer); media?.removeEventListener?.('change', changed) }
  }, [open, present])
  return { value: value ?? (present ? lastValue.current : null), closing: !open }
}

/** Navigation removes a mission drawer; finish its exit only after leave guards succeed. */
export async function leaveDrawer(element: HTMLElement | null, commit: () => void | Promise<void>): Promise<void> {
  if (!element?.animate || element.inert || window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) { await commit(); return }
  const wasInert = element.inert
  element.inert = true
  const animation = element.animate([{ opacity: 1, transform: 'translateX(0)' }, { opacity: 0, transform: 'translateX(16px)' }],
    { duration: 200, easing: 'cubic-bezier(.22, 1, .36, 1)', fill: 'forwards' })
  try { await animation.finished.catch(() => undefined); await commit() }
  finally { animation.cancel(); element.inert = wasInert }
}
