import {useEffect,useRef} from 'react'
import type {DesktopDeepLink,DesktopDeepLinksApi} from '@contracts'

/** Same FIFO startup drain as Codeg DeepLinkBootstrap; uses the original guarded navigation. */
export function useDesktopDeepLinks(api:DesktopDeepLinksApi|undefined,ready:boolean,onOpen:(link:DesktopDeepLink)=>Promise<void>,onError:(message:string)=>void):void{
  const latest=useRef({ready,onOpen,onError});latest.current={ready,onOpen,onError}
  const wake=useRef<()=>void>(()=>undefined)
  useEffect(()=>{
    if(!api)return
    let disposed=false,running=false,notified=false
    const drain=async():Promise<void>=>{
      notified=true;if(running)return;running=true
      try{
        while(!disposed&&latest.current.ready){
          notified=false
          const link=await api.peek();if(disposed||!latest.current.ready||!link)break
          try{await latest.current.onOpen(link)}catch(error){if(!disposed)latest.current.onError(error instanceof Error?error.message:String(error))}
          if(disposed)break
          await api.acknowledge(link.requestId)
        }
      }catch(error){if(!disposed)latest.current.onError(error instanceof Error?error.message:String(error))}
      finally{running=false;if(notified&&!disposed&&latest.current.ready)void drain()}
    }
    wake.current=()=>{void drain()}
    const unsubscribe=api.onAvailable(wake.current);wake.current()
    return()=>{disposed=true;wake.current=()=>undefined;unsubscribe()}
  },[api])
  useEffect(()=>{if(ready)wake.current()},[ready])
}
