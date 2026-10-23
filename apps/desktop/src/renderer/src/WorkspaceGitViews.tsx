import {useEffect,useMemo,useRef,useState} from 'react'
import {GitBranch,GitCommitHorizontal,GitMerge,Plus,Minus} from 'lucide-react'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {UnifiedDiffPreview} from './codeg-native-content/components/diff/unified-diff-preview'
import {WorkspaceGitFileAt} from './WorkspaceGitFileAt'
import {parseWorkspaceStatus,parseWorkspaceLog,parseWorkspaceBranches,parseWorkspaceStashes,parseWorkspaceTrees,type GitChange} from './workspace-git-records'
type Result={success:boolean;output:string;error:string}
type CommitFile={path:string;originalPath?:string;kind:'added'|'deleted'|'renamed'|'modified';binary:boolean}

function readGitPathToken(value:string,offset=0):{path:string;next:number}{
  while(value[offset]===' ')offset++
  if(value[offset]!=='"'){const end=value.indexOf(' ',offset);return{path:value.slice(offset,end<0?value.length:end),next:end<0?value.length:end}}
  const bytes:number[]=[],encoder=new TextEncoder()
  const append=(character:string):void=>{bytes.push(...encoder.encode(character))}
  for(let index=offset+1;index<value.length;index++){
    const character=value[index]!
    if(character==='"')return{path:new TextDecoder().decode(Uint8Array.from(bytes)),next:index+1}
    if(character!=='\\'){
      if(character.charCodeAt(0)>=0xd800&&character.charCodeAt(0)<=0xdbff&&value.charCodeAt(index+1)>=0xdc00&&value.charCodeAt(index+1)<=0xdfff){append(value.slice(index,index+2));index++}else append(character)
      continue
    }
    const escaped=value[++index]
    if(escaped===undefined)break
    if(/[0-7]/u.test(escaped)){let octal=escaped;while(octal.length<3&&/[0-7]/u.test(value[index+1]??''))octal+=value[++index];bytes.push(Number.parseInt(octal,8));continue}
    const decoded=({a:'\x07',b:'\b',t:'\t',n:'\n',v:'\x0b',f:'\f',r:'\r','\\':'\\','"':'"'} as Record<string,string>)[escaped]
    append(decoded??escaped)
  }
  return{path:'',next:value.length}
}

function parseCommitFiles(output:string):CommitFile[]{
  const files:CommitFile[]=[]
  let current:CommitFile|null=null
  const finish=():void=>{if(current?.path)files.push(current);current=null}
  for(const line of output.split('\n')){
    if(line.startsWith('diff --git ')){
      finish()
      const left=readGitPathToken(line,'diff --git '.length),right=readGitPathToken(line,left.next)
      if(!left.path.startsWith('a/')||!right.path.startsWith('b/'))continue
      current={path:right.path.slice(2),kind:'modified',binary:false}
    }else if(current){
      if(line.startsWith('new file mode '))current.kind='added'
      else if(line.startsWith('deleted file mode '))current.kind='deleted'
      else if(line.startsWith('rename from ')){current.originalPath=readGitPathToken(line,'rename from '.length).path;current.kind='renamed'}
      else if(line.startsWith('rename to ')){current.path=readGitPathToken(line,'rename to '.length).path;current.kind='renamed'}
      else if(line.startsWith('GIT binary patch')||line.startsWith('Binary files '))current.binary=true
    }
  }
  finish()
  return files
}

function isSafeCommitPath(path:string):boolean{return!!path&&!path.startsWith('/')&&!/[\\:\0\r\n]/u.test(path)&&path.split('/').every(part=>part!==''&&part!=='.'&&part!=='..')}

