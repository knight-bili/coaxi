import {useEffect,useRef,useState} from 'react'
import type {NativeSteerEvidence} from '@contracts'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {readErrorMessage} from './error-message'
import {JsonTreeView} from './codeg-native-content/components/ai-elements/json-tree'

export function NativeSteerEvidenceDetails({commandId}:{commandId:string}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0),busyRef=useRef(false)
  const [open,setOpen]=useState(false),[value,setValue]=useState<NativeSteerEvidence|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('')
  useEffect(()=>{generation.current++;setValue(null);setError('');busyRef.current=false;setBusy(false);return()=>{generation.current++}},[client,commandId])
  const read=async():Promise<void>=>{
    if(busyRef.current)return;busyRef.current=true;setBusy(true);setError('');const observed=generation.current
    try{const value=await client.request<NativeSteerEvidence>('runtime.steer.evidence',{schemaVersion:1,commandId});if(value.receipt.commandId!==commandId)throw new Error(t('原回执的操作归属不同，请保留编号并核对。'));if(observed===generation.current)setValue(value)}catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current){busyRef.current=false;setBusy(false)}}
  }
  return <details className="native-session-reasoning" open={open} onToggle={event=>setOpen(event.currentTarget.open)}><summary>{t('原投递与普通续办记录')}</summary><p>{t('这里只读取原记录，不会重发输入。写入连接或取得原程序回执，不代表模型已经阅读。')}</p><button type="button" className="quiet-button" disabled={busy} onClick={()=>void read()}>{t(busy?'正在读取…':'读取原投递记录')}</button>{error&&<p role="alert" className="feedback-error">{error}</p>}{value&&<JsonTreeView value={value}/>}</details>
}
