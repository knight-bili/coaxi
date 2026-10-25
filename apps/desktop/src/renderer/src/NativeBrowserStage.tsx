import { useEffect, useRef, useState } from 'react'
import type { BrowserWorkspaceApi } from '@contracts'
import { readErrorMessage } from './error-message'
import { useUiText } from './interface-language'

let presentationSequence = 0
const overlaySelector = '[role="dialog"],[role="menu"],[data-radix-popper-content-wrapper],[data-native-browser-occluder]'
export function browserOverlayIntersects(page: DOMRect, overlay: DOMRect): boolean {
  return overlay.width > 0 && overlay.height > 0 && overlay.left < page.right && overlay.right > page.left && overlay.top < page.bottom && overlay.bottom > page.top
}
/** Codeg NativeSurfaceHost: paint a decoded still before hiding a native page under DOM overlays. */
export function NativeBrowserStage({ api, id }: { api: BrowserWorkspaceApi; id: string }): React.JSX.Element {
  const root = useRef<HTMLDivElement>(null), t = useUiText()
  const [error, setError] = useState(''), [frozen, setFrozen] = useState<string | null>(null)
  useEffect(() => {
    if (!api.present || !root.current) return
    let live = true, frame: number | null = null, last = '', revision = 0
    const bounded = <T,>(work:Promise<T>,ms:number):Promise<T> => new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(new Error(t('网页画面暂不可用'))),ms);work.then(value=>{clearTimeout(timer);resolve(value)},failure=>{clearTimeout(timer);reject(failure)})})
    const paint = (): Promise<void> => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
    const present = (): void => {
      frame = null
      if (!root.current || !live) return
      const rect = root.current.getBoundingClientRect()
      const visible = !root.current.closest('[hidden],[inert]') && document.visibilityState !== 'hidden' && rect.width > 1 && rect.height > 1
      const covered = visible && [...document.querySelectorAll<HTMLElement>(overlaySelector)].some(element => getComputedStyle(element).visibility !== 'hidden' && browserOverlayIntersects(rect, element.getBoundingClientRect()))
      const bounds = visible ? { x: rect.x, y: rect.y, width: rect.width, height: rect.height } : null
      const key = JSON.stringify([bounds, covered])
      if (key === last) return
      last = key
      const token = ++revision, current = (): boolean => live && revision === token
      void (async () => {
        let snapshotError = ''
        if (covered && api.inspect) {
          try {
          const shot = await bounded(api.inspect({ id, action: 'screenshot' }), 800)
          if (!current()) return
          if (!shot.data) throw new Error(t('暂时无法保留网页画面，请关闭菜单后重试。'))
          const src = 'data:image/png;base64,' + shot.data, image = new Image()
          image.src = src
          await bounded(image.decode(), 100)
          if (!current()) return
          setFrozen(src)
          await bounded(paint(), 100)
          if (!current()) return
          } catch (failure) { snapshotError = readErrorMessage(failure) }
        }
        if (!current()) return
        await api.present!({ id, sequence: ++presentationSequence, bounds: covered ? null : bounds })
        if (current()) { setError(snapshotError); if (!covered) setFrozen(null) }
      })().catch(failure => { if (current()) { last = ''; setError(readErrorMessage(failure)) } })
    }
    const schedule = (): void => { if (frame === null) frame = requestAnimationFrame(present) }
    const resize = new ResizeObserver(schedule); resize.observe(root.current)
    const mutations = new MutationObserver(schedule)
    mutations.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['hidden', 'inert', 'data-state', 'aria-hidden', 'class', 'style'] })
    window.addEventListener('resize', schedule); window.addEventListener('scroll', schedule, true); document.addEventListener('visibilitychange', schedule)
    present()
    return () => {
      live = false; revision++; if (frame !== null) cancelAnimationFrame(frame)
      resize.disconnect(); mutations.disconnect(); window.removeEventListener('resize', schedule); window.removeEventListener('scroll', schedule, true); document.removeEventListener('visibilitychange', schedule)
      void api.present!({ id, sequence: ++presentationSequence, bounds: null }).catch(() => undefined)
    }
  }, [api, id, t])
  return <div className="native-browser-stage" ref={root} aria-label={t('网页内容')}>{frozen && <img src={frozen} alt="" draggable={false} data-browser-frozen-frame="" style={{position:'absolute',inset:0,width:'100%',height:'100%',objectFit:'fill',pointerEvents:'none'}}/>}{!api.present && <p>{t('当前主机没有内嵌网页能力，请更新桌面端。')}</p>}{error && <p role="alert" className="workbench-error">{error}</p>}</div>
}
