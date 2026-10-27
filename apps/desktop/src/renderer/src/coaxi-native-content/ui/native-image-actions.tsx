// Fixed Coaxi image-actions semantics; original Coaxi clipboard/menu own the host.
import { createContext, useContext, useEffect, useRef, useState, type ComponentProps, type ReactNode } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { ImageContextMenu, type ImageMenuPosition } from '../../ImageContextMenu'
import { writeClipboardImage } from '../../clipboard'
import { useUiText } from '../../interface-language'
import { nativeImageSource } from './native-image'
import type { UserImageDisplay } from '../lib/adapters/ai-elements-adapter'

export const NativeImageScope = createContext<{ visible: boolean; notice(message: string): void }>({ visible: true, notice() {} })
function Root({ open, onOpenChange, ...props }: ComponentProps<typeof Dialog.Root>): React.JSX.Element {
  const scope = useContext(NativeImageScope), owner = useRef(scope)
  const current = owner.current === scope
  useEffect(() => {
    if ((!scope.visible || owner.current !== scope) && open) onOpenChange?.(false)
    owner.current = scope
  }, [scope, open, onOpenChange])
  return <Dialog.Root {...props} open={Boolean(open && scope.visible && current)} onOpenChange={onOpenChange} />
}
function Portal({ children, ...props }: ComponentProps<typeof Dialog.Portal>): React.JSX.Element {
  return <Dialog.Portal {...props}><div className="coaxi-native-rich native-image-overlay">{children}</div></Dialog.Portal>
}
export const ImageDialog = { ...Dialog, Root, Portal }
export function useImageActions(): { canCopy: boolean; copy(image: UserImageDisplay): Promise<void>; download(image: UserImageDisplay): Promise<void> } {
  const scope = useContext(NativeImageScope), t = useUiText()
  const owner = useRef(scope), active = useRef(true); owner.current = scope
  useEffect(() => { active.current = true; return () => { active.current = false } }, [])
  const current = () => active.current && owner.current === scope && scope.visible
  const canCopy = typeof navigator !== 'undefined' && typeof navigator.clipboard?.write === 'function' && typeof ClipboardItem !== 'undefined'
  return { canCopy,
    async copy(image) {
      if (!current()) return
      const src = nativeImageSource(image)
      try {
        if (!src) throw new Error('image_unavailable')
        const element = new Image(); element.src = src; await element.decode()
        if (!current()) return
        await writeClipboardImage(element); if (current()) scope.notice(t('已复制图片'))
      } catch { if (current()) scope.notice(t('未能复制图片，请重试或保存图片。')) }
    },
    async download(image) {
      if (!current()) return
      const src = nativeImageSource(image)
      if (!src) { scope.notice(t('该图片暂不可读。')); return }
      const link = document.createElement('a')
      link.href = src; link.download = image.name.replace(/[\\/:*?"<>|\u0000-\u001f]/g, '_') || 'image'
      document.body.appendChild(link); link.click(); link.remove()
    }
  }
}
export function ImageActions({ image, className, children, as: Tag = 'div' }: { image: UserImageDisplay; className?: string; children: ReactNode; as?: 'div' | 'span' }): React.JSX.Element {
  const scope = useContext(NativeImageScope), { canCopy, copy, download } = useImageActions()
  const owner = useRef(scope), current = owner.current === scope
  const [menu, setMenu] = useState<ImageMenuPosition | null>(null)
  useEffect(() => { setMenu(null); owner.current = scope }, [scope])
  return <><Tag data-image-actions="" data-image-actions-native={!canCopy || undefined} className={className}
    onPointerDown={event => { if (event.pointerType !== 'mouse') event.stopPropagation() }}
    onContextMenu={event => { event.stopPropagation(); if (canCopy) { event.preventDefault(); setMenu({ x: event.clientX, y: event.clientY, origin: event.currentTarget }) } }}>
    {children}
  </Tag>{canCopy && <ImageContextMenu position={scope.visible && current ? menu : null} onClose={() => setMenu(null)} displayName={image.name}
    ready={Boolean(nativeImageSource(image))} busy={null} hasPath={false} onAction={action => { setMenu(null); if (action === 'copy') void copy(image); else if (action === 'save') void download(image) }} />}</>
}
