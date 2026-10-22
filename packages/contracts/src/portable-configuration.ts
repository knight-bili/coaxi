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
export interface PortableMemberConnectionView {agentId:string;agentVersion:number;binding:PortableMemberBindingV1|null}
export interface PortableProviderPage {items:PortableProviderViewV1[];next:string|null}
/** 原文由加密存储和显式导出拥有；普通视图的原文字段为占位提示。 */
export type PortableProviderDefinitionV1 = Omit<PortableProviderViewV1,'revision'|'digest'|'createdAt'|'updatedAt'>
