import {contextBridge,ipcRenderer} from 'electron'
import type {ProjectLauncherWindowApi} from '../shared/project-launcher'
const api:ProjectLauncherWindowApi={bootstrap:()=>ipcRenderer.invoke('coaxi:project-launcher','bootstrap'),chooseDirectory:()=>ipcRenderer.invoke('coaxi:project-launcher','chooseDirectory'),request:input=>ipcRenderer.invoke('coaxi:project-launcher','request',input),enter:commandId=>ipcRenderer.invoke('coaxi:project-launcher','enter',commandId)}
contextBridge.exposeInMainWorld('coaxiLauncher',api)
