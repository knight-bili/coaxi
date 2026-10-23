// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import {
  applyConversationFindHighlights,
  centeredConversationFindScrollTop,
  conversationFindRanges,
  nextConversationFindIndex,
  pendingConversationFindStatus
} from './camp-conversation-find'

const cssDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'CSS')
const highlightDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'Highlight')

afterEach(() => {
  document.body.replaceChildren()
  document.head.querySelectorAll('style[data-conversation-find-owner]').forEach(style => style.remove())
  if (cssDescriptor) Object.defineProperty(globalThis, 'CSS', cssDescriptor)
  else Reflect.deleteProperty(globalThis, 'CSS')
  if (highlightDescriptor) Object.defineProperty(globalThis, 'Highlight', highlightDescriptor)
  else Reflect.deleteProperty(globalThis, 'Highlight')
})

describe('conversation find helpers', () => {
  it('finds case-insensitive, non-overlapping UTF-16 ranges for the DOM', () => {
    expect(conversationFindRanges('Needle NEEDLE', 'needle')).toEqual([
      { start: 0, end: 6 },
      { start: 7, end: 13 }
    ])
    expect(conversationFindRanges('aaaa', 'aa')).toEqual([
      { start: 0, end: 2 },
      { start: 2, end: 4 }
    ])
    expect(conversationFindRanges('İstanbul', 'i\u{307}')).toEqual([{ start: 0, end: 1 }])
    expect(conversationFindRanges('🙂a', 'a')).toEqual([{ start: 2, end: 3 }])
    expect(conversationFindRanges('无结果', '')).toEqual([])
  })

  it('wraps traversal in both directions', () => {
    expect(nextConversationFindIndex(3, 4, 1)).toBe(0)
    expect(nextConversationFindIndex(0, 4, -1)).toBe(3)
    expect(nextConversationFindIndex(null, 4, 1)).toBe(0)
    expect(nextConversationFindIndex(null, 4, -1)).toBe(3)
    expect(nextConversationFindIndex(null, 0, 1)).toBeNull()
  })

  it('enters a pending status synchronously when a ready query is edited', () => {
    expect(pendingConversationFindStatus('next query')).toBe('searching')
    expect(pendingConversationFindStatus('   ')).toBe('idle')
  })

  it('centers the exact occurrence inside the unobstructed find viewport', () => {
    expect(centeredConversationFindScrollTop({
      currentScrollTop: 300,
      maximumScrollTop: 900,
      viewportTop: 100,
      viewportBottom: 700,
      targetTop: 680,
      targetBottom: 700,
      topInset: 60,
      bottomInset: 20
    })).toBe(570)

    expect(centeredConversationFindScrollTop({
      currentScrollTop: 300,
      maximumScrollTop: 900,
      viewportTop: 100,
      viewportBottom: 700,
      targetTop: 110,
      targetBottom: 130,
      topInset: 60,
      bottomInset: 20
    })).toBe(0)
  })

  it('isolates two canvas highlight owners and keeps their styles and match counts aligned', () => {
    const registry = new Map<string, unknown>()
    class TestHighlight {
      readonly ranges: Range[]
      constructor(...ranges: Range[]) { this.ranges = ranges }
    }
    Object.defineProperty(globalThis, 'CSS', { configurable: true, value: { highlights: registry } })
    Object.defineProperty(globalThis, 'Highlight', { configurable: true, value: TestHighlight })

    const firstTimeline = document.createElement('div')
    firstTimeline.innerHTML = '<div class="conversation-bubble user" data-message-id="first"><div class="message-bubble">needle needle</div></div>'
    const secondTimeline = document.createElement('div')
    secondTimeline.innerHTML = '<div class="conversation-bubble agent" data-message-id="second"><div class="final-copy">needle needle</div></div>'
    document.body.append(firstTimeline, secondTimeline)

    const clearFirst = applyConversationFindHighlights(firstTimeline, 'needle', null, null, 'canvas-first')
    const clearSecond = applyConversationFindHighlights(secondTimeline, 'needle', 'second', 0, 'canvas-second')
    const names = [
      'conversation-find-match-canvas-first',
      'conversation-find-match-canvas-second',
      'conversation-find-current-canvas-second'
    ]
    expect([...registry.keys()].sort()).toEqual([...names].sort())
    expect((registry.get(names[0]!) as TestHighlight).ranges).toHaveLength(conversationFindRanges('needle needle', 'needle').length)
    expect((registry.get(names[1]!) as TestHighlight).ranges).toHaveLength(1)
    expect((registry.get(names[2]!) as TestHighlight).ranges).toHaveLength(1)

    const firstStyle = document.head.querySelector<HTMLStyleElement>('[data-conversation-find-owner="canvas-first"]')!
    const secondStyle = document.head.querySelector<HTMLStyleElement>('[data-conversation-find-owner="canvas-second"]')!
    expect(firstStyle.textContent).toContain('::highlight(conversation-find-match-canvas-first)')
    expect(secondStyle.textContent).toContain('::highlight(conversation-find-match-canvas-second)')
    expect(secondStyle.textContent).toContain('::highlight(conversation-find-current-canvas-second)')

    clearFirst()
    expect(registry.has('conversation-find-match-canvas-first')).toBe(false)
    expect(registry.has('conversation-find-match-canvas-second')).toBe(true)
    expect(registry.has('conversation-find-current-canvas-second')).toBe(true)
    expect(firstStyle.isConnected).toBe(false)
    expect(secondStyle.isConnected).toBe(true)

    clearSecond()
    expect(registry.size).toBe(0)
  })
})
