import {createContext,useContext,useMemo,type ReactNode} from 'react'
import {useOptionalThreadClient} from './camp-client'
import {desktopThreadClient} from './desktop-camp-client'

export interface FeedbackRequest{<T>(action:string,payload:unknown):Promise<T>}
const FeedbackRequestContext=createContext<FeedbackRequest|null>(null)
export function FeedbackRequestProvider({request,children}:{request:FeedbackRequest;children:ReactNode}):React.JSX.Element{return <FeedbackRequestContext.Provider value={request}>{children}</FeedbackRequestContext.Provider>}
/** 生产 Core 请求或原生卡片的受限运输；不伪造整套工作区能力。 */
export function useFeedbackRequest():FeedbackRequest{
  const supplied=useContext(FeedbackRequestContext),client=useOptionalThreadClient()
  return useMemo(()=>supplied??(<T,>(action:string,payload:unknown):Promise<T>=>{
    const owner=client??(typeof window!=='undefined'&&window.rovai?desktopThreadClient:null)
    if(!owner)return Promise.reject(new Error('反馈运输不可用'))
    return owner.request<T>('feedback.operation',{action,payload})
  }),[supplied,client])
}
