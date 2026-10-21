import {createInterface} from 'node:readline'
import {randomUUID} from 'node:crypto'
import type {CoreEvent} from '@contracts'

type Pending={resolve(value:unknown):void;reject(error:Error):void;timer:ReturnType<typeof setTimeout>}
export class ChannelCorePipe {
  readonly #pending=new Map<string,Pending>()
  readonly #events=new Set<(event:CoreEvent)=>void>()
  #closed=false
  constructor(readonly incoming:(value:Record<string,unknown>)=>Promise<unknown>,readonly stopping:()=>Promise<void>){
    const input=createInterface({input:process.stdin,crlfDelay:Infinity})
    input.on('line',line=>{if(Buffer.byteLength(line)>8*1024*1024){void this.close();return}let frame:Record<string,unknown>;try{frame=JSON.parse(line)}catch{void this.close();return}
      if(typeof frame.coreId==='string'){const pending=this.#pending.get(frame.coreId);if(!pending)return;this.#pending.delete(frame.coreId);clearTimeout(pending.timer);if(frame.error)pending.reject(new Error(String((frame.error as {message?:string}).message??'channel_core_request_failed')));else pending.resolve(frame.result);return}
      if(frame.event){for(const listener of this.#events)listener(frame.event as CoreEvent);return}
      if(typeof frame.id==='string')void this.incoming(frame).then(result=>this.write({id:frame.id,result}),()=>this.write({id:frame.id,error:'channel_operation_failed'}))
    })
    input.on('close',()=>void this.close());process.once('SIGTERM',()=>void this.close());process.once('SIGINT',()=>void this.close())
  }
  async request<T>(method:string,params:unknown={}):Promise<T>{
    if(this.#closed||this.#pending.size>=128)throw new Error('channel_core_unavailable')
    const coreId=randomUUID()
    return new Promise<T>((resolve,reject)=>{const timer=setTimeout(()=>{this.#pending.delete(coreId);reject(new Error('channel_core_result_unknown'))},90000);this.#pending.set(coreId,{resolve:value=>resolve(value as T),reject,timer});this.write({coreId,method,params})})
  }
  onEvent(listener:(event:CoreEvent)=>void):()=>void{this.#events.add(listener);return()=>this.#events.delete(listener)}
  write(value:unknown):void{if(this.#closed)return;const line=JSON.stringify(value);if(Buffer.byteLength(line)>8*1024*1024)throw new Error('channel_frame_too_large');process.stdout.write(line+'\n')}
  async close():Promise<void>{if(this.#closed)return;this.#closed=true;for(const pending of this.#pending.values()){clearTimeout(pending.timer);pending.reject(new Error('channel_core_unavailable'))}this.#pending.clear();this.#events.clear();await this.stopping();process.exitCode=0;process.stdin.destroy()}
}
