import React from 'react'
import type { CanvasGraphSnapshot, CanvasNodeKind, ResolvedTheme } from '@contracts'
import { CanvasSurface } from '../../../apps/desktop/src/renderer/src/CanvasSurface'
import { CanvasFolderProvider } from '../../../apps/desktop/src/renderer/src/CanvasFolderProvider'
import { ThreadClientProvider, type ThreadClient } from '../../../apps/desktop/src/renderer/src/camp-client'

// Diagnostic observations only: do not swallow browser errors or defer library callbacks.
export function instrumentCanvasResize() {
 const native=window.ResizeObserver, trace:unknown[]=[], errors:unknown[]=[]
 ;(window as any).canvasResize={trace,errors}
 window.ResizeObserver=class extends native {
  constructor(callback:ResizeObserverCallback){
   const origin=new Error().stack
   super((entries,observer)=>{
    trace.push({origin,entries:entries.map(entry=>({target:entry.target.className,width:entry.contentRect.width,height:entry.contentRect.height}))})
    if(trace.length>32)trace.shift()
    callback.call(observer,entries,observer)
   })
  }
 }
 window.addEventListener('error',event=>{if(event.message.includes('ResizeObserver')){errors.push({message:event.message,trace:trace.slice(-6)});if(errors.length>6)errors.shift()}})
}

export function CanvasSourceFixture({client,theme}:{client:ThreadClient;theme:ResolvedTheme}){
 const board={id:'canvas-source-fixture',name:'源码卡片布局',description:null,color:null,revision:1,createdAt:'2026-10-16T00:00:00Z',updatedAt:'2026-10-16T00:00:00Z'}
 const kinds:CanvasNodeKind[]=[...Array<CanvasNodeKind>(64).fill('conversation'),'note','custom','file','terminal']
 const graph:CanvasGraphSnapshot={board,throughGlobalSequence:1,nodes:kinds.map((kind,i)=>({id:`source-node-${i}`,boardId:board.id,
  createdAt:board.createdAt,updatedAt:board.updatedAt,data:{kind,title:kind==='conversation'?`中文长标题 ${i} 保留四行内容与紧凑操作，核对缩放后的卡片布局`:null,
   color:kind==='note'?'orange':null,x:(i%8)*250,y:Math.floor(i/8)*165,width:kind==='conversation'?224:360,height:kind==='conversation'?132:260,
   collapsed:false,gridColumns:0,gridRows:0,path:null,folderPath:null,folderGroupId:null,agentId:null,runtimeKind:null,
   conversation:kind==='conversation'?{threadId:'source-thread',conversationId:null}:null,conversations:[],
   content:kind==='note'?'便笺正文应保持原版密度，双击可以编辑。':null}}))}
 const sourceClient={...client,request:async(method:string)=>{
  if(method==='canvas.nodes.snapshot')return graph
  throw new Error(`Unexpected source fixture request: ${method}`)
 }} as ThreadClient
 return <div className="canvas-workspace" style={{height:'100vh'}}><ThreadClientProvider client={sourceClient}><CanvasFolderProvider client={sourceClient}>
  <CanvasSurface boardId={board.id} scope={{host:'source-fixture',core:'source-fixture'}} agents={[]} theme={theme} onOpenReference={()=>{}}/>
 </CanvasFolderProvider></ThreadClientProvider></div>
}
