import { createHash,randomUUID } from 'node:crypto'
import { lstatSync,mkdirSync,readFileSync,readdirSync,renameSync,unlinkSync,writeFileSync } from 'node:fs'
import { join } from 'node:path'

/** 桌面原稿的主副本位于受备份覆盖的用户目录；Chromium 存储只作缓存。 */
export class WorkbenchEditorRecovery {
  constructor(private readonly root:()=>string){}
  private directory(key:string):string{const marker=key.lastIndexOf(':copy:'),base=marker>=0&&/^[0-9a-f-]{36}$/i.test(key.slice(marker+6))?key.slice(0,marker):key;const directory=join(this.root(),'workbench-editor-recovery-v1',createHash('sha256').update(base).digest('hex'));mkdirSync(directory,{recursive:true,mode:0o700});const stat=lstatSync(directory);if(!stat.isDirectory()||stat.isSymbolicLink())throw new Error('工程编辑恢复目录不可用。');return directory}
  private path(key:unknown):string{if(typeof key!=='string'||!key||key.length>32768||key.includes('\0'))throw new Error('工程编辑恢复归属无效。');return join(this.directory(key),`${createHash('sha256').update(key).digest('hex')}.json`)}
  private read(path:string):Record<string,unknown>{const stat=lstatSync(path);if(!stat.isFile()||stat.isSymbolicLink()||stat.size>40*1024*1024)throw new Error('工程编辑恢复记录无效。');const value=JSON.parse(readFileSync(path,'utf8'))as Record<string,unknown>;if(!value||value.schemaVersion!==1||typeof value.key!=='string'||typeof value.editorId!=='string')throw new Error('工程编辑恢复记录损坏，原文件保留。');if(this.path(value.key)!==path)throw new Error('工程编辑恢复记录归属不一致。');return value}
  get(key:string):unknown|null{const path=this.path(key);try{return this.read(path)}catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')return null;throw error}}
  put(value:unknown):void{
    const row=value as Record<string,unknown>;if(!row||row.schemaVersion!==1||typeof row.key!=='string'||typeof row.editorId!=='string')throw new Error('工程编辑恢复记录无效。')
    const path=this.path(row.key),bytes=Buffer.from(JSON.stringify(row));if(bytes.length>40*1024*1024)throw new Error('工程编辑恢复记录过大。')
    this.get(row.key)
    const temporary=`${path}.${randomUUID()}.tmp`
    try{writeFileSync(temporary,bytes,{flag:'wx',mode:0o600,flush:true});renameSync(temporary,path)}finally{try{unlinkSync(temporary)}catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT')throw error}}
  }
  copies(key:string,after:string|null):unknown{
    this.path(key);const prefix=`${key}:copy:`
    if(after&&!after.startsWith(prefix))throw new Error('原编辑副本不属于这个工程。')
    const items:Array<{key:string;documentCount:number;paths:string[]}>=[]
    const directory=this.directory(key)
    for(const name of readdirSync(directory).sort()){
      if(!/^[0-9a-f]{64}\.json$/.test(name))continue
      const row=this.read(join(directory,name));if(typeof row.key!=='string'||!row.key.startsWith(prefix)||after&&row.key<=after)continue
      const latest=row.latest as {documents?:Array<{path:string}>}|null
      if(latest&&Array.isArray(latest.documents))items.push({key:row.key,documentCount:latest.documents.length,paths:latest.documents.slice(0,8).map(document=>document.path)})
    }
    items.sort((a,b)=>a.key<b.key?-1:a.key>b.key?1:0);const more=items.length>50;return {items:items.slice(0,50),next:more?items[49].key:null}
  }
}
