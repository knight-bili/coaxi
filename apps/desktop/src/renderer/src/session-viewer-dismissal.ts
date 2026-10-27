// Coaxi drawer.tsx and the existing fused DelegationViewer: share the same
// Radix/Base UI Escape sample across both kinds of nested session viewer.
import { useEffect } from 'react'
import type { Drawer } from '@base-ui/react/drawer'

let escapeProbeUsers = 0
let lastEscape: { event: object; shielded: boolean } | null = null
const blockingOverlayOpen = (): boolean => document.body.style.pointerEvents === 'none'
  || document.querySelector('.native-tooltip-overlay [data-slot="tooltip-content"]') !== null
export function shieldSessionViewerEscape(event: object): void { lastEscape = { event, shielded: true } }
function sampleEscapeShield(event: KeyboardEvent): void {
  if (event.key === 'Escape' && lastEscape?.event !== event) lastEscape = { event, shielded: blockingOverlayOpen() }
}
export function useEscapeShield(): void {
  useEffect(() => {
    if (escapeProbeUsers++ === 0) document.addEventListener('keydown', sampleEscapeShield, true)
    return () => { if (--escapeProbeUsers === 0) { document.removeEventListener('keydown', sampleEscapeShield, true); lastEscape = null } }
  }, [])
}
export function shieldedDismissal(details: Drawer.Root.ChangeEventDetails): boolean {
  if (!['outside-press', 'escape-key', 'focus-out'].includes(details.reason)) return false
  return details.reason === 'escape-key' && lastEscape?.event === details.event ? lastEscape.shielded : blockingOverlayOpen()
}
