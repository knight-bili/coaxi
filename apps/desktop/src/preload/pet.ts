import { contextBridge,ipcRenderer } from 'electron'
contextBridge.exposeInMainWorld('coaxiPet',{
  request:(input:unknown)=>ipcRenderer.invoke('coaxi:pet-request',input),
  hitTest:(input:unknown)=>ipcRenderer.invoke('coaxi:pet-hit-test',input),
  control:(action:'focus'|'hide'|'menu')=>ipcRenderer.invoke('coaxi:pet-control',action),
  onState:(listener:(value:{state:string;changed?:boolean})=>void)=>{const receive=(_event:Electron.IpcRendererEvent,value:{state:string;changed?:boolean}):void=>listener(value);ipcRenderer.on('coaxi:pet-state',receive);return()=>ipcRenderer.removeListener('coaxi:pet-state',receive)}
})
