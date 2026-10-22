/** 客户确认的文字和卡片反馈 r1；不含私有 MCP 凭据。 */
export interface FeedbackScope {threadId:string;conversationId:string|null;scopeKind:'public'|'single_chat'|'delegation_child'}
export interface FeedbackTarget {agentId:string;displayName:string;agentRunId:string;executionEpoch:number;runtimeKind:string|null;feedbackEnabled:boolean;nativeBindingReady:boolean;scope:FeedbackScope}
export interface FeedbackPolicy {schemaVersion:1;revision:number;members:Array<{agentId:string;enabled:boolean}>}
export interface FeedbackField {name:string;label:string;statePath:string;value:unknown}
export interface FeedbackCardOrigin {messageId:string;conversationId:string|null;messageKind:FeedbackScope['scopeKind'];cardOrdinal:number;sourceDigest:string}
export type FeedbackSource = {kind:'live_note'}|{kind:'generated_card';origin:FeedbackCardOrigin}
export interface FeedbackPreviewRequest {schemaVersion:1;scope:FeedbackScope;agentId:string;agentRunId:string;executionEpoch:number;source:FeedbackSource;text:string;title:string;fields:FeedbackField[]}
export interface FeedbackPreview {schemaVersion:1;previewId:string;previewDigest:string;scope:FeedbackScope;agentId:string;agentRunId:string;executionEpoch:number;text:string;expiresAt:string}
export interface FeedbackSubmit {schemaVersion:1;commandId:string;previewId:string;previewDigest:string}
export type FeedbackStatus = 'pending'|'response_prepared'|'connection_written'|'unknown'|'requires_user_review'|'continued_as_message'
export interface FeedbackReceipt {schemaVersion:1;feedbackId:string;scope:FeedbackScope;agentId:string;sourceAgentRunId:string;sourceExecutionEpoch:number;sourceKind:'live_note'|'generated_card';textDigest:string;status:FeedbackStatus;deliveryChannel:'none'|'mcp'|'normal_message';bridgeInstanceId:string|null;transportRpcId:string|number|null;nativeToolCallId:string|null;resultDigest:string|null;writtenAt:string|null;continuedMessageId:string|null}
export interface FeedbackList {items:Array<{receipt:FeedbackReceipt;text:string;createdAt:string}>;next:string|null}
export interface FeedbackPreviewRecoveryList {items:Array<{preview:FeedbackPreview;receipt:FeedbackReceipt|null}>;next:string|null}
export interface FeedbackContinuationReference {schemaVersion:1;feedbackId:string;previewId:string;previewDigest:string}
export interface FeedbackNormalPreview {preview:FeedbackPreview;reference:FeedbackContinuationReference}
export interface FeedbackNormalSend {commandId:string;reference:FeedbackContinuationReference;scope:FeedbackScope;agentId:string;body:string;draftRevision?:number;composerScope?:import('./index').SingleChatComposerScope}
export type FeedbackNormalResult = {state:'unknown'}|{state:'recorded';commandResult:import('./index').StoredCommandResult;replayed:boolean;receipt:FeedbackReceipt}
