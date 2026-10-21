import { BrowserWindow, dialog, type IpcMainInvokeEvent } from 'electron'
import type { BackupSummary } from '@contracts'
import { spawn } from 'node:child_process'
import { createReadStream } from 'node:fs'
import { createHash, randomUUID } from 'node:crypto'
import { resolveCoreBinary, type CoreClient } from './core-client'
type Manifest={formatVersion:number;product:string;appVersion:string;createdAt:string;entries:Array<{path:string;size:number;sha256:string}>;roots?:NonNullable<BackupSummary['roots']>}
const summary=(manifest:Manifest):BackupSummary=>({formatVersion:manifest.formatVersion,roots:manifest.roots??[],createdAt:manifest.createdAt,appVersion:manifest.appVersion,fileCount:manifest.entries.length,totalBytes:manifest.entries.reduce((sum,entry)=>sum+entry.size,0)})
export async function backupFileDigest(path:string):Promise<string>{const hash=createHash('sha256');for await(const chunk of createReadStream(path))hash.update(chunk);return hash.digest('hex')}
/** Offline helper is the sole reader/writer of backup archives. Renderer
 * receives an expiring selection id and a summary, never a passphrase receipt. */
export class BackupWorkspace{
  private busy=false
  private readonly selections=new Map<string,{path:string;digest:string;expires:number}>()
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
  async invoke(event:IpcMainInvokeEvent,operation:unknown,input:unknown):Promise<unknown>{
    const owner=this.owner(),root=this.dataRoot()
    if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame||!root)throw new Error('backup.local_owner_required')
    if(this.busy)throw new Error('backup.operation_in_progress')
    if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('backup.invalid_request')
    const {passphrase,archiveId}=input as {passphrase:string;archiveId?:string}
    if(typeof passphrase!=='string'||passphrase.length>16000)throw new Error('backup.invalid_passphrase')
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
}
