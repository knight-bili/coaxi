// Browser/Electron host bindings for the fixed source renderer; no donor store or Tauri.
import { useContext, useEffect, useRef, useState, type ComponentProps } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import * as Menu from '@radix-ui/react-dropdown-menu'
import { NativeMarkdownScope } from './native-markdown-context'
import { NativeImageScope } from './native-image-actions'
export { useChatAnimationsEnabled } from './native-turn-actions'
export function useAppearance(): { isDarkMode: boolean } { return { isDarkMode: useContext(NativeMarkdownScope)?.theme === 'night' } }
const announce = (message: string) => { window.dispatchEvent(new CustomEvent('native-content-notice', { detail: message })) }
export const toast = { success: announce, error: announce }
// The host has DOM iframes, not Coaxi's separately painted native webviews.
// DOM modal/portal stacking handles occlusion here; native-browser leases stay I07-owned.
export function acquireNativeSurfaceOcclusionFor(_kind: string, _active: boolean): () => void { return () => undefined }
function Root({ open, defaultOpen, onOpenChange, ...props }: ComponentProps<typeof Dialog.Root>): React.JSX.Element {
  const scope = useContext(NativeImageScope), owner = useRef(scope), [localOpen, setOpen] = useState(defaultOpen ?? false)
  const requested = open ?? localOpen, current = owner.current === scope
  useEffect(() => { if (owner.current !== scope || !scope.visible) { setOpen(false); if (requested) onOpenChange?.(false) }; owner.current = scope }, [scope, requested, onOpenChange])
  return <Dialog.Root {...props} open={scope.visible && current && requested} onOpenChange={value => { setOpen(value); onOpenChange?.(value) }} />
}
function DialogPortal({ children, ...props }: ComponentProps<typeof Dialog.Portal>): React.JSX.Element {
  return <Dialog.Portal {...props}><div className="coaxi-native-rich native-content-overlay">{children}</div></Dialog.Portal>
}
function MenuPortal({ children, ...props }: ComponentProps<typeof Menu.Portal>): React.JSX.Element {
  return <Menu.Portal {...props}><div className="coaxi-native-rich native-content-menu">{children}</div></Menu.Portal>
}
function MenuRoot({ open, defaultOpen, onOpenChange, ...props }: ComponentProps<typeof Menu.Root>): React.JSX.Element {
  const scope = useContext(NativeImageScope), owner = useRef(scope), [localOpen, setOpen] = useState(defaultOpen ?? false)
  const requested = open ?? localOpen, current = owner.current === scope
  useEffect(() => { if (owner.current !== scope || !scope.visible) { setOpen(false); if (requested) onOpenChange?.(false) }; owner.current = scope }, [scope, requested, onOpenChange])
  return <Menu.Root {...props} open={scope.visible && current && requested} onOpenChange={value => { setOpen(value); onOpenChange?.(value) }} />
}
export const NativeContentDialog = { ...Dialog, Root, Portal: DialogPortal }
export const NativeContentMenu = { ...Menu, Root: MenuRoot, Portal: MenuPortal }
export type SaveFileResult = 'saved' | 'cancelled'
function saveBlob(blob: Blob, name: string): void {
  const url = URL.createObjectURL(blob), anchor = document.createElement('a')
  anchor.href = url; anchor.download = name.replace(/[\\/:*?"<>|\u0000-\u001f]/g, '_')
  document.body.append(anchor); anchor.click(); anchor.remove(); setTimeout(() => URL.revokeObjectURL(url), 0)
}
export async function saveTextFile({ content, suggestedName, mimeType }: { content: string; suggestedName: string; mimeType: string; filterName: string; ext: string }): Promise<SaveFileResult> {
  saveBlob(new Blob([content], { type: mimeType }), suggestedName); return 'saved'
}
export async function downloadImage({ data, mime_type, suggestedName }: { data: string; mime_type: string; suggestedName: string }): Promise<boolean> {
  const bytes = Uint8Array.from(atob(data), char => char.charCodeAt(0))
  saveBlob(new Blob([bytes], { type: mime_type }), suggestedName); return true
}
