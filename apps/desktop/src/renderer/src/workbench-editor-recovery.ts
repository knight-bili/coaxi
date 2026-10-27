import type { CoaxiApi,WorkbenchEditorState } from '@contracts'
export interface EditorSaveRequest {operation:'save';scope:{threadId:string};editorId:string;workspaceRoot:string;expectedRevision:string|null;commandId:string;value:WorkbenchEditorState}
export interface EditorRecoveryRecord {schemaVersion:1;key:string;editorId:string;workspaceRoot:string|null;revision:string|null;latest:WorkbenchEditorState|null;pending:EditorSaveRequest|null}
export interface EditorRecoveryCopy {key:string;documentCount:number;paths:string[]}
const DATABASE='coaxi-editor-v1',STORE='records'
type NativeRecovery=CoaxiApi['workbenchEditorRecovery']
function record(value:unknown,key:string):EditorRecoveryRecord{
  const row=value as EditorRecoveryRecord
  if(!row||Object.keys(row).sort().join(',')!=='editorId,key,latest,pending,revision,schemaVersion,workspaceRoot'||row.schemaVersion!==1||row.key!==key||typeof row.editorId!=='string'||row.workspaceRoot!==null&&typeof row.workspaceRoot!=='string'||row.revision!==null&&typeof row.revision!=='string'||row.latest!==null&&(!row.latest||typeof row.latest!=='object')||row.pending!==null&&(!row.pending||typeof row.pending!=='object'))throw new Error('工程编辑恢复记录损坏，原记录仍保留。')
  for(const state of [row.latest,row.pending?.value])if(state){if(!Array.isArray(state.documents)||state.documents.length>128||state.selected!==null&&typeof state.selected!=='string'||state.documents.some(document=>!document||typeof document.path!=='string'||typeof document.text!=='string'||typeof document.saved!=='string'||document.digest!==null&&typeof document.digest!=='string'))throw new Error('工程编辑恢复正文损坏，原记录仍保留。')}
  return structuredClone(row)
}
const emptyRecord=(key:string):EditorRecoveryRecord=>({schemaVersion:1,key,editorId:crypto.randomUUID(),workspaceRoot:null,revision:null,latest:null,pending:null})
function open():Promise<IDBDatabase>{
  return new Promise((resolve,reject)=>{const request=indexedDB.open(DATABASE,1);request.onupgradeneeded=()=>{if(!request.result.objectStoreNames.contains(STORE))request.result.createObjectStore(STORE,{keyPath:'key'})};request.onerror=()=>reject(request.error??new Error('工程草稿的本机存储不可用。'));request.onblocked=()=>reject(new Error('工程草稿存储正在被另一版本占用。'));request.onsuccess=()=>resolve(request.result)})
}
export async function loadEditorRecovery(key:string,native?:NativeRecovery):Promise<EditorRecoveryRecord>{
  if(native){const found=await native.get(key);if(found!==null)return record(found,key);const value=emptyRecord(key);await native.put(value);return value}
  const database=await open()
  try{return await new Promise((resolve,reject)=>{const transaction=database.transaction(STORE,'readwrite'),store=transaction.objectStore(STORE),request=store.get(key);let result:EditorRecoveryRecord
    request.onsuccess=()=>{try{const found=request.result;if(found)result=record(found,key);else{result=emptyRecord(key);store.add(result)}}catch(error){transaction.abort();reject(error)}}
    transaction.oncomplete=()=>resolve(structuredClone(result));transaction.onerror=()=>reject(transaction.error??new Error('工程草稿记录读取失败。'));transaction.onabort=()=>reject(transaction.error??new Error('工程草稿记录读取中断。'))
  })}finally{database.close()}
}
export async function saveEditorRecovery(value:EditorRecoveryRecord,native?:NativeRecovery):Promise<void>{
  record(value,value.key)
  if(value.schemaVersion!==1||JSON.stringify(value).length>40*1024*1024)throw new Error('工程草稿超过本机恢复记录上限。')
  if(native){await native.put(value);return}
  const database=await open()
  try{await new Promise<void>((resolve,reject)=>{const transaction=database.transaction(STORE,'readwrite');transaction.objectStore(STORE).put(structuredClone(value));transaction.oncomplete=()=>resolve();transaction.onerror=()=>reject(transaction.error??new Error('工程草稿未保存到本机。'));transaction.onabort=()=>reject(transaction.error??new Error('工程草稿本机保存中断。'))})}finally{database.close()}
}
export async function listEditorCopies(key:string,after:string|null,native?:NativeRecovery):Promise<{items:EditorRecoveryCopy[];next:string|null}>{
  if(native)return native.copies(key,after)
  const database=await open(),prefix=`${key}:copy:`
  try{return await new Promise((resolve,reject)=>{const transaction=database.transaction(STORE,'readonly'),range=IDBKeyRange.bound(after??prefix,`${prefix}\uffff`,Boolean(after),false),request=transaction.objectStore(STORE).openCursor(range),items:EditorRecoveryCopy[]=[];let next:string|null=null
    request.onsuccess=()=>{const cursor=request.result;if(!cursor)return;if(items.length>=50){next=items.at(-1)?.key??null;return}const record=cursor.value as EditorRecoveryRecord;if(record.schemaVersion===1&&record.key.startsWith(prefix)&&record.latest)items.push({key:record.key,documentCount:record.latest.documents.length,paths:record.latest.documents.slice(0,8).map(document=>document.path)});cursor.continue()}
    transaction.oncomplete=()=>resolve({items,next});transaction.onerror=()=>reject(transaction.error??new Error('未能读取本机编辑副本。'));transaction.onabort=()=>reject(transaction.error??new Error('本机编辑副本读取中断。'))
  })}finally{database.close()}
}
export async function readEditorCopy(key:string,native?:NativeRecovery):Promise<EditorRecoveryRecord>{
  if(native){const value=await native.get(key);if(value===null)throw new Error('原编辑副本已不可用。');return record(value,key)}
  const database=await open()
  try{return await new Promise((resolve,reject)=>{const transaction=database.transaction(STORE,'readonly'),request=transaction.objectStore(STORE).get(key);let value:EditorRecoveryRecord|undefined;request.onsuccess=()=>{value=request.result};transaction.oncomplete=()=>{if(!value||value.schemaVersion!==1||value.key!==key)reject(new Error('原编辑副本已不可用。'));else resolve(structuredClone(value))};transaction.onerror=()=>reject(transaction.error??new Error('未能读取原编辑副本。'))})}finally{database.close()}
}
