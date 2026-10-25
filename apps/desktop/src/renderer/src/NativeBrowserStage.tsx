import { useEffect, useRef, useState } from 'react'
import type { BrowserWorkspaceApi } from '@contracts'
import { readErrorMessage } from './error-message'
import { useUiText } from './interface-language'

let presentationSequence = 0
/** A native view is hidden before DOM menus/dialogs cover its rectangle. */
export function NativeBrowserStage({ api, id }: { api: BrowserWorkspaceApi; id: string }): React.JSX.Element {
  const root = useRef<HTMLDivElement>(null), t = useUiText()
  const [error, setError] = useState('')
  useEffect(() => {
    if (!api.present || !root.current) return
    let live = true, frame: number | null = null, last = ''
    const present = (): void => {
      frame = null
      if (!root.current || !live) return
      const rect = root.current.getBoundingClientRect()
      const occluded = [...document.querySelectorAll<HTMLElement>('[role="dialog"],[role="menu"],[data-radix-popper-content-wrapper],[data-native-browser-occluder]')].some(element => element.getBoundingClientRect().width > 0 && getComputedStyle(element).visibility !== 'hidden')
      const visible = !occluded && !root.current.closest('[hidden],[inert]') && document.visibilityState !== 'hidden' && rect.width > 1 && rect.height > 1
      const bounds = visible ? { x: rect.x, y: rect.y, width: rect.width, height: rect.height } : null
      const key = JSON.stringify(bounds)
      if (key === last) return
      last = key
      void api.present!({ id, sequence: ++presentationSequence, bounds }).catch(failure => { if (live) setError(readErrorMessage(failure)) })
    }
    const schedule = (): void => { if (frame === null) frame = requestAnimationFrame(present) }
    const resize = new ResizeObserver(schedule); resize.observe(root.current)
    const mutations = new MutationObserver(schedule)
    mutations.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['hidden', 'inert', 'data-state', 'aria-hidden', 'class', 'style'] })
    window.addEventListener('resize', schedule); window.addEventListener('scroll', schedule, true); document.addEventListener('visibilitychange', schedule)
    present()
    return () => {
      live = false; if (frame !== null) cancelAnimationFrame(frame)
      resize.disconnect(); mutations.disconnect(); window.removeEventListener('resize', schedule); window.removeEventListener('scroll', schedule, true); document.removeEventListener('visibilitychange', schedule)
      void api.present!({ id, sequence: ++presentationSequence, bounds: null }).catch(() => undefined)
    }
  }, [api, id])
  return <div className="native-browser-stage" ref={root} aria-label={t('网页内容')}>{!api.present && <p>{t('当前主机没有内嵌网页能力，请更新桌面端。')}</p>}{error && <p role="alert" className="workbench-error">{error}</p>}</div>
}
