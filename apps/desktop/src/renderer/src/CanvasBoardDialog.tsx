import { useRef, useState, type ReactNode } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import type { CanvasBoard } from '@contracts'
import type { CanvasBoardIntent } from './canvas-boards-controller'
import { AppDialogBody, AppDialogContent, AppDialogFooter, AppDialogHeader } from './AppDialog'
import { canvasColors, canvasColorName } from './CanvasNodeEditor'
import { UiText, uiAttribute } from './interface-language'
import { readErrorMessage } from './error-message'

export function CanvasBoardDialog({ board, remove, latest, blocked, pending, recovery, commandError, onClose, onSave }: {
  board: CanvasBoard | null; remove: boolean; latest: CanvasBoard | null
  blocked: boolean; pending: boolean; recovery: ReactNode
  commandError: string | null
  onClose(): void; onSave(intent: CanvasBoardIntent): Promise<void>
}): React.JSX.Element {
  const [name, setName] = useState(board?.name ?? ''), [description, setDescription] = useState(board?.description ?? '')
  const [color, setColor] = useState(board?.color ?? ''), [revision, setRevision] = useState(board?.revision ?? null)
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), saving = useRef(false)
  const locked = busy || pending, stale = Boolean(board && latest?.revision !== revision)
  const invalid = Array.from(name.trim()).length > 200 || Array.from(description.trim()).length > 2000
  const save = async () => {
    if (locked || saving.current || blocked || stale || invalid) return
    saving.current = true; setBusy(true); setError(null)
    try {
      const intent: CanvasBoardIntent = remove && board ? { operation: 'canvas.boards.delete', command: { boardId: board.id, expectedRevision: revision! } }
        : board ? { operation: 'canvas.boards.update', command: { boardId: board.id, expectedRevision: revision!, name, description, color } }
        : { operation: 'canvas.boards.create', command: { name, description, color } }
      await onSave(intent)
    } catch (error) { setError(readErrorMessage(error)) } finally { saving.current = false; setBusy(false) }
  }
  const close = () => { if (!saving.current && !pending) onClose() }
  return <Dialog.Root open onOpenChange={open => { if (!open) close() }}><Dialog.Portal>
    <Dialog.Overlay className="dialog-overlay app-dialog-overlay" />
    <AppDialogContent className="canvas-board-dialog" tone={remove ? 'danger' : 'neutral'} onPointerDownOutside={event => event.preventDefault()}
      onEscapeKeyDown={event => { if (locked) event.preventDefault() }}>
      <AppDialogHeader title={uiAttribute(remove ? '删除画板' : board ? '编辑画板' : '新建画布')}
        description={uiAttribute(remove ? '删除画板及其中全部节点和便签正文，保留工程、对话、文件与历史。' : '名称可以留空，布局和节点内容会保存到当前工作区。')}
        hideDescription={!remove}
        closeDisabled={locked} />
      <form className="app-dialog-form" onSubmit={event => { event.preventDefault(); void save() }}>
        <AppDialogBody>
          {remove ? <p>{board?.name || uiAttribute('未命名画板')}</p> : <fieldset disabled={locked} className="canvas-editor-fields">
            <label className="field-label"><UiText zh="名称" /><input data-dialog-autofocus placeholder={uiAttribute('未命名画板')} value={name} onChange={event => setName(event.target.value)}
              onKeyDown={event => { if (event.key === 'Enter' && event.nativeEvent.isComposing) event.preventDefault() }} /></label>
            <label className="field-label"><UiText zh="描述" /><textarea placeholder={uiAttribute('这个画布用来做什么？（可选）')} value={description} rows={3} onChange={event => setDescription(event.target.value)} /></label>
            <div className="field-label"><span id="canvas-board-color-label"><UiText zh="颜色" /></span>
              <div className="canvas-color-palette" role="group" aria-labelledby="canvas-board-color-label">
                {canvasColors.map(value => <button key={value} type="button" data-color={value} className="canvas-color-choice" aria-label={uiAttribute(canvasColorName(value))}
                  title={uiAttribute(canvasColorName(value))} aria-pressed={color === value} onClick={() => setColor(color === value ? '' : value)}><span className="canvas-color-dot" /></button>)}
              </div>
            </div>
          </fieldset>}
          {invalid && <p role="alert"><UiText zh="名称最多 200 个字符，说明最多 2000 个字符。" /></p>}
          {board && !latest && <p role="alert"><UiText zh="画板已删除或暂时不可用，请重新读取列表。" /></p>}
          {stale && latest && <p className="canvas-notice"><UiText zh="画板已有新修改，输入仍保留，请核对后继续。" />
            {!remove && <button type="button" disabled={locked || blocked} onClick={() => { setRevision(latest.revision); setError(null) }}><UiText zh="使用最新画布版本" /></button>}
          </p>}
          {(commandError || error) && <p role="alert">{commandError || error}</p>}{recovery}
        </AppDialogBody>
        <AppDialogFooter><button type="button" className="quiet-button" disabled={locked} onClick={close}><UiText zh="取消" /></button>
          <button className={remove ? 'danger-button' : 'primary-button conversation-primary-button'} type="submit" disabled={blocked || locked || stale || invalid}>
            <UiText zh={locked ? '保存中…' : remove ? '删除画板' : board ? '保存' : '创建'} />
          </button>
        </AppDialogFooter>
      </form>
    </AppDialogContent>
  </Dialog.Portal></Dialog.Root>
}
