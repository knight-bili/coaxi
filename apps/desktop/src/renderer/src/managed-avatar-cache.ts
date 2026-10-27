import type { MemberAvatarRendition, MemberAvatarsApi } from '@contracts'

export type ManagedAvatarRenditionKind = 'icon' | 'portrait'
export type ManagedAvatarRead = MemberAvatarsApi['read']

type CacheEntry = {
  token: object
  promise: Promise<string | null>
}

const managedAvatarCaches = new Map<ManagedAvatarRead, Map<string, CacheEntry>>()
const desktopRead: ManagedAvatarRead = (avatarRef, rendition) => window.coaxi.memberAvatars.read(avatarRef, rendition)
function cacheFor(read: ManagedAvatarRead): Map<string, CacheEntry> {
  let cache = managedAvatarCaches.get(read)
  if (!cache) { cache = new Map(); managedAvatarCaches.set(read, cache) }
  return cache
}

function cacheKey(avatarRef: string, rendition: ManagedAvatarRenditionKind): string {
  return `${avatarRef}\u0000${rendition}`
}

function renditionObjectUrl(rendition: MemberAvatarRendition): string {
  const bytes = Uint8Array.from(rendition.bytes)
  return URL.createObjectURL(new Blob([bytes], { type: rendition.mediaType }))
}

export function managedAvatarObjectUrl(
  avatarRef: string,
  rendition: ManagedAvatarRenditionKind,
  read: ManagedAvatarRead = desktopRead
): Promise<string | null> {
  const managedAvatarCache = cacheFor(read)
  const key = cacheKey(avatarRef, rendition)
  const cached = managedAvatarCache.get(key)
  if (cached) return cached.promise

  const token = {}
  const promise = read(avatarRef, rendition)
    .then((result) => {
      if (!result) return null
      const objectUrl = renditionObjectUrl(result)
      if (managedAvatarCache.get(key)?.token !== token) {
        URL.revokeObjectURL(objectUrl)
        return null
      }
      return objectUrl
    })
    .catch(() => null)
  managedAvatarCache.set(key, { token, promise })
  return promise
}

export async function invalidateManagedAvatarObjectUrl(
  avatarRef: string,
  rendition?: ManagedAvatarRenditionKind,
  read: ManagedAvatarRead = desktopRead
): Promise<void> {
  const managedAvatarCache = cacheFor(read)
  const keys = rendition
    ? [cacheKey(avatarRef, rendition)]
    : [
        cacheKey(avatarRef, 'icon'),
        cacheKey(avatarRef, 'portrait')
      ]
  const entries = keys.flatMap((key) => {
    const entry = managedAvatarCache.get(key)
    managedAvatarCache.delete(key)
    return entry ? [entry] : []
  })
  const objectUrls = await Promise.all(entries.map((entry) => entry.promise))
  for (const objectUrl of objectUrls) {
    if (objectUrl) URL.revokeObjectURL(objectUrl)
  }
}

export async function clearManagedAvatarObjectUrlCache(): Promise<void> {
  const entries = [...managedAvatarCaches.values()].flatMap((cache) => [...cache.values()])
  for (const cache of managedAvatarCaches.values()) cache.clear()
  managedAvatarCaches.clear()
  const objectUrls = await Promise.all(entries.map((entry) => entry.promise))
  for (const objectUrl of objectUrls) {
    if (objectUrl) URL.revokeObjectURL(objectUrl)
  }
}
