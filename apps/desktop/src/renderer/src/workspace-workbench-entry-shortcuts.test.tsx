// @vitest-environment jsdom
import {createElement} from 'react'
import {afterEach,expect,it,vi} from 'vitest'
import {act,cleanup,fireEvent,render,waitFor} from '@testing-library/react'
import {ThreadClientProvider,type ThreadClient} from './camp-client'
import {WorkspaceWorkbenchEntry} from './WorkspaceWorkbenchEntry'
import {dispatchShortcutAction} from './shortcut-action-driver'
import type {FilePreviewContextValue} from './FilePreviewContext'
const state=vi.hoisted(()=>({preview:null as FilePreviewContextValue|null,host:null as HTMLElement|null}))
vi.mock('./FilePreviewContext',()=>({useOptionalFilePreview:()=>state.preview,useWorkbenchPreviewHost:()=>state.host}))
vi.mock('./WorkspaceWorkbench',()=>({WorkspaceWorkbench:(props:any)=>createElement('section',{'data-testid':'workbench','data-thread':props.threadId,'data-root':props.projectPath,'data-visible':String(props.visible),'data-terminal':props.terminalRequest})}))
afterEach(()=>{cleanup();state.host?.remove();state.host=null;state.preview=null;document.querySelectorAll('[data-engineering-entry]').forEach(el=>el.remove())})
function fixture(request: ReturnType<typeof vi.fn<() => Promise<unknown>>> =vi.fn(async()=>({workspaceRoot:'/verified/project'}))){
 state.host=document.createElement('div');document.body.append(state.host)
 const header=document.createElement('span');header.dataset.engineeringEntry='thread-a';document.body.append(header)
 const preview={paneVisible:false,tabs:[],activeTab:null,openWorkspace:vi.fn((tool:string)=>{preview.tabs=[{id:'tool',tool}];preview.paneVisible=true}),showPane:vi.fn(()=>{preview.paneVisible=true}),hidePane:vi.fn(()=>{preview.paneVisible=false})} as any;state.preview=preview
 let client={request,presentationScope:'fixture'} as unknown as ThreadClient,thread='thread-a'
 const tree=()=> <ThreadClientProvider client={client}><WorkspaceWorkbenchEntry threadId={thread} missionId="mission-current" projectPath="unverified" members={[]} bindLeaveGuard={()=>{}}><div>对话</div></WorkspaceWorkbenchEntry></ThreadClientProvider>
 const view=render(tree())
 return {view,request,preview,refresh:()=>view.rerender(tree()),switchOwner:(nextClient:ThreadClient,nextThread=thread)=>{client=nextClient;thread=nextThread;header.dataset.engineeringEntry=thread;view.rerender(tree())}}
}
it('只向顶栏提供一个开关，工具使用现有工作区宿主，收起保留实例',async()=>{
 const f=fixture();fireEvent.click(f.view.getByRole('button',{name:'展开工作区'}));await waitFor(()=>expect(f.preview.openWorkspace).toHaveBeenCalledWith('files',undefined,'工程文件'))
 f.refresh();const editor=f.view.getByTestId('workbench');expect(editor.parentElement).toBe(state.host);expect(document.querySelectorAll('.engineering-pane')).toHaveLength(0)
 fireEvent.click(f.view.getByRole('button',{name:'收起工作区'}));f.refresh();expect(f.preview.hidePane).toHaveBeenCalledOnce();expect(f.view.getByTestId('workbench')).toBe(editor)
 fireEvent.click(f.view.getByRole('button',{name:'展开工作区'}));await waitFor(()=>expect(f.preview.showPane).toHaveBeenCalledOnce());expect(f.request).toHaveBeenCalledTimes(2)
})
it('终端快捷键不打开第二个右栏，模态框打开时不截获',async()=>{
 const f=fixture();const overlay=document.createElement('div');overlay.setAttribute('role','dialog');overlay.setAttribute('aria-modal','true');document.body.append(overlay)
 await act(async()=>{expect(dispatchShortcutAction('toggle_terminal')).toBe(false)});expect(f.request).not.toHaveBeenCalled();overlay.remove()
 await act(async()=>{expect(dispatchShortcutAction('toggle_terminal')).toBe(true)});await waitFor(()=>expect(f.view.getByTestId('workbench').getAttribute('data-terminal')).toBe('-1'));expect(f.preview.openWorkspace).not.toHaveBeenCalled()
})
it('再次展开发现目录变更时保留旧编辑器，不把草稿迁移到新根目录',async()=>{
 const request=vi.fn().mockResolvedValueOnce({workspaceRoot:'/verified/first'}).mockResolvedValueOnce({workspaceRoot:'/verified/changed'}),f=fixture(request)
 fireEvent.click(f.view.getByRole('button',{name:'展开工作区'}));await waitFor(()=>expect(f.view.getByTestId('workbench').getAttribute('data-root')).toBe('/verified/first'));f.refresh()
 fireEvent.click(f.view.getByRole('button',{name:'收起工作区'}));f.refresh();fireEvent.click(f.view.getByRole('button',{name:'展开工作区'}));await waitFor(()=>expect(f.view.getByRole('alert').textContent).toContain('工程目录已变化'));expect(f.preview.showPane).not.toHaveBeenCalled();expect(f.view.getByTestId('workbench').getAttribute('data-root')).toBe('/verified/first')
})
it('旧主机的延迟目录结果不能挂到新主机；重复打开合并目录核验',async()=>{
 let resolveOld!:(value:unknown)=>void;const old=vi.fn(()=>new Promise(resolve=>{resolveOld=resolve})),f=fixture(old),fresh=vi.fn(async()=>({workspaceRoot:'/verified/new-host'}))
 fireEvent.click(f.view.getByRole('button',{name:'展开工作区'}));await act(async()=>{dispatchShortcutAction('toggle_terminal')});expect(old).toHaveBeenCalledTimes(1)
 f.switchOwner({request:fresh,presentationScope:'new'} as unknown as ThreadClient)
 fireEvent.click(f.view.getByRole('button',{name:'展开工作区'}));await waitFor(()=>expect(f.view.getByTestId('workbench').getAttribute('data-root')).toBe('/verified/new-host'));await act(async()=>resolveOld({workspaceRoot:'/stale'}));expect(f.view.getByTestId('workbench').getAttribute('data-root')).toBe('/verified/new-host')
})
it('没有经过主机核实的目录时不挂载工作区',async()=>{
 const f=fixture(vi.fn(async()=>({workspaceRoot:''})));fireEvent.click(f.view.getByRole('button',{name:'展开工作区'}));await waitFor(()=>expect(f.view.getByRole('alert').textContent).toContain('尚未提供已核实的工程目录'));expect(f.view.queryByTestId('workbench')).toBeNull()
})
