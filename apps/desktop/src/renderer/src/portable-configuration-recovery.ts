import type {ThreadClient} from './camp-client'
export interface PortableConfigurationPending {operation:'configuration.providers.upsert'|'configuration.providers.remove'|'configuration.memberConnection.set'|'configuration.migration.confirm'|'configuration.migration.undoConfirm'|'configuration.archive.register';commandId:string;targetId?:string;previewId?:string;previewDigest?:string}
const operations=new Set(['configuration.providers.upsert','configuration.providers.remove','configuration.memberConnection.set','configuration.migration.confirm','configuration.migration.undoConfirm','configuration.archive.register'])
function decode(value:unknown):PortableConfigurationPending|null{
  if(!value||typeof value!=='object'||Array.isArray(value))return null
  const row=value as Record<string,unknown>
  if(Object.keys(row).some(key=>!['operation','commandId','targetId','previewId','previewDigest'].includes(key))||!operations.has(String(row.operation))||typeof row.commandId!=='string'||!/^[a-f0-9-]{36}$/i.test(row.commandId))return null
  if(Object.values(row).some(value=>typeof value!=='string'||value.length>256))return null
  return row as unknown as PortableConfigurationPending
}
export async function loadPortablePending(client:ThreadClient,key:string):Promise<PortableConfigurationPending|null>{
  let raw:unknown
  if(client.workbenchEditorRecovery){const stored=await client.workbenchEditorRecovery.get(key) as {schemaVersion?:number;key?:string;editorId?:string;pending?:unknown}|null;if(stored===null)return null;if(stored.schemaVersion!==1||stored.key!==key||stored.editorId!=='portable-configuration')throw new Error('配置原请求的工作区归属不同，已保留原记录。');raw=stored.pending}
  else{if(!client.editingRecovery)throw new Error('原请求恢复存储尚不可用。');raw=client.editingRecovery.get(key)}
  if(raw==null)return null;const value=decode(raw);if(!value)throw new Error('配置原请求记录无法读取，已保留。');return value
}
export async function savePortablePending(client:ThreadClient,key:string,value:PortableConfigurationPending|null):Promise<void>{
  if(client.workbenchEditorRecovery){await client.workbenchEditorRecovery.put({schemaVersion:1,key,editorId:'portable-configuration',pending:value});return}
  if(!client.editingRecovery)throw new Error('原请求恢复存储尚不可用，本次未发送。');client.editingRecovery.set(key,value)
}
