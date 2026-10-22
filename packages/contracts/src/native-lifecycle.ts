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
