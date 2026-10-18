import { expect, it, vi } from 'vitest'
import type { FilePreviewApi } from '@contracts'
import { readNativeTranscriptImage } from './native-transcript-files'
const source = { threadId: 'own-thread', readId: 'verified-read' }
function api(kind = 'image') {
  return { open: vi.fn(async () => ({ ok: true, value: { kind: 'file_preview', file: { kind, handleId: 'handle', contentGeneration: 'generation' } } })),
    readBinary: vi.fn(async () => ({ ok: true, value: { bytes: [1, 2, 3], mime: 'image/png', contentGeneration: 'generation' } })),
    release: vi.fn(async () => ({ released: true })) } as unknown as FilePreviewApi
}
it.each(['image', 'svg'])('reads %s only through the exact Core-owned transcript and generation, then releases its handle', async kind => {
  const files = api(kind)
  expect(await readNativeTranscriptImage(files, source, '../材料/图.png')).toEqual({ bytes: [1, 2, 3], mime: 'image/png', contentGeneration: 'generation' })
  expect(files.open).toHaveBeenCalledWith({ kind: 'native_child_file', ...source, rawReference: '../材料/图.png' })
  expect(files.readBinary).toHaveBeenCalledWith({ handleId: 'handle', expectedGeneration: 'generation' })
  expect(files.release).toHaveBeenCalledExactlyOnceWith({ handleId: 'handle' })
})
it('does not convert a text or HTML handle into an image and releases even on that refusal', async () => {
  const files = api('html')
  await expect(readNativeTranscriptImage(files, source, 'report.html')).rejects.toThrow('image_required')
  expect(files.readBinary).not.toHaveBeenCalled(); expect(files.release).toHaveBeenCalledTimes(1)
})
it('keeps Core source refusal authoritative instead of guessing a global directory', async () => {
  const files = api()
  vi.mocked(files.open).mockResolvedValue({ ok: false, error: { code: 'source_not_authorized', message: '拒绝', retryable: false } })
  await expect(readNativeTranscriptImage(files, source, 'https://example.test/x.png')).rejects.toThrow('file_unavailable')
  expect(files.readBinary).not.toHaveBeenCalled(); expect(files.release).not.toHaveBeenCalled()
})
it('releases the new handle after a rejected binary read', async () => {
  const files = api()
  vi.mocked(files.readBinary).mockRejectedValue(new Error('source_changed'))
  await expect(readNativeTranscriptImage(files, source, 'x.png')).rejects.toThrow('source_changed')
  expect(files.release).toHaveBeenCalledExactlyOnceWith({ handleId: 'handle' })
})
it.each(['generation', 'mime'] as const)('refuses contradictory binary %s even after a valid open and releases the handle', async field => {
  const files = api()
  vi.mocked(files.readBinary).mockResolvedValue({ ok: true, value: { bytes: Uint8Array.of(1), mime: field === 'mime' ? 'text/html' : 'image/png', contentGeneration: field === 'generation' ? 'other' : 'generation', contentVersion: { size: 1, mtimeMs: 1 } } })
  await expect(readNativeTranscriptImage(files, source, 'image.png')).rejects.toThrow(field === 'mime' ? 'image_required' : 'image_changed')
  expect(files.release).toHaveBeenCalledTimes(1)
})
