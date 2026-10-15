import { describe, expect, it } from 'vitest'
import { CanvasNotificationVisibility, intersectNotificationRects } from './canvas-notification-visibility'
const source = (threadId: string, messageId: string, conversationId: string | null = null) => ({ threadId, conversationId,
  surfaceVisible: true, snapshotSequence: 10, messageIds: [messageId], campTurnIds: [], agentRunIds: [], approvalIds: [] })

describe('canvas visibility reports', () => {
  it('retains both mirrors and private reports while closing only their own lease', () => {
    const store = new CanvasNotificationVisibility(), first = store.register('first', 'a'), mirror = store.register('mirror', 'a'), other = store.register('other', 'b')
    first.publish('public', source('a', 'first')); mirror.publish('public', source('a', 'mirror')); other.publish('private', source('b', 'private', 'conversation-b'))
    const reports = store.getSnapshot()
    expect(reports).toHaveLength(3)
    first.release()
    expect(store.getSnapshot().map(s => s.messageIds[0])).toEqual(['mirror', 'private'])
    expect(reports).toHaveLength(3)
    other.release(); expect(store.getSnapshot()[0].threadId).toBe('a')
  })
  it('rejects foreign Thread reports and late publication/cleanup from replaced mounts', () => {
    const store = new CanvasNotificationVisibility(), old = store.register('same-card', 'a'), current = store.register('same-card', 'b')
    old.publish('public', source('a', 'old')); current.publish('public', source('a', 'foreign')); old.release()
    expect(store.getSnapshot()).toEqual([])
    current.publish('public', source('b', 'current')); old.release()
    expect(store.getSnapshot()[0].messageIds).toEqual(['current'])
  })
  it('clones reports and avoids publishing unchanged snapshots', () => {
    const store = new CanvasNotificationVisibility(), card = store.register('first', 'a'), input = source('a', 'message')
    let updates = 0; const stop = store.subscribe(() => updates++)
    card.publish('public', input); const stable = store.getSnapshot()
    card.publish('public', input); input.messageIds.push('later')
    expect(store.getSnapshot()).toBe(stable); expect(stable[0].messageIds).toEqual(['message']); expect(updates).toBe(1)
    stop(); card.release(); expect(updates).toBe(1)
  })
  it('keeps separate client-owned registries even with identical context keys', () => {
    const first = new CanvasNotificationVisibility(), next = new CanvasNotificationVisibility()
    first.register('same', 'a').publish('public', source('a', 'old'))
    expect(next.getSnapshot()).toEqual([])
  })
})
it('clips transformed cards and nested scroll bounds to the window, excluding zero-area edges', () => {
  const window = { left: 0, top: 0, right: 100, bottom: 100 }
  expect(intersectNotificationRects([window, {left: 90, top: 50, right: 200, bottom: 200}])).toEqual({left:90,top:50,right:100,bottom:100})
  expect(intersectNotificationRects([window, {left: 100, top: 0, right: 200, bottom: 100}])).toBeNull()
  expect(intersectNotificationRects([window, {left: 10, top: 10, right: 90, bottom: 90}, {left: 20, top: 20, right: 30, bottom: 30}])).toEqual({left:20,top:20,right:30,bottom:30})
})
