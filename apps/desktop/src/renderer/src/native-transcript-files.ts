import type { FilePreviewApi, FilePreviewBinaryContent } from '@contracts'
export type NativeFileSource = { threadId: string; readId: string }

/** The Core resolves a raw reference against this exact signed transcript. */
export async function readNativeTranscriptImage(api: FilePreviewApi, source: NativeFileSource, rawReference: string): Promise<FilePreviewBinaryContent> {
  const opened = await api.open({ kind: 'native_child_file', ...source, rawReference })
  if (!opened.ok || opened.value.kind !== 'file_preview') throw new Error('native_child.file_unavailable')
  const file = opened.value.file
  try {
    if (file.kind !== 'image' && file.kind !== 'svg') throw new Error('native_child.image_required')
    const result = await api.readBinary({ handleId: file.handleId, expectedGeneration: file.contentGeneration })
    if (!result.ok) throw new Error('native_child.image_unavailable')
    if (result.value.contentGeneration !== file.contentGeneration) throw new Error('native_child.image_changed')
    if (!/^image\/(?:png|jpeg|gif|webp|avif|bmp|svg\+xml)$/i.test(result.value.mime)) throw new Error('native_child.image_required')
    return result.value
  } finally { await api.release({ handleId: file.handleId }) }
}
