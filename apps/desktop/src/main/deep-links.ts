import {randomUUID} from 'node:crypto'
import type {DesktopDeepLink} from '@contracts'
import {parseDesktopDeepLink} from '../shared/deep-link'

/** Keep startup links until the real business shell has hydrated and read them. */
export function createDeepLinkInbox(publish:()=>void){
  const pending:DesktopDeepLink[]=[]
  return {
    offer(raw:string):boolean{
      const target=parseDesktopDeepLink(raw)
      if(!target||pending.length>=32)return false
      pending.push({...target,requestId:randomUUID()});publish();return true
    },
    peek():DesktopDeepLink|null{return pending[0]?{...pending[0]}:null},
    acknowledge(requestId:string):void{
      if(pending[0]?.requestId!==requestId)return
      pending.shift();if(pending.length)publish()
    }
  }
}
