// @vitest-environment jsdom
import { createElement } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import type { ThreadClient } from './camp-client'
import { ThreadClientProvider } from './camp-client'
import { WorkspaceWorkbench } from './WorkspaceWorkbench'

const previewActions = vi.hoisted(()=>({open:vi.fn(async()=>({kind:'preview',tabId:'image'})),openWorkspace:vi.fn(),updateWorkspaceTab:vi.fn(),bindWorkspaceClose:vi.fn(()=>()=>{}),close:vi.fn()}))
vi.mock('./FilePreviewContext', () => ({
  useOptionalFilePreview: () => ({ resolvedTheme: 'night',...previewActions }),
  useOptionalFilePreviewApi: () => ({}),
  PinnedFilePreview: ({ source }: { source: { rawReference: string } }) => createElement('img', { alt: source.rawReference })
}))
vi.mock('react-resizable-panels', () => ({
  PanelGroup: ({ children }: { children: React.ReactNode }) => createElement('div', null, children),
  Panel: ({ children }: { children: React.ReactNode }) => createElement('div', null, children),
  PanelResizeHandle: () => null
}))
vi.mock('./WorkspaceCodeEditor', () => ({ WorkspaceCodeEditor: () => null }))
vi.mock('./SplitTabDeck', () => ({ SplitTabDeck: () => createElement('div', null, '文本编辑器') }))
vi.mock('./WorkspaceTerminals', () => ({ WorkspaceTerminals: () => null }))
vi.mock('./ProjectLauncher', () => ({ ProjectLauncher: () => null }))
vi.mock('./WorkspaceGitViews', () => ({ WorkspaceGitViews: () => null }))
vi.mock('./WorkspaceMergeEditor', () => ({ WorkspaceMergeEditor: () => null }))
vi.mock('./WorkspaceOrganizer', () => ({ WorkspaceOrganizer: () => null }))
vi.mock('./ForgeWorkbench', () => ({ ForgeWorkbench: () => null }))
vi.mock('./BrowserWorkbench', () => ({ BrowserWorkbench: () => null }))
vi.mock('./ComputerWorkbench', () => ({ ComputerWorkbench: () => null }))
vi.mock('./OfficePreviewWorkbench', () => ({ OfficePreviewWorkbench: () => null }))
vi.mock('./NativeHistoryWorkbench', () => ({ NativeHistoryWorkbench: () => null }))
vi.mock('./codeg-native-content/components/diff/unified-diff-preview', () => ({ UnifiedDiffPreview: () => null }))
vi.mock('./useShortcutActions', () => ({ useShortcutActions: () => undefined }))
afterEach(cleanup)

function fixture(readError = '文件无法读取，请检查访问权限。') {
  const records = new Map<string, unknown>()
  let finishSave: (() => void) | undefined
  const request = vi.fn(async (method: string, input?: unknown) => {
    if (method === 'app.info') return { dataDir: '/fixture-core' }
    if (method === 'workbench.files.list') return { entries: ['image.png', 'broken.txt'].map(path => ({ name: path, path, directory: false, link: false, bytes: 10 })), next: null }
    if (method === 'workbench.files.read') throw { message: readError }
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
    expect(previewActions.open).toHaveBeenCalledWith({kind:'camp_workspace',threadId:'thread',rawReference:'image.png'})
    expect(request.mock.calls.some(([method]) => method === 'workbench.files.read')).toBe(false)
    finish()
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
