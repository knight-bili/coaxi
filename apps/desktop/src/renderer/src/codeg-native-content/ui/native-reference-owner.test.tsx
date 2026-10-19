// @vitest-environment jsdom
import { useContext, type ContextType } from 'react'
import { render, cleanup } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { NativeMarkdownProvider, NativeMarkdownScope } from './native-markdown-context'
afterEach(cleanup)
describe('native file action owner after its menu unmounts', () => {
  it('retains user selection when only the menu closes, then expires it on reader replacement/hide/unmount', () => {
    const values: Array<NonNullable<ContextType<typeof NativeMarkdownScope>>> = []
    function Capture() { const value = useContext(NativeMarkdownScope); if (value) values.push(value); return null }
    const first = { source: { threadId: 'a', readId: 'reader-a' }, api: null, visible: true, theme: 'day' as const, open: null }
    const view = render(<NativeMarkdownProvider value={first}><Capture /></NativeMarkdownProvider>)
    const original = values.at(-1)!
    view.rerender(<NativeMarkdownProvider value={first}>{null}</NativeMarkdownProvider>)
    expect(original.isCurrent?.()).toBe(true)
    view.rerender(<NativeMarkdownProvider value={{ ...first, source: { threadId: 'a', readId: 'reader-b' } }}><Capture /></NativeMarkdownProvider>)
    expect(original.isCurrent?.()).toBe(false)
    const replacement = values.at(-1)!
    view.rerender(<NativeMarkdownProvider value={{ ...first, visible: false }}><Capture /></NativeMarkdownProvider>)
    expect(replacement.isCurrent?.()).toBe(false); expect(values.at(-1)!.isCurrent?.()).toBe(false)
    view.rerender(<NativeMarkdownProvider value={first}><Capture /></NativeMarkdownProvider>)
    const final = values.at(-1)!
    view.unmount(); expect(final.isCurrent?.()).toBe(false)
  })
})
