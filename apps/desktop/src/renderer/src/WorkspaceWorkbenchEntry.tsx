import {createContext,useContext,useEffect,useId,useLayoutEffect,useMemo,useRef,useState,type ComponentProps,type ReactNode} from 'react'
import {createPortal} from 'react-dom'
import {FolderOpen,LoaderCircle,Maximize2,Minimize2} from 'lucide-react'
import {PanelToggleIcon} from './PanelToggleIcon'
import {useOptionalFilePreview} from './FilePreviewContext'
import {readErrorMessage} from './error-message'
import {useThreadClient} from './camp-client'
import {WorkspaceWorkbench} from './WorkspaceWorkbench'
import {useUiText} from './interface-language'
import {useShortcutActions} from './useShortcutActions'
import './engineering-dock.css'

type Props=Omit<ComponentProps<typeof WorkspaceWorkbench>,'initialTab'|'initiallyOpen'|'onClosed'|'onMaximize'|'maximized'|'visible'|'terminalRequest'> & {missionId?:string|null;children?:ReactNode;headerEntry?:boolean;enabled?:boolean}
type Ready={root:string;owner:object}
const EngineeringDockContext=createContext({opened:0,collapse:(_restoreFocus?:boolean)=>{}})
export const useEngineeringDock=()=>useContext(EngineeringDockContext)

