import type { ReactNode } from 'react'
import type { CanvasConversationTarget } from './canvas-conversation'
import type { ThreadLeavePreparation } from './ThreadWorkspace'
import { UiText, uiAttribute } from './interface-language'

export interface CanvasConversationBridge {
  instances: Array<{ key: string; target: CanvasConversationTarget; content: ReactNode; busy: boolean }>
  opening: boolean
  issue: string | null
  open(target: CanvasConversationTarget): Promise<boolean>
  close(keys?: readonly string[]): Promise<boolean>
  prepare(keys: readonly string[]): Promise<ThreadLeavePreparation>
}

export function CanvasConversationFrame({ bridge, instance, title, orphaned = false, onOpenWorkspace }: {
  bridge: CanvasConversationBridge; instance: CanvasConversationBridge['instances'][number]; title: string; orphaned?: boolean; onOpenWorkspace(): void
}): React.JSX.Element {
  const { target, key, busy } = instance
  return <section className="canvas-conversation-frame nodrag nowheel nopan" aria-label={uiAttribute('画布对话：{0}', title)}
    onKeyDown={event => {
      if (event.key === 'Escape' && !event.defaultPrevented) { event.preventDefault(); void bridge.close([key]) }
      // Preserve document-level conversation shortcuts, while isolating canvas movement/deletion.
      if (['Escape', 'Delete', 'Backspace', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(event.key)) event.stopPropagation()
    }} onWheel={event => event.stopPropagation()}
    onPointerDown={event => event.stopPropagation()}>
    <header className="canvas-conversation-heading">
      <strong title={title}>{title}</strong>
      {!orphaned && <button type="button" disabled={bridge.opening} onClick={() => { void bridge.open({ ...target, presentation: target.presentation === 'inline' ? 'drawer' : 'inline' }) }}>
        <UiText zh={target.presentation === 'inline' ? '移至侧边' : '放回画布'} />
      </button>}
      <button type="button" disabled={bridge.opening} onClick={onOpenWorkspace}><UiText zh="打开完整工作区" /></button>
      <button type="button" disabled={bridge.opening} onClick={() => { void bridge.close([key]) }}><UiText zh="收起对话" /></button>
    </header>
    {orphaned && <p className="canvas-notice" role="status"><UiText zh="原画布卡片已变更，当前对话保留在侧边，请先保存或收起。" /></p>}
    <div className="canvas-conversation-content" inert={busy} aria-busy={busy}>{instance.content}</div>
  </section>
}
