import {createRoot,type Root} from 'react-dom/client'
import type {FeedbackScope} from '@contracts'
import {FeedbackRequestProvider,type FeedbackRequest} from './FeedbackRequestProvider'
import {CardFeedbackMessageScope} from './CardFeedbackFields'
import {JsonRenderScope} from './codeg-native-content/components/ai-elements/json-render-pre'
import {MessageResponse} from './codeg-native-content/components/ai-elements/source-message-response'
import {initializeInterfaceLanguage,uiAttribute} from './interface-language'
import {systemInterfaceLanguage} from '../../shared/interface-languages'
import './styles.css'
import './codeg-native-content/source-html.css'
import './user-feedback.css'
import './mobile-card-page.css'

interface Card{scope:FeedbackScope;agentId:string;messageId:string;cardOrdinal:number;source:string;cardCount:number}
interface Init{channel:string;card:Card;dark:boolean;language?:string}
interface Reply{channel:string;requestId:string;ok:boolean;result?:unknown;error?:string}
declare global{interface Window{
  RovaiCardNative?:{postMessage(value:string):void}
  webkit?:{messageHandlers?:{rovaiCardNative?:{postMessage(value:string):void}}}
  RovaiCardPage?:{mount(input:Init):void;reply(value:Reply):void;dispose():void}
}}
let channel='',root:Root|null=null
const pending=new Map<string,{resolve(value:unknown):void;reject(error:Error):void;timer:ReturnType<typeof setTimeout>}>()
function requestId():string{if(typeof crypto.randomUUID==='function')return crypto.randomUUID();const bytes=crypto.getRandomValues(new Uint8Array(16));bytes[6]=(bytes[6]&15)|64;bytes[8]=(bytes[8]&63)|128;const hex=Array.from(bytes,value=>value.toString(16).padStart(2,'0')).join('');return hex.slice(0,8)+'-'+hex.slice(8,12)+'-'+hex.slice(12,16)+'-'+hex.slice(16,20)+'-'+hex.slice(20)}
function dispose():void{for(const item of pending.values()){clearTimeout(item.timer);item.reject(new Error(uiAttribute('卡片已关闭，原请求可从反馈记录核对')))}pending.clear();root?.unmount();root=null;channel=''}
function send(value:unknown):void{
  const raw=JSON.stringify(value);if(new TextEncoder().encode(raw).length>65536)throw new Error(uiAttribute('卡片请求超过限制'))
  const owner=window.RovaiCardNative??window.webkit?.messageHandlers?.rovaiCardNative
  if(!owner)throw new Error(uiAttribute('原生反馈连接不可用'));owner.postMessage(raw)
}
const request:FeedbackRequest=<T,>(action:string,payload:unknown):Promise<T>=>{
  if(!channel||!['targets','preview','submit'].includes(action)||pending.size>=16)return Promise.reject(new Error(uiAttribute('卡片反馈请求不可用')))
  const id=requestId()
  return new Promise<T>((resolve,reject)=>{
    const timer=setTimeout(()=>{pending.delete(id);reject(new Error(uiAttribute('结果待核对，原编号和预览仍保留，请从会话反馈记录查看')))},30000)
    pending.set(id,{resolve:value=>resolve(value as T),reject,timer})
    try{send({schemaVersion:1,channel,requestId:id,action,payload})}catch(error){clearTimeout(timer);pending.delete(id);reject(error)}
  })
}
window.RovaiCardPage={
  dispose,
  mount(input){
    dispose();initializeInterfaceLanguage({interfaceLanguage:systemInterfaceLanguage([input.language??navigator.language])})
    if(!/^[a-f0-9-]{36}$/i.test(input.channel)||input.card.source.length>262144)throw new Error(uiAttribute('卡片来源不可用'))
    channel=input.channel;document.documentElement.dataset.theme=input.dark?'night':'day';document.documentElement.classList.toggle('dark',input.dark)
    const host=document.getElementById('root');if(!host)return;root=createRoot(host)
    const identity={scope:input.card.scope,agentId:input.card.agentId,messageId:input.card.messageId,cardOrdinal:input.card.cardOrdinal}
    root.render(<FeedbackRequestProvider request={request}><CardFeedbackMessageScope value={identity}><main className="codeg-native-rich mobile-card-page"><JsonRenderScope><MessageResponse mode="static" parseIncompleteMarkdown={false}>{'```spec\n'+input.card.source+(input.card.source.endsWith('\n')?'':'\n')+'```'}</MessageResponse></JsonRenderScope></main></CardFeedbackMessageScope></FeedbackRequestProvider>)
  },
  reply(value){if(value.channel!==channel)return;const item=pending.get(value.requestId);if(!item)return;clearTimeout(item.timer);pending.delete(value.requestId);if(value.ok)item.resolve(value.result);else item.reject(new Error(value.error??uiAttribute('反馈未能完成')))},
}
window.addEventListener('pagehide',dispose)
