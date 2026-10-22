import { BrowserWindow,dialog,type IpcMainInvokeEvent } from 'electron'
import { constants } from 'node:fs'
import { lstat,open,rename,unlink } from 'node:fs/promises'
import { basename,dirname,join } from 'node:path'
import { createHash,randomUUID } from 'node:crypto'
import type { CoreClient } from './core-client'

type Selection={path:string;sourceDigest:string;byteSize:number;expires:number}
export class PortableConfigurationPrivate{
  private readonly selections=new Map<string,Selection>()
  constructor(private readonly core:Pick<CoreClient,'request'>,private readonly owner:()=>BrowserWindow|null,private readonly logging?:(input:unknown)=>Promise<unknown>){}
  async registerDownloaded(event:IpcMainInvokeEvent,path:string,passphrase:string,expectedDigest:string,operationId:string):Promise<unknown>{
    const owner=this.owner()
    if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame)throw new Error('configuration.local_owner_required')
    if(!/^[a-f0-9]{64}$/i.test(expectedDigest)||passphrase.length>16000)throw new Error('configuration.download_invalid')
    const file=await lstat(path)
    if(!file.isFile()||file.isSymbolicLink()||file.size>64*1024*1024)throw new Error('configuration.download_invalid')
    const archive=await this.core.request('configuration.archive.registerSelected',{operationId,path,sourceDigest:`sha256:${expectedDigest.toLowerCase()}`,byteSize:file.size,passphrase,manifest:null})
    return {kind:'configuration-migration',archive}
  }
  async invoke(event:IpcMainInvokeEvent,operation:unknown,input:unknown):Promise<unknown>{
    const owner=this.owner()
    if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame)throw new Error('configuration.local_owner_required')
    if(!input||typeof input!=='object'||Array.isArray(input)||Buffer.byteLength(JSON.stringify(input),'utf8')>2*1024*1024)throw new Error('configuration.input_invalid')
    const value=input as Record<string,unknown>
    for(const[id,selection]of this.selections)if(selection.expires<Date.now())this.selections.delete(id)
    if(operation==='secret')return this.core.request('configuration.secrets.prepare',input)
    if(operation==='edit')return this.core.request('configuration.providers.privateEdit',input)
    if(operation==='receipt')return this.core.request('configuration.archive.receipt',input)
    if(operation==='exportPrepare')return this.core.request('configuration.export.prepare',input)
    if(operation==='exportReceipt')return this.core.request('configuration.export.receipt',input)
    if(operation==='sync')return this.core.request('configuration.sync',input)
    if(operation==='logging'&&this.logging)return this.logging(input)
    if(operation==='exportSave'){
      if(Object.keys(value).length!==1||typeof value.operationId!=='string')throw new Error('configuration.export_input_invalid')
      const result=await this.core.request<{artifactPath:string;encryptedDigest:string;byteSize:number;state:string}>('configuration.export.read',input)
      if(result.state!=='ready'||typeof result.artifactPath!=='string'||result.byteSize>64*1024*1024)throw new Error('configuration.export_not_ready')
      const selected=await dialog.showSaveDialog(owner,{title:'保存完整配置加密副本',defaultPath:'configuration.rovai-config',filters:[{name:'加密配置',extensions:['rovai-config']}]})
      if(selected.canceled||!selected.filePath)return {saved:false}
      const original=await lstat(result.artifactPath)
      if(!original.isFile()||original.isSymbolicLink()||original.size!==result.byteSize)throw new Error('configuration.export_changed')
      const source=await open(result.artifactPath,constants.O_RDONLY|(constants.O_NOFOLLOW??0))
      const temporary=join(dirname(selected.filePath),`.rovai-export-${randomUUID()}.tmp`)
      let output:Awaited<ReturnType<typeof open>>|null=null
      try{
        const observed=await source.stat();if(observed.ino!==original.ino||observed.dev!==original.dev||observed.size!==original.size)throw new Error('configuration.export_changed')
        output=await open(temporary,'wx',0o600);const hash=createHash('sha256'),buffer=Buffer.alloc(64*1024);let bytes=0
        for(;;){const read=await source.read(buffer,0,buffer.length,null);if(!read.bytesRead)break;bytes+=read.bytesRead;if(bytes>result.byteSize)throw new Error('configuration.export_changed');const chunk=buffer.subarray(0,read.bytesRead);hash.update(chunk);await output.writeFile(chunk)}
        if(bytes!==result.byteSize||`sha256:${hash.digest('hex')}`!==result.encryptedDigest)throw new Error('configuration.export_changed')
        await output.sync();await output.close();output=null;await rename(temporary,selected.filePath)
        return {saved:true}
      }finally{await source.close();await output?.close();await unlink(temporary).catch(()=>undefined)}
    }
    if(operation==='select'){
      if(Object.keys(value).length||this.selections.size>=128)throw new Error('configuration.selection_limit')
      const chosen=await dialog.showOpenDialog(owner,{title:'选择原来的模型和设置配置文件',properties:['openFile'],filters:[{name:'配置文件',extensions:['json','codeg-config','rovai-config']}]})
      if(chosen.canceled||!chosen.filePaths[0])return null
      const path=chosen.filePaths[0],before=await lstat(path)
      if(!before.isFile()||before.isSymbolicLink()||before.size>64*1024*1024)throw new Error('configuration.source_invalid')
      const file=await open(path,constants.O_RDONLY|(constants.O_NOFOLLOW??0))
      let bytes=0;const hash=createHash('sha256')
      try{
        const observed=await file.stat()
        if(!observed.isFile()||observed.ino!==before.ino||observed.dev!==before.dev||observed.size!==before.size)throw new Error('configuration.source_changed')
        const buffer=Buffer.alloc(64*1024)
        for(;;){const result=await file.read(buffer,0,buffer.length,null);if(!result.bytesRead)break;bytes+=result.bytesRead;if(bytes>64*1024*1024)throw new Error('configuration.source_too_large');hash.update(buffer.subarray(0,result.bytesRead))}
        const after=await file.stat();if(bytes!==before.size||after.size!==before.size||after.mtimeMs!==observed.mtimeMs)throw new Error('configuration.source_changed')
      }finally{await file.close()}
      const selectionId=randomUUID(),sourceDigest=`sha256:${hash.digest('hex')}`
      this.selections.set(selectionId,{path,sourceDigest,byteSize:bytes,expires:Date.now()+10*60000})
      return {selectionId,displayName:basename(path),sourceDigest,byteSize:bytes}
    }
    if(operation==='import'){
      if(Object.keys(value).some(key=>!['selectionId','operationId','passphrase','manifest'].includes(key))||typeof value.selectionId!=='string'||typeof value.operationId!=='string'||typeof value.passphrase!=='string'||value.passphrase.length>16000)throw new Error('configuration.input_invalid')
      const source=this.selections.get(value.selectionId)
      if(!source)throw new Error('configuration.selection_expired')
      return this.core.request('configuration.archive.registerSelected',{operationId:value.operationId,path:source.path,sourceDigest:source.sourceDigest,byteSize:source.byteSize,passphrase:value.passphrase,manifest:value.manifest??null})
    }
    throw new Error('configuration.private_operation_unknown')
  }
}
