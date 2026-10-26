// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { leaveDrawer, useDrawerPresence } from './useDrawerPresence'

afterEach(() => { cleanup(); vi.useRealTimers(); vi.unstubAllGlobals() })

it('retains the same content for the complete exit and cancels an interrupted exit', () => {
  vi.useFakeTimers()
  const view = renderHook(({ value }) => useDrawerPresence(value), { initialProps: { value: 'a' as string | null } })
  view.rerender({ value: null })
  expect(view.result.current).toEqual({ value: 'a', closing: true })
  act(() => vi.advanceTimersByTime(100))
  view.rerender({ value: 'b' })
  act(() => vi.advanceTimersByTime(200))
  expect(view.result.current).toEqual({ value: 'b', closing: false })
  view.rerender({ value: null })
  act(() => vi.advanceTimersByTime(199))
  expect(view.result.current.value).toBe('b')
  act(() => vi.advanceTimersByTime(1))
  expect(view.result.current.value).toBeNull()
})

it('does not wait for motion when the person requests reduced motion', () => {
  vi.useFakeTimers()
  vi.stubGlobal('matchMedia', () => ({ matches: true }))
  const view = renderHook(({ value }) => useDrawerPresence(value), { initialProps: { value: 'a' as string | null } })
  view.rerender({ value: null })
  expect(view.result.current.value).toBeNull()
  expect(vi.getTimerCount()).toBe(0)
})

it('cleans up exit work on unmount and reacts to a reduced-motion change', () => {
  vi.useFakeTimers()
  const listeners = new Set<() => void>()
  const media = { matches: false, addEventListener: (_: string, fn: () => void) => listeners.add(fn), removeEventListener: (_: string, fn: () => void) => listeners.delete(fn) }
  vi.stubGlobal('matchMedia', () => media)
  const view = renderHook(({ value }) => useDrawerPresence(value), { initialProps: { value: 'a' as string | null } })
  view.rerender({ value: null })
  act(() => { media.matches = true; for (const listener of listeners) listener() })
  expect(view.result.current.value).toBeNull()
  expect(listeners.size).toBe(0)
  view.unmount()
  expect(vi.getTimerCount()).toBe(0)
})

it('waits for a mission exit before navigation and releases inert state even on failure', async () => {
  let finish!: () => void
  const animation = { finished: new Promise<void>(done => { finish = done }), cancel: vi.fn() }
  const element = document.createElement('section')
  element.inert = false
  element.animate = vi.fn(() => animation) as unknown as typeof element.animate
  const commit = vi.fn(async () => { throw Error('blocked') })
  const pending = leaveDrawer(element, commit)
  expect(element.inert).toBe(true); expect(commit).not.toHaveBeenCalled()
  const newerNavigation = vi.fn()
  await leaveDrawer(element, newerNavigation)
  expect(newerNavigation).toHaveBeenCalledOnce(); expect(element.animate).toHaveBeenCalledOnce()
  finish()
  await expect(pending).rejects.toThrow('blocked')
  expect(element.inert).toBe(false); expect(animation.cancel).toHaveBeenCalledOnce()
})
