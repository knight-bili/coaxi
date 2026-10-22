import { readFileSync } from 'node:fs'
import type { WindowCloseBehavior } from '@contracts'
import { isWindowCloseBehavior } from '../shared/window-close'
import { writePrivateJson } from './general-preferences'
import { applyOwnedPreferenceFile, type PortablePreferencePlan, type PreferenceApplyState } from './portable-preference-file'

/** Desktop-local preference; never added to Host/Web or conversation state. */
export class WindowClosePreferences {
  #behavior: WindowCloseBehavior = 'ask'
  readonly loadFailed: boolean
  #writeTail: Promise<unknown> = Promise.resolve()

  constructor(private readonly filePath: string) {
    let failed = false
    try {
      const value = JSON.parse(readFileSync(filePath, 'utf8'))
      if (value?.schemaVersion !== 1 || !isWindowCloseBehavior(value.behavior)) throw new Error('Invalid close preference')
      this.#behavior = value.behavior
    } catch (error) {
      failed = !(error instanceof Error && 'code' in error && error.code === 'ENOENT')
    }
    this.loadFailed = failed
  }

  get(): WindowCloseBehavior { return this.#behavior }

  // The owning close controller serializes all mutations, including remembered choices.
  set(behavior: WindowCloseBehavior): Promise<void> {
    if (!isWindowCloseBehavior(behavior)) throw new Error('Invalid close behavior')
    return this.#enqueue(async () => {
      await writePrivateJson(this.filePath, { schemaVersion: 1, behavior })
      this.#behavior = behavior
    })
  }

  applyConfiguration(plan: PortablePreferencePlan): Promise<PreferenceApplyState> {
    return this.#enqueue(async () => {
      const value = plan.after as { schemaVersion?: unknown; behavior?: unknown } | null
      if (plan.targetId !== 'windowClose' || (value !== null && (!value || value.schemaVersion !== 1 || Object.keys(value).length !== 2 || !isWindowCloseBehavior(value.behavior)))) throw new Error('configuration.close_preferences_invalid')
      const state = await applyOwnedPreferenceFile(this.filePath, plan, next => writePrivateJson(this.filePath, next))
      if (state === 'applied') this.#behavior = value === null ? 'ask' : value.behavior as WindowCloseBehavior
      return state
    })
  }

  #enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const next = this.#writeTail.then(operation, operation)
    this.#writeTail = next.catch(() => undefined)
    return next
  }
}
