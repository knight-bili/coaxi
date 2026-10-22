import type {ThreadClient} from './camp-client'

/** 复用原生私有恢复目录和 Web 编辑器存储；程序定义正文和环境新值不进入此记录。 */
export async function loadCustomRuntimeRecovery<T>(client:ThreadClient,key:string,decode:(value:unknown)=>T|null):Promise<T|null>{
  if(client.workbenchEditorRecovery){const raw=await client.workbenchEditorRecovery.get(key);if(raw===null)return null;const row=raw as {schemaVersion?:number;key?:string;editorId?:string;pending?:unknown};if(row.schemaVersion!==1||row.key!==key||row.editorId!=='custom-runtime')throw new Error('自定义程序的原请求记录归属不同，保留原记录。');if(row.pending==null)return null;const value=decode(row.pending);if(!value)throw new Error('自定义程序的原请求记录损坏，保留原记录。');return value}
  if(!client.editingRecovery)throw new Error('原请求恢复存储尚不可用，请重新打开设置。')
  const raw=client.editingRecovery.get(key);if(raw==null)return null;const value=decode(raw);if(!value)throw new Error('自定义程序的原请求记录损坏，保留原记录。');return value
}
export async function saveCustomRuntimeRecovery(client:ThreadClient,key:string,value:unknown):Promise<void>{
  if(client.workbenchEditorRecovery){await client.workbenchEditorRecovery.put({schemaVersion:1,key,editorId:'custom-runtime',pending:value});return}
  if(!client.editingRecovery)throw new Error('原请求恢复存储尚不可用，本次未发送。')
  client.editingRecovery.set(key,value)
}
