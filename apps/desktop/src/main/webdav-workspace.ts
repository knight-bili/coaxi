import { BrowserWindow, dialog, type IpcMainInvokeEvent } from 'electron'
import type { CoreMethod } from '@contracts'
import { backupFileDigest } from './backup-workspace'
type Request=<T>(method:CoreMethod,params?:unknown)=>Promise<T>
export class WebdavWorkspace{
  constructor(private readonly request:Request,private readonly owner:()=>BrowserWindow|null,private readonly preferencesRoot?:()=>string){}
  async invoke(event:IpcMainInvokeEvent,value:unknown):Promise<unknown>{
    const owner=this.owner()
    if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame)throw new Error('webdav.local_owner_required')
    if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('webdav.invalid_request')
    const input=value as {operation:string;expectedRevision:string|null;commandId?:string;snapshotId?:string;expectedDigest?:string;expectedIndexEtag?:string;confirmed?:boolean;kind?:'backup'|'configuration';enabled?:boolean;autoSync?:boolean;intervalMinutes?:number;passphrase?:string;clearPassphrase?:boolean;keepBackups?:number;keepConfigurations?:number}
    if(typeof input.operation!=='string')throw new Error('webdav.invalid_request')
    if(input.operation.startsWith('config_')){
      const operation=input.operation.slice(7);if(!['get','set','upload','cancel','resume','archive'].includes(operation))throw new Error('config_sync.operation_not_admitted')
      if(['set','upload'].includes(operation)&&this.preferencesRoot)await this.request('backup.storage.desktop.register',{preferencesRoot:this.preferencesRoot()})
      return this.request('backup.config.local',{operation,expectedRevision:input.expectedRevision,commandId:input.commandId,enabled:input.enabled,autoSync:input.autoSync,intervalMinutes:input.intervalMinutes,passphrase:input.passphrase,clearPassphrase:input.clearPassphrase,confirmed:input.confirmed})
    }
    if(typeof input.expectedRevision!=='string')throw new Error('webdav.settings_revision_required')
    if(['list','probe','create_collection','transfers'].includes(input.operation))return this.request('backup.webdav.local',{operation:input.operation,expectedRevision:input.expectedRevision})
    if(['removal_preview','removal_status','remove'].includes(input.operation)){
      if(typeof input.snapshotId!=='string'||typeof input.expectedDigest!=='string')throw new Error('webdav.snapshot_required')
      if(input.operation==='remove'&&(input.confirmed!==true||typeof input.expectedIndexEtag!=='string'))throw new Error('webdav.confirmation_required')
      return this.request('backup.webdav.local',{operation:input.operation,expectedRevision:input.expectedRevision,snapshotId:input.snapshotId,expectedDigest:input.expectedDigest,...(input.operation==='remove'?{confirmed:true,expectedIndexEtag:input.expectedIndexEtag}:{})})
    }
    if(['retention_preview','retention_apply','retention_status','cleanup_preview','cleanup_removed'].includes(input.operation)){
      if(input.operation==='retention_preview'&&(!Number.isInteger(input.keepBackups)||!Number.isInteger(input.keepConfigurations)))throw new Error('webdav.retention_required')
      if(['retention_apply','cleanup_removed'].includes(input.operation)&&input.confirmed!==true)throw new Error('webdav.confirmation_required')
      return this.request('backup.webdav.local',{operation:input.operation,expectedRevision:input.expectedRevision,commandId:input.commandId,keepBackups:input.keepBackups,keepConfigurations:input.keepConfigurations,snapshotId:input.snapshotId,expectedDigest:input.expectedDigest,confirmed:input.confirmed})
    }
    if(typeof input.commandId!=='string')throw new Error('webdav.command_required')
    if(['status','cancel','resume'].includes(input.operation))return this.request('backup.webdav.local',{operation:input.operation,commandId:input.commandId,expectedRevision:input.expectedRevision})
    if(input.operation==='push'){
      const selected=await dialog.showOpenDialog(owner,{title:'选择要上传的加密备份',properties:['openFile'],filters:[{name:'加密备份',extensions:['rovai-backup','codegbak','rovai-config']}]})
      if(selected.canceled||!selected.filePaths[0])return null
      const path=selected.filePaths[0],expectedDigest=await backupFileDigest(path)
      return this.request('backup.webdav.local',{operation:'push',expectedRevision:input.expectedRevision,commandId:input.commandId,path,expectedDigest,kind:path.toLowerCase().endsWith('.rovai-config')?'configuration':'backup'})
    }
    if(input.operation==='pull'){
      if(typeof input.snapshotId!=='string'||typeof input.expectedDigest!=='string')throw new Error('webdav.snapshot_required')
      const selected=await dialog.showSaveDialog(owner,{title:'保存下载的加密备份',defaultPath:`Rovai-${input.snapshotId}.${input.kind==='configuration'?'rovai-config':'rovai-backup'}`,filters:[{name:'加密备份',extensions:['rovai-backup']}]})
      if(selected.canceled||!selected.filePath)return null
      return this.request('backup.webdav.local',{operation:'pull',expectedRevision:input.expectedRevision,commandId:input.commandId,snapshotId:input.snapshotId,path:selected.filePath,expectedDigest:input.expectedDigest})
    }
    throw new Error('webdav.operation_not_admitted')
  }
}
