import type { FilePreviewApi, ResolvedFilePreview } from '@contracts'
import type { NativeFileSource } from '../../native-transcript-files'

export type NativeFileAction = 'reveal' | 'relative' | 'absolute' | 'download'
type Owner = { source: NativeFileSource; api: FilePreviewApi; current(): boolean }
export type NativeFileActionMetadata = Pick<ResolvedFilePreview, 'fileName' | 'pathPresentation' | 'sourceRelativePath' | 'absolutePath' | 'capabilities'>

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
  return withFile(owner, rawReference, file => ({ fileName: file.fileName, pathPresentation: file.pathPresentation,
    sourceRelativePath: file.sourceRelativePath, absolutePath: file.absolutePath, capabilities: [...file.capabilities] }))
}
export function performNativeFileAction(owner: Owner, rawReference: string, action: NativeFileAction, copy: (text: string) => Promise<boolean>): Promise<void> {
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
