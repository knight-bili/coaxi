/** 客户确认的自定义智能体接入 r1；定义回显永不返回环境明文。 */
export interface CustomRuntimeSecretRef {secretId:string;revision:number}
export interface CustomRuntimeRefV1 {id:string;definitionRevision:number;definitionDigest:string}
export type CustomRuntimePlatform='windows-x64'|'windows-arm64'|'macos-x64'|'macos-arm64'|'linux-x64'|'linux-arm64'
export type CustomRuntimeBinaryPlatform='darwin-aarch64'|'darwin-x86_64'|'linux-aarch64'|'linux-x86_64'|'windows-aarch64'|'windows-x86_64'|'aarch64-apple-darwin'|'x86_64-apple-darwin'|'aarch64-unknown-linux-gnu'|'x86_64-unknown-linux-gnu'|'aarch64-pc-windows-msvc'|'x86_64-pc-windows-msvc'
interface PackageSpec<E>{package:string;args:string[];env:Record<string,E>;cmd:string|null}
export interface CustomRuntimeDefinitionBase<E>{
  schemaVersion:1;id:string;registryId:string;name:string;description:string;version:string
  distributionKind:'npx'|'uvx'|'binary'
  spec:{npx:(PackageSpec<E>&{nodeRequired:string|null})|null;uvx:(PackageSpec<E>&{uvRequired:string|null;python:string|null})|null;binary:Partial<Record<CustomRuntimeBinaryPlatform,{archive:string;cmd:string;args:string[];env:Record<string,E>;sha256:string|null}>>}
  iconUrl:string|null;skillsSharedStore:boolean;skillsDir:string|null;source:'registry'|'manual';sourceRef:string|null;versionProbe:string|null;supportsMcp:boolean;enabled:boolean
}
export type CustomRuntimeDefinitionInput=CustomRuntimeDefinitionBase<string|CustomRuntimeSecretRef>
export type CustomRuntimeDefinition=CustomRuntimeDefinitionBase<CustomRuntimeSecretRef>&{revision:number;definitionDigest:string;createdAt:string;updatedAt:string}
export interface CustomAcpLaunchLockV1{
  schemaVersion:1;definition:CustomRuntimeRefV1;platform:CustomRuntimePlatform;executable:string;arguments:string[];environmentDigest:string;artifactDigest:string;resolvedPackage:string|null;resolvedVersion:string;skillRoots:string[];supportsMcpDeclared:boolean;protocolCapabilitiesDigest:string|null
}
export interface CustomRuntimeInstallReceipt{
  commandId:string;reference:CustomRuntimeRefV1;platform:CustomRuntimePlatform;state:'running'|'completed'|'failed'|'unknown';startedAt:string;completedAt?:string;log?:string;error?:string;info?:unknown
}
export interface CustomRuntimeInspection{
  reference:CustomRuntimeRefV1;definition:CustomRuntimeDefinition;platform:CustomRuntimePlatform
  installation:{status:string;qualification:'not_verified';installationId?:string|null;operationId?:string|null;receipt?:CustomRuntimeInstallReceipt|null;launchLock?:CustomAcpLaunchLockV1|null;launchLockDigest?:string|null}
}
export interface CustomRuntimeDefinitionPage{items:CustomRuntimeDefinition[];next:string|null}
export const customRuntimeReference=(definition:CustomRuntimeDefinition):CustomRuntimeRefV1=>({id:definition.id,definitionRevision:definition.revision,definitionDigest:definition.definitionDigest})
