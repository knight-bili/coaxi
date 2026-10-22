import { createHash, randomUUID } from 'node:crypto'
import { lstatSync, mkdirSync, readFileSync, readdirSync, renameSync, unlinkSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

export interface InteractionFinish {
  schemaVersion:1;desktopPeerId:string;connectionGeneration:number;bridgeCallId:string;
  bindingDigest:string;argumentsDigest:string;outcome:'completed'|'rejected'|'cancelled'|'unknown';result:unknown|null
}
const uuid=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const digest=/^[0-9a-f]{64}$/
const MAX_BYTES=33*1024*1024,MAX_TOTAL=256*1024*1024,MAX_FILES=256

/** 仅保存原请求及原结果，绝不把日志当成重新执行队列。 */
export class InteractionResultJournal {
  constructor(private readonly preferencesRoot:string,private readonly dataRoot:()=>string|null){}
  private directory():string{
    const root=this.dataRoot();if(!root)throw new Error('interaction.data_root_unavailable')
    const directory=join(this.preferencesRoot,'interaction-results-v1',createHash('sha256').update(root).digest('hex'))
    mkdirSync(directory,{recursive:true,mode:0o700})
    const stat=lstatSync(directory);if(!stat.isDirectory()||stat.isSymbolicLink())throw new Error('interaction.result_directory_invalid')
    return directory
  }
  private valid(value:unknown):InteractionFinish{
    const row=value as InteractionFinish
    if(!row||typeof row!=='object'||Array.isArray(row)||row.schemaVersion!==1
      ||Object.keys(row).sort().join(',')!=='argumentsDigest,bindingDigest,bridgeCallId,connectionGeneration,desktopPeerId,outcome,result,schemaVersion'
      ||!uuid.test(row.bridgeCallId)||!uuid.test(row.desktopPeerId)||!digest.test(row.bindingDigest)||!digest.test(row.argumentsDigest)
      ||!Number.isSafeInteger(row.connectionGeneration)||row.connectionGeneration<0
      ||!['completed','rejected','cancelled','unknown'].includes(row.outcome)||row.result===undefined)throw new Error('interaction.result_record_invalid')
    return row
  }
  load():InteractionFinish[]{
    const root=this.directory(),entries=readdirSync(root).filter(name=>name.endsWith('.json'))
    if(entries.length>MAX_FILES)throw new Error('interaction.result_journal_full')
    let total=0
    return entries.map(name=>{
      const id=name.slice(0,-5);if(!uuid.test(id))throw new Error('interaction.result_record_invalid')
      const file=join(root,name),stat=lstatSync(file)
      total+=stat.size
      if(!stat.isFile()||stat.isSymbolicLink()||stat.size>MAX_BYTES||total>MAX_TOTAL)throw new Error('interaction.result_journal_full')
      const row=this.valid(JSON.parse(readFileSync(file,'utf8')))
      if(row.bridgeCallId!==id)throw new Error('interaction.result_identity_changed')
      return row
    })
  }
  save(value:InteractionFinish):void{
    this.valid(value)
    const root=this.directory(),file=join(root,`${value.bridgeCallId}.json`),saved=this.load(),existing=saved.find(row=>row.bridgeCallId===value.bridgeCallId)
    if(existing&&(existing.bindingDigest!==value.bindingDigest||existing.argumentsDigest!==value.argumentsDigest||existing.desktopPeerId!==value.desktopPeerId))throw new Error('interaction.result_identity_changed')
    const bytes=Buffer.from(JSON.stringify(value))
    if(bytes.length>MAX_BYTES||saved.length-(existing?1:0)>=MAX_FILES
      ||saved.filter(row=>row!==existing).reduce((sum,row)=>sum+Buffer.byteLength(JSON.stringify(row)),bytes.length)>MAX_TOTAL)throw new Error('interaction.result_journal_full')
    const temporary=join(root,`${value.bridgeCallId}.${randomUUID()}.tmp`)
    try{writeFileSync(temporary,bytes,{flag:'wx',mode:0o600,flush:true});renameSync(temporary,file)}
    finally{try{unlinkSync(temporary)}catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT')throw error}}
  }
  remove(id:string):void{
    if(!uuid.test(id))throw new Error('interaction.result_identity_changed')
    const file=join(this.directory(),`${id}.json`)
    try{const stat=lstatSync(file);if(!stat.isFile()||stat.isSymbolicLink())throw new Error('interaction.result_record_invalid');unlinkSync(file)}catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT')throw error}
  }
}
