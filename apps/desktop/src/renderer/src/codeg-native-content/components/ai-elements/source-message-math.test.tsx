// @vitest-environment jsdom
import { render, waitFor, cleanup, fireEvent } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { MessageResponse } from './source-message-response'
import { __resetStreamdownPluginsForTest } from './streamdown-plugins'
import { JsonRenderScope } from './json-render-pre'

afterEach(cleanup)
beforeEach(__resetStreamdownPluginsForTest)
describe('actual source message math pipeline', () => {
  it('renders both inline formulas and a block formula after its real lazy engine loads', async () => {
    const view = render(<MessageResponse>{'行内公式 \\(E=mc^2\\)，以及 $$a^2+b^2=c^2$$。\n\n\\[\\frac{1}{2}\\]'}</MessageResponse>)
    await waitFor(() => expect(view.container.querySelectorAll('.katex')).toHaveLength(3), { timeout: 4000 })
  })
  it('refreshes original nested paragraph, list, strong, quote and table containers after lazy math arrives', async () => {
    const view = render(<MessageResponse>{'# $$x$$\n\n- **$$y$$**\n\n> $$z$$\n\n| 数式 |\n|---|\n| $$w$$ |'}</MessageResponse>)
    await waitFor(() => expect(view.container.querySelectorAll('.katex')).toHaveLength(4), { timeout: 4000 })
  })
  it('retains an interactive card state when later text introduces math', async () => {
    // This case checks state continuity, not Vite's cold transform latency.
    // Load the real card module before the DOM assertion; no renderer is mocked.
    await import('../message/json-render-block')
    const patches = [
      { op: 'add', path: '/root', value: 'main' },
      { op: 'add', path: '/elements/main', value: { type: 'Stack', props: {}, children: ['action', 'status'] } },
      { op: 'add', path: '/elements/action', value: { type: 'Link', props: { label: '切换状态' }, on: { press: { action: 'setState', params: { statePath: '/status', value: '已切换' } } } } },
      { op: 'add', path: '/elements/status', value: { type: 'Text', props: { text: { $state: '/status' } } } },
      { op: 'add', path: '/state/status', value: '未切换' }
    ].map(value => JSON.stringify(value)).join('\n')
    const text = '```spec\n' + patches + '\n```'
    const view = render(<JsonRenderScope><MessageResponse>{text}</MessageResponse></JsonRenderScope>)
    fireEvent.click(await view.findByRole('button', { name: '切换状态' }))
    expect(view.getByText('已切换')).toBeTruthy()
    view.rerender(<JsonRenderScope><MessageResponse>{text + '\n\n公式 $$x$$'}</MessageResponse></JsonRenderScope>)
    await waitFor(() => expect(view.container.querySelectorAll('.katex')).toHaveLength(1), { timeout: 4000 })
    expect(view.getByText('已切换')).toBeTruthy()
  })
})
