import {useEffect,useMemo,useState} from 'react'
import type {StoredCommandResult} from '@contracts'
import {TerminalSquare,Plus} from 'lucide-react'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {useCanvasBoards} from './useCanvasBoards'
import {useCanvasGraph} from './useCanvasGraph'
import {readCanvasViewScope} from './canvas-view-state'
import {CanvasTerminal} from './CanvasTerminal'
import {useShortcutActions} from './useShortcutActions'
import {NUMBERED_TAB_ACTION_IDS} from '../../shared/codeg-keyboard-shortcuts'
import type {ShortcutActions} from './shortcut-action-driver'
export function WorkspaceTerminals({projectPath,onUnsettledChange}:{projectPath:string;onUnsettledChange(value:boolean):void}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),boards=useCanvasBoards(),[boardId,setBoardId]=useState(''),[scope,setScope]=useState<string|null>(null),[error,setError]=useState<string|null>(null)
  useEffect(()=>{let active=true;void readCanvasViewScope(client).then(value=>{if(active)setScope(value?JSON.stringify([value.host,value.core]):null)}).catch(reason=>{if(active)setError(String(reason))});return()=>{active=false}},[client])
  useEffect(()=>{if(!boardId&&boards.state.snapshot?.boards[0])setBoardId(boards.state.snapshot.boards[0].id)},[boardId,boards.state.snapshot])
  const createBoard=async():Promise<void>=>{try{const controller=boards.controller.current;if(!controller)return;onUnsettledChange(true);const result=await controller.apply({operation:'canvas.boards.create',command:{name:t('工程终端')}});if(result.status==='rejected')throw new Error(result.code);const board=result.payload.board as {id?:string};if(board?.id)setBoardId(board.id)}catch(reason){setError(String(reason))}finally{onUnsettledChange(!!boards.controller.current?.getSnapshot().pending)}}
  useEffect(()=>()=>onUnsettledChange(false),[onUnsettledChange])
  return <div className="workbench-terminals"><div className="workbench-toolbar"><TerminalSquare size={16}/><strong>{t('工程终端')}</strong><select aria-label={t('终端所在画布')} value={boardId} onChange={event=>setBoardId(event.target.value)} disabled={!!boards.state.pending}>{boards.state.snapshot?.boards.map(board=><option key={board.id} value={board.id}>{board.name||t('未命名画布')}</option>)}</select><button disabled={boards.state.loading||!!boards.state.pending} onClick={()=>void createBoard()}><Plus size={14}/>{t('新建终端画布')}</button></div><p className="workbench-terminal-note">{t('这里与画布展示同一个终端，切换入口会保留进程。启动目录为当前工程，终端仍可访问当前用户有权限的其他目录。')}</p>{Boolean(error||boards.state.readError)&&<p role="alert" className="workbench-error">{error||String(boards.state.readError)}</p>}{boards.state.pending?.state==='unknown'&&<button onClick={()=>void boards.controller.current?.reconcile().then(()=>onUnsettledChange(!!boards.controller.current?.getSnapshot().pending)).catch(reason=>setError(String(reason)))}>{t('核对原画布操作')}</button>}{boardId&&scope?<BoardTerminals key={boardId} boardId={boardId} projectPath={projectPath} scope={scope} onUnsettledChange={onUnsettledChange}/>:<div className="workbench-empty">{t('选择或创建一个画布来放置终端。')}</div>}</div>
}
function BoardTerminals({boardId,projectPath,scope,onUnsettledChange}:{boardId:string;projectPath:string;scope:string;onUnsettledChange(value:boolean):void}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),graph=useCanvasGraph(boardId),[selected,setSelected]=useState<string|null>(null),[error,setError]=useState<string|null>(null)
  const normalize=(path:string):string=>{const value=path.replaceAll('\\','/').replace(/\/$/u,'');return client.platform==='win32'?value.toLowerCase():value}
  const terminals=useMemo(()=>(graph.snapshot?.nodes??[]).filter(node=>node.data.kind==='terminal'&&normalize(node.data.path??'')===normalize(projectPath)),[graph.snapshot,projectPath,client.platform])
  const current=terminals.find(node=>node.id===selected)??terminals[0]
  useEffect(()=>{onUnsettledChange(!!graph.pending);return()=>onUnsettledChange(false)},[graph.pending,onUnsettledChange])
  const accept=(result:StoredCommandResult|null):void=>{if(!result)return;if(result.status==='rejected')throw new Error(result.code);const ids=result.payload.nodeIds;if(Array.isArray(ids)&&typeof ids[0]==='string')setSelected(ids[0])}
  const create=async():Promise<void>=>{try{setError(null);const nodes=graph.snapshot?.nodes??[];const y=nodes.length?Math.max(...nodes.map(node=>node.data.y+node.data.height))+40:80;accept(await graph.apply({type:'create',node:{kind:'terminal',path:projectPath,title:t('工程终端'),x:80,y,width:760,height:460}}))}catch(reason){setError(String(reason))}}
  const board=graph.snapshot?.board
  const disabled=graph.loading||Boolean(graph.readError)||!!graph.pending||!board||graph.snapshot?.terminalVersion!==1
  const move=(delta:number):void=>{if(terminals.length){const index=terminals.findIndex(row=>row.id===current?.id);setSelected(terminals[(Math.max(index,0)+delta+terminals.length)%terminals.length].id)}}
  const close=async():Promise<void>=>{if(disabled||!current||!board)return;if(!window.confirm(t('关闭并删除这个工程终端？运行中的命令会停止。')))return;try{setError(null);accept(await graph.apply({type:'delete',nodeIds:[current.id]},board.revision));setSelected(null)}catch(error){setError(String(error))}}
  useShortcutActions({new_terminal_tab:()=>{if(!disabled)void create()},close_current_terminal_tab:()=>{void close()},close_current_tab:()=>{void close()},next_tab:()=>move(1),prev_tab:()=>move(-1),...Object.fromEntries(NUMBERED_TAB_ACTION_IDS.map((id,index)=>[id,()=>{if(terminals[index])setSelected(terminals[index].id)}])) as ShortcutActions},{priority:100,active:()=>{const dialogs=[...document.querySelectorAll('[role="dialog"][data-state="open"]')];return dialogs.at(-1)?.classList.contains('workspace-workbench')===true}})
  return <><div className="workbench-tabs">{terminals.map((node,index)=><button key={node.id} className={current?.id===node.id?'quiet-button is-active':'quiet-button'} onClick={()=>setSelected(node.id)}>{node.data.title||`${t('终端')} ${index+1}`}</button>)}<button disabled={disabled} onClick={()=>void create()}><Plus size={14}/>{t('新建终端')}</button></div>{Boolean(error||graph.readError||graph.commandError)&&<p className="workbench-error" role="alert">{error||String(graph.readError||graph.commandError)}</p>}{graph.pending?.state==='unknown'&&<div className="workbench-toolbar"><button onClick={()=>void graph.reconcile().then(accept).catch(reason=>setError(String(reason)))}>{t('核对原创建')}</button><button onClick={()=>void graph.retry().then(accept).catch(reason=>setError(String(reason)))}>{t('重试原创建')}</button></div>}{current?<div className="workbench-terminal-body"><CanvasTerminal row={current} scope={scope} terminalVersion={graph.snapshot?.terminalVersion} disabled={disabled}/></div>:<div className="workbench-empty">{t('还没有当前工程的终端，点击新建终端。')}</div>}</>
}
