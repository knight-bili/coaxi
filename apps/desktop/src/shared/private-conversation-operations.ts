import type { CoreMethod } from '@contracts'

/** Presentation selects an explicit Core entry point, never an owner override
 * on a Single Chat request. Core still verifies every child/takeover/editor. */
export type PrivateConversationKind = 'single_chat' | 'delegation_child'
const singleChat = {
  get: 'singleChat.get', recovery: 'singleChat.composerDraft.recovery',
  mutate: 'singleChat.composerDraft.mutate', edit: 'singleChat.pendingInputs.edit',
  send: 'singleChat.send', end: 'singleChat.end'
} as const satisfies Record<string, CoreMethod>
const child = {
  get: 'delegations.conversation.get', recovery: 'delegations.composer.recovery',
  mutate: 'delegations.composer.mutate', edit: 'delegations.pendingInputs.edit',
  send: 'delegations.message.send', end: 'delegations.conversation.end'
} as const satisfies Record<string, CoreMethod>
export function privateConversationOperations(kind?: PrivateConversationKind): typeof singleChat | typeof child {
  return kind === 'delegation_child' ? child : singleChat
}
export type PrivateInputMethod = typeof singleChat['mutate' | 'edit' | 'send'] | typeof child['mutate' | 'edit' | 'send']
export type DelegationActionMethod = 'delegations.takeover' | 'delegations.cancel' | 'delegations.resume' | 'delegations.conversation.end'
export function privateInputCommandType(method: PrivateInputMethod): string {
  if (method.startsWith('delegations.')) return method
  return method === 'singleChat.send' ? 'single_chat.send' : method === 'singleChat.pendingInputs.edit' ? 'single_chat.pending_input.edit' : 'single_chat.composer_draft.mutate'
}
