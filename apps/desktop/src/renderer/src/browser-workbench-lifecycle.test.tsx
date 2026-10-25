// @vitest-environment jsdom
import {createElement} from 'react'
import {afterEach,expect,it,vi} from 'vitest'
import {act,cleanup,fireEvent,render,waitFor} from '@testing-library/react'
import type {ThreadClient} from './camp-client'
import {ThreadClientProvider} from './camp-client'
import {BrowserWorkbench} from './BrowserWorkbench'
vi.mock('@radix-ui/react-dropdown-menu',()=>({Root:({children}:any)=>children,Trigger:({children}:any)=>children,Portal:({children}:any)=>children,Content:({children}:any)=>createElement('div',null,children),Label:({children}:any)=>children,Separator:()=>null,Item:({children,onSelect}:any)=>createElement('button',{onClick:onSelect},children)}))
vi.mock('./FilePreviewContext',()=>({useOptionalFilePreview:()=>({openWorkspace:vi.fn(),updateWorkspaceTab:vi.fn()})}))
vi.mock('./BrowserSharePopover',()=>({BrowserSharePopover:()=>null}))
vi.mock('./NativeBrowserStage',()=>({NativeBrowserStage:()=>null}))
vi.mock('./BrowserSettingsDialog',()=>({BrowserSettingsDialog:()=>null}))
vi.mock('./BrowserProfiles',()=>({BrowserProfiles:()=>null}))
vi.mock('./BrowserDownloadBar',()=>({BrowserDownloadBar:()=>null}))
vi.mock('./InteractionActivity',()=>({InteractionActivity:()=>null}))
afterEach(cleanup)
function fixture(){
 const rows=[{id:'a',url:'http://one.test',title:'One'},{id:'b',url:'http://two.test',title:'Two'}]
 let finish:(value:unknown)=>void=()=>{}
 const inspect=vi.fn((input:any)=>input.action==='pick_cancel'?Promise.resolve({}):new Promise(resolve=>{finish=resolve}))
 const client={browserWorkspace:{list:async()=>rows,preferences:async()=>({devtools:false}),inspect}} as unknown as ThreadClient
 const append=vi.fn(()=>true)
 const element=(resource:string,threadId='thread')=><ThreadClientProvider client={client}><BrowserWorkbench resource={resource} threadId={threadId} onQuickMessage={append}/></ThreadClientProvider>
 const view=render(element('a'));return {view,inspect,append,finish:(value:unknown)=>finish(value),switch:(resource:string,threadId?:string)=>view.rerender(element(resource,threadId))}
}
it('replaces an edited address with the real URL when switching page tabs',async()=>{
 const f=fixture();await waitFor(()=>expect((f.view.getByLabelText('网页地址') as HTMLInputElement).value).toBe('http://one.test'))
 fireEvent.focus(f.view.getByLabelText('网页地址'));fireEvent.change(f.view.getByLabelText('网页地址'),{target:{value:'http://draft.test'}})
 f.switch('b');await waitFor(()=>expect((f.view.getByLabelText('网页地址') as HTMLInputElement).value).toBe('http://two.test'))
})
it('does not deliver an old page result to another conversation',async()=>{
 const f=fixture();await waitFor(()=>expect(f.view.getByText('页面文字与链接')).toBeTruthy())
 fireEvent.click(f.view.getByText('页面文字与链接'));expect(f.inspect).toHaveBeenCalledWith({id:'a',action:'page'})
 f.switch('b','other-thread');await act(async()=>f.finish({title:'Old private page',url:'http://one.test',text:'old text'}))
 expect(f.append).not.toHaveBeenCalled()
})
it('cancels an element picker whose delayed start finishes after changing tabs',async()=>{
 const f=fixture();await waitFor(()=>expect((f.view.getByLabelText('网页地址') as HTMLInputElement).value).toBe('http://one.test'))
 fireEvent.click(f.view.getByText('选择一个元素'));expect(f.inspect).toHaveBeenCalledWith({id:'a',action:'pick_start'})
 f.switch('b');await act(async()=>f.finish({documentId:'old-document',url:'http://one.test',title:'Old'}))
 expect(f.inspect).toHaveBeenCalledWith({id:'a',action:'pick_cancel',documentId:'old-document'})
 expect(f.append).not.toHaveBeenCalled()
})
