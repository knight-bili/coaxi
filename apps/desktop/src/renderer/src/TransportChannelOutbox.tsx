import {useEffect,useRef,useState} from 'react'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {SafeMarkdown} from './SafeMarkdown'
import {newCommandId} from '../../shared/command-id'

type Output={id:string;digest:string;state:string;data:{adapterId:string;body:string;senderId:string}}
export function TransportChannelOutbox({revision}:{revision:string}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),requests=useRef(new Map<string,Record<string,unknown>>())
  const [items,setItems]=useState<Output[]>([]),[after,setAfter]=useState(0),[next,setNext]=useState<number|null>(null),[busy,setBusy]=useState(false),[issue,setIssue]=useState<string|null>(null)
  const read=async(cursor=0):Promise<void>=>{const page=await client.request<{items:Output[];hasMore:boolean;scanAfter:number|null}>('channels.adapters',{operation:'outbox_inspect',after:cursor});const values=page.items.filter(item=>['sending','unknown'].includes(item.state));setItems(previous=>cursor===0?values:[...previous,...values.filter(value=>!previous.some(item=>item.id===value.id))]);setAfter(cursor);setNext(page.hasMore?page.scanAfter:null)}
  useEffect(()=>{let alive=true;void read().catch(error=>{if(alive)setIssue(String(error))});return()=>{alive=false}},[client,revision])
  const resolve=async(item:Output,operation:'outbox_retry'|'outbox_confirm'):Promise<void>=>{
    const message=operation==='outbox_retry'?t('远端可能已收到这条回复，再次发送可能产生重复。确认已核对并仍要重试？'):t('确认已在远端看到了这条回复？此操作会结束本地待核对状态。')
    if(busy||!window.confirm(message))return;setBusy(true);setIssue(null)
    try{const key=`${item.id}:${operation}`;const request=requests.current.get(key)??{operation,outboxId:item.id,expectedOutputDigest:item.digest,expectedRevision:revision,confirmed:true,commandId:newCommandId()};requests.current.set(key,request);await client.request('channels.adapters',request);await read(after)}catch(error){setIssue(String(error))}finally{setBusy(false)}
  }
  return <section className="transport-channel-outbox"><div className="settings-card-heading"><h3>{t('回复投递核对')}</h3><button type="button" className="quiet-button" disabled={busy} onClick={()=>void read().catch(error=>setIssue(String(error)))}>{t('刷新记录')}</button></div><p>{t('发送中断或结果未知的回复会保留在这里，系统不会自动重发。')}</p>{issue&&<p role="alert" className="settings-error">{issue}</p>}{items.map(item=><details key={item.id} className="settings-card"><summary>{t(item.state==='sending'?'发送中或等待结果核对':'发送结果未知')} · {item.data.senderId}</summary><SafeMarkdown>{item.data.body}</SafeMarkdown><div className="settings-actions"><button type="button" disabled={busy} onClick={()=>void resolve(item,'outbox_confirm')}>{t('已在远端收到')}</button><button type="button" disabled={busy} onClick={()=>void resolve(item,'outbox_retry')}>{t('核对后再次发送')}</button></div></details>)}{next!==null&&<button type="button" disabled={busy} onClick={()=>void read(next).catch(error=>setIssue(String(error)))}>{t('更多记录')}</button>}</section>
}
