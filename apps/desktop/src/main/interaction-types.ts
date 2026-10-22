/** 与已确认 r1 的闭集对应；Core 仍负责真实归属、授权和运行状态。 */
export interface InteractionScope {threadId:string;conversationId:string|null;scopeKind:'public'|'single_chat'|'delegation_child'}
export interface InteractionBinding {
  schemaVersion:1;bridgeInstanceId:string;coreInstanceId:string;desktopPeerId:string|null;scope:InteractionScope;
  agentId:string;agentRunId:string;executionEpoch:number;nativeBindingId:string;nativeBindingGeneration:number;
  nativeSessionId:string|null;membershipGeneration:number;permissionRevision:number;catalogDigest:string;
  policyDigest:string;expiresAt:string;revocationEpoch:number
}
export interface InteractionGrant {
  schemaVersion:1;grantId:string;scope:InteractionScope;agentId:string;desktopPeerId:string;
  targetKind:'browser_tab'|'computer_window'|'computer_app'|'computer_desktop';targetId:string;
  origin:string|null;level:'read'|'control';grantGeneration:number;expiresAt:string
}
export interface InteractionCall {
  schemaVersion:1;bridgeInstanceId:string;bridgeCallId:string;rpcId:string|number;nativeToolCallId:string|null;
  toolName:string;arguments:Record<string,unknown>;argumentsDigest:string
}
export interface InteractionWork {
  binding:InteractionBinding;call:InteractionCall;grants:InteractionGrant[];
  receipt:{bindingDigest:string;argumentsDigest:string;bridgeCallId:string;state:string}
}
export function sameInteractionScope(left:InteractionScope,right:InteractionScope):boolean {
  return left.threadId===right.threadId&&left.conversationId===right.conversationId&&left.scopeKind===right.scopeKind
}