export function WorkspaceGitViews({threadId,view,result,disabled,onAction,onDirtyChange,onOpenFile}:{threadId:string;view:string;result:Result;disabled:boolean;onAction(action:string,options:Record<string,unknown>):void;onDirtyChange(dirty:boolean):void;onOpenFile(path:string):void}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),[diff,setDiff]=useState<Result|null>(null),[selected,setSelected]=useState(''),[historyFile,setHistoryFile]=useState<CommitFile|null>(null),[filesOpen,setFilesOpen]=useState(true),[message,setMessage]=useState(''),[error,setError]=useState<string|null>(null),[reading,setReading]=useState(false),read=useRef(0)
  useEffect(()=>{onDirtyChange(!!message.trim());return()=>onDirtyChange(false)},[message,onDirtyChange])
  useEffect(()=>{read.current++;setDiff(null);setSelected('');setHistoryFile(null);setFilesOpen(true);setError(null)},[view])
  const show=async(action:string,options:Record<string,unknown>,key:string):Promise<void>=>{const generation=++read.current;setSelected(key);setHistoryFile(null);setFilesOpen(true);setDiff(null);setReading(true);setError(null);try{const value=await client.request<Result>('workbench.git.read',{scope:{threadId},action,...options});if(generation===read.current)setDiff(value)}catch(reason){if(generation===read.current)setError(String(reason))}finally{if(generation===read.current)setReading(false)}}
  const status=useMemo(()=>parseWorkspaceStatus(result.output),[result.output])
  const section=(label:string,entries:GitChange[],staged:boolean):React.ReactNode=><section className="workbench-git-group"><header><strong>{t(label)}</strong><span>{entries.length}</span>{entries.length>0&&<button disabled={disabled} title={t(staged?'全部取消暂存':'全部暂存')} onClick={()=>onAction(staged?'unstage':'stage',{paths:entries.map(entry=>entry.path)})}>{staged?<Minus size={13}/>:<Plus size={13}/>}</button>}</header>{entries.map(entry=><div className={`workbench-change-row ${selected===`${staged}:${entry.path}`?'is-active':''}`} key={entry.path}><button title={entry.path} onClick={()=>{if(entry.untracked){onOpenFile(entry.path);return}void show(staged?'diff_staged':'diff',{paths:[entry.path]},`${staged}:${entry.path}`)}}><span data-status={entry.conflict?'U':entry.untracked?'?':staged?entry.index:entry.worktree}>{entry.conflict?'U':entry.untracked?'?':staged?entry.index:entry.worktree}</span><strong>{entry.path}</strong>{entry.originalPath&&<small>← {entry.originalPath}</small>}</button><button disabled={disabled} title={t(staged?'取消暂存':'暂存')} onClick={()=>onAction(staged?'unstage':'stage',{paths:[entry.path]})}>{staged?<Minus size={13}/>:<Plus size={13}/>}</button></div>)}</section>
  const detail=<div className="workbench-git-detail">{reading?<p>{t('正在读取差异…')}</p>:diff?.success?<UnifiedDiffPreview diffText={diff.output} clickableFilePath={false}/>:diff?<pre>{diff.error||diff.output}</pre>:<p>{t('选择记录查看差异')}</p>}{error&&<p role="alert">{error}</p>}</div>
  useEffect(()=>()=>{read.current++},[])
  if(!result.success)return <pre className="workbench-git-output">{result.error||result.output}</pre>
  if(view==='status')return <div className="workbench-git-records"><aside><header className="workbench-git-branch"><GitBranch size={15}/><strong>{status.branch||t('尚无提交')}</strong><small>↑{status.ahead} ↓{status.behind}</small></header>{section('已暂存',status.changes.filter(entry=>!entry.untracked&&!entry.conflict&&entry.index!=='.'),true)}{section('未暂存',status.changes.filter(entry=>!entry.untracked&&!entry.conflict&&entry.worktree!=='.'),false)}{section('未跟踪',status.changes.filter(entry=>entry.untracked),false)}{section('冲突',status.changes.filter(entry=>entry.conflict),false)}<div className="workbench-commit-compose"><textarea aria-label={t('提交说明')} placeholder={t('提交说明')} value={message} onChange={event=>setMessage(event.target.value)}/><button disabled={disabled||!message.trim()} onClick={()=>onAction('commit',{message})}><GitCommitHorizontal size={14}/>{t('预览提交')}</button></div></aside>{detail}</div>
  if(view==='log'){
    const commitFiles=diff?.success&&/^[a-f0-9]{40,64}$/u.test(selected)?parseCommitFiles(diff.output):[]
    return <div className="workbench-git-records"><aside>{parseWorkspaceLog(result.output).map(commit=><button className={`workbench-history-row ${selected===commit.sha?'is-active':''}`} key={commit.sha} onClick={()=>void show(commit.parents.length>1?'diff_compare':'show',{target:commit.parents.length>1?`${commit.parents[0]}...${commit.sha}`:commit.sha},commit.sha)}><strong>{commit.message.split('\n')[0]}</strong><small><GitCommitHorizontal size={12}/>{commit.sha.slice(0,8)} · {commit.author}</small><time>{new Date(commit.date).toLocaleString()}</time><span className="workbench-history-actions"><span role="button" tabIndex={0} onClick={event=>{event.stopPropagation();onAction('cherry_pick',{target:commit.sha})}}>{t('拣选')}</span><span role="button" tabIndex={0} onClick={event=>{event.stopPropagation();onAction('revert',{target:commit.sha})}}>{t('撤销')}</span><span role="button" tabIndex={0} onClick={event=>{event.stopPropagation();onAction('reset',{target:commit.sha,mode:'mixed'})}}>{t('重置到此处')}</span></span></button>)}</aside><div className="workbench-git-detail">
      {commitFiles.length>0&&<section className="workbench-git-group"><header><strong>{t('提交文件')}</strong><span>{commitFiles.length}</span><button type="button" aria-expanded={filesOpen} onClick={()=>setFilesOpen(value=>!value)}>{filesOpen?t('收起'):t('展开')}</button></header>{filesOpen&&commitFiles.map(file=><div className={`workbench-change-row ${historyFile?.path===file.path?'is-active':''}`} key={`${file.path}:${file.originalPath??''}`}><button type="button" title={file.path} disabled={!isSafeCommitPath(file.path)} onClick={()=>setHistoryFile(file)}><strong>{file.path}</strong>{file.originalPath&&<small>← {file.originalPath}</small>}<small>{t(file.kind==='added'?'新增':file.kind==='deleted'?'已删除':file.kind==='renamed'?'已重命名':'已修改')}{file.binary?` · ${t('二进制')}`:''}</small></button></div>)}</section>}
      {historyFile?<><header className="workbench-git-branch"><strong title={historyFile.path}>{historyFile.path}</strong><button type="button" onClick={()=>setHistoryFile(null)}>{t('返回提交差异')}</button></header>{!isSafeCommitPath(historyFile.path)?<p role="alert">{t('此文件路径不允许读取。')}</p>:<WorkspaceGitFileAt key={`${selected}:${historyFile.path}`} threadId={threadId} commit={selected} path={historyFile.path} deleted={historyFile.kind==='deleted'} binary={historyFile.binary}/>}</>:detail}
    </div></div>
  }
  if(view==='branches')return <div className="workbench-git-cards">{parseWorkspaceBranches(result.output).map(branch=><section key={branch.ref}><header><GitBranch size={15}/><strong>{branch.ref.replace(/^refs\/(heads|remotes)\//u,'')}</strong>{branch.current&&<span>{t('当前')}</span>}</header><small>{branch.sha.slice(0,8)}{branch.upstream?` → ${branch.upstream}`:''}</small><div className="workbench-toolbar"><button disabled={disabled||branch.current} onClick={()=>onAction('checkout',{target:branch.ref.replace(/^refs\/heads\//u,'')})}>{t('切换')}</button><button disabled={disabled} onClick={()=>onAction('merge',{target:branch.ref})}><GitMerge size={13}/>{t('合并到当前')}</button><button disabled={disabled||branch.current} onClick={()=>onAction('worktree_add',{target:branch.ref.replace(/^refs\/heads\//u,'')})}>{t('创建独立工作目录')}</button></div></section>)}</div>
  if(view==='stash')return <div className="workbench-git-records"><aside>{parseWorkspaceStashes(result.output).map(stash=><section className="workbench-stash-row" key={stash.ref}><button onClick={()=>void show('stash_diff',{target:stash.ref},stash.ref)}><strong>{stash.ref}</strong><span>{stash.message}</span></button><div className="workbench-toolbar">{([['stash_apply','应用'],['stash_pop','应用并移除'],['stash_drop','删除']] as const).map(([action,label])=><button key={action} disabled={disabled} onClick={()=>onAction(action,{target:stash.ref})}>{t(label)}</button>)}</div></section>)}</aside>{detail}</div>
  if(view==='worktrees')return <div className="workbench-git-cards">{parseWorkspaceTrees(result.output).map(tree=><section key={tree.path}><header><GitBranch size={15}/><strong>{tree.branch||t('游离提交')}</strong></header><p>{tree.path}</p><small>{tree.head.slice(0,8)}{tree.locked?` · ${t('已锁定')}`:''}{tree.prunable?` · ${t('可清理')}`:''}</small><button disabled={disabled||tree.locked||!tree.branch} onClick={()=>onAction('worktree_remove',{target:tree.branch})}>{t('移除托管工作目录')}</button></section>)}</div>
  return <pre className="workbench-git-output">{result.output.replaceAll('\0','\n').replaceAll('\x1e','\n')}{result.error}</pre>
}
