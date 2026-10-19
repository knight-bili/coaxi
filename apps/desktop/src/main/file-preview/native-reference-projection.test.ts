import { mkdir, mkdtemp, realpath, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { isAbsolute, join, resolve } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { OpenFilePreviewRequest } from '@contracts'
import { FilePreviewService, type FilePreviewNativeActions, type FilePreviewSourceAuthority } from './file-preview-service'

const roots: string[] = [], services: FilePreviewService[] = []
afterEach(async () => {
  await Promise.all(services.splice(0).map(service => service.closeAll()))
  for (const root of roots.splice(0)) {
    if (!isAbsolute(root) || resolve(root) !== root || !root.startsWith(join(tmpdir(), 'rovai-native-reference-'))) throw new Error('unexpected test cleanup path')
    await rm(root, { recursive: true, force: true })
  }
})
async function fixture() {
  const root = await mkdtemp(join(tmpdir(), 'rovai-native-reference-')); roots.push(root)
  await mkdir(join(root, 'src')); await writeFile(join(root, 'src/a.ts'), 'first\nsecond\n')
  let active = true
  const authority: FilePreviewSourceAuthority = { async resolve(request) {
    if (!active || request.kind !== 'native_child_file' || request.readId !== 'reader') return null
    return { kind: 'file_target', threadId: request.threadId, sourceKind: 'native_child_file', sourceIdentity: 'native-child:reader', rootPath: root, basePath: root, rawReference: request.rawReference, allowChildren: true }
  } }
  const native = { selectRoot: vi.fn(async () => null), confirmOpen: vi.fn(async () => true), openPath: vi.fn(async () => ''), revealPath: vi.fn(), copyText: vi.fn(), publishExternalUpdate: vi.fn() } satisfies FilePreviewNativeActions
  const service = new FilePreviewService(authority, native); services.push(service); await service.bindThread(1, 'thread')
  const request = (rawReference: string): OpenFilePreviewRequest => ({ kind: 'native_child_file', threadId: 'thread', readId: 'reader', rawReference })
  return { root, service, native, request, revoke: () => { active = false } }
}
describe('native reader root relative presentation', () => {
  it('projects the verified native root independently of Camp project display and line suffixes', async () => {
    const { root, service, native, request } = await fixture(), opened = await service.open(1, request('./src/a.ts:2'))
    if (!opened.ok || opened.value.kind !== 'file_preview') throw new Error('expected preview')
    const file = opened.value.file
    expect(file.sourceRelativePath).toBe('src/a.ts'); expect(file.pathPresentation).not.toBe('project_relative')
    expect(file.absolutePath).toBe(await realpath(join(root, 'src/a.ts')))
    expect(file.restoreRequest).toBeUndefined(); expect(file.target?.line).toBe(2)
    expect(await service.copyPath(1, { handleId: file.handleId, format: 'absolute' })).toMatchObject({ ok: true })
    expect(native.copyText).toHaveBeenCalledWith(await realpath(join(root, 'src/a.ts')))
    await service.release(1, { handleId: file.handleId }); expect(service.handleCount).toBe(0)
  })
  it('uses canonical contained paths for symlink aliases and preserves the source root for child files', async () => {
    const { root, service, request } = await fixture()
    await symlink(join(root, 'src'), join(root, 'alias'), 'junction')
    const opened = await service.open(1, request('alias/a.ts'))
    if (!opened.ok || opened.value.kind !== 'file_preview') throw new Error('expected preview')
    expect(opened.value.file.sourceRelativePath).toBe('src/a.ts')
    const child = await service.open(1, { kind: 'child_of_handle', parentHandleId: opened.value.file.handleId, rawReference: './a.ts' })
    if (!child.ok || child.value.kind !== 'file_preview') throw new Error('expected child preview')
    expect(child.value.file.sourceRelativePath).toBe('src/a.ts'); expect(child.value.file.restoreRequest).toBeUndefined()
  })
  it('does not issue metadata or copy paths after the exact reader is revoked', async () => {
    const { service, request, native, revoke } = await fixture(), opened = await service.open(1, request('src/a.ts'))
    if (!opened.ok || opened.value.kind !== 'file_preview') throw new Error('expected preview')
    revoke()
    expect(await service.open(1, request('src/a.ts'))).toMatchObject({ ok: false })
    expect(await service.copyPath(1, { handleId: opened.value.file.handleId, format: 'absolute' })).toMatchObject({ ok: false })
    expect(native.copyText).not.toHaveBeenCalled()
  })
})
