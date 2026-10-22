import { createContext,useCallback,useContext,useEffect,useId,useMemo,useRef,useState,type ComponentType,type ReactNode } from 'react'
import { useStateStore,type BaseComponentProps } from '@json-render/react'
import { sha256 } from '@noble/hashes/sha2.js'
import type { FeedbackCardOrigin,FeedbackField,FeedbackScope } from '@contracts'
import { useUiText } from './interface-language'
import { FeedbackSubmitDialog } from './FeedbackSubmitDialog'
import { readErrorMessage } from './error-message'

export interface FeedbackMessageSource{scope:FeedbackScope;agentId:string;messageId:string;cardOrdinal:number}
const MessageSource=createContext<FeedbackMessageSource|null>(null)
export function CardFeedbackMessageScope({value,children}:{value:FeedbackMessageSource;children:ReactNode}):React.JSX.Element{return <MessageSource.Provider value={value}>{children}</MessageSource.Provider>}
interface RegisteredField {name:string;label:string;statePath:string;read():unknown;visible():boolean}
interface FieldRegistry {fields:Map<string,RegisteredField>;register(id:string,value:RegisteredField):()=>void;revision:number}
const Fields=createContext<FieldRegistry|null>(null)
export function CardFeedbackFieldsProvider({children}:{children:ReactNode}):React.JSX.Element{
  const fields=useRef(new Map<string,RegisteredField>()),[revision,setRevision]=useState(0)
  const register=useCallback((id:string,value:RegisteredField)=>{fields.current.set(id,value);setRevision(current=>current+1);return()=>{fields.current.delete(id);setRevision(current=>current+1)}},[])
  const value=useMemo(()=>({fields:fields.current,register,revision}),[register,revision])
  return <Fields.Provider value={value}>{children}</Fields.Provider>
}

/** 注册库已解析好的实际绑定路径，重复列表索引由原 Renderer 决定，不从模型 state 扫描字段。 */
export function feedbackControl<P extends Record<string,unknown>>(Original:ComponentType<BaseComponentProps<P>>,kind:'Input'|'Textarea'|'Select'|'Checkbox'|'Radio'|'Switch'){
  return function FeedbackControl(input:BaseComponentProps<P>):React.JSX.Element{
    const context=useContext(Fields),message=useContext(MessageSource),state=useStateStore(),id=useId(),root=useRef<HTMLSpanElement|null>(null),latest=useRef({input,state})
    latest.current={input,state}
    const path=input.bindings?.[kind==='Checkbox'||kind==='Switch'?'checked':'value'],name=typeof input.props.name==='string'?input.props.name:'',label=typeof input.props.label==='string'?input.props.label:name
    const password=input.props.type==='password',wasPassword=useRef(false);if(password)wasPassword.current=true
    const register=context?.register
    useEffect(()=>{
      if(!register||!message||!path||!name||wasPassword.current||input.loading)return
      return register(id,{name,label,statePath:path,read:()=>latest.current.state.get(path),visible:()=>{
        const element=root.current?.querySelector<HTMLElement>('input,textarea,select,[role="combobox"],[role="radio"],[role="switch"],[role="checkbox"]')
        return Boolean(element&&element.getClientRects().length>0&&getComputedStyle(element).visibility!=='hidden'&&element.getAttribute('type')!=='password'&&element.getAttribute('type')!=='hidden'&&!wasPassword.current)
      }})
    },[register,message,path,name,label,id,input.loading,password])
    return <span ref={root} style={{display:'contents'}}><Original {...input}/></span>
  }
}
function sourceDigest(source:string):string{return 'sha256:'+Array.from(sha256(new TextEncoder().encode(JSON.stringify(source))),byte=>byte.toString(16).padStart(2,'0')).join('')}

export function CardFeedbackFooter({source,streaming}:{source:string;streaming:boolean}):React.JSX.Element|null{
  const message=useContext(MessageSource),context=useContext(Fields),t=useUiText()
  const [fields,setFields]=useState<Array<FeedbackField&{selected:boolean}>>([]),[choosing,setChoosing]=useState(false),[confirming,setConfirming]=useState(false),[error,setError]=useState('')
  if(!message||!context||streaming)return null
  const choose=():void=>{try{
    const seen=new Set<string>();const selected:Array<FeedbackField&{selected:boolean}>=[]
    for(const row of context.fields.values()){
      const key=row.statePath+'\u0000'+row.name;if(!row.visible()||seen.has(key))continue;seen.add(key)
      const value=row.read();if(value===undefined)continue;selected.push({name:row.name,label:row.label,statePath:row.statePath,value:structuredClone(value),selected:true})
    }
    setFields(selected);setChoosing(true);setError('')
  }catch(reason){setError(readErrorMessage(reason))}}
  const origin:FeedbackCardOrigin={messageId:message.messageId,conversationId:message.scope.conversationId,messageKind:message.scope.scopeKind,cardOrdinal:message.cardOrdinal,sourceDigest:sourceDigest(source)}
  const selected=fields.filter(field=>field.selected).map(({selected:_selected,...field})=>field)
  return <div className="feedback-card-footer" data-quote-exclude>
    {!choosing?<button type="button" className="quiet-button compact" disabled={context.fields.size===0} onClick={event=>{if(event.nativeEvent.isTrusted)choose()}}>{t('把填写内容交给队员')}</button>:<div className="feedback-card-fields"><p>{t('选择这次要提交的可编辑字段')}</p>{fields.map((field,index)=><label key={field.statePath+'\u0000'+field.name}><input type="checkbox" checked={field.selected} onChange={event=>setFields(current=>current.map((row,position)=>position===index?{...row,selected:event.target.checked}:row))}/><span>{field.label||field.name}</span></label>)}{fields.length===0&&<p>{t('这张卡片当前没有可以提交的可见字段。')}</p>}<div className="feedback-dock-actions"><button className="quiet-button compact" type="button" onClick={()=>setChoosing(false)}>{t('取消')}</button><button className="quiet-button compact" type="button" disabled={!selected.length} onClick={event=>{if(event.nativeEvent.isTrusted)setConfirming(true)}}>{t('查看完整内容')}</button></div></div>}
    {error&&<p role="alert">{error}</p>}
    {confirming&&<FeedbackSubmitDialog scope={message.scope} agentId={message.agentId} draft={{source:{kind:'generated_card',origin},text:'',title:t('卡片填写内容'),fields:selected}} onClose={()=>setConfirming(false)} onSubmitted={()=>setChoosing(false)}/>}
  </div>
}
