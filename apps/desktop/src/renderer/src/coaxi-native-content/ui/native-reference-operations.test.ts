import { describe, expect, it, vi } from 'vitest'
import type { FilePreviewApi, ResolvedFilePreview, NativeFileReferenceInfo } from '@contracts'
import { activateNativeFileReference, inspectNativeFileActions, performNativeFileAction } from './native-reference-operations'

const file = (extra: Partial<ResolvedFilePreview> = {}): ResolvedFilePreview => ({ handleId: 'handle', reopenToken: 'never-persist', previewKey: 'key', fileName: 'a.ts', displayPath: '/source/src/a.ts', pathPresentation: 'external', sourceRelativePath: 'src/a.ts', size: 1, mime: 'text/plain', extension: '.ts', kind: 'code', hasExternalUpdate: false, contentVersion: { size: 1, mtimeMs: 1 }, contentGeneration: 'generation-1', capabilities: ['read'], ...extra })
function fixture() {
  const api = { open: vi.fn(async () => ({ ok: true, value: { kind: 'file_preview', file: file() } })),
    release: vi.fn(async () => ({ released: true })), copyPath: vi.fn(async () => ({ ok: true, value: { copied: true } })),
    revealInFolder: vi.fn(async () => ({ ok: true, value: { revealed: true } })), download: vi.fn(async () => ({ ok: true, value: { started: true } })) }
  let active = true
  return { api, owner: { api: api as unknown as FilePreviewApi, source: { threadId: 'thread', readId: 'exact-reader' }, current: () => active }, close: () => { active = false } }
}
describe('Core-scoped native file menu operations', () => {
  function currentReference() {
    const state = fixture(), reference: NativeFileReferenceInfo = { fileName: 'src', kind: 'directory', previewable: false, pathPresentation: 'external', sourceRelativePath: 'src', absolutePath: '/captured/src' }
    const nativeReference = vi.fn(async (input: { action: string }) => ({ ok: true as const, value: input.action === 'inspect'
      ? { kind: 'reference' as const, reference } : { kind: 'performed' as const } }))
    state.owner.api.nativeReference = nativeReference
    return { ...state, nativeReference, reference }
  }
  it('inspects a directory without preview opens and copies only freshly issued paths', async () => {
    const { api, owner, nativeReference, reference } = currentReference(), copy = vi.fn(async () => true)
    expect(await inspectNativeFileActions(owner, './src')).toMatchObject({ kind: 'directory', capabilities: [], sourceRelativePath: 'src' })
    reference.absolutePath = '/captured/renamed-src'
    await performNativeFileAction(owner, './src', 'absolute', copy)
    expect(copy).toHaveBeenCalledWith('/captured/renamed-src'); expect(nativeReference).toHaveBeenCalledTimes(2)
    expect(api.open).not.toHaveBeenCalled(); expect(api.release).not.toHaveBeenCalled()
  })
  it('keeps ordinary file clicks on the preview and uses explicit native opening for directories/system files', async () => {
    const { owner, reference, nativeReference } = currentReference(), preview = vi.fn(async () => {})
    await activateNativeFileReference(owner, './src', preview)
    expect(nativeReference).toHaveBeenLastCalledWith({ source: { kind: 'native_child_file', ...owner.source, rawReference: './src' }, action: 'open' })
    expect(preview).not.toHaveBeenCalled()
    reference.previewable = true
    await activateNativeFileReference(owner, './a.ts', preview)
    expect(preview).toHaveBeenCalledTimes(1)
  })
  it('releases a transient server download grant during inspection and uses a new one for the selection', async () => {
    const { owner, api, nativeReference, reference } = currentReference()
    Object.assign(reference, { kind: 'file', download: { handleId: 'metadata-only', contentGeneration: 'old' } })
    const metadata = await inspectNativeFileActions(owner, './archive.bin')
    expect(metadata).not.toHaveProperty('download'); expect(api.release).toHaveBeenCalledWith({ handleId: 'metadata-only' })
    Object.assign(reference, { download: { handleId: 'fresh-download', contentGeneration: 'current' } })
    await performNativeFileAction(owner, './archive.bin', 'download', vi.fn())
    expect(api.download).toHaveBeenCalledWith({ handleId: 'fresh-download', expectedGeneration: 'current' })
    expect(api.release).toHaveBeenLastCalledWith({ handleId: 'fresh-download' }); expect(nativeReference).toHaveBeenCalledTimes(2)
  })
  it('downloads non-previewable remote files on explicit left click instead of asking the server to run a program', async () => {
    const { owner, reference, api, nativeReference } = currentReference(), preview = vi.fn(async () => {})
    Object.assign(reference, { kind: 'file', download: { handleId: 'binary', contentGeneration: 'current' } })
    await activateNativeFileReference(owner, './archive.bin', preview)
    expect(preview).not.toHaveBeenCalled(); expect(api.download).toHaveBeenCalledWith({ handleId: 'binary', expectedGeneration: 'current' })
    expect(nativeReference.mock.calls.every(([input]) => input.action === 'inspect')).toBe(true)
    expect(api.release).toHaveBeenCalledTimes(2)
  })
  it('rejects directory downloads and opaque paths without retaining any metadata grant', async () => {
    const { owner, reference, api } = currentReference(), copy = vi.fn(async () => true)
    await expect(performNativeFileAction(owner, './src', 'download', copy)).rejects.toThrow('download_unavailable')
    reference.pathPresentation = 'file_name_only'
    await expect(performNativeFileAction(owner, './src', 'absolute', copy)).rejects.toThrow('copy_unavailable')
    expect(copy).not.toHaveBeenCalled(); expect(api.download).not.toHaveBeenCalled()
  })
  it('drops a late reference grant after an owner change and releases its server handle', async () => {
    const { owner, close, api } = currentReference()
    let finish!: (value: Awaited<ReturnType<NonNullable<FilePreviewApi['nativeReference']>>>) => void
    owner.api.nativeReference = vi.fn(() => new Promise<Awaited<ReturnType<NonNullable<FilePreviewApi['nativeReference']>>>>(resolve => { finish = resolve }))
    const pending = activateNativeFileReference(owner, './bin', vi.fn()); close()
    finish({ ok: true, value: { kind: 'reference', reference: { fileName: 'bin', kind: 'file', previewable: false, pathPresentation: 'external', download: { handleId: 'late', contentGeneration: 'g' } } } })
    await expect(pending).rejects.toThrow('scope_changed'); expect(api.release).toHaveBeenCalledWith({ handleId: 'late' })
  })
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
