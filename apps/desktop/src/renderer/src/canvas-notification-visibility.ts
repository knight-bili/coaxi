import type { VisibleNotificationSources } from './ThreadWorkspace'

/** Ephemeral reports only. Core still owns acknowledgement and unread state. */
export class CanvasNotificationVisibility {
  private readonly owners = new Map<string, { threadId: string; sources: Map<string, VisibleNotificationSources> }>()
  private readonly listeners = new Set<() => void>()
  private snapshot: readonly VisibleNotificationSources[] = []
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener) } }
  getSnapshot = (): readonly VisibleNotificationSources[] => this.snapshot
  readVisibleSources = (): readonly VisibleNotificationSources[] => {
    if (document.visibilityState !== 'visible' || !document.hasFocus()) return []
    return [...this.owners].flatMap(([key, owner]) => {
      const frame = [...document.querySelectorAll<HTMLElement>('.canvas-conversation-frame')].find(node => node.dataset.canvasConversationKey === key)
      if (!frame) return []
      const visibleIds = (attribute: string): Set<string> => new Set([...frame.querySelectorAll<HTMLElement>(`[${attribute}]`)]
        .filter(node => canvasNotificationNodeVisible(node)).map(node => node.getAttribute(attribute)!))
      const messages = visibleIds('data-message-id'), turns = new Set([...visibleIds('data-thread-turn-id'), ...visibleIds('data-notification-turn-id')])
      const runs = visibleIds('data-agent-run-id'), approvals = visibleIds('data-approval-id')
      return [...owner.sources.values()].filter(source => source.surfaceVisible === true).map(source => ({ ...source,
        surfaceVisible: canvasNotificationNodeVisible(frame), messageIds: source.messageIds.filter(id => messages.has(id)),
        campTurnIds: source.campTurnIds.filter(id => turns.has(id)), agentRunIds: source.agentRunIds.filter(id => runs.has(id)),
        approvalIds: source.approvalIds.filter(id => approvals.has(id)) }))
    })
  }
  private changed(): void {
    const next = [...this.owners.values()].flatMap(owner => [...owner.sources.values()])
    if (JSON.stringify(next) === JSON.stringify(this.snapshot)) return
    this.snapshot = next
    for (const listener of this.listeners) listener()
  }
  register(key: string, threadId: string) {
    const owner = { threadId, sources: new Map<string, VisibleNotificationSources>() }
    this.owners.set(key, owner); this.changed()
    return {
      publish: (channel: 'public' | 'private', source: VisibleNotificationSources): void => {
        if (this.owners.get(key) !== owner || source.threadId !== threadId) return
        owner.sources.set(channel, structuredClone(source)); this.changed()
      },
      release: (): void => { if (this.owners.get(key) === owner) { this.owners.delete(key); this.changed() } }
    }
  }
}

type Rect = { top: number; right: number; bottom: number; left: number }
export function intersectNotificationRects(rects: readonly Rect[]): Rect | null {
  const rect = { top: Math.max(...rects.map(r => r.top)), right: Math.min(...rects.map(r => r.right)),
    bottom: Math.min(...rects.map(r => r.bottom)), left: Math.max(...rects.map(r => r.left)) }
  return rect.right > rect.left && rect.bottom > rect.top ? rect : null
}

/** Canvas transforms do not change local scroll visibility. Also clip to ancestors and hit-test occlusion. */
export function canvasNotificationNodeVisible(node: HTMLElement, viewport?: HTMLElement): boolean {
  if (!node.closest('.canvas-conversation-frame') || !node.getClientRects().length) return false
  const rects: Rect[] = [node.getBoundingClientRect(), { top: 0, left: 0, right: window.innerWidth, bottom: window.innerHeight }]
  if (viewport) rects.push(viewport.getBoundingClientRect())
  for (let parent: HTMLElement | null = node; parent; parent = parent.parentElement) {
    const style = getComputedStyle(parent)
    if (parent.hidden || parent.inert || style.visibility === 'hidden' || Number(style.opacity) === 0) return false
    const bounds = parent.getBoundingClientRect()
    const clips = (value: string) => /^(hidden|clip|scroll|auto)$/.test(value)
    rects.push({ left: clips(style.overflowX) ? bounds.left : -Infinity, right: clips(style.overflowX) ? bounds.right : Infinity,
      top: clips(style.overflowY) ? bounds.top : -Infinity, bottom: clips(style.overflowY) ? bounds.bottom : Infinity })
  }
  const bounds = intersectNotificationRects(rects)
  if (!bounds) return false
  const xs = [0.5, 0.1, 0.9].map(fraction => bounds.left + (bounds.right - bounds.left) * fraction)
  const ys = [0.5, 0.1, 0.9].map(fraction => bounds.top + (bounds.bottom - bounds.top) * fraction)
  return xs.some(x => ys.some(y => {
    const hit = document.elementFromPoint(x, y)
    return hit !== null && node.contains(hit)
  }))
}

export function observeCanvasNotificationVisibility(node: HTMLElement | null, schedule: () => void): () => void {
  if (!node) return () => undefined
  const mutation = new MutationObserver(schedule), resize = new ResizeObserver(schedule)
  // Sibling drawers/dialogs can cover a card; ancestor transforms can move it without resizing it.
  mutation.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['style', 'class', 'hidden', 'inert'] })
  for (let parent: HTMLElement | null = node; parent; parent = parent.parentElement) resize.observe(parent)
  window.addEventListener('scroll', schedule, { capture: true, passive: true })
  window.addEventListener('blur', schedule)
  return () => { mutation.disconnect(); resize.disconnect(); window.removeEventListener('scroll', schedule, true); window.removeEventListener('blur', schedule) }
}
