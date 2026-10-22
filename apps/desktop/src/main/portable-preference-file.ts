import { constants } from 'node:fs'
import { lstat, open, unlink } from 'node:fs/promises'
import { resolve } from 'node:path'
import { isDeepStrictEqual } from 'node:util'

export type PortablePreferencePlan = {
  commandId: string; domain: string; targetId: 'appearance' | 'general' | 'windowClose' | 'logging'
  ownerPath: string; expectedDigest: string; planDigest: string; before: unknown; after: unknown
}
export type PreferenceApplyState = 'applied' | 'conflict'

export function samePreferenceValue(left: unknown, right: unknown): boolean { return isDeepStrictEqual(left, right) }

async function readOwnedFile(path: string): Promise<unknown> {
  let metadata: Awaited<ReturnType<typeof lstat>>
  try { metadata = await lstat(path) } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return null
    throw error
  }
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 1024 * 1024) throw new Error('configuration.file_invalid')
  const file = await open(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0))
  try {
    const observed = await file.stat()
    if (!observed.isFile() || observed.ino !== metadata.ino || observed.dev !== metadata.dev || observed.size !== metadata.size) throw new Error('configuration.file_changed')
    const bytes = Buffer.alloc(1024 * 1024 + 1)
    let size = 0
    while (size < bytes.length) {
      const result = await file.read(bytes, size, bytes.length - size, null)
      if (!result.bytesRead) break
      size += result.bytesRead
    }
    const after = await file.stat()
    if (size > 1024 * 1024 || size !== observed.size || after.size !== observed.size || after.mtimeMs !== observed.mtimeMs) throw new Error('configuration.file_changed')
    return JSON.parse(bytes.subarray(0, size).toString('utf8'))
  } finally { await file.close() }
}

/** 必须在原 Store 写队列内部调用；已落盘而未回执的计划只恢复内存。 */
export async function applyOwnedPreferenceFile(
  path: string, plan: PortablePreferencePlan, write: (value: unknown) => Promise<void>
): Promise<PreferenceApplyState> {
  const expected = resolve(path), actual = resolve(plan.ownerPath)
  if ((process.platform === 'win32' ? expected.toLowerCase() !== actual.toLowerCase() : expected !== actual)) throw new Error('configuration.owner_path_changed')
  const current = await readOwnedFile(path)
  if (samePreferenceValue(current, plan.after)) return 'applied'
  if (!samePreferenceValue(current, plan.before)) return 'conflict'
  if (!samePreferenceValue(await readOwnedFile(path), current)) return 'conflict'
  if (plan.after === null) await unlink(path).catch(error => { if (error.code !== 'ENOENT') throw error })
  else await write(plan.after)
  return 'applied'
}
