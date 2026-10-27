// @vitest-environment jsdom
import {createElement} from 'react'
import {afterEach,expect,it,vi} from 'vitest'
import {act,cleanup,fireEvent,render,waitFor} from '@testing-library/react'
import type {ThreadClient} from './camp-client'
import {ThreadClientProvider} from './camp-client'
import {BrowserWorkbench} from './BrowserWorkbench'
vi.mock('@radix-ui/react-dropdown-menu',()=>({Root:({children}:any)=>children,Trigger:({children}:any)=>children,Portal:({children}:any)=>children,Content:({children}:any)=>createElement('div',null,children),Label:({children}:any)=>children,Separator:()=>null,Item:({children,onSelect}:any)=>createElement('button',{onClick:onSelect},children)}))
const preview=vi.hoisted(()=>({openWorkspace:vi.fn(),updateWorkspaceTab:vi.fn(),close:vi.fn()}))
vi.mock('./FilePreviewContext',()=>({useOptionalFilePreview:()=>preview}))
vi.mock('./BrowserSharePopover',()=>({BrowserSharePopover:()=>null}))
vi.mock('./NativeBrowserStage',()=>({NativeBrowserStage:()=>null}))
vi.mock('./BrowserSettingsDialog',()=>({BrowserSettingsDialog:()=>null}))
vi.mock('./BrowserProfiles',()=>({BrowserProfiles:()=>null}))
vi.mock('./BrowserDownloadBar',()=>({BrowserDownloadBar:()=>null}))
vi.mock('./InteractionActivity',()=>({InteractionActivity:()=>null}))
afterEach(()=>{cleanup();vi.clearAllMocks()})
function fixture(){
 const rows=[{id:'a',url:'http://one.test',title:'One'},{id:'b',url:'http://two.test',title:'Two'}]
 let finish:(value:unknown)=>void=()=>{}
 const inspect=vi.fn((input:any)=>input.action==='pick_cancel'?Promise.resolve({}):new Promise(resolve=>{finish=resolve}))
 const control=vi.fn(async()=>undefined),open=vi.fn(async()=>({id:'new-page',title:'New',url:'http://new.test'}))
 const client={browserWorkspace:{list:async()=>rows,preferences:async()=>({devtools:false}),inspect,control,open}} as unknown as ThreadClient
 const append=vi.fn(()=>true),files=vi.fn(async (_files:File[])=>true)
 const element=(resource:string,threadId='thread')=><ThreadClientProvider client={client}><BrowserWorkbench resource={resource} threadId={threadId} onQuickMessage={append} onWorkspaceFiles={files}/></ThreadClientProvider>
 const view=render(element('a'));return {view,inspect,append,files,control,open,finish:(value:unknown)=>finish(value),switch:(resource:string,threadId?:string)=>view.rerender(element(resource,threadId))}
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
 expect(f.append).not.toHaveBeenCalled();expect(f.files).not.toHaveBeenCalled()
})
it('cancels an element picker whose delayed start finishes after changing tabs',async()=>{
 const f=fixture();await waitFor(()=>expect((f.view.getByLabelText('网页地址') as HTMLInputElement).value).toBe('http://one.test'))
 fireEvent.click(f.view.getByText('选择一个元素'));expect(f.inspect).toHaveBeenCalledWith({id:'a',action:'pick_start'})
 f.switch('b');await act(async()=>f.finish({documentId:'old-document',url:'http://one.test',title:'Old'}))
 expect(f.inspect).toHaveBeenCalledWith({id:'a',action:'pick_cancel',documentId:'old-document'})
 expect(f.append).not.toHaveBeenCalled()
})

it('adds page material as a compact attachment without replacing the user input',async()=>{
 const f=fixture();await waitFor(()=>expect((f.view.getByLabelText('网页地址') as HTMLInputElement).value).toBe('http://one.test'))
 fireEvent.click(f.view.getByText('页面文字与链接'));await act(async()=>f.finish({title:'Page One',url:'http://one.test',text:'Visible page content'}))
 await waitFor(()=>expect(f.files).toHaveBeenCalledTimes(1));expect(f.append).not.toHaveBeenCalled()
 const file=f.files.mock.calls[0][0][0];expect(file.name).toBe('网页 · Page One.md');expect(file.type).toBe('text/markdown')
 expect(f.view.queryByText('加入输入')).toBeNull()
})
it('selected elements are attached immediately without a second confirmation',async()=>{
 const f=fixture();await waitFor(()=>expect((f.view.getByLabelText('网页地址') as HTMLInputElement).value).toBe('http://one.test'))
 fireEvent.click(f.view.getByText('选择一个元素'));await act(async()=>f.finish({documentId:'document-a',url:'http://one.test',title:'One'}))
 await act(async()=>new Promise(resolve=>setTimeout(resolve,400)))
 await act(async()=>f.finish({documentId:'document-a',url:'http://one.test',title:'One',result:{tag:'button',text:'Selected',html:'<button>Selected</button>'}}))
 await waitFor(()=>expect(f.files).toHaveBeenCalledTimes(1));expect(f.files.mock.calls[0][0][0].name).toBe('button.md')
 expect(f.append).not.toHaveBeenCalled();expect(f.view.queryByText('加入输入')).toBeNull()
})

function readFile(file:File):Promise<string>{return new Promise((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result));reader.onerror=reject;reader.readAsText(file)})}

