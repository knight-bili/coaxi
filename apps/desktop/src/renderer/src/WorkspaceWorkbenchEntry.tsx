import {useMemo,useRef,useState,type ComponentProps} from 'react'
import {Files} from 'lucide-react'
import {useThreadClient} from './camp-client'
import {WorkspaceWorkbench} from './WorkspaceWorkbench'
import {useUiText} from './interface-language'

type Props=Omit<ComponentProps<typeof WorkspaceWorkbench>,'initiallyOpen'|'onClosed'> & {missionId?:string|null}
type Ready={root:string;owner:object}

/** All engineering panels share the directory verified by Core for this exact Thread/Mission. */
export function WorkspaceWorkbenchEntry(props:Props):React.JSX.Element{
  const client=useThreadClient(),t=useUiText()
  const owner=useMemo(()=>({client,threadId:props.threadId,missionId:props.missionId??null}),[client,props.threadId,props.missionId])
  const current=useRef(owner);current.current=owner
  const [ready,setReady]=useState<Ready|null>(null),[busyOwner,setBusyOwner]=useState<object|null>(null),[issue,setIssue]=useState<{owner:object;message:string}|null>(null)
  const visible=ready?.owner===owner?ready:null
  const busy=busyOwner===owner
  const open=async():Promise<void>=>{
    if(busy)return
    const captured=owner;setBusyOwner(captured);setIssue(null)
    try{
      const result=await client.request<{workspaceRoot:string}>('workbench.files.list',{scope:{threadId:props.threadId,missionId:props.missionId??null},path:''})
      if(current.current!==captured)return
      if(typeof result.workspaceRoot!=='string'||!result.workspaceRoot)throw new Error(t('当前主机尚未提供已核实的工程目录。'))
      setReady({root:result.workspaceRoot,owner:captured})
    }catch(error){if(current.current===captured)setIssue({owner:captured,message:String(error)})}
    finally{if(current.current===captured)setBusyOwner(null)}
  }
  if(visible)return <WorkspaceWorkbench {...props} projectPath={visible.root} initiallyOpen onClosed={()=>setReady(null)}/>
  return <><button type="button" className="quiet-button conversation-tool-button" disabled={busy} title={t('文件与 Git 工程工作区')} onClick={()=>void open()}><Files size={15}/><span>{t(busy?'正在核对工程…':'工程')}</span></button>{issue?.owner===owner&&<span role="alert" className="workbench-error">{issue.message}</span>}</>
}
