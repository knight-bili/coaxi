import { useSyncExternalStore } from 'react'
import type { SingleChatPendingInputEditSessionView, SingleChatPendingInputView } from '@contracts'
import type { PrivateInputController } from './private-input-controller'
import type { SingleChatComposerStore } from './single-chat-composer-store'
import { privateInputErrorMessage } from './PrivateInputRecoveryPanel'
import { UiText, uiAttribute } from './interface-language'

export function PrivatePendingEditor({ item, session, controller, bodies, busy, onError }: {
  item: SingleChatPendingInputView
  session: SingleChatPendingInputEditSessionView
  controller: PrivateInputController
  bodies: SingleChatComposerStore
  busy: boolean
  onError(message: string): void
}): React.JSX.Element {
  const state = useSyncExternalStore(bodies.subscribe, bodies.getSnapshot, bodies.getSnapshot)
  const key = `${controller.origin.threadId}:${controller.origin.conversationId}:pending-edit:${session.editToken}`
  const body = state.drafts[key] ?? session.workingBody
  const perform = (operation: Promise<void>): void => { void operation.catch(error => onError(privateInputErrorMessage(error))) }
  return <div className="single-chat-pending-editor">
    <label><UiText zh="编辑排队消息正文" />
      <textarea value={body} readOnly={busy || session.recoveryRequired} aria-label={uiAttribute('编辑排队消息正文')}
        onChange={event => {
          if (!bodies.update(current => ({ ...current, [key]: event.target.value }))) return
          try { controller.rememberBody(key) } catch (error) { onError(privateInputErrorMessage(error)) }
        }} />
    </label>
    {session.workingAttachments.length > 0 && <ul>{session.workingAttachments.map(attachment => <li key={attachment.id}>
      {attachment.displayName}
      <button type="button" disabled={busy || session.recoveryRequired} onClick={() => perform(controller.edit(item, { type: 'remove_attachment', attachmentRefId: attachment.id }, session.editToken))}><UiText zh="移除附件" /></button>
    </li>)}</ul>}
    {session.workingQuotes.map(quote => <blockquote key={quote.quoteId}>{quote.text}</blockquote>)}
    <div className="pending-input-actions">
      {session.recoveryRequired
        ? <button type="button" disabled={busy} onClick={() => perform(controller.edit(item, { type: 'takeover' }, session.editToken))}><UiText zh="从已保存版本接管" /></button>
        : <button type="button" disabled={busy || Boolean(state.error)} onClick={() => perform(controller.edit(item, { type: 'save', body }, session.editToken))}><UiText zh="保存排队消息" /></button>}
      <button type="button" disabled={busy} onClick={() => perform(controller.edit(item, { type: 'cancel' }, session.editToken))}><UiText zh="放弃本次编辑" /></button>
    </div>
  </div>
}
