import type { WorkbenchEditorState } from '@contracts'
export interface EditorSaveRequest {operation:'save';scope:{threadId:string};editorId:string;workspaceRoot:string;expectedRevision:string|null;commandId:string;value:WorkbenchEditorState}
export interface EditorRecoveryRecord {schemaVersion:1;key:string;editorId:string;workspaceRoot:string|null;revision:string|null;latest:WorkbenchEditorState|null;pending:EditorSaveRequest|null}
export interface EditorRecoveryCopy {key:string;documentCount:number;paths:string[]}
const DATABASE='codeg-rovai-workbench-editor-v1',STORE='records'
function open():Promise<IDBDatabase>{
  return new Promise((resolve,reject)=>{const request=indexedDB.open(DATABASE,1);request.onupgradeneeded=()=>{if(!request.result.objectStoreNames.contains(STORE))request.result.createObjectStore(STORE,{keyPath:'key'})};request.onerror=()=>reject(request.error??new Error('工程草稿的本机存储不可用。'));request.onblocked=()=>reject(new Error('工程草稿存储正在被另一版本占用。'));request.onsuccess=()=>resolve(request.result)})
}
export async function loadEditorRecovery(key:string):Promise<EditorRecoveryRecord>{
  const database=await open()
  try{return await new Promise((resolve,reject)=>{const transaction=database.transaction(STORE,'readwrite'),store=transaction.objectStore(STORE),request=store.get(key);let result:EditorRecoveryRecord
    request.onsuccess=()=>{try{const found=request.result as EditorRecoveryRecord|undefined;if(found){if(found.schemaVersion!==1||found.key!==key||typeof found.editorId!=='string')throw new Error('工程草稿记录损坏，原记录仍保留。');result=found}else{result={schemaVersion:1,key,editorId:crypto.randomUUID(),workspaceRoot:null,revision:null,latest:null,pending:null};store.add(result)}}catch(error){transaction.abort();reject(error)}}
    transaction.oncomplete=()=>resolve(structuredClone(result));transaction.onerror=()=>reject(transaction.error??new Error('工程草稿记录读取失败。'));transaction.onabort=()=>reject(transaction.error??new Error('工程草稿记录读取中断。'))
  })}finally{database.close()}
}
export async function saveEditorRecovery(value:EditorRecoveryRecord):Promise<void>{
  if(value.schemaVersion!==1||JSON.stringify(value).length>40*1024*1024)throw new Error('工程草稿超过本机恢复记录上限。')
  const database=await open()
  try{await new Promise<void>((resolve,reject)=>{const transaction=database.transaction(STORE,'readwrite');transaction.objectStore(STORE).put(structuredClone(value));transaction.oncomplete=()=>resolve();transaction.onerror=()=>reject(transaction.error??new Error('工程草稿未保存到本机。'));transaction.onabort=()=>reject(transaction.error??new Error('工程草稿本机保存中断。'))})}finally{database.close()}
}
export async function listEditorCopies(key:string,after:string|null):Promise<{items:EditorRecoveryCopy[];next:string|null}>{
  const database=await open(),prefix=`${key}:copy:`
  try{return await new Promise((resolve,reject)=>{const transaction=database.transaction(STORE,'readonly'),range=IDBKeyRange.bound(after??prefix,`${prefix}\uffff`,Boolean(after),false),request=transaction.objectStore(STORE).openCursor(range),items:EditorRecoveryCopy[]=[];let next:string|null=null
    request.onsuccess=()=>{const cursor=request.result;if(!cursor)return;if(items.length>=50){next=items.at(-1)?.key??null;return}const record=cursor.value as EditorRecoveryRecord;if(record.schemaVersion===1&&record.key.startsWith(prefix)&&record.latest)items.push({key:record.key,documentCount:record.latest.documents.length,paths:record.latest.documents.slice(0,8).map(document=>document.path)});cursor.continue()}
    transaction.oncomplete=()=>resolve({items,next});transaction.onerror=()=>reject(transaction.error??new Error('未能读取本机编辑副本。'));transaction.onabort=()=>reject(transaction.error??new Error('本机编辑副本读取中断。'))
  })}finally{database.close()}
}
export async function readEditorCopy(key:string):Promise<EditorRecoveryRecord>{
  const database=await open()
  try{return await new Promise((resolve,reject)=>{const transaction=database.transaction(STORE,'readonly'),request=transaction.objectStore(STORE).get(key);let value:EditorRecoveryRecord|undefined;request.onsuccess=()=>{value=request.result};transaction.oncomplete=()=>{if(!value||value.schemaVersion!==1||value.key!==key)reject(new Error('原编辑副本已不可用。'));else resolve(structuredClone(value))};transaction.onerror=()=>reject(transaction.error??new Error('未能读取原编辑副本。'))})}finally{database.close()}
}
