import {EventEmitter} from 'node:events'
import {Readable} from 'node:stream'
import {Cookie,CookieJar} from 'tough-cookie'

// A Node transport port for the existing channel modules. It implements their
// small session/cookie/request seam; it never emulates an Electron renderer.
type CookieDetails={name:string;value:string;domain?:string;path?:string;secure?:boolean;httpOnly?:boolean;sameSite?:string;expirationDate?:number;url?:string;session?:boolean;hostOnly?:boolean}
export class NodeSession {
  readonly #jar=new CookieJar(undefined,{rejectPublicSuffixes:true,allowSpecialUseDomain:false})
  readonly cookies={
    get:async(filter:{url?:string;name?:string;domain?:string}={}):Promise<CookieDetails[]>=>{
      const values=filter.url?await this.#jar.getCookies(filter.url):(await this.#jar.serialize()).cookies.map(value=>Cookie.fromJSON(value)).filter((value):value is Cookie=>value!==undefined)
      return values.filter(value=>(!filter.name||value.key===filter.name)&&(!filter.domain||value.domain===filter.domain.replace(/^\./,''))).map(value=>({name:value.key??'',value:value.value,domain:value.hostOnly?value.domain??'':`.${value.domain}`,path:value.path??'/',secure:value.secure,httpOnly:value.httpOnly,hostOnly:value.hostOnly,session:value.expires==='Infinity',...(value.expires instanceof Date?{expirationDate:value.expires.getTime()/1000}:{}),sameSite:value.sameSite==='none'?'no_restriction':value.sameSite??'unspecified'}))
    },
    set:async(details:CookieDetails):Promise<void>=>{
      if(!details.url||details.name.length+details.value.length>65536)throw new Error('channel_cookie_invalid')
      const origin=new URL(details.url);if(origin.protocol!=='https:')throw new Error('channel_cookie_origin_rejected')
      const value=new Cookie({key:details.name,value:details.value,path:details.path??'/',secure:details.secure??true,httpOnly:details.httpOnly??false,...(details.domain?{domain:details.domain.replace(/^\./,'')}:{}),...(details.expirationDate?{expires:new Date(details.expirationDate*1000)}:{}),...(['strict','lax','no_restriction'].includes(details.sameSite??'')?{sameSite:details.sameSite==='no_restriction'?'none':details.sameSite}:{} )})
      const serialized=await this.#jar.serialize();if(serialized.cookies.length>=2048&&!serialized.cookies.some(cookie=>cookie.key===details.name&&cookie.domain===(details.domain??origin.hostname).replace(/^\./,'')))throw new Error('channel_cookie_limit')
      await this.#jar.setCookie(value,origin.href)
    },
    flushStore:async():Promise<void>=>{},
  }
  setPermissionRequestHandler():void{}
  setPermissionCheckHandler():void{}
  async clearStorageData():Promise<void>{await this.#jar.removeAllCookies()}
  async clearCache():Promise<void>{}
  async fetch(input:string,init:RequestInit={}):Promise<Response>{
    const url=new URL(input);if(url.protocol!=='https:'||url.username||url.password)throw new Error('channel_request_origin_rejected')
    const headers=new Headers(init.headers);headers.delete('cookie');const cookies=await this.#jar.getCookieString(url.href);if(cookies)headers.set('cookie',cookies)
    const signal=init.signal?AbortSignal.any([init.signal,AbortSignal.timeout(45000)]):AbortSignal.timeout(45000)
    const response=await fetch(url,{...init,headers,signal,redirect:'manual'})
    for(const value of response.headers.getSetCookie()){if(value.length>65536)throw new Error('channel_cookie_invalid');await this.#jar.setCookie(value,url.href)}
    return response
  }
}

// Each supplied partition is ephemeral. SQLite remains the only durable store
// and supplies the exact cookie snapshots on every process restart.
export const session={fromPartition:(_partition:string,_options?:unknown):NodeSession=>new NodeSession()}
export const net={request:(options:{session:NodeSession;url:string;method?:string;headers?:Record<string,string>;redirect?:string}):EventEmitter&{end(body?:string|Buffer):void;abort():void}=>{
  const outgoing=new EventEmitter() as EventEmitter&{end(body?:string|Buffer):void;abort():void}
  const abort=new AbortController();let incoming:Readable|undefined;let ended=false
  outgoing.abort=()=>{abort.abort();incoming?.destroy();outgoing.emit('close')}
  outgoing.end=(body)=>{if(ended)throw new Error('channel_request_already_sent');ended=true;void options.session.fetch(options.url,{method:options.method??'GET',headers:options.headers,body:body as BodyInit|undefined,signal:abort.signal}).then(response=>{
    const headers:Record<string,string[]>={};for(const [key,value]of response.headers){headers[key]=[value]}headers['set-cookie']=response.headers.getSetCookie()
    const location=response.headers.get('location');if(response.status>=300&&response.status<400&&location){outgoing.emit('redirect',response.status,options.method??'GET',location,headers);void response.body?.cancel();return}
    incoming=(response.body?Readable.fromWeb(response.body as never):Readable.from([]))
    Object.assign(incoming,{statusCode:response.status,statusMessage:response.statusText,headers})
    incoming.once('end',()=>outgoing.emit('close'));outgoing.emit('response',incoming)
  }).catch(error=>{if(!abort.signal.aborted)outgoing.emit('error',error);outgoing.emit('close')})}
  return outgoing
}}

// These names are imported by the shared desktop-only fallback, but are never
// available on the standalone Host. The caller retains its pending login state.
export class BrowserWindow {constructor(..._args:unknown[]){throw new Error('channel_native_interaction')} }
export class WebContentsView {constructor(..._args:unknown[]){throw new Error('channel_native_interaction')} }
export class BaseWindow extends BrowserWindow {}
export const app={isPackaged:true}
