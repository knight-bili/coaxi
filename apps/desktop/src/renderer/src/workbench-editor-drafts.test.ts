import { afterEach, describe, expect, it, vi } from 'vitest'
import type { WorkbenchEditorState } from '@contracts'
import type { ThreadClient } from './camp-client'
import { WorkbenchEditorDrafts } from './workbench-editor-drafts'
import type { EditorRecoveryRecord } from './workbench-editor-recovery'

const root = 'D:/fixture/project'
const key = JSON.stringify(['fixture', 'fixture-core', 'thread', root])
const empty: WorkbenchEditorState = { documents: [], selected: null, layout: null }
const drafts: WorkbenchEditorDrafts[] = []
afterEach(() => { drafts.splice(0).forEach(draft => draft.dispose()) })

function fixture(record?: EditorRecoveryRecord, host: WorkbenchEditorState | null = null) {
  const records = new Map<string, EditorRecoveryRecord>()
  if (record) records.set(key, structuredClone(record))
  const request = vi.fn(async (method: string, input?: unknown) => {
    if (method === 'app.info') return { dataDir: 'fixture-core' }
    if (method !== 'workbench.editor') throw new Error(method)
    const params = input as { operation: string; value?: WorkbenchEditorState }
    if (params.operation === 'get') return { workspaceRoot: root, revision: null, value: host }
    if (params.value?.selected && !params.value.documents.some(document => document.path === params.value!.selected)) {
      throw { message: 'workbench.editor_selection_missing' }
    }
    return { workspaceRoot: root, revision: 'saved', value: params.value }
  })
  const client = { request, presentationScope: 'fixture', workbenchEditorRecovery: {
    get: async (key: string) => structuredClone(records.get(key) ?? null),
    put: async (value: EditorRecoveryRecord) => { records.set(value.key, structuredClone(value)) }
  } } as unknown as ThreadClient
  const draft = new WorkbenchEditorDrafts(client, 'thread', root)
  drafts.push(draft)
  return { draft, records, request }
}

describe('工程编辑草稿', () => {
  it('修复失败图片读取留下的无效选择，保留完整原副本和原请求版本', async () => {
    const invalid = { ...empty, selected: 'drafts/UI-A-gravity.png', layout: { panes: [] } }
    const original: EditorRecoveryRecord = { schemaVersion: 1, key, editorId: crypto.randomUUID(), workspaceRoot: root, revision: null,
      latest: invalid, pending: { operation: 'save', scope: { threadId: 'thread' }, editorId: 'editor', workspaceRoot: root,
        expectedRevision: null, commandId: crypto.randomUUID(), value: invalid } }
    const { draft, records, request } = fixture(original)
    expect(await draft.start()).toEqual({ ...invalid, selected: null })
    const copy = [...records.values()].find(record => record.key.startsWith(`${key}:copy:`))!
    expect(copy.latest).toEqual(original.latest)
    expect(copy.pending).toEqual(original.pending)
    await draft.flush()
    const save = request.mock.calls.find(([, input]) => (input as { operation?: string })?.operation === 'save')![1] as typeof original.pending
    expect(save!.commandId).not.toBe(original.pending!.commandId)
    expect(save!.expectedRevision).toBeNull()
    expect(save!.value).toEqual({ ...invalid, selected: null })
    expect(draft.snapshot()).toMatchObject({ pending: false, saving: false, issue: null })
  })

  it('外部版本变化时仍保留冲突，不覆盖主机草稿', async () => {
    const original: EditorRecoveryRecord = { schemaVersion: 1, key, editorId: crypto.randomUUID(), workspaceRoot: root, revision: 'old',
      latest: { ...empty, selected: 'image.png' }, pending: null }
    const { draft, request } = fixture(original, { ...empty, layout: { host: true } })
    await draft.start()
    expect(draft.snapshot().issue).toContain('其他窗口')
    expect(request.mock.calls.some(([, input]) => (input as { operation?: string })?.operation === 'save')).toBe(false)
  })

  it('新的未打开选择不会再次污染草稿', async () => {
    const { draft } = fixture()
    await draft.start()
    draft.update({ ...empty, selected: 'missing.txt' })
    expect(draft.localDraft()?.selected).toBeNull()
    await draft.flush()
    expect(draft.snapshot().pending).toBe(false)
  })

  it('桥接错误保留可读原因，不显示对象字符串', async () => {
    const { draft, request } = fixture()
    await draft.start()
    draft.update(empty)
    request.mockImplementationOnce(async () => { throw { message: '工作区暂不可访问' } })
    await expect(draft.flush()).rejects.toEqual({ message: '工作区暂不可访问' })
    expect(draft.snapshot().issue).toBe('工作区暂不可访问')
    expect(draft.snapshot().pending).toBe(true)
  })
})
