// @vitest-environment jsdom
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { expect, it, vi } from 'vitest'

// Owns the real static Web bootstrap's message boundary. Browser rendering and
// the original nested sandbox/CSP are verified separately in the Chromium fixture.
it.each([false, true])('accepts only the current parent initialization and preserves inner script choice (%s)', scripts => {
  const html = readFileSync(resolve('apps/web/public/preview.html'), 'utf8')
  const id = '12345678-1234-1234-1234-123456789abc'
  const parentDescriptor = Object.getOwnPropertyDescriptor(window, 'parent')!
  const listen = vi.spyOn(window, 'addEventListener'), previous = location.href
  history.replaceState(null, '', '/preview.html#codeg.'+id)
  document.documentElement.innerHTML = html
  try {
    const parent = { postMessage: vi.fn() }, foreign = {}
    Object.defineProperty(window, 'parent', { configurable: true, value: parent })
    window.eval(document.querySelector('script')!.textContent!)
    expect(parent.postMessage).toHaveBeenCalledWith({ protocol: 'rovai-codeg-preview-v1', id, type: 'ready' }, location.origin)
    const send = (data: unknown, source: unknown = parent, origin = location.origin) => window.dispatchEvent(new MessageEvent('message', { data, source: source as Window, origin }))
    const init = { protocol: 'rovai-codeg-preview-v1', id, type: 'initialize', html: '<p>完整末尾😀</p>', scripts }
    send(init, foreign); send(init, parent, 'http://invalid.example'); send({ ...init, id: 'stale' }); send({ ...init, scripts: 'true' })
    expect(document.querySelector('iframe')).toBeNull()
    send(init)
    const frame = document.querySelector('iframe')!
    expect(frame.getAttribute('sandbox')).toBe(scripts ? 'allow-scripts' : '')
    expect(frame.getAttribute('sandbox')).not.toContain('allow-same-origin')
    expect(frame.srcdoc).toBe(init.html)
    send({ ...init, html: '<p>第二份</p>' })
    expect(frame.srcdoc).toBe(init.html)
    parent.postMessage.mockClear()
    send({ type: 'codeg-visualize:size', height: 1800 }, foreign)
    send({ type: 'codeg-visualize:size', height: Number.POSITIVE_INFINITY }, frame.contentWindow)
    expect(parent.postMessage).not.toHaveBeenCalled()
    send({ type: 'codeg-visualize:size', height: 1800 }, frame.contentWindow)
    expect(parent.postMessage).toHaveBeenCalledOnce()
    expect(parent.postMessage).toHaveBeenCalledWith({ type: 'codeg-visualize:size', height: 1800 }, location.origin)
  } finally {
    for (const [type, listener] of listen.mock.calls) if (type === 'message') window.removeEventListener(type, listener)
    listen.mockRestore(); Object.defineProperty(window, 'parent', parentDescriptor)
    document.body.replaceChildren(); history.replaceState(null, '', previous)
  }
})
