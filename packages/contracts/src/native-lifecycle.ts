export interface NativeSteerPolicy {
  schemaVersion:1
  revision:number
  members:Array<{agentId:string;enabled:boolean}>
}
export type NativeContinuationMode='continue'|'fork_tail'|'fork_point'
export interface NativeSourceReference {
  schemaVersion:1;importId:string;importRevision:string;provider:string;nativeSessionId:string;sourceHandleId:string;sourceRevision:string
}
export interface NativeForkPoint {selectedTurnId:string;messageId:string;messageFingerprint:string|null;messageOccurrence:number|null}
export interface NativeContinuationPreview {
  schemaVersion:1;previewId:string;previewDigest:string;expiresAt:string;source:NativeSourceReference
  mode:NativeContinuationMode;point:NativeForkPoint|null;sourceThreadId:string;newThreadId:string;newConversationId:string
  agentId:string;expectedAgentVersion:number;runtimeConfigDigest:string;workspaceDigest:string;title:string;historyMessageCount:number
  capabilityStatus:'available'|'requires_acp_program'|'unavailable';reason:string|null
}
export interface NativeContinuationReceipt {
  schemaVersion:1;commandId:string;state:'prepared'|'starting'|'binding_recorded'|'failed'|'unknown';originId:string
  threadId:string;conversationId:string;reportedSessionId:string|null;error:string|null
}
export interface NativeSteerScope {threadId:string;conversationId:string|null;scopeKind:'public'|'single_chat'|'delegation_child'}
export interface NativeSteerTarget {
  scope:NativeSteerScope;agentId:string;agentRunId:string;executionEpoch:number
  nativeBindingId:string;nativeBindingGeneration:number;bindingCompatibilityDigest:string;activeTurnId:string
}
export interface NativeSteerMaterial {sourceId:string;snapshotDigest:string;displayName:string;kind:'image'|'file'|'text_reference';byteSize:number}
export interface NativeSteerInput {text:string;materials:NativeSteerMaterial[]}
export interface NativeSteerPreview {
  schemaVersion:1;previewId:string;previewDigest:string;expiresAt:string;target:NativeSteerTarget
  input:NativeSteerInput;inputDigest:string;capabilityDigest:string
}
export interface NativeSteerReceipt {
  schemaVersion:1;commandId:string;previewId:string;target:NativeSteerTarget;inputDigest:string
  status:'prepared'|'connection_written'|'native_acknowledged'|'not_consumed'|'requires_user_review'|'unknown'
  nativeOutcome:'injected'|'promptRequired'|'startedNewTurn'|null;nativeReceiptDigest:string|null;error:string|null
}
export interface NativeSteerRecoveryPage {schemaVersion:1;items:Array<{preview:NativeSteerPreview;receipt:NativeSteerReceipt|null}>;next:string|null}
export interface NativeNormalReference {sourceCommandId:string;previewId:string;previewDigest:string}
export interface NativeNormalPreview {
  schemaVersion:1;previewId:string;previewDigest:string;sourceCommandId:string;scope:NativeSteerScope;agentId:string
  input:NativeSteerInput;inputDigest:string;expiresAt:string
}
export interface NativeNormalConfirm {schemaVersion:1;commandId:string;reference:NativeNormalReference;draftRevision?:number;composerScope?:import('./index').SingleChatComposerScope|null}
export interface NativeNormalReceipt {schemaVersion:1;commandId:string;state:'prepared'|'recorded'|'failed'|'unknown';commandResult:import('./index').StoredCommandResult|null;error:string|null}
