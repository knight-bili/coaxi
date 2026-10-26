// @vitest-environment jsdom
import { createElement } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import type { ThreadClient } from './camp-client'
import { ThreadClientProvider } from './camp-client'
import { WorkspaceWorkbench } from './WorkspaceWorkbench'

const previewActions = vi.hoisted(()=>({open:vi.fn(async()=>({kind:'preview',tabId:'image'})),openWorkspace:vi.fn(),updateWorkspaceTab:vi.fn(),bindWorkspaceClose:vi.fn(()=>()=>{}),close:vi.fn()}))
vi.mock('./FilePreviewContext', () => ({
  useOptionalFilePreview: () => ({ resolvedTheme: 'night', tabs:[], activeTab:{kind:'workspace',tool:'files'},...previewActions }),
  useOptionalFilePreviewApi: () => ({}),
  PinnedFilePreview: ({ source }: { source: { rawReference: string } }) => createElement('img', { alt: source.rawReference })
}))
vi.mock('react-resizable-panels', () => ({
  PanelGroup: ({ children }: { children: React.ReactNode }) => createElement('div', null, children),
  Panel: ({ children }: { children: React.ReactNode }) => createElement('div', null, children),
  PanelResizeHandle: () => null
}))
vi.mock('./WorkspaceCodeEditor', () => ({ WorkspaceCodeEditor: ({text,onChange}:{text:string;onChange(text:string):void}) => createElement('textarea',{ 'aria-label':'文本编辑器',value:text,onChange:(event:React.ChangeEvent<HTMLTextAreaElement>)=>onChange(event.target.value) }) }))
vi.mock('./SplitTabDeck', () => ({ SplitTabDeck: ({items,selected,renderBody}:{items:Array<{id:string}>;selected:string;renderBody(item:{id:string},visible:boolean):React.ReactNode}) => createElement('div',null,items.filter(item=>item.id===selected).map(item=>createElement('div',{key:item.id},renderBody(item,true)))) }))
vi.mock('./WorkspaceTerminals', () => ({ WorkspaceTerminals: () => null }))
vi.mock('./ProjectLauncher', () => ({ ProjectLauncher: () => null }))
vi.mock('./WorkspaceGitViews', () => ({ WorkspaceGitViews: () => null }))
vi.mock('./WorkspaceMergeEditor', () => ({ WorkspaceMergeEditor: () => null }))
vi.mock('./WorkspaceOrganizer', () => ({ WorkspaceOrganizer: () => null }))
vi.mock('./ForgeWorkbench', () => ({ ForgeWorkbench: () => null }))
vi.mock('./BrowserWorkbench', () => ({ BrowserWorkbench: () => null }))
vi.mock('./ComputerWorkbench', () => ({ ComputerWorkbench: () => null }))
vi.mock('./OfficePreviewWorkbench', () => ({ OfficePreviewWorkbench: ({path}:{path:string}) => createElement('div',null,'文档预览：'+path) }))
vi.mock('./NativeHistoryWorkbench', () => ({ NativeHistoryWorkbench: () => null }))
vi.mock('./codeg-native-content/components/diff/unified-diff-preview', () => ({ UnifiedDiffPreview: () => null }))
vi.mock('./useShortcutActions', () => ({ useShortcutActions: () => undefined }))
afterEach(()=>{cleanup();vi.clearAllMocks()})

function fixture(readError = '文件无法读取，请检查访问权限。', readable = false) {
  const records = new Map<string, unknown>()
  let finishSave: (() => void) | undefined
  const request = vi.fn(async (method: string, input?: unknown) => {
    if (method === 'app.info') return { dataDir: '/fixture-core' }
    if (method === 'workbench.files.list') return { entries: ['image.png', 'other.jpg', 'document.pdf', 'page.html', 'report.docx', 'sheet.xlsx', 'slides.pptx', 'broken.txt', 'second.txt'].map(path => ({ name: path, path, directory: false, link: false, bytes: 10 })), next: null }
    if (method === 'workbench.files.read') {if(readable)return {path:(input as {path:string}).path,text:'原有中文内容',digest:'digest'};throw { message: readError }}
    if (method !== 'workbench.editor') throw new Error(method)
    const params = input as { operation: string; value?: unknown }
    if (params.operation === 'save') await new Promise<void>(resolve => { finishSave = resolve })
    return { revision: params.operation === 'save' ? 'saved' : null, workspaceRoot: '/project', value: params.value ?? null }
  })
  const client = { request, presentationScope: 'fixture', workbenchEditorRecovery: {
    get: async (key: string) => structuredClone(records.get(key) ?? null),
    put: async (value: { key: string }) => { records.set(value.key, structuredClone(value)) }
  } } as unknown as ThreadClient
  const closed = vi.fn();let leaveGuard:(()=>void)|null=null
  const view = render(<ThreadClientProvider client={client}><WorkspaceWorkbench threadId="thread" projectPath="/project" members={[]} bindLeaveGuard={guard=>{leaveGuard=guard}} initiallyOpen onClosed={closed}/></ThreadClientProvider>)
  return { view, request, closed, records, guard:()=>leaveGuard?.(), finish: () => finishSave?.() }
}

