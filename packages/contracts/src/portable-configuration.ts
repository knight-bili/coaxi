export interface PortableSecretRefV1 {id:string;revision:number}
export interface PortableConnectionRefV1 {id:string;revision:number;digest:string}
export type PortableProviderModel =
  | {kind:'claude';main:string;reasoning:string;haiku:string;sonnet:string;opus:string;fable:string;customOption:string;customOptionName:string;customOptionDescription:string}
  | {kind:'codex';customs:Array<{slug:string;displayName:string;contextWindow:number|null;base:string;overrides:Record<string,unknown>}>;excludedOfficials:string[];default:string|null}
  | {kind:'gemini';model:string|null}
export interface PortableProviderViewV1 {
  schemaVersion:1;id:string;revision:number;digest:string;name:string;agentType:'claude_code'|'codex'|'gemini';apiUrl:string;
  credential:PortableSecretRefV1|null;rawAgentTypesJson:string;rawModel:string|null;model:PortableProviderModel;enabled:boolean;createdAt:string;updatedAt:string
}
export interface PortableMemberBindingV1 {schemaVersion:1;agentId:string;revision:number;connection:PortableConnectionRefV1|null}
export interface PortableMemberConnectionView {agentId:string;agentVersion:number;binding:PortableMemberBindingV1|null;runtime:{adapterKind:import('./index').AdapterKind;installationId:string;installationGeneration:number;modelSelection:import('./index').ModelSelection}|null}
export interface PortableProviderPage {items:PortableProviderViewV1[];next:string|null}
/** 原文由加密存储和显式导出拥有；普通视图的原文字段为占位提示。 */
export type PortableProviderDefinitionV1 = Omit<PortableProviderViewV1,'revision'|'digest'|'createdAt'|'updatedAt'>
export interface PortableArchiveReference {archiveId:string;sourceDigest:string;sourceKind:'codeg_v1'|'fusion_v1'|'fusion_v2';counts:Record<string,number>}
export interface PortableArchiveSelection {selectionId:string;displayName:string;sourceDigest:string;byteSize:number}
export interface PortableExportReceipt {operationId:string;state:'unknown'|'prepared'|'ready';archiveId?:string;createdAt?:string;includeSecrets?:boolean;encryptedDigest?:string|null;byteSize?:number|null}
export interface PortableSyncRequest {operation:'get'|'set'|'upload'|'cancel'|'resume'|'archive'|'cache_preview'|'cache_remove'|'cache_status'|'receipt';expectedRevision?:string|null;commandId?:string;enabled?:boolean;autoSync?:boolean;fullConfiguration?:boolean;includeSecrets?:boolean;intervalMinutes?:number;passphrase?:string;clearPassphrase?:boolean;confirmed?:boolean}
export interface PortableConfigurationPrivateApi {
  logging?(input:{operation:'get'}|{operation:'set';expectedDigest:string;settings:{level:string;targets:Array<{target:string;level:string}>}}):Promise<{settings:{level:string;targets:Array<{target:string;level:string}>};digest:string}>
  sync<T>(input:PortableSyncRequest):Promise<T>
  prepareExport(input:{operationId:string;includeSecrets:boolean;passphrase:string}):Promise<PortableExportReceipt>
  exportReceipt(operationId:string):Promise<PortableExportReceipt>
  saveExport?(operationId:string):Promise<{saved:boolean}>
  downloadExport?(receipt:PortableExportReceipt):Promise<Blob>
  providerForEdit(reference:PortableConnectionRefV1):Promise<PortableProviderViewV1>
  prepareSecret(input:{schemaVersion:1;operationId:string;purpose:'provider_key'|'member_environment';value:string}):Promise<{reference:PortableSecretRefV1;expiresAt:string}>
  archiveReceipt(operationId:string):Promise<{archive:PortableArchiveReference|null}>
  selectArchive?():Promise<PortableArchiveSelection|null>
  importSelected?(input:{selectionId:string;operationId:string;passphrase:string;manifest?:Record<string,unknown>|null}):Promise<PortableArchiveReference>
  uploadArchive?(operationId:string,file:File,passphrase:string,manifest?:Record<string,unknown>|null):Promise<PortableArchiveReference>
}
export type PortableMigrationDomain='modelProviders'|'agentSettings'|'customAgents'|'quickMessages'|'taskTemplates'|'preferences'
export interface PortableMigrationSelection {entryId:string;domain:PortableMigrationDomain;decision:{entryId:string;action:'skip'|'create'|'replace';targetId:string|null;expectedRevision:number|null}}
export interface PortableMigrationMember {agentId:string;expectedVersion:number;providerEntryId:string;environmentEntryId:string|null;modelSelection:import('./index').ModelSelection;installationId:string;expectedInstallationGeneration:number}
export interface PortableMigrationRow {entryId:string;domain:string;name:string;action:'skip'|'create'|'replace';status:'ready'|'conflict'|'needs_environment'|'unsupported';before:Record<string,unknown>;after:Record<string,unknown>;reason:string|null}
export interface PortableMigrationPreview {schemaVersion:1;previewId:string;previewDigest:string;expiresAt:string;archiveId:string;sourceDigest:string;localDigest:string;selections:PortableMigrationSelection[];members:PortableMigrationMember[];rows:PortableMigrationRow[];canApply:boolean}
export interface PortableMigrationReceipt {schemaVersion:1;commandId:string;previewId:string;previewDigest:string;state:'prepared'|'applied'|'rejected'|'unknown';counts:Record<string,number>;changedMembers:string[];errorCode:string|null;historyPreserved:true;files:Array<{domain:string;targetId:string;state:'pending'|'applied'|'conflict'|'failed';errorCode:string|null}>}
export interface PortableMigrationTarget {id:string;name:string;revision:number}
