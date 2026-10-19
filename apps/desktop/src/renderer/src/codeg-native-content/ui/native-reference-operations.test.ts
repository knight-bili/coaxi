import { describe, expect, it, vi } from 'vitest'
import type { FilePreviewApi, ResolvedFilePreview } from '@contracts'
import { inspectNativeFileActions, performNativeFileAction } from './native-reference-operations'

const file = (extra: Partial<ResolvedFilePreview> = {}): ResolvedFilePreview => ({ handleId: 'handle', reopenToken: 'never-persist', previewKey: 'key', fileName: 'a.ts', displayPath: '/source/src/a.ts', pathPresentation: 'external', sourceRelativePath: 'src/a.ts', size: 1, mime: 'text/plain', extension: '.ts', kind: 'code', hasExternalUpdate: false, contentVersion: { size: 1, mtimeMs: 1 }, contentGeneration: 'generation-1', capabilities: ['read'], ...extra })
function fixture() {
  const api = { open: vi.fn(async () => ({ ok: true, value: { kind: 'file_preview', file: file() } })),
    release: vi.fn(async () => ({ released: true })), copyPath: vi.fn(async () => ({ ok: true, value: { copied: true } })),
    revealInFolder: vi.fn(async () => ({ ok: true, value: { revealed: true } })), download: vi.fn(async () => ({ ok: true, value: { started: true } })) }
  let active = true
  return { api, owner: { api: api as unknown as FilePreviewApi, source: { threadId: 'thread', readId: 'exact-reader' }, current: () => active }, close: () => { active = false } }
}
describe('Core-scoped native file menu operations', () => {
  it('inspects exact source and releases its temporary handle without retaining IDs in menu metadata', async () => {
    const { api, owner } = fixture(), metadata = await inspectNativeFileActions(owner, './src/a.ts:2')
    expect(api.open).toHaveBeenCalledWith({ kind: 'native_child_file', threadId: 'thread', readId: 'exact-reader', rawReference: './src/a.ts:2' })
    expect(metadata).toEqual({ fileName: 'a.ts', pathPresentation: 'external', sourceRelativePath: 'src/a.ts', capabilities: ['read'] })
    expect(metadata).not.toHaveProperty('handleId'); expect(api.release).toHaveBeenCalledWith({ handleId: 'handle' })
  })
  it('reopens for selection and copies the newly reauthorized source-relative path', async () => {
    const { api, owner } = fixture(), copy = vi.fn(async () => true)
    await inspectNativeFileActions(owner, './a.ts')
    api.open.mockResolvedValueOnce({ ok: true, value: { kind: 'file_preview', file: file({ handleId: 'fresh', sourceRelativePath: 'renamed/a.ts' }) } })
    await performNativeFileAction(owner, './a.ts', 'relative', copy)
    expect(api.open).toHaveBeenCalledTimes(2); expect(copy).toHaveBeenCalledWith('renamed/a.ts')
    expect(api.copyPath).not.toHaveBeenCalled(); expect(api.release).toHaveBeenLastCalledWith({ handleId: 'fresh' })
  })
  it('rejects a hidden or replaced owner before opening anything', async () => {
    const { api, owner, close } = fixture(); close()
    await expect(performNativeFileAction(owner, './a.ts', 'reveal', vi.fn())).rejects.toThrow('scope_changed')
    expect(api.open).not.toHaveBeenCalled()
  })
  it('drops a late grant after the owner changes and releases it without a native action', async () => {
    const { api, owner, close } = fixture()
    let finish!: (value: Awaited<ReturnType<typeof api.open>>) => void
    api.open.mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
    const pending = performNativeFileAction(owner, './a.ts', 'reveal', vi.fn()); close()
    finish({ ok: true, value: { kind: 'file_preview', file: file() } })
    await expect(pending).rejects.toThrow('scope_changed')
    expect(api.revealInFolder).not.toHaveBeenCalled(); expect(api.release).toHaveBeenCalledTimes(1)
  })
  it('uses the new handle for absolute copy and releases it on a native failure', async () => {
    const { api, owner } = fixture()
    api.copyPath.mockRejectedValueOnce(new Error('native failure'))
    await expect(performNativeFileAction(owner, './a.ts:2', 'absolute', vi.fn())).rejects.toThrow('native failure')
    expect(api.copyPath).toHaveBeenCalledWith({ handleId: 'handle', format: 'absolute' })
    expect(api.release).toHaveBeenCalledTimes(1)
  })
  it('does not guess relative paths when authoritative metadata is absent', async () => {
    const { api, owner } = fixture(), copy = vi.fn(async () => true)
    api.open.mockResolvedValueOnce({ ok: true, value: { kind: 'file_preview', file: file({ sourceRelativePath: undefined }) } })
    await expect(performNativeFileAction(owner, './a.ts', 'relative', copy)).rejects.toThrow('copy_unavailable')
    expect(copy).not.toHaveBeenCalled(); expect(api.release).toHaveBeenCalledTimes(1)
  })
  it('keeps name-only paths private even when a previous menu allowed absolute copy', async () => {
    const { api, owner } = fixture()
    api.open.mockResolvedValueOnce({ ok: true, value: { kind: 'file_preview', file: file({ pathPresentation: 'file_name_only' }) } })
    await expect(performNativeFileAction(owner, './a.ts', 'absolute', vi.fn())).rejects.toThrow('copy_unavailable')
    expect(api.copyPath).not.toHaveBeenCalled(); expect(api.release).toHaveBeenCalledTimes(1)
  })
  it('downloads through the host with the exact current generation', async () => {
    const { api, owner } = fixture()
    api.open.mockResolvedValueOnce({ ok: true, value: { kind: 'file_preview', file: file({ capabilities: ['read', 'download'] }) } })
    await performNativeFileAction(owner, './a.ts', 'download', vi.fn())
    expect(api.download).toHaveBeenCalledWith({ handleId: 'handle', expectedGeneration: 'generation-1' })
    expect(api.release).toHaveBeenCalledTimes(1)
  })
  it('does not copy a relative hint from a name-only source', async () => {
    const { api, owner } = fixture(), copy = vi.fn(async () => true)
    api.open.mockResolvedValueOnce({ ok: true, value: { kind: 'file_preview', file: file({ pathPresentation: 'file_name_only' }) } })
    await expect(performNativeFileAction(owner, './a.ts', 'relative', copy)).rejects.toThrow('copy_unavailable')
    expect(copy).not.toHaveBeenCalled(); expect(api.release).toHaveBeenCalledTimes(1)
  })
  it('never starts an unavailable download or leaks its temporary handle', async () => {
    const { api, owner } = fixture()
    await expect(performNativeFileAction(owner, './a.ts', 'download', vi.fn())).rejects.toThrow('download_unavailable')
    expect(api.download).not.toHaveBeenCalled(); expect(api.release).toHaveBeenCalledTimes(1)
  })
})
