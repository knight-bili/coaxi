import { BrowserWindow, dialog, type IpcMainInvokeEvent } from 'electron'
import type { BackupSummary, ConfigurationSnapshotPreview, LegacyDataPreview } from '@contracts'
import { spawn } from 'node:child_process'
import { createReadStream } from 'node:fs'
import { createHash, randomUUID } from 'node:crypto'
import { resolveCoreBinary, type CoreClient } from './core-client'
type Manifest={formatVersion:number;product:string;appVersion:string;createdAt:string;entries:Array<{rootId?:string;path:string;size:number;sha256:string}>;roots?:NonNullable<BackupSummary['roots']>;runtimeLinks?:Array<{path:string;target:string;directory:boolean}>;fileModes?:Record<string,number>}
const summary=(manifest:Manifest):BackupSummary=>({formatVersion:manifest.formatVersion,roots:manifest.roots??[],createdAt:manifest.createdAt,appVersion:manifest.appVersion,fileCount:manifest.entries.length,totalBytes:manifest.entries.reduce((sum,entry)=>sum+entry.size,0),runtimeLinkCount:manifest.runtimeLinks?.length??0,executableModeCount:Object.values(manifest.fileModes??{}).filter(mode=>(mode&0o111)!==0).length,runtimeEnvironmentCheckRequired:Boolean(manifest.runtimeLinks?.length||manifest.entries.some(entry=>(!entry.rootId||entry.rootId==='data')&&(entry.path.startsWith('custom-runtimes/')||entry.path==='custom-runtime-restore-needs-environment-v1.json')))})
export async function backupFileDigest(path:string):Promise<string>{const hash=createHash('sha256');for await(const chunk of createReadStream(path))hash.update(chunk);return hash.digest('hex')}
/** Offline helper is the sole reader/writer of backup archives. Renderer
 * receives an expiring selection id and a summary, never a passphrase receipt. */