describe('工程文件打开与关闭', () => {
  it('单击图片进入受控预览，不向文本接口读取图片', async () => {
    const { view, request, finish } = fixture()
    await waitFor(() => expect(view.getByRole('treeitem', { name: 'image.png' })).toBeTruthy())
    fireEvent.click(view.getByRole('treeitem', { name: 'image.png' }))
    await waitFor(()=>expect(view.getByAltText('image.png')).toBeTruthy())
    expect(previewActions.open).not.toHaveBeenCalled()
    expect(previewActions.openWorkspace).not.toHaveBeenCalled()
    expect(request.mock.calls.some(([method]) => method === 'workbench.files.read')).toBe(false)
    finish()
  })

  it('图片、网页、PDF 和办公文档在文件树旁切换，不新增顶层页签；返回文本保留未保存修改',async()=>{
    const {view,request}=fixture('',true)
    await waitFor(()=>expect(view.getByText('编辑草稿需要处理')).toBeTruthy())
    fireEvent.click(view.getByRole('treeitem',{name:'broken.txt'}))
    await waitFor(()=>expect(view.getByLabelText('文本编辑器')).toBeTruthy())
    fireEvent.change(view.getByLabelText('文本编辑器'),{target:{value:'尚未保存的中文修改'}})
    for(const path of ['image.png','other.jpg','document.pdf','page.html']){
      fireEvent.click(view.getByRole('treeitem',{name:path}))
      await waitFor(()=>expect(view.getByAltText(path)).toBeTruthy())
      expect(view.queryByLabelText('文本编辑器')).toBeNull()
    }
    for(const path of ['report.docx','sheet.xlsx','slides.pptx']){
      fireEvent.click(view.getByRole('treeitem',{name:path}))
      await waitFor(()=>expect(view.getByText('文档预览：'+path)).toBeTruthy())
    }
    fireEvent.click(view.getByRole('treeitem',{name:'broken.txt'}))
    await waitFor(()=>expect((view.getByLabelText('文本编辑器') as HTMLTextAreaElement).value).toBe('尚未保存的中文修改'))
    fireEvent.click(view.getByRole('treeitem',{name:'second.txt'}))
    await waitFor(()=>expect((view.getByLabelText('文本编辑器') as HTMLTextAreaElement).value).toBe('原有中文内容'))
    expect(previewActions.open).not.toHaveBeenCalled()
    expect(previewActions.openWorkspace).not.toHaveBeenCalled()
    expect(request.mock.calls.filter(([method])=>method==='workbench.files.read')).toHaveLength(2)
  })

  it('文本读取失败保留可读错误；未补存的草稿仍阻止离开工程', async () => {
    const { view, request, records, finish, guard } = fixture()
    await waitFor(() => expect(view.getByText('编辑草稿需要处理')).toBeTruthy())
    fireEvent.click(view.getByRole('treeitem', { name: 'broken.txt' }))
    await waitFor(() => expect(view.getByRole('alert').textContent).toContain('文件无法读取'))
    expect(view.queryByText('[object Object]')).toBeNull()
    expect(view.getByText('选择文件，开始查看')).toBeTruthy()
    expect(guard).toThrow('编辑草稿或布局仍未补存')
    await waitFor(() => expect(request.mock.calls.some(([, input]) => (input as { operation?: string })?.operation === 'save')).toBe(true))
    expect(records.size).toBeGreaterThan(0)
    finish()
    await waitFor(() => expect(guard).not.toThrow())
    expect([...records.values()].some(value => (value as { latest?: { selected: unknown } }).latest?.selected)).toBe(false)
  })

  it('将 Core 的非文本拒绝转换为可理解的提示', async () => {
    const { view } = fixture('workbench.text_required: invalid utf-8 sequence')
    await waitFor(() => expect(view.getByText('编辑草稿需要处理')).toBeTruthy())
    fireEvent.click(view.getByRole('treeitem', { name: 'broken.txt' }))
    await waitFor(() => expect(view.getByRole('alert').textContent).toContain('这个文件不是可编辑的文本'))
    expect(view.getByRole('alert').textContent).not.toContain('workbench.text_required')
  })
})
