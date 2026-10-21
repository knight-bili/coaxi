import { useRef,useState } from 'react'
import type { StoredCommandResult } from '@contracts'
import { newCommandId } from '../../shared/command-id'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { SafeMarkdown } from './SafeMarkdown'
type Preview={planId:string;skillId:string;sourceRevision:string;content:string;contentDigest:string;binaryDigest:string}
export function OfficeSkillPanel({skillId,expectedVersion,onInstalled,onBusyChange}:{skillId:string;expectedVersion:string;onInstalled():Promise<void>;onBusyChange(busy:boolean):void}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),[preview,setPreview]=useState<Preview|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),pending=useRef<{commandId:string;command:{skillId:string;sourceRevision:string;expectedVersion:string};planId:string}|null>(null)
  const run=async(action:()=>Promise<void>):Promise<void>=>{if(busy)return;setBusy(true);onBusyChange(true);setError(null);try{await action()}catch(failure){setError(String(failure))}finally{setBusy(false);onBusyChange(false)}}
  const load=async():Promise<void>=>{const value=await client.request<Preview>('skillPacks.office.prepare',{skillId});setPreview(value);pending.current=null}
  const install=async():Promise<void>=>{
    if(!preview)return
    const request=pending.current??{commandId:newCommandId(),command:{skillId,sourceRevision:preview.sourceRevision,expectedVersion},planId:preview.planId};pending.current=request
    const result=await client.request<StoredCommandResult>('skillPacks.office.install',request)
    pending.current=null;await onInstalled();if(result.status==='rejected')throw new Error(result.code)
  }
  return <div className="office-skill-panel"><div className="workbench-toolbar"><button disabled={busy||pending.current!==null} onClick={()=>void run(load)}>{t(preview?'重新加载 Office 技能':'从 OfficeCLI 加载技能')}</button><button disabled={busy||!preview} onClick={()=>void run(install)}>{t(pending.current?'核对原安装':'安装这个版本')}</button></div><p>{t('加载后先查看正文，再安装。能使用的队员仍由你选择；进行中的任务保留原先的技能版本。')}</p>{error&&<p className="workbench-error" role="alert">{error}</p>}{preview&&<SafeMarkdown className="capability-reading" mode="document">{preview.content}</SafeMarkdown>}</div>
  }
