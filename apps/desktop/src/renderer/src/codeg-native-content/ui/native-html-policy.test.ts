// @vitest-environment jsdom
import { expect, it, vi } from 'vitest'
import { inlineHtmlResources, withSandboxCsp } from '../lib/html-preview-inline'

it.each(["script-src 'none'", 'default-src *'])('intersects author CSP with native sandbox policy (%s)', async policy => {
  const readFileBase64 = vi.fn(async () => '')
  const document = '<!doctype html><html><head><meta http-equiv="Content-Security-Policy" content="'+policy+'"><meta http-equiv="refresh" content="0;url=https://invalid.example"><base href="https://invalid.example"></head><body><p>完整末尾</p></body></html>'
  const inlined = await inlineHtmlResources(document, { fileDir: '/source', folderPath: '/source', readFileBase64, preserveAuthorCsp: true })
  const output = new DOMParser().parseFromString(withSandboxCsp(inlined, { trusted: true }), 'text/html')
  const policies = [...output.querySelectorAll('meta[http-equiv="Content-Security-Policy"]')].map(node => node.getAttribute('content'))
  expect(policies).toHaveLength(2)
  expect(policies[0]).toContain("frame-src 'none'")
  expect(policies[1]).toBe(policy)
  expect(output.querySelector('meta[http-equiv="refresh"]')).toBeNull()
  expect(output.querySelector('base')?.getAttribute('href')).toBe('about:srcdoc')
  expect(output.querySelector('p')?.textContent).toBe('完整末尾')
  expect(readFileBase64).not.toHaveBeenCalled()
})