export class BackupWorkspace{
  private busy=false
  private readonly configSelections=new Map<string,{path:string;digest:string;localDigest:string;preferencesRoot:string;expires:number}>()
  private readonly selections=new Map<string,{path:string;digest:string;expires:number}>()
  private readonly legacySelections=new Map<string,{source:string;digest:string;expires:number}>()
  constructor(private readonly core:Pick<CoreClient,'shutdown'|'start'>,private readonly dataRoot:()=>string|null,private readonly owner:()=>BrowserWindow|null,private readonly prepare:()=>Promise<void>,private readonly beforeOffline:()=>void,private readonly restartApp:()=>void,private readonly preferencesRoot:()=>string|null=dataRoot){}
  private execute<T={manifest:Manifest}>(operation:string,root:string,file:string|undefined,passphrase:string,event:IpcMainInvokeEvent,command='backup',extra:string[]=[]):Promise<T>{
    return new Promise((resolve,reject)=>{
      const env:NodeJS.ProcessEnv={LANG:'C.UTF-8',LC_ALL:'C.UTF-8'};for(const key of ['PATH','SystemRoot','WINDIR','HOME','USERPROFILE','TEMP','TMP','LOCALAPPDATA'])if(process.env[key])env[key]=process.env[key]
      const child=spawn(resolveCoreBinary(),[command,...(operation?[operation]:[]),'--data-dir',root,...(file?['--file',file]:[]),...extra],{stdio:['pipe','pipe','pipe'],windowsHide:true,env})
      let output='',errors='',pending=''
      child.stdout.setEncoding('utf8');child.stderr.setEncoding('utf8')
      child.stdout.on('data',(chunk:string)=>{output+=chunk;if(output.length>40*1024*1024)child.kill()})
      child.stderr.on('data',(chunk:string)=>{errors=(errors+chunk).slice(-16000);pending+=chunk;const lines=pending.split('\n');pending=lines.pop()??'';for(const line of lines){try{const value=JSON.parse(line) as {phase?:unknown;done?:unknown;total?:unknown};if(typeof value.phase==='string'&&typeof value.done==='number'&&typeof value.total==='number'&&!event.sender.isDestroyed())event.sender.send('rovai:backup-progress',value)}catch{/* diagnostic text stays outside the progress stream */}}})
      child.once('error',reject)
      child.once('close',code=>{if(code!==0){reject(new Error(errors||'backup.helper_failed'));return}try{resolve(JSON.parse(output.trim()) as T)}catch{reject(new Error('backup.invalid_helper_response'))}})
      child.stdin.on('error',()=>undefined);child.stdin.end(JSON.stringify({passphrase}))
    })
  }
  private async invokeConfiguration(event:IpcMainInvokeEvent,operation:string,input:{selectionId?:string},owner:BrowserWindow,root:string,passphrase:string):Promise<unknown>{
    if(!['config_export','config_inspect','config_apply'].includes(operation))throw new Error('config_sync.operation_not_admitted')
    const preferences=this.preferencesRoot()??root;const extra=['--preferences-root',preferences];this.busy=true;let offline=false,restarting=false
    try{
      if(operation==='config_export'){
        if([...passphrase].length<8)throw new Error('config_sync.passphrase_too_short')
        const selected=await dialog.showSaveDialog(owner,{title:'导出加密配置',defaultPath:`Rovai-settings-${new Date().toISOString().slice(0,10)}.rovai-config`,filters:[{name:'加密配置',extensions:['rovai-config']}]});if(selected.canceled||!selected.filePath)return null
        return this.execute('export',root,selected.filePath,passphrase,event,'config-snapshot',extra)
      }
      if(operation==='config_inspect'){
        const selected=await dialog.showOpenDialog(owner,{title:'预览加密配置',properties:['openFile'],filters:[{name:'加密配置',extensions:['rovai-config']}]});if(selected.canceled||!selected.filePaths[0])return null
        const path=selected.filePaths[0],digest=await backupFileDigest(path),preview=await this.execute<ConfigurationSnapshotPreview>('inspect',root,path,passphrase,event,'config-snapshot',[...extra,'--summary-only']);if(await backupFileDigest(path)!==digest)throw new Error('config_sync.archive_changed')
        const id=randomUUID();this.configSelections.set(id,{path,digest,localDigest:preview.localDigest,preferencesRoot:preferences,expires:Date.now()+15*60000});return {...preview,selectionId:id}
      }
      const selection=input.selectionId?this.configSelections.get(input.selectionId):undefined
      if(!selection||selection.expires<Date.now()||selection.preferencesRoot!==preferences||await backupFileDigest(selection.path)!==selection.digest)throw new Error('config_sync.selection_expired_or_changed')
      await this.prepare();this.beforeOffline();const stopped=await this.core.shutdown();offline=true;if(stopped.forcedSignal!==null)throw new Error('config_sync.core_shutdown_incomplete')
      const result=await this.execute<{applied:boolean;restartRequired:boolean}>('apply',root,selection.path,passphrase,event,'config-snapshot',[...extra,'--expected-local-digest',selection.localDigest]);restarting=true;setImmediate(this.restartApp);return result
    }finally{this.busy=false;if(offline&&!restarting)this.core.start()}
  }
  /** Main-only handoff from an immutable, completed WebDAV pull receipt. */
  async inspectDownloaded(event:IpcMainInvokeEvent,path:string,passphrase:string,kind:'backup'|'configuration',expectedDigest:string):Promise<unknown>{
    const owner=this.owner(),root=this.dataRoot(),preferences=this.preferencesRoot()
    if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame||!root||!preferences)throw new Error('backup.local_owner_required')
    if(this.busy)throw new Error('backup.operation_in_progress')
    if(typeof passphrase!=='string'||passphrase.length>16000||await backupFileDigest(path)!==expectedDigest)throw new Error('backup.archive_changed')
    this.busy=true
    try{
      if(kind==='configuration'){
        const preview=await this.execute<ConfigurationSnapshotPreview>('inspect',root,path,passphrase,event,'config-snapshot',['--preferences-root',preferences,'--summary-only'])
        if(await backupFileDigest(path)!==expectedDigest)throw new Error('config_sync.archive_changed')
        const id=randomUUID();this.configSelections.set(id,{path,digest:expectedDigest,localDigest:preview.localDigest,preferencesRoot:preferences,expires:Date.now()+15*60000})
        return {kind,selectionId:id,...preview}
      }
      const result=await this.execute('inspect',root,path,passphrase,event)
      if(await backupFileDigest(path)!==expectedDigest)throw new Error('backup.archive_changed')
      const id=randomUUID();this.selections.set(id,{path,digest:expectedDigest,expires:Date.now()+15*60000})
      return {kind,archiveId:id,...summary(result.manifest)}
    }finally{this.busy=false}
  }
  async invoke(event:IpcMainInvokeEvent,operation:unknown,input:unknown):Promise<unknown>{
    const owner=this.owner(),root=this.dataRoot()
    if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame||!root)throw new Error('backup.local_owner_required')
    if(this.busy)throw new Error('backup.operation_in_progress')
    if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('backup.invalid_request')
    const {passphrase,archiveId}=input as {passphrase:string;archiveId?:string}
    if(typeof passphrase!=='string'||passphrase.length>16000)throw new Error('backup.invalid_passphrase')
    if(operation==='legacy_inspect'||operation==='legacy_stage')return this.invokeLegacy(event,operation,input as {selectionId?:string},owner,root)
    if(typeof operation==='string'&&operation.startsWith('config_'))return this.invokeConfiguration(event,operation,input as {selectionId?:string},owner,root,passphrase)
    this.busy=true;let offline=false,restarting=false
    try{
      if(operation==='inspect'){
        const chosen=await dialog.showOpenDialog(owner,{title:'选择加密备份',properties:['openFile'],filters:[{name:'加密备份',extensions:['rovai-backup','codegbak']}]})
        if(chosen.canceled||!chosen.filePaths[0])return null
        const path=chosen.filePaths[0],before=await backupFileDigest(path),result=await this.execute('inspect',root,path,passphrase,event)
        if(await backupFileDigest(path)!==before)throw new Error('backup.archive_changed')
        const id=randomUUID();this.selections.set(id,{path,digest:before,expires:Date.now()+15*60000});return {archiveId:id,...summary(result.manifest)}
      }
      let file:string
      if(operation==='create'){
        if([...passphrase].length<8)throw new Error('backup.passphrase_too_short')
        const destination=await dialog.showSaveDialog(owner,{title:'保存加密备份',defaultPath:`Rovai-${new Date().toISOString().slice(0,10)}.rovai-backup`,filters:[{name:'加密备份',extensions:['rovai-backup']}]})
        if(destination.canceled||!destination.filePath)return null
        file=destination.filePath
      }else if(operation==='restore'){
        const selection=archiveId?this.selections.get(archiveId):undefined
        if(!selection||selection.expires<Date.now()||await backupFileDigest(selection.path)!==selection.digest)throw new Error('backup.selection_expired_or_changed')
        file=selection.path
      }else throw new Error('backup.operation_not_admitted')
      await this.execute<{registered:boolean}>('',root,undefined,'',event,'backup-preferences',['--preferences-root',this.preferencesRoot()??root])
      await this.prepare();this.beforeOffline()
      const stopped=await this.core.shutdown();offline=true
      if(stopped.forcedSignal!==null)throw new Error('backup.core_shutdown_incomplete')
      const result=await this.execute(operation==='create'?'create':'stage',root,file,passphrase,event)
      if(operation==='restore'){restarting=true;setImmediate(this.restartApp);return {restoreStaged:true,...summary(result.manifest)}}
      return {path:file,...summary(result.manifest)}
    }finally{this.busy=false;if(offline&&!restarting)this.core.start()}
  }
  private async invokeLegacy(event:IpcMainInvokeEvent,operation:string,input:{selectionId?:string},owner:BrowserWindow,root:string):Promise<unknown>{
    this.busy=true;let offline=false,restarting=false
    try{
      for(const [id,value]of this.legacySelections)if(value.expires<Date.now())this.legacySelections.delete(id)
      if(operation==='legacy_inspect'){
        const chosen=await dialog.showOpenDialog(owner,{title:'选择已退出的旧 Rovai Core 数据目录',properties:['openDirectory']})
        if(chosen.canceled||!chosen.filePaths[0])return null
        if(this.legacySelections.size>=16)throw new Error('migration.too_many_selections')
        const value=await this.execute<{preview:Omit<LegacyDataPreview,'selectionId'>}>('legacy-inspect',root,undefined,'',event,'backup',['--source-dir',chosen.filePaths[0]])
        const id=randomUUID();this.legacySelections.set(id,{source:value.preview.source,digest:value.preview.digest,expires:Date.now()+15*60000})
        return {selectionId:id,...value.preview}
      }
      const selection=input.selectionId?this.legacySelections.get(input.selectionId):null
      if(!selection||selection.expires<Date.now())throw new Error('migration.preview_expired')
      await this.prepare();this.beforeOffline()
      const stopped=await this.core.shutdown();offline=true
      if(stopped.forcedSignal!==null)throw new Error('backup.core_shutdown_incomplete')
      const value=await this.execute('legacy-stage',root,undefined,'',event,'backup',['--source-dir',selection.source,'--expected-digest',selection.digest])
      restarting=true;setImmediate(this.restartApp)
      return {restoreStaged:true,...summary(value.manifest)}
    }finally{this.busy=false;if(offline&&!restarting)this.core.start()}
  }
}
