import type { CoreEvent, DelegationConversationView, PrivateAttachmentPreparationRequest, ScopedSingleChatSnapshot, StoredCommandResult, ThreadMemberView } from '@contracts'

// Transport fixture only: real production components, no Core/provider execution.
const at = '2026-10-18T00:00:00Z'
export const chatMembers: ThreadMemberView[] = ['unrelated-lead', 'owner-a'].map((agentId, memberOrder) => ({
  agentId, displayName: agentId === 'owner-a' ? '发起队员' : '其他主队员', avatarRef: null, teamRole: '开发', accent: '',
  membershipStatus: 'active', leaveRequestedAt: null, profilePresence: 'present', memberOrder, isDefaultLead: memberOrder === 0, version: 1
}))
export function childChatFixture(getDetail: () => DelegationConversationView, setDetail: (detail: DelegationConversationView) => void, emit: () => void) {
  let enabled = false, loseTakeover = true, loseSend = false, loseEnd = false
  const storage = new Map<string, unknown>(), receipts = new Map<string, StoredCommandResult>()
  const uploads: PrivateAttachmentPreparationRequest[] = []
  let snapshot: ScopedSingleChatSnapshot
  const result = (commandId: string, commandType: string, payload: Record<string, unknown> = {}): StoredCommandResult => {
    const receipt: StoredCommandResult = { commandId, commandType, status: 'applied', code: 'fixture.applied', payload, resultEntity: null, recordedAt: at }
    receipts.set(commandId, receipt); return receipt
  }
  const updateDetail = () => { const detail = getDetail(); setDetail({ ...detail, activity: { ...detail.activity, runs: snapshot.agentRuns, executionEvidence: snapshot.executionEvidence } }); emit() }
  const attachments = {
    prepare: async () => { throw Error('Forbidden legacy attachment prepare') },
    preparePending: async () => { throw Error('Forbidden legacy pending attachment prepare') },
    remove: async () => { throw Error('Forbidden legacy attachment remove') },
    pendingPreparations: async () => [],
    prepareInput: async (request: PrivateAttachmentPreparationRequest, file: File) => {
      uploads.push(structuredClone(request))
      if (request.target.kind !== 'delegation_child' || request.target.conversationId !== 'child-1' || request.threadId !== 'thread-a' || request.expectedRevision !== snapshot.draft.revision) throw Error('Wrong upload owner')
      snapshot.draft = { ...snapshot.draft, revision: snapshot.draft.revision + 1, attachments: [...snapshot.draft.attachments, {
        id: request.commandId, displayName: file.name, kind: 'file', fileCount: 1, mediaType: file.type, byteSize: file.size, previewKind: 'none', availability: 'available'
      }] }
      return { state: 'recorded' as const, commandId: request.commandId, receipt: result(request.commandId, 'camp.source_attachment.upload.bind', { attachmentRefId: request.commandId }) }
    },
    reconcileInput: async (request: PrivateAttachmentPreparationRequest) => ({ state: 'recorded' as const, commandId: request.commandId, receipt: receipts.get(request.commandId)! }),
    retryInput: async () => { throw Error('Unexpected upload retry') }, finishInput: async () => {}
  }
  const request = async (method: string, params: any): Promise<{ handled: boolean; value?: unknown }> => {
    if (!enabled) return { handled: false }
    const handled = (value: unknown) => ({ handled: true, value: structuredClone(value) })
    if (method.startsWith('singleChat.')) throw Error('Forbidden default singleChat fallback: ' + method)
    if (method === 'runtime.interactions.list') return handled({ conversationId: params.conversationId, items: [], nextCursor: null })
    if (method === 'delegations.conversation.get') {
      if (params.conversationId !== 'child-1' || params.threadId !== undefined) throw Error('Wrong exact child read')
      return handled(snapshot)
    }
    if (method === 'delegations.composer.recovery') return handled({ items: [], complete: true, nextCursor: null })
    if (method === 'commands.reconcile') return handled(receipts.has(params.params.commandId) ? { state: 'recorded', result: receipts.get(params.params.commandId) } : { state: 'unknown' })
    if (!['delegations.takeover', 'delegations.message.send', 'delegations.conversation.end', 'delegations.composer.mutate', 'delegations.pendingInputs.edit'].includes(method)) return { handled: false }
    const { commandId, command } = params
    if (receipts.has(commandId)) return handled(receipts.get(commandId))
    if (method === 'delegations.takeover') {
      if (command.delegationId !== 'delegation-1' || command.expectedControlVersion !== 1) throw Error('Wrong takeover')
      snapshot.agentRuns = snapshot.agentRuns.map(run => ({ ...run, status: 'cancelled', endedAt: at }))
      setDetail({ ...getDetail(), takenOver: true, controlVersion: 2, cleanupPending: true, status: 'cancelled', blockedOn: undefined })
      const receipt = result(commandId, method); updateDetail()
      if (loseTakeover) { loseTakeover = false; throw Error('Lost takeover reply after apply') }
      return handled(receipt)
    }
    if (command.conversationId !== 'child-1') throw Error('Wrong child mutation')
    if (method === 'delegations.pendingInputs.edit') {
      const pending = snapshot.pendingInputs.items[0]
      if (command.threadId !== undefined || command.pendingInputId !== pending?.id || command.expectedRevision !== pending.revision || command.action.type !== 'return_to_composer' || command.action.expectedDraftRevision !== snapshot.draft.revision || command.action.targetBody !== '') throw Error('Wrong pending return')
      snapshot.pendingInputs.items = []
      snapshot.draft = { ...snapshot.draft, revision: snapshot.draft.revision + 1, attachments: pending.attachments, quotes: pending.quotes }
      const receipt = result(commandId, method, { placement: 'active', body: pending.body, draft: snapshot.draft })
      emit()
      // A completed Core return whose reply never reached the original UI.
      throw Error('Lost pending return reply after apply')
    }
    if (method === 'delegations.composer.mutate') {
      if (command.threadId !== 'thread-a' || command.expectedRevision !== snapshot.draft.revision || command.action.type !== 'remove_attachment') throw Error('Wrong draft mutation')
      snapshot.draft = { ...snapshot.draft, revision: snapshot.draft.revision + 1, attachments: snapshot.draft.attachments.filter(item => item.id !== command.action.attachmentRefId) }
      return handled(result(commandId, method, { draft: snapshot.draft }))
    }
    if (method === 'delegations.message.send') {
      if (command.threadId !== undefined || command.expectedRevision !== snapshot.draft.revision || snapshot.conversation.status !== 'active') throw Error('Wrong send scope/version')
      const id = 'chat-run-' + commandId, inputId = 'input-' + commandId, finalId = 'reply-' + commandId, sequence = snapshot.messages.length + 1
      snapshot.messages.push({ id: inputId, sequence, authorType: 'user', authorId: 'local_user', body: command.body, attachments: snapshot.draft.attachments, quotes: snapshot.draft.quotes, agentRunId: id, createdAt: at },
        { id: finalId, sequence: sequence + 1, authorType: 'agent', authorId: 'owner-a', body: '接管后的完整回复。\n\n' + '原会话材料保留，继续核对实现。\n\n'.repeat(30), attachments: [], quotes: [], agentRunId: id, createdAt: at })
      snapshot.agentRuns.push({ ...snapshot.agentRuns[0], id, status: 'succeeded', triggerConversationMessageId: inputId, finalConversationMessageId: finalId, executionEvidenceCount: 0, executionEvidenceChangeSequence: 0 })
      snapshot.draft = { ...snapshot.draft, revision: snapshot.draft.revision + 1, attachments: [], quotes: [] }
      snapshot.conversation = { ...snapshot.conversation, lastMessageSequence: sequence + 1 }
      const receipt = result(commandId, method); updateDetail()
      if (loseSend) { loseSend = false; throw Error('Lost send reply after apply') }
      return handled(receipt)
    }
    snapshot.conversation = { ...snapshot.conversation, status: 'ended', endedAt: at }
    snapshot.draft = { ...snapshot.draft, revision: snapshot.draft.revision + 1, attachments: [], quotes: [] }
    const receipt = result(commandId, method); updateDetail()
    if (loseEnd) { loseEnd = false; throw Error('Lost end reply after apply') }
    return handled(receipt)
  }
  return {
    request, attachments,
    editingRecovery: { get: (key: string) => structuredClone(storage.get(key)), set: (key: string, value: unknown) => { storage.set(key, structuredClone(value)) } },
    start: () => {
      enabled = true
      storage.set('single-chat:thread-a', { 'thread-a:pending:owner-a': 'OTHER_PARENT_DRAFT' })
      const detail = getDetail()
      snapshot = {
        conversation: { id: 'child-1', threadId: 'thread-a', agentId: 'owner-a', version: 1, status: 'active', lastMessageSequence: 1, lastAcceptedPublicBoundarySequence: 0, activeAgentRunId: null, createdAt: at, updatedAt: at, endedAt: null },
        messages: [{ id: 'input-1', sequence: 1, authorType: 'system', authorId: 'delegation-parent', body: '原任务：保留自己的材料与执行记录。', attachments: [], quotes: [], agentRunId: null, createdAt: at }],
        draft: { revision: 0, attachments: [], quotes: [], updatedAt: null, scopeVersion: 1, scopeKey: '', composerScope: null },
        pendingInputs: { items: [], executionActive: false, editSession: null, editingElsewhere: false },
        approvals: [], agentRuns: [{ ...detail.activity.runs[0], status: 'running', endedAt: null, finalConversationMessageId: null }], executionEvidence: detail.activity.executionEvidence
      }
      setDetail({ ...detail, status: 'running', blockedOn: undefined, activity: { ...detail.activity, approvals: [], runs: snapshot.agentRuns } }); emit()
    },
    cleanup: () => { setDetail({ ...getDetail(), cleanupPending: false }); emit() },
    queue: () => {
      snapshot.pendingInputs.items = [{ id: 'pending-child-1', conversationId: 'child-1', enqueueSequence: 1, revision: 1, state: 'queued', body: '排队消息完整移回', lastAttemptErrorCode: null, quotes: [], attachments: [{ id: 'queued-file', displayName: 'queued-child.txt', kind: 'file', fileCount: 1, mediaType: 'text/plain', byteSize: 8, previewKind: 'none', availability: 'available' }] }]
      emit()
    },
    loseSend: () => { loseSend = true }, loseEnd: () => { loseEnd = true },
    snapshot: () => ({ uploads, storage: Object.fromEntries(storage), coreSnapshot: snapshot })
  }
}
