export interface NativeSteerPolicy {
  schemaVersion:1
  revision:number
  members:Array<{agentId:string;enabled:boolean}>
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
