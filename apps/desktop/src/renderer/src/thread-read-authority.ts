import { useSyncExternalStore } from 'react'

/** Local cache lifetime only. Every resource is still authorized by Core. */
export interface ThreadReadSnapshot { generation: number; available: boolean }
export interface ThreadReadAuthority {
  getSnapshot(): ThreadReadSnapshot
  subscribe(listener: () => void): () => void
}
const nativeSnapshot: ThreadReadSnapshot = Object.freeze({ generation: 0, available: true })
const nativeRead = () => nativeSnapshot
const nativeSubscribe = () => () => undefined
const clientKeys = new WeakMap<object, number>()
let nextClientKey = 0

export function useThreadReadAuthority(client: { readAuthority?: ThreadReadAuthority }): ThreadReadSnapshot & { key: string } {
  const authority = client.readAuthority
  const snapshot = useSyncExternalStore(authority?.subscribe ?? nativeSubscribe, authority?.getSnapshot ?? nativeRead, authority?.getSnapshot ?? nativeRead)
  let key = clientKeys.get(client)
  if (key === undefined) { key = ++nextClientKey; clientKeys.set(client, key) }
  return { ...snapshot, key: `${key}:${snapshot.generation}:${snapshot.available}` }
}