/** The verified root and mounted editor survive folding; another owner never inherits them. */
export function WorkspaceWorkbenchEntry({children,headerEntry=true,enabled=true,...props}:Props):React.JSX.Element{
  const client=useThreadClient(),t=useUiText()
  const filePreview=useOptionalFilePreview(),previewVisible=!!filePreview?.paneVisible
  const paneId=useId()
  const owner=useMemo(()=>({client,threadId:props.threadId,missionId:props.missionId??null}),[client,props.threadId,props.missionId])
  const current=useRef(owner);current.current=owner
  const generation=useRef(0),generationOwner=useRef(owner)
  if(generationOwner.current!==owner){generationOwner.current=owner;generation.current+=1}
  const busyOwnerRef=useRef<object|null>(null),shell=useRef<HTMLDivElement>(null),trigger=useRef<HTMLButtonElement>(null)
  const [ready,setReady]=useState<Ready|null>(null),[busyOwner,setBusyOwner]=useState<object|null>(null),[issue,setIssue]=useState<{owner:object;message:string}|null>(null)
  const [shown,setShown]=useState<object|null>(null),[maximized,setMaximized]=useState(false),[opened,setOpened]=useState(0),[terminalCommand,setTerminalCommand]=useState({owner,value:0})
  const terminalRequest=terminalCommand.owner===owner?terminalCommand.value:0
  const [host,setHost]=useState<Element|null>(null),[previewHost,setPreviewHost]=useState<Element|null>(null),[width,setWidth]=useState(()=>{try{const n=Number(localStorage.getItem('coaxi.engineering.width'));return n>=30&&n<=75?n:54}catch{return 54}})
  const active=ready?.owner===owner?ready:null,visible=enabled&&shown===owner&&!!active,busy=busyOwner===owner
  const lastPanel=useRef({owner,preview:false})
  useEffect(()=>{if(previewVisible||visible)lastPanel.current={owner,preview:previewVisible}},[owner,previewVisible,visible])
  const previewMode=previewVisible||!visible&&lastPanel.current.owner===owner&&lastPanel.current.preview&&!!filePreview?.activeTab
  const toggleLabel=previewMode?(previewVisible?'收起文件预览':'展开文件预览'):visible?'收起工程区':'展开工程区'
  const collapse=(restoreFocus=false):void=>{setShown(null);setMaximized(false);if(restoreFocus&&visible)trigger.current?.focus()}
  useLayoutEffect(()=>{
    if(!headerEntry){setHost(null);setPreviewHost(null);return}
    let target:Element|null=null,previewTarget:Element|null=null
    const resolve=():void=>{
      if(!target?.isConnected){target=[...document.querySelectorAll('[data-engineering-entry]')].find(el=>(el as HTMLElement).dataset.engineeringEntry===props.threadId)??null;setHost(target)}
      if(!previewTarget?.isConnected){previewTarget=[...document.querySelectorAll('[data-engineering-preview-entry]')].find(el=>(el as HTMLElement).dataset.engineeringPreviewEntry===(filePreview?.domScope??'global'))??null;setPreviewHost(previewTarget)}
    }
    resolve();const observer=new MutationObserver(resolve);observer.observe(document.body,{childList:true,subtree:true});return()=>observer.disconnect()
  },[props.threadId,headerEntry,filePreview?.domScope])
  const open=async(terminal=false):Promise<void>=>{
    if(busyOwnerRef.current===owner)return
    const captured=owner,capturedGeneration=generation.current
    const isCurrent=():boolean=>current.current===captured&&generation.current===capturedGeneration
    busyOwnerRef.current=captured;setBusyOwner(captured);setIssue(null)
    try{
      const result=await client.request<{workspaceRoot:string}>('workbench.files.list',{scope:{threadId:props.threadId,missionId:props.missionId??null},path:''})
      if(!isCurrent())return
      if(typeof result.workspaceRoot!=='string'||!result.workspaceRoot)throw new Error(t('当前主机尚未提供已核实的工程目录。'))
      if(active&&active.root!==result.workspaceRoot)throw new Error(t('工程目录已变化，请先保存当前草稿，再重新进入对话。'))
      setReady(previous=>previous?.owner===captured?previous:{root:result.workspaceRoot,owner:captured});setShown(captured);setOpened(value=>value+1)
      if(terminal)setTerminalCommand(value=>({owner:captured,value:visible?-(Math.abs(value.owner===captured?value.value:0)+1):Math.abs(value.owner===captured?value.value:0)+1}))
    }catch(error){if(isCurrent())setIssue({owner:captured,message:readErrorMessage(error)})}
    finally{if(isCurrent()){busyOwnerRef.current=null;setBusyOwner(null)}}
  }
  useShortcutActions({toggle_terminal:()=>{void open(true)},toggle_aux_panel:()=>{if(visible)collapse();else void open()}},{priority:25,active:()=>enabled&&!document.querySelector('.launcher-web-page, [role="dialog"][aria-modal="true"], [role="menu"][data-state="open"]')})
  const rememberWidth=(value:number):void=>{const next=Math.min(75,Math.max(30,value));setWidth(next);try{localStorage.setItem('coaxi.engineering.width',String(next))}catch{}}
  const control=<div className="engineering-controls" role="group" aria-label={t('工程区显示')} data-floating={!host||undefined}>
    {visible&&<button type="button" className="quiet-button engineering-toggle" title={t(maximized?'恢复并排':'放大工程区')} aria-label={t(maximized?'恢复并排':'放大工程区')} aria-controls={paneId} onClick={()=>setMaximized(value=>!value)}>{maximized?<Minimize2 size={18}/>:<Maximize2 size={18}/>}</button>}
    <button ref={trigger} type="button" className="file-preview-toggle engineering-unified-toggle" disabled={busy} title={t(busy?'正在核对工程…':toggleLabel)} aria-label={t(toggleLabel)} aria-expanded={visible||previewVisible} aria-controls={previewMode?'file-preview-pane':paneId} onClick={()=>previewMode?previewVisible?filePreview!.hidePane():filePreview!.showPane():visible?collapse():void open()}>{busy?<LoaderCircle size={18}/>:<PanelToggleIcon side="right" visible={visible||previewVisible}/>}</button>
  </div>
  return <EngineeringDockContext.Provider value={{opened,collapse}}><div ref={shell} className={`engineering-layout${visible?' is-open':''}${visible&&maximized?' is-maximized':''}`} style={{'--engineering-width':`${width}%`} as React.CSSProperties}>
    <div className="engineering-conversation" hidden={visible&&maximized}>{children}{issue?.owner===owner&&<div role="alert" className="workbench-notice">{issue.message}</div>}</div>
    {!host&&control}
    {host&&createPortal(<>{control}{issue?.owner===owner&&<span role="alert" className="engineering-entry-error">{issue.message}</span>}</>,host)}
    {previewHost&&previewVisible&&enabled&&createPortal(<button type="button" className="file-preview-toggle" disabled={busy} title={t('打开工程区')} aria-label={t('打开工程区')} onClick={()=>void open()}><FolderOpen size={16}/></button>,previewHost)}
    <div className="engineering-divider" hidden={!visible||maximized} role="separator" tabIndex={0} aria-label={t('调整工程区宽度')} aria-orientation="vertical" aria-valuemin={30} aria-valuemax={75} aria-valuenow={Math.round(width)} onDoubleClick={()=>rememberWidth(54)} onKeyDown={event=>{if(event.key==='ArrowLeft'||event.key==='ArrowRight'){event.preventDefault();rememberWidth(width+(event.key==='ArrowLeft'?2:-2))}if(event.key==='Home')rememberWidth(54)}} onPointerDown={event=>{if(event.button!==0)return;event.preventDefault();event.currentTarget.setPointerCapture(event.pointerId)}} onPointerMove={event=>{if(!event.currentTarget.hasPointerCapture(event.pointerId)||!shell.current)return;const rect=shell.current.getBoundingClientRect();rememberWidth((rect.right-event.clientX)/rect.width*100)}}/>
    <aside id={paneId} className="engineering-pane" aria-label={t('工程区')} hidden={!visible}>
      {active&&<WorkspaceWorkbench {...props} projectPath={active.root} initiallyOpen visible={visible} terminalRequest={terminalRequest} onClosed={()=>collapse(true)}/>}
    </aside>
  </div></EngineeringDockContext.Provider>
}
