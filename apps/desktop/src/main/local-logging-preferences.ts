import {lstatSync,readFileSync} from 'node:fs'
import {createHash} from 'node:crypto'
import {writePrivateJson} from './general-preferences'
import {applyOwnedPreferenceFile,type PortablePreferencePlan,type PreferenceApplyState} from './portable-preference-file'

const levels=['off','error','warn','info','debug','trace'] as const
type Level=typeof levels[number]
export type LocalLogSettings={level:Level;targets:Array<{target:string;level:Level}>}
let active:LocalLogSettings={level:'trace',targets:[]}
let installed=false
function parse(value:unknown):LocalLogSettings{
  if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('logging.settings_invalid')
  const row=value as Record<string,unknown>,targets=row.targets??[]
  if(Object.keys(row).some(key=>!['schemaVersion','level','targets'].includes(key))||!levels.includes(row.level as Level)||!Array.isArray(targets)||targets.length>1024)throw new Error('logging.settings_invalid')
  const seen=new Set<string>()
  return {level:row.level as Level,targets:targets.map(input=>{
    const item=input as Record<string,unknown>
    if(!item||Object.keys(item).length!==2||typeof item.target!=='string'||!/^[a-zA-Z0-9_.:-]{1,256}$/.test(item.target)||!levels.includes(item.level as Level)||seen.has(item.target))throw new Error('logging.target_invalid')
    seen.add(item.target);return {target:item.target,level:item.level as Level}
  })}
}
function persisted(value:LocalLogSettings):unknown{return {schemaVersion:1,...value}}
function read(path:string):unknown{
  try{const metadata=lstatSync(path);if(!metadata.isFile()||metadata.isSymbolicLink()||metadata.size>1024*1024)throw new Error('logging.file_invalid');return JSON.parse(readFileSync(path,'utf8'))}
  catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')return null;throw error}
}
function digest(value:unknown):string{return createHash('sha256').update(JSON.stringify(value===null?null:persisted(parse(value)))).digest('hex')}
function install():void{
  if(installed)return;installed=true
  const names={error:'error',warn:'warn',info:'info',log:'info',debug:'debug',trace:'trace'} as const
  for(const method of Object.keys(names) as Array<keyof typeof names>){
    const original=console[method].bind(console)
    console[method]=(...args:unknown[]):void=>{
      const target=typeof args[0]==='string'?/^\[([a-zA-Z0-9_.:-]{1,256})\]/.exec(args[0])?.[1]??'rovai-main':'rovai-main'
      const specific=active.targets.filter(value=>target.startsWith(value.target)).sort((a,b)=>b.target.length-a.target.length)[0]
      const threshold=specific?.level??active.level
      if(threshold!=='off'&&levels.indexOf(names[method])<=levels.indexOf(threshold))original(...args)
    }
  }
}

/** 控制台输出与结构化 Core 帧分开；过滤不影响事件、回执、审批或运行状态。 */
export class LocalLoggingPreferences {
  private tail:Promise<unknown>=Promise.resolve()
  private settings:LocalLogSettings={level:'trace',targets:[]}
  constructor(private readonly path:string){try{const value=read(path);if(value!==null)this.settings=parse(value)}catch{/* 原文件保留，只用内存默认值。 */}}
  install():void{active=this.settings;install()}
  private enqueue<T>(work:()=>Promise<T>):Promise<T>{const next=this.tail.then(work,work);this.tail=next.catch(()=>undefined);return next}
  get():Promise<{settings:LocalLogSettings;digest:string}>{return this.enqueue(async()=>{
    const value=read(this.path);this.settings=value===null?{level:'trace',targets:[]}:parse(value);active=this.settings
    return {settings:structuredClone(this.settings),digest:digest(value)}
  })}
  set(expected:string,value:unknown):Promise<{settings:LocalLogSettings;digest:string}>{return this.enqueue(async()=>{
    if(digest(read(this.path))!==expected)throw new Error('logging.settings_changed')
    const settings=parse(value);await writePrivateJson(this.path,persisted(settings));this.settings=settings;active=settings
    return {settings:structuredClone(settings),digest:digest(persisted(settings))}
  })}
  applyConfiguration(plan:PortablePreferencePlan):Promise<PreferenceApplyState>{return this.enqueue(async()=>{
    if(plan.targetId!=='logging')throw new Error('configuration.owner_target_invalid')
    const settings=plan.after===null?{level:'trace' as const,targets:[]}:parse(plan.after)
    const state=await applyOwnedPreferenceFile(this.path,plan,value=>writePrivateJson(this.path,value))
    if(state==='applied'){this.settings=settings;active=settings}return state
  })}
}
