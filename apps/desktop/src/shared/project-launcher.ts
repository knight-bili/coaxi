export interface CreatedProject {id:string;path:string}
export interface ProjectLauncherWindowApi {
  bootstrap():Promise<{directory:string;platform:NodeJS.Platform;appearance?:import('@contracts').AppearanceSnapshot;language?:import('@contracts').InterfaceLanguage}>
  chooseDirectory():Promise<string|null>
  request<T>(input:Record<string,unknown>):Promise<T>
  enter(commandId:string):Promise<void>
}
declare global {interface Window {coaxiLauncher:ProjectLauncherWindowApi}}
