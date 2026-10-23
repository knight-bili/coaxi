export interface ConversationToolFlags { question: boolean; sessionInfo: boolean; automations: boolean; workTasks: boolean }
export interface ConversationToolMemberPolicy { agentId: string; flags: ConversationToolFlags }
export interface ConversationToolPolicy { schemaVersion: 1; revision: number; globalFlags: ConversationToolFlags; feedbackGate: boolean | null; members: ConversationToolMemberPolicy[] }
export interface ConversationToolScope { threadId: string; conversationId: string | null; scopeKind: 'public' | 'single_chat' | 'delegation_child' }
export interface ConversationSourceGrant { grantId: string; revision: number; scope: ConversationToolScope; sourceSessionId: string; sourceKind: 'public' | 'single_chat' | 'delegation_child' | 'imported'; sourceDigest: string; throughSequence: number; expiresAt: string; legacySessionId: number | null }
export interface ConversationSourceDescriptor { sourceSessionId: string; sourceDigest: string; throughSequence: number; sourceKind: ConversationSourceGrant['sourceKind']; title: string }
export interface ConversationProjectGrant { projectId: string; projectPath: string; revision: number; memberAgentIds: string[] }
export interface FrozenConversationTools { schemaVersion: 1; policyRevision: number; policyDigest: string; scope: ConversationToolScope; agentId: string; flags: ConversationToolFlags; feedbackGate: boolean | null; sourceGrants: ConversationSourceGrant[]; projectGrants: ConversationProjectGrant[]; catalogDigest: string }
export interface ConversationQuestionInput { header: string; question: string; multiSelect: boolean; options: Array<{ label: string; description: string }> }
export interface ConversationQuestionSet { requestId: string; revision: number; scope: ConversationToolScope; agentId: string; agentRunId: string; executionEpoch: number; state: 'awaiting_answer' | 'answered' | 'delivered' | 'cancelled' | 'outcome_unknown'; questions: Array<{ id: string; spec: ConversationQuestionInput }>; createdAt: string; expiresAt: string }
export interface ConversationQuestionAnswer { questionId: string; labels: string[] }
export interface ConversationQuestionOutcome { answers: Array<{ header: string; question: string; multi_select: boolean; selected: string[] }>; declined: boolean }
export interface ConversationAuthoringOutcome { created: boolean; kind: 'automation' | 'work_task'; id?: string; title?: string; folder_name?: string; folder_path?: string; agent_type?: string; cron?: string; timezone?: string; next_run_at?: string; target_kind?: 'automation' | 'mission'; note?: string }
export interface ConversationRequestStatus { request_id: string; state: 'not_seen' | 'prepared' | 'awaiting_answer' | 'answered' | 'applied' | 'rejected' | 'cancelled' | 'outcome_unknown'; outcome?: ConversationQuestionOutcome | ConversationAuthoringOutcome | null; note?: string }
export interface ConversationGrantPreview { previewId: string; previewDigest: string; expiresAt: string; grant: ConversationSourceGrant }
export interface ConversationRunRevocation { commandId: string; agentRunId: string; executionEpoch: number; revoked: boolean }
export interface ConversationAutomationOwner { automationId: string; revision: number; automationVersion: number; action: 'launch_session' | 'enqueue_task'; timezone: string; sourceRequestId: string }
export interface ConversationQueuedMission { occurrenceId: string; scheduledAtUtc: string; missionId: string; title: string; createdAt: string }
export interface ConversationToolsInputs {
  'conversationTools.policy.get': Record<string, never>
  'conversationTools.policy.set': { commandId: string; expectedRevision: number; globalFlags: ConversationToolFlags; feedbackGate: boolean | null; members: ConversationToolMemberPolicy[] }
  'conversationTools.questions.list': { scope: ConversationToolScope; after: string | null; limit: number }
  'conversationTools.questions.answer': { commandId: string; requestId: string; expectedRevision: number; answers: ConversationQuestionAnswer[] }
  'conversationTools.questions.decline': { commandId: string; requestId: string; expectedRevision: number }
  'conversationTools.questions.reconcile': { requestId: string }
  'conversationTools.grants.preview': { scope: ConversationToolScope; sourceSessionId: string; sourceDigest: string; throughSequence: number }
  'conversationTools.grants.confirm': { commandId: string; previewId: string; previewDigest: string }
  'conversationTools.grants.revoke': { commandId: string; grantId: string; expectedRevision: number }
  'conversationTools.grants.list': { scope: ConversationToolScope; after: string | null; limit: number }
  'conversationTools.requests.get': { requestId: string }
  'conversationTools.requests.resolve': { commandId: string; requestId: string; action: 'cancel_remaining' } | { commandId: string; scope: ConversationToolScope; agentRunId: string; expectedExecutionEpoch: number; action: 'revoke_run' }
  'conversationTools.automationOwner.get': { automationId: string }
  'conversationTools.automationOwner.set': { commandId: string; automationId: string; expectedAutomationVersion: number; expectedOwnerRevision: number; action: ConversationAutomationOwner['action']; timezone: string }
}
export interface ConversationToolsOutputs {
  'conversationTools.policy.get': ConversationToolPolicy
  'conversationTools.policy.set': ConversationToolPolicy
  'conversationTools.questions.list': { items: ConversationQuestionSet[]; next: string | null }
  'conversationTools.questions.answer': ConversationRequestStatus
  'conversationTools.questions.decline': ConversationRequestStatus
  'conversationTools.questions.reconcile': ConversationRequestStatus
  'conversationTools.grants.preview': ConversationGrantPreview
  'conversationTools.grants.confirm': ConversationSourceGrant
  'conversationTools.grants.revoke': { grantId: string; revoked: boolean }
  'conversationTools.grants.list': { items: ConversationSourceGrant[]; next: string | null }
  'conversationTools.requests.get': ConversationRequestStatus
  'conversationTools.requests.resolve': ConversationRequestStatus | ConversationRunRevocation
  'conversationTools.automationOwner.get': ConversationAutomationOwner | null
  'conversationTools.automationOwner.set': ConversationAutomationOwner
}
