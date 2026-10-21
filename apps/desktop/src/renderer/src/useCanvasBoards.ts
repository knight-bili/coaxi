import {useEffect,useRef,useState} from 'react'
import {useThreadClient} from './camp-client'
import {CanvasBoardsController,initialCanvasBoardsState} from './canvas-boards-controller'
export function useCanvasBoards() {
  const client = useThreadClient(), controller = useRef<CanvasBoardsController | null>(null)
  const [owned, setOwned] = useState(() => ({ client, state: initialCanvasBoardsState() }))
  useEffect(() => {
    const current = new CanvasBoardsController(client)
    controller.current = current
    const unsubscribe = current.subscribe(() => setOwned({ client, state: current.getSnapshot() }))
    setOwned({ client, state: current.getSnapshot() }); current.start()
    const foreground = () => current.setVisible(document.visibilityState === 'visible')
    const poll = setInterval(() => { if (document.visibilityState === 'visible') void current.refresh().catch(() => undefined) }, 20_000)
    window.addEventListener('focus', foreground); document.addEventListener('visibilitychange', foreground)
    return () => {
      unsubscribe(); current.dispose(); if (controller.current === current) controller.current = null
      clearInterval(poll); window.removeEventListener('focus', foreground); document.removeEventListener('visibilitychange', foreground)
    }
  }, [client])
  return { controller, state: owned.client === client ? owned.state : initialCanvasBoardsState() }
}

