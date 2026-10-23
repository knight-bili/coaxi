import {useEffect,useState} from 'react'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'

/** Global/project lists are choices; selecting one assigns its name to this Thread. */
export function WorkspaceTagChoices({threadId,selected,onChoose,disabled}:{
  threadId:string;selected:string[];onChoose(tag:string):void;disabled:boolean
}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText()
  const [choices,setChoices]=useState<Array<{level:string;tags:string[]}>>([])
  const [issue,setIssue]=useState<string|null>(null),[attempt,setAttempt]=useState(0)
  useEffect(()=>{
    let active=true;setChoices([]);setIssue(null)
    void Promise.all(['global','project'].map(async level=>{
      const value=await client.request<{value:{tags:string[]}}>('workbench.organizer.get',{scope:{threadId},level})
      return {level,tags:value.value.tags}
    })).then(value=>{if(active)setChoices(value)},error=>{if(active)setIssue(String(error))})
    return()=>{active=false}
  },[client,threadId,attempt])
  return <div className="workbench-tag-choices">
    {issue&&<p role="alert" className="workbench-error">{issue}<button type="button" disabled={disabled} onClick={()=>setAttempt(value=>value+1)}>{t('重新读取标签')}</button></p>}
    {choices.map(group=><div key={group.level} className="workbench-tags"><small>{t(group.level==='global'?'全局已有标签':'工程已有标签')}</small>
      {group.tags.map(tag=><button type="button" key={tag} disabled={disabled||selected.includes(tag)} aria-pressed={selected.includes(tag)} onClick={()=>onChoose(tag)}>{tag}</button>)}
    </div>)}
  </div>
}