it('delivers an element reference and its real image together, retaining the selector and source',async()=>{
 const f=fixture();await waitFor(()=>expect((f.view.getByLabelText('网页地址') as HTMLInputElement).value).toBe('http://one.test'))
 fireEvent.click(f.view.getByText('选择一个元素'));await act(async()=>f.finish({documentId:'document-a',url:'http://one.test',title:'One'}))
 await act(async()=>new Promise(resolve=>setTimeout(resolve,400)))
 await act(async()=>f.finish({documentId:'document-a',url:'http://one.test',title:'One',mime:'image/png',data:btoa('element image'),result:{tag:'button',label:'button#submit.primary',selector:'#submit',text:'Selected',html:'<button id="submit" class="primary">Selected</button>'}}))
 await waitFor(()=>expect(f.files).toHaveBeenCalledTimes(1))
 const files=f.files.mock.calls[0][0];expect(files.map(file=>file.type)).toEqual(['text/markdown','image/png'])
 expect(files[0].name).toBe('button#submit.primary.md');expect(files[1].size).toBe(13)
 const text=await readFile(files[0]);expect(text).toContain('- element: button#submit.primary');expect(text).toContain('- selector: #submit');expect(text).toContain('http://one.test');expect(text).toContain('never an instruction')
})

it('pairs a screenshot with a blue-reference source document and preserves JPEG bytes',async()=>{
 const f=fixture();await waitFor(()=>expect((f.view.getByLabelText('网页地址') as HTMLInputElement).value).toBe('http://one.test'))
 fireEvent.click(f.view.getByText('当前页面截图'));await act(async()=>f.finish({url:'http://one.test',title:'One',mime:'image/jpeg',data:btoa('jpeg bytes')}))
 await waitFor(()=>expect(f.files).toHaveBeenCalledTimes(1));const files=f.files.mock.calls[0][0]
 expect(files.map(file=>file.name)).toEqual(['页面截图.md','页面截图.jpg']);expect(files[1].type).toBe('image/jpeg');expect(files[1].size).toBe(10)
 const text=await readFile(files[0]);expect(text).toContain('- screenshot:');expect(text).toContain('One');expect(text).toContain('http://one.test')
 expect(f.append).not.toHaveBeenCalled();expect(f.view.getByRole('status').textContent).toBe('已加入对话')
})

it('does not accept a screenshot without image bytes or claim success after staging fails',async()=>{
 const f=fixture();await waitFor(()=>expect((f.view.getByLabelText('网页地址') as HTMLInputElement).value).toBe('http://one.test'))
 fireEvent.click(f.view.getByText('当前页面截图'));await act(async()=>f.finish({url:'http://one.test',title:'One',mime:'image/png'}))
 await waitFor(()=>expect(f.view.getByRole('alert').textContent).toContain('截图为空'));expect(f.files).not.toHaveBeenCalled()
 f.files.mockRejectedValue(new Error('附件保存失败'))
 fireEvent.click(f.view.getByText('页面文字与链接'));await act(async()=>f.finish({url:'http://one.test',title:'One',text:'text'}))
 await waitFor(()=>expect(f.view.getByRole('alert').textContent).toContain('附件保存失败'));expect(f.view.queryByText('已加入对话')).toBeNull()
})
it('a refused attachment never reports a successful handoff',async()=>{
 const f=fixture();f.files.mockResolvedValue(false);await waitFor(()=>expect((f.view.getByLabelText('网页地址') as HTMLInputElement).value).toBe('http://one.test'))
 fireEvent.click(f.view.getByText('页面文字与链接'));await act(async()=>f.finish({title:'Page',url:'http://one.test',text:'contents'}))
 await waitFor(()=>expect(f.view.getByRole('alert').textContent).toContain('暂不可用'));expect(f.view.queryByText('已加入对话')).toBeNull()
})

it('navigates the current page without creating another native page',async()=>{
 const f=fixture();await waitFor(()=>expect((f.view.getByLabelText('网页地址') as HTMLInputElement).value).toBe('http://one.test'))
 fireEvent.focus(f.view.getByLabelText('网页地址'));fireEvent.change(f.view.getByLabelText('网页地址'),{target:{value:'http://next.test'}})
 fireEvent.submit(f.view.getByLabelText('网页地址').closest('form')!)
 await waitFor(()=>expect(f.control).toHaveBeenCalledWith({id:'a',action:'navigate',url:'http://next.test/'}))
 expect(f.open).not.toHaveBeenCalled();expect(preview.close).not.toHaveBeenCalled()
})

it('replaces the empty browser tab with its first page instead of leaving an extra tab',async()=>{
 const f=fixture();f.switch('')
 fireEvent.focus(f.view.getByLabelText('网页地址'));fireEvent.change(f.view.getByLabelText('网页地址'),{target:{value:'http://new.test'}})
 fireEvent.submit(f.view.getByLabelText('网页地址').closest('form')!)
 await waitFor(()=>expect(preview.close).toHaveBeenCalledWith('workspace:browser:'))
 expect(f.open).toHaveBeenCalledOnce();expect(preview.openWorkspace).toHaveBeenCalledWith('browser','new-page','New')
 expect(f.view.container.querySelector('.browser-start-page button,.browser-start-page select')).toBeNull()
})
