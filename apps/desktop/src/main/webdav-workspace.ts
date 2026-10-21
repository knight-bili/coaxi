import { BrowserWindow, dialog, type IpcMainInvokeEvent } from 'electron'
import type { CoreMethod } from '@contracts'
import { backupFileDigest } from './backup-workspace'
type Request=<T>(method:CoreMethod,params?:unknown)=>Promise<T>
export class WebdavWorkspace{
  constructor(private readonly request:Request,private readonly owner:()=>BrowserWindow|null){}
  async invoke(event:IpcMainInvokeEvent,value:unknown):Promise<unknown>{
    const owner=this.owner()
    if(!owner||owner.isDestroyed()||event.sender!==owner.webContents||event.senderFrame!==owner.webContents.mainFrame)throw new Error('webdav.local_owner_required')
    if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('webdav.invalid_request')
    const input=value as {operation:string;expectedRevision:string;commandId?:string;snapshotId?:string;expectedDigest?:string}
    if(typeof input.expectedRevision!=='string')throw new Error('webdav.settings_revision_required')
    if(['list','probe','create_collection'].includes(input.operation))return this.request('backup.webdav.local',{operation:input.operation,expectedRevision:input.expectedRevision})
    if(typeof input.commandId!=='string')throw new Error('webdav.command_required')
    if(input.operation==='status')return this.request('backup.webdav.local',{operation:'status',commandId:input.commandId,expectedRevision:input.expectedRevision})
    if(input.operation==='push'){
      const selected=await dialog.showOpenDialog(owner,{title:'选择要上传的加密备份',properties:['openFile'],filters:[{name:'加密备份',extensions:['rovai-backup','codegbak']}]})
      if(selected.canceled||!selected.filePaths[0])return null
      const path=selected.filePaths[0],expectedDigest=await backupFileDigest(path)
      return this.request('backup.webdav.local',{operation:'push',expectedRevision:input.expectedRevision,commandId:input.commandId,path,expectedDigest})
    }
    if(input.operation==='pull'){
      if(typeof input.snapshotId!=='string'||typeof input.expectedDigest!=='string')throw new Error('webdav.snapshot_required')
      const selected=await dialog.showSaveDialog(owner,{title:'保存下载的加密备份',defaultPath:`Rovai-${input.snapshotId}.rovai-backup`,filters:[{name:'加密备份',extensions:['rovai-backup']}]})
      if(selected.canceled||!selected.filePath)return null
      return this.request('backup.webdav.local',{operation:'pull',expectedRevision:input.expectedRevision,commandId:input.commandId,snapshotId:input.snapshotId,path:selected.filePath,expectedDigest:input.expectedDigest})
    }
    throw new Error('webdav.operation_not_admitted')
  }
}
