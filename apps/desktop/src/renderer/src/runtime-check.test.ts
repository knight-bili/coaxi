import { afterEach, describe, expect, it, vi } from 'vitest'
import { openRuntimeModelCatalog, requestProductRuntimeCheck } from './runtime-check'

describe('explicit Runtime check targeting', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('checks only the Qwen Runtime selected by the user', async () => {
    const request = vi.fn().mockResolvedValue({
      scheduled: true,
      completed: true,
      ready: true,
      runtimeKind: 'qwen-code'
    })
    vi.stubGlobal('window', { coaxi: { request } })

    await requestProductRuntimeCheck('qwen-code')

    expect(request).toHaveBeenCalledTimes(1)
    expect(request).toHaveBeenCalledWith('runtime.product.check', {
      runtimeKind: 'qwen-code'
    })
  })

  it('opens only the selected Runtime model catalog', async () => {
    const request = vi.fn().mockResolvedValue({
      runtimeKind: 'copilot-cli',
      cache: {
        status: 'fresh',
        observedAt: '2026-08-18T00:00:00Z',
        revalidateAfter: '2026-08-18T00:01:00Z',
        expiresAt: '2026-08-19T00:00:00Z'
      },
      models: [],
      refreshStatus: 'not_required',
      diagnosticCode: null
    })
    vi.stubGlobal('window', { coaxi: { request } })

    await openRuntimeModelCatalog('copilot-cli')

    expect(request).toHaveBeenCalledWith('runtime.modelCatalog.open', {
      runtimeKind: 'copilot-cli'
    })
  })

  it('waits for the backend-owned fresh check and preserves its failure outcome without a separate rescan', async () => {
    let finishCheck!: (result: unknown) => void
    const completion = new Promise(resolve => { finishCheck = resolve })
    const request = vi.fn().mockReturnValueOnce(completion)
    vi.stubGlobal('window', { coaxi: { request } })

    const check = requestProductRuntimeCheck('codex-cli')
    expect(request).toHaveBeenCalledExactlyOnceWith('runtime.product.check', { runtimeKind: 'codex-cli' })
    finishCheck({ ready: false, outcome: 'stable_failure' })
    await expect(check).resolves.toEqual({ ready: false, outcome: 'stable_failure' })
    expect(request).toHaveBeenCalledTimes(1)
  })

  it('does not hide a backend environment-read failure or retry with cached discovery', async () => {
    const request = vi.fn().mockRejectedValue(new Error('discovery failed'))
    vi.stubGlobal('window', { coaxi: { request } })
    await expect(requestProductRuntimeCheck('codex-cli')).rejects.toThrow('discovery failed')
    expect(request).toHaveBeenCalledTimes(1)
  })
})
