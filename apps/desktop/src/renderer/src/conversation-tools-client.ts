import type { ConversationToolsInputs, ConversationToolsOutputs } from '@contracts'
import type { ThreadClient } from './camp-client'

export function conversationToolRequest<K extends keyof ConversationToolsInputs>(client: ThreadClient, operation: K, input: ConversationToolsInputs[K]): Promise<ConversationToolsOutputs[K]> {
  return client.request<ConversationToolsOutputs[K]>(operation, input)
}
export interface ConversationOriginal { operation: 'conversationTools.policy.set' | 'conversationTools.questions.answer' | 'conversationTools.questions.decline' | 'conversationTools.grants.confirm' | 'conversationTools.grants.revoke' | 'conversationTools.requests.resolve' | 'conversationTools.automationOwner.set'; commandId: string; requestId?: string }
const operations = new Set<ConversationOriginal['operation']>(['conversationTools.policy.set', 'conversationTools.questions.answer', 'conversationTools.questions.decline', 'conversationTools.grants.confirm', 'conversationTools.grants.revoke', 'conversationTools.requests.resolve', 'conversationTools.automationOwner.set'])
export async function conversationOriginal(client: ThreadClient, key: string, value?: ConversationOriginal | null): Promise<ConversationOriginal | null> {
  if (!client.presentationScope) throw new Error('工作区身份尚未确定，本次未发送。')
  if (value !== undefined) {
    if (client.workbenchEditorRecovery) await client.workbenchEditorRecovery.put({ schemaVersion: 1, key, editorId: 'native-lifecycle', pending: value })
    else if (client.editingRecovery) client.editingRecovery.set(key, value)
    else throw new Error('原请求记录存储暂不可用，本次未发送。')
    return value
  }
  const record = client.workbenchEditorRecovery ? await client.workbenchEditorRecovery.get(key) : null
  const raw = client.workbenchEditorRecovery ? (record as { key?: string; editorId?: string; pending?: unknown } | null)?.pending : client.editingRecovery?.get(key)
  if (record && ((record as { key?: string }).key !== key || (record as { editorId?: string }).editorId !== 'native-lifecycle')) throw new Error('原请求记录归属不同，请保留原记录。')
  if (raw == null) return null
  const item = raw as ConversationOriginal
  if (typeof raw !== 'object' || Object.keys(raw).some(key => !['operation', 'commandId', 'requestId'].includes(key)) || !operations.has(item.operation) || !/^[0-9a-f-]{36}$/.test(item.commandId) || item.requestId !== undefined && !/^[0-9a-f-]{36}$/.test(item.requestId)) throw new Error('原请求记录损坏，请保留原编号。')
  return item
}
