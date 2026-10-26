import { useEffect, useState } from 'react'
import { createRoot } from 'react-dom/client'
import type { FilePreviewApi,ThreadSnapshot } from '@contracts'
import { ThreadClientProvider,type ThreadClient } from '../../../apps/desktop/src/renderer/src/camp-client'
import { FilePreviewProvider,useFilePreview } from '../../../apps/desktop/src/renderer/src/FilePreviewContext'
import { FilePreviewWorkspace,FilePreviewResizeHandle } from '../../../apps/desktop/src/renderer/src/FilePreviewLayout'
import { FilePreviewPane } from '../../../apps/desktop/src/renderer/src/FilePreviewPane'
import { AppHeader } from '../../../apps/desktop/src/renderer/src/AppHeader'
import { WorkspaceWorkbenchEntry } from '../../../apps/desktop/src/renderer/src/WorkspaceWorkbenchEntry'
import { CoaxiStatusBar } from '../../../apps/desktop/src/renderer/src/CoaxiStatusBar'
import { requestWorkspaceTool } from '../../../apps/desktop/src/renderer/src/WorkspaceToolMenu'
import '../../../apps/desktop/src/renderer/src/styles.css'

const bridge=(window as any).unifiedFixture
const memory=new Map<string,unknown>()
const client={request:bridge.request,officeWorkspace:bridge.office,browserWorkspace:bridge.browser,interactionWorkspace:{request:bridge.interaction},presentationScope:'unified-isolated-test',onEvent:()=>()=>{},workbenchEditorRecovery:{get:async(key:string)=>memory.get(key)??null,put:async(value:any)=>{memory.set(value.key,structuredClone(value))}}} as unknown as ThreadClient
const previewApi=new Proxy({open:(source:unknown)=>bridge.request('fixture.preview.resolve',source),restore:(source:unknown)=>bridge.request('fixture.preview.resolve',source),readBinary:()=>bridge.request('fixture.preview.binary'),onExternalUpdate:()=>()=>{},onResourcesReleased:()=>()=>{}},{get:(target,key)=>key in target?(target as any)[key]:async()=>({ok:true})}) as FilePreviewApi
const thread={thread:{id:'test-thread'},approvals:[]} as unknown as ThreadSnapshot
function Workspace(){const preview=useFilePreview(),[text,setText]=useState(''),[files,setFiles]=useState<string[]>([])
 useEffect(()=>{(window as any).unifiedTest={open:(tool:string)=>requestWorkspaceTool('test-thread',tool as any),activate:(id:string)=>preview.activate(id),state:()=>({tabs:preview.tabs.map(tab=>({id:tab.id,kind:tab.kind,...(tab.kind==='workspace'?{tool:tab.tool,resource:tab.resource,dirty:tab.dirty}:{})})),active:preview.activeTabId,visible:preview.paneVisible,text,files}),hide:preview.hidePane,show:preview.showPane}},[preview,text,files])
 return <div className="app-shell app-shell-camp" style={{height:'100vh'}}><aside className="unified-sidebar" style={{gridRow:'1 / span 2',padding:'24px',background:'var(--rail)'}}><h2>Coaxi</h2><p>测试项目</p><p>工程协作</p></aside><AppHeader threadTitle="工程协作" contextLabel="测试项目" thread={thread} onFocusApprovals={()=>{}} conversationActions={<div style={{display:'flex',gap:12,fontSize:12}}>执行 4　任务 0　队员 4　单聊 0</div>}/><main className="content task-content" ><WorkspaceWorkbenchEntry threadId="test-thread" projectPath="" members={[]} bindLeaveGuard={()=>{}} onQuickMessage={value=>{setText(previous=>previous+(previous?'\n':'')+value);return true}} onWorkspaceFiles={async value=>{setFiles(previous=>[...previous,...value.map(file=>file.name)]);return true}}><section className="workspace-shell"><FilePreviewWorkspace><div className="timeline-pane" style={{display:'flex',flexDirection:'column',height:'100%',padding:24,minWidth:0}}><h3>把队员、工程和网页放在同一个工作区</h3><p style={{color:'var(--muted)',lineHeight:1.8}}>此实例仅使用本机测试数据，验证原生网页、工程编辑器、页签和菜单。</p><textarea aria-label="对话输入" value={text} onChange={event=>setText(event.target.value)} style={{marginTop:'auto',minHeight:140,width:'100%'}}/><p>{files.join('、')}</p></div><FilePreviewResizeHandle onClose={preview.hidePane}/><FilePreviewPane/></FilePreviewWorkspace></section></WorkspaceWorkbenchEntry></main><CoaxiStatusBar threadId="test-thread" sessionCount={1} updates={null} onSettings={()=>setText('设置入口已打开')} onUpdates={()=>{}} onOpenFolder={()=>{}} onProjectLauncher={()=>{}} onNotification={async()=>{}}/></div>
}
document.documentElement.dataset.theme='night';document.documentElement.style.colorScheme='dark'
createRoot(document.getElementById('root')!).render(<ThreadClientProvider client={client}><FilePreviewProvider api={previewApi} threadId="test-thread" resolvedTheme="night"><Workspace/></FilePreviewProvider></ThreadClientProvider>)
