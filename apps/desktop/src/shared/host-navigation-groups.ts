import type {NavigationPreferencesApi, NavigationPreferencesSnapshot, NavigationProjectGroup} from '@contracts'
import {applyNavigationProjectLayout, sanitizeNavigationProjectGroups, isNavigationProjectGroupKey} from './navigation-project-groups'
import {isProjectPath} from './navigation-project-path'

export interface HostProjectGroups {
  schemaVersion:1; initialized:boolean; projectGroupsRevision:number
  projectGroups:NavigationProjectGroup[]; projectOrder:string[]|null
}
type Request = <T>(method:'navigation.projectGroups',params:unknown)=>Promise<T>
const same=(a:unknown,b:unknown):boolean=>JSON.stringify(a)===JSON.stringify(b)
function parsed(value:HostProjectGroups):HostProjectGroups {
  if (!value || value.schemaVersion!==1 || typeof value.initialized!=='boolean'
    || !Number.isSafeInteger(value.projectGroupsRevision) || value.projectGroupsRevision<0
    || value.initialized!== (value.projectGroupsRevision>0) || !Array.isArray(value.projectGroups)
    || !same(sanitizeNavigationProjectGroups(value.projectGroups),value.projectGroups)) throw new Error('工作区项目分组无法读取，原记录保留。')
  const ids=new Set(value.projectGroups.map(group=>`folder-group:${group.id}`)), grouped=new Set(value.projectGroups.flatMap(group=>group.projectPaths))
  if(value.projectOrder!==null && (!Array.isArray(value.projectOrder) || new Set(value.projectOrder).size!==value.projectOrder.length
    || value.projectOrder.some(key=>!ids.has(key) && !(key.startsWith('directory:') && isProjectPath(key.slice(10)) && !grouped.has(key.slice(10)))))) throw new Error('工作区项目分组顺序无法读取。')
  return value
}
/** Only group identity/layout becomes shared. Local pins, names, read marks, registrations and removal grants stay local. */
export function withHostProjectGroups(local:NavigationPreferencesApi,request:Request):NavigationPreferencesApi {
  const listeners=new Set<(snapshot:NavigationPreferencesSnapshot)=>void>()
  let unsubscribe:(()=>void)|undefined, timer:ReturnType<typeof setInterval>|undefined
  let serial:Promise<unknown>=Promise.resolve(), lastPublished=''
  const queued=<T>(run:()=>Promise<T>):Promise<T>=>{const next=serial.then(run,run);serial=next.catch(()=>undefined);return next}
  const shared=async(snapshot:NavigationPreferencesSnapshot):Promise<HostProjectGroups>=>{
    let value=parsed(await request<HostProjectGroups>('navigation.projectGroups',{action:'get'}))
    if(!value.initialized && snapshot.projectGroups.length && !snapshot.readError){
      const payload={projectGroups:snapshot.projectGroups,projectOrder:snapshot.projectOrder}
      try {value=parsed(await request<HostProjectGroups>('navigation.projectGroups',{action:'replace',expectedRevision:0,value:payload}))}
      catch(error){const latest=parsed(await request<HostProjectGroups>('navigation.projectGroups',{action:'get'}));if(!latest.initialized)throw error;value=latest}
    }
    return value
  }
  const merge=(snapshot:NavigationPreferencesSnapshot,value:HostProjectGroups):NavigationPreferencesSnapshot=>{
    if(!value.initialized)return {...snapshot,projectGroupsRevision:0}
    const ids=new Set(value.projectGroups.map(group=>`folder-group:${group.id}`)),grouped=new Set(value.projectGroups.flatMap(group=>group.projectPaths.map(path=>`directory:${path}`)))
    const order=[...new Set([...(value.projectOrder??[]),...(snapshot.projectOrder??[])])].filter(key=>!grouped.has(key)&&(!isNavigationProjectGroupKey(key)||ids.has(key)))
    return {...snapshot,projectGroups:value.projectGroups,projectGroupsRevision:value.projectGroupsRevision,projectOrder:order}
  }
  const get=async():Promise<NavigationPreferencesSnapshot>=>{const snapshot=await local.get();return merge(snapshot,await shared(snapshot))}
  const publish=(snapshot:NavigationPreferencesSnapshot):NavigationPreferencesSnapshot=>{const key=JSON.stringify(snapshot);if(key!==lastPublished){lastPublished=key;for(const listener of listeners)listener(structuredClone(snapshot))};return snapshot}
  const refresh=():void=>{void queued(get).then(publish).catch(()=>undefined)}
  const localChange=(run:()=>Promise<NavigationPreferencesSnapshot>):Promise<NavigationPreferencesSnapshot>=>queued(async()=>{const snapshot=await run();return publish(merge(snapshot,await shared(snapshot)))})
  return {
    get:()=>queued(get),
    applyProjectLayout:(expectedRevision,mutation)=>queued(async()=>{
      const snapshot=await get();if(snapshot.readError)throw new Error(snapshot.readError)
      const next=applyNavigationProjectLayout(snapshot,expectedRevision,mutation)
      if(next===snapshot)return snapshot
      const value=parsed(await request<HostProjectGroups>('navigation.projectGroups',{action:'replace',expectedRevision,value:{projectGroups:next.projectGroups,projectOrder:next.projectOrder}}))
      return publish(merge(snapshot,value))
    }),
    registerProjects:(paths,legacy)=>localChange(()=>local.registerProjects(paths,legacy)),
    setThreadReadState:(id,state)=>localChange(()=>local.setThreadReadState(id,state)),
    replacePins:pins=>localChange(()=>local.replacePins(pins)),
    setProjectName:(key,name)=>localChange(()=>local.setProjectName(key,name)),
    removeProject:(key,ids)=>localChange(()=>local.removeProject(key,ids)),
    restoreProject:key=>localChange(()=>local.restoreProject(key)),
    onChanged:listener=>{
      listeners.add(listener)
      if(listeners.size===1){unsubscribe=local.onChanged(refresh);timer=setInterval(()=>{if(typeof document==='undefined'||document.visibilityState==='visible')refresh()},20_000);refresh()}
      return()=>{listeners.delete(listener);if(!listeners.size){unsubscribe?.();unsubscribe=undefined;if(timer)clearInterval(timer);timer=undefined}}
    }
  }
}
