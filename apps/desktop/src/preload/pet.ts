import { contextBridge,ipcRenderer } from 'electron'
contextBridge.exposeInMainWorld('rovaiPet',{
  request:(input:unknown)=>ipcRenderer.invoke('rovai:pet-request',input),
  control:(action:'focus'|'hide'|'menu')=>ipcRenderer.invoke('rovai:pet-control',action),
  onState:(listener:(value:{state:string;changed?:boolean})=>void)=>{const receive=(_event:Electron.IpcRendererEvent,value:{state:string;changed?:boolean}):void=>listener(value);ipcRenderer.on('rovai:pet-state',receive);return()=>ipcRenderer.removeListener('rovai:pet-state',receive)}
})
