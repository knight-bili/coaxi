import { randomUUID } from 'node:crypto'
import { promises as fs } from 'node:fs'
import { clipboard, dialog, ipcMain, nativeImage, type BrowserWindow, type IpcMainInvokeEvent } from 'electron'

/** 只接受当前可信应用窗口显式生成的有界 PNG；网页和模型不能调用。 */
export function installUsageCardExport(owner:()=>BrowserWindow|null):void {
  const read=(event:IpcMainInvokeEvent,value:unknown):{window:BrowserWindow;bytes:Buffer}=>{
    const window=owner()
    if(!window||window.isDestroyed()||event.sender!==window.webContents||event.senderFrame!==window.webContents.mainFrame)throw new Error('usage_card.local_owner_required')
    if(typeof value!=='string'||value.length>24*1024*1024||!/^data:image\/png;base64,[A-Za-z0-9+/]+=*$/.test(value))throw new Error('usage_card.invalid_image')
    const bytes=Buffer.from(value.slice(value.indexOf(',')+1),'base64')
    if(bytes.length<33||!bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))||bytes.toString('ascii',12,16)!=='IHDR')throw new Error('usage_card.invalid_image')
    const width=bytes.readUInt32BE(16),height=bytes.readUInt32BE(20)
    if(!width||!height||width>4096||height>4096||width*height>16_000_000)throw new Error('usage_card.image_too_large')
    return {window,bytes}
  }
  ipcMain.handle('rovai:usage-card-copy',(event,value:unknown)=>{
    const {bytes}=read(event,value),image=nativeImage.createFromBuffer(bytes)
    if(image.isEmpty())throw new Error('usage_card.invalid_image')
    clipboard.writeImage(image)
  })
  ipcMain.handle('rovai:usage-card-save',async(event,value:unknown)=>{
    const {window,bytes}=read(event,value)
    const result=await dialog.showSaveDialog(window,{title:'保存用量分享卡片',defaultPath:`codeg-rovai-usage-${new Date().toISOString().slice(0,10)}.png`,filters:[{name:'PNG',extensions:['png']}]})
    if(result.canceled||!result.filePath||window.isDestroyed())return null
    const temporary=`${result.filePath}.${randomUUID()}.tmp`
    try{await fs.writeFile(temporary,bytes,{mode:0o600,flag:'wx'});await fs.rename(temporary,result.filePath);await fs.chmod(result.filePath,0o600)}catch(error){await fs.unlink(temporary).catch(()=>undefined);throw error}
    return result.filePath
  })
}
