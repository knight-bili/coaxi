import { useCallback, useEffect, useRef, useState } from 'react'
import { CanvasFolderController } from './canvas-folder-controller'
import type { CanvasFolderResolution } from './canvas-reference-model'
import { useThreadClient } from './camp-client'

const EMPTY: ReadonlyMap<string, CanvasFolderResolution> = new Map()
export function useCanvasFolders(paths: readonly string[]) {
  const client = useThreadClient(), owner = useRef<{ client: typeof client; controller: CanvasFolderController } | null>(null)
  const [owned, setOwned] = useState(() => ({ client, state: EMPTY }))
  const key = JSON.stringify([...new Set(paths)].sort())
  useEffect(() => {
    const current = new CanvasFolderController(client)
    owner.current = { client, controller: current }
    const unsubscribe = current.subscribe(() => setOwned({ client, state: current.getSnapshot() }))
    setOwned({ client, state: current.getSnapshot() })
    const refresh = () => { if (document.visibilityState === 'visible') void current.refresh().catch(() => undefined) }
    const poll = setInterval(refresh, 20_000)
    window.addEventListener('focus', refresh); document.addEventListener('visibilitychange', refresh)
    return () => {
      unsubscribe(); current.dispose(); if (owner.current?.controller === current) owner.current = null
      clearInterval(poll); window.removeEventListener('focus', refresh); document.removeEventListener('visibilitychange', refresh)
    }
  }, [client])
  useEffect(() => { if (owner.current?.client === client) owner.current.controller.setPaths(JSON.parse(key)) }, [client, key])
  const refresh = useCallback(() => owner.current?.client === client ? owner.current.controller.refresh() : Promise.resolve(), [client])
  return { resolutions: owned.client === client ? owned.state : EMPTY, refresh }
}
