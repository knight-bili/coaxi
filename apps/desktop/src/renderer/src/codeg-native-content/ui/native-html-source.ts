import type { FilePreviewApi, ResolvedFilePreview } from '@contracts'
import type { NativeFileSource } from '../../native-transcript-files'

export type NativeHtmlOwner = { source: NativeFileSource; api: FilePreviewApi; current(): boolean }
export type NativeHtmlDocument = { base64: string; path: string; readResource(root: string, relative: string): Promise<string> }
function failure(code: string): never { throw new Error(code) }
const base64 = (bytes: Uint8Array): string => {
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += 32_768) binary += String.fromCharCode(...bytes.subarray(offset, offset + 32_768))
  return btoa(binary)
}
function canonicalDirectory(path: string): string { return path.replaceAll('\\', '/').replace(/\/[^/]*$/, '').replace(/\/+$/, '') }
function inDirectory(directory: string, path: string): boolean {
  let root = directory, candidate = path.replaceAll('\\', '/')
  if (/^[A-Za-z]:\//.test(root)) { root = root.toLowerCase(); candidate = candidate.toLowerCase() }
  return candidate.startsWith(root + '/')
}
async function withFile<T>(owner: NativeHtmlOwner, request: Parameters<FilePreviewApi['open']>[0], use: (file: ResolvedFilePreview) => Promise<T>): Promise<T> {
  if (!owner.current()) failure('native_child.scope_changed')
  const result = await owner.api.open(request)
  if (!result.ok) failure(result.error.code)
  if (result.value.kind !== 'file_preview') failure('native_child.file_unavailable')
  const file = result.value.file
  try { if (!owner.current()) failure('native_child.scope_changed'); return await use(file) }
  finally { await owner.api.release({ handleId: file.handleId }) }
}
async function read(owner: NativeHtmlOwner, file: ResolvedFilePreview, maximum: number): Promise<string> {
  if (!Number.isSafeInteger(file.size) || file.size < 0 || file.size > maximum) failure('file_too_large')
  if (!owner.current()) failure('native_child.scope_changed')
  const readBytes = owner.api.readFileBytes ?? owner.api.readBinary
  const result = await readBytes.call(owner.api, { handleId: file.handleId, expectedGeneration: file.contentGeneration })
  if (!result.ok) failure(result.error.code)
  if (!owner.current()) failure('native_child.scope_changed')
  if (result.value.contentGeneration !== file.contentGeneration || result.value.bytes.byteLength !== file.size) failure('read_failed')
  return base64(result.value.bytes)
}
/** Core issues every path and grant. Source code's inliner is an additional filter, never authority. */
export function withNativeHtmlDocument<T>(owner: NativeHtmlOwner, rawReference: string, maximum: number, use: (document: NativeHtmlDocument) => Promise<T>): Promise<T> {
  return withFile(owner, { kind: 'native_child_file', ...owner.source, rawReference }, async file => {
    const source = await read(owner, file, maximum), absolutePath = file.pathPresentation === 'file_name_only' ? undefined : file.absolutePath
    const directory = absolutePath ? canonicalDirectory(absolutePath) : null
    const document: NativeHtmlDocument = { base64: source, path: absolutePath ?? '/__rovai_native_html__/document.html',
      async readResource(_root, relative) {
        if (!directory) failure('source_not_authorized')
        return withFile(owner, { kind: 'child_of_handle', parentHandleId: file.handleId, rawReference: relative, allowSystemOpen: false }, async child => {
          if (!child.absolutePath || !inDirectory(directory, child.absolutePath)) failure('outside_authorized_root')
          return read(owner, child, 32 * 1024 * 1024)
        })
      }
    }
    const result = await use(document)
    if (!owner.current()) failure('native_child.scope_changed')
    return result
  })
}
