import {contextBridge,ipcRenderer} from 'electron'
import type {ProjectLauncherWindowApi} from '../shared/project-launcher'
const api:ProjectLauncherWindowApi={bootstrap:()=>ipcRenderer.invoke('rovai:project-launcher','bootstrap'),chooseDirectory:()=>ipcRenderer.invoke('rovai:project-launcher','chooseDirectory'),request:input=>ipcRenderer.invoke('rovai:project-launcher','request',input),enter:commandId=>ipcRenderer.invoke('rovai:project-launcher','enter',commandId)}
contextBridge.exposeInMainWorld('coaxiLauncher',api)
