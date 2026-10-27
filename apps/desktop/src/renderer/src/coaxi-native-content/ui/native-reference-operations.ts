import type { FilePreviewApi, ResolvedFilePreview, NativeFileReferenceInfo } from '@contracts'
import type { NativeFileSource } from '../../native-transcript-files'

export type NativeFileAction = 'reveal' | 'relative' | 'absolute' | 'download'
type Owner = { source: NativeFileSource; api: FilePreviewApi; current(): boolean }
export type NativeFileActionMetadata = Pick<ResolvedFilePreview, 'fileName' | 'pathPresentation' | 'sourceRelativePath' | 'absolutePath' | 'capabilities'> & { kind?: 'file' | 'directory'; previewable?: boolean }

async function withReference<T>(owner: Owner, rawReference: string, use: (reference: NativeFileReferenceInfo) => Promise<T> | T): Promise<T> {
  if (!owner.current()) throw new Error('native_child.scope_changed')
  const result = await owner.api.nativeReference!({ source: { kind: 'native_child_file', ...owner.source, rawReference }, action: 'inspect' })
  if (!result.ok) throw new Error(result.error.code)
  if (result.value.kind !== 'reference') throw new Error('native_child.file_unavailable')
  const reference = result.value.reference
  try {
    if (!owner.current()) throw new Error('native_child.scope_changed')
    return await use(reference)
  } finally { if (reference.download) await owner.api.release({ handleId: reference.download.handleId }) }
}

export async function activateNativeFileReference(owner: Owner, rawReference: string, preview: () => Promise<void>): Promise<void> {
  if (!owner.current()) throw new Error('native_child.scope_changed')
  if (!owner.api.nativeReference) return preview()
  const target = await withReference(owner, rawReference, reference => ({ previewable: reference.previewable, downloadable: Boolean(reference.download) }))
  if (!owner.current()) throw new Error('native_child.scope_changed')
  if (target.previewable) return preview()
  if (target.downloadable) return performNativeFileAction(owner, rawReference, 'download', async () => false)
  const result = await owner.api.nativeReference({ source: { kind: 'native_child_file', ...owner.source, rawReference }, action: 'open' })
  if (!result.ok) throw new Error(result.error.code)
}

/** Each menu/action obtains its own current grant. No handle survives the call. */
async function withFile<T>(owner: Owner, rawReference: string, use: (file: ResolvedFilePreview) => Promise<T> | T): Promise<T> {
  if (!owner.current()) throw new Error('native_child.scope_changed')
  const opened = await owner.api.open({ kind: 'native_child_file', ...owner.source, rawReference })
  if (!opened.ok) throw new Error(opened.error.code)
  if (opened.value.kind !== 'file_preview') throw new Error('native_child.file_unavailable')
  const file = opened.value.file
  try {
    if (!owner.current()) throw new Error('native_child.scope_changed')
    return await use(file)
  } finally { await owner.api.release({ handleId: file.handleId }) }
}
export function inspectNativeFileActions(owner: Owner, rawReference: string): Promise<NativeFileActionMetadata> {
  if (owner.api.nativeReference) return withReference(owner, rawReference, reference => ({
    fileName: reference.fileName, kind: reference.kind, previewable: reference.previewable, pathPresentation: reference.pathPresentation,
    sourceRelativePath: reference.sourceRelativePath, absolutePath: reference.absolutePath, capabilities: reference.download ? ['download'] : []
  }))
  return withFile(owner, rawReference, file => ({ fileName: file.fileName, pathPresentation: file.pathPresentation,
    sourceRelativePath: file.sourceRelativePath, absolutePath: file.absolutePath, capabilities: [...file.capabilities] }))
}
export function performNativeFileAction(owner: Owner, rawReference: string, action: NativeFileAction, copy: (text: string) => Promise<boolean>): Promise<void> {
  if (owner.api.nativeReference) {
    if (action === 'reveal') return (async () => {
      if (!owner.current()) throw new Error('native_child.scope_changed')
      const result = await owner.api.nativeReference!({ source: { kind: 'native_child_file', ...owner.source, rawReference }, action: 'reveal' })
      if (!result.ok) throw new Error(result.error.code)
    })()
    return withReference(owner, rawReference, async reference => {
      if (action === 'download') {
        if (!reference.download || !owner.api.download) throw new Error('native_child.download_unavailable')
        const result = await owner.api.download({ handleId: reference.download.handleId, expectedGeneration: reference.download.contentGeneration })
        if (!result.ok) throw new Error(result.error.code)
      } else {
        const value = action === 'relative' ? reference.sourceRelativePath : reference.absolutePath
        if (reference.pathPresentation === 'file_name_only' || !value || !owner.current() || !await copy(value)) throw new Error('native_child.copy_unavailable')
      }
    })
  }
  return withFile(owner, rawReference, async file => {
    if (action === 'relative') {
      if (file.pathPresentation === 'file_name_only' || !file.sourceRelativePath || !owner.current() || !await copy(file.sourceRelativePath)) throw new Error('native_child.copy_unavailable')
      return
    }
    if (action === 'absolute' && file.pathPresentation === 'file_name_only') throw new Error('native_child.copy_unavailable')
    if (action === 'download' && (!file.capabilities.includes('download') || !owner.api.download)) throw new Error('native_child.download_unavailable')
    const result = action === 'reveal' ? await owner.api.revealInFolder({ handleId: file.handleId })
      : action === 'absolute' ? await owner.api.copyPath({ handleId: file.handleId, format: 'absolute' })
        : await owner.api.download!({ handleId: file.handleId, expectedGeneration: file.contentGeneration })
    if (!result.ok) throw new Error(result.error.code)
  })
}
