import type { CoreClient } from './core-client'
import type { PortablePreferencePlan, PreferenceApplyState } from './portable-preference-file'

type Cursor = { commandId: string; domain: string; targetId: string }
type Page = { items: PortablePreferencePlan[]; nextAfter: Cursor | null }

/** 只订阅已提交 outbox；重启后按原计划继续，绝不重新发起配置迁移。 */
export class PortablePreferenceOwner {
  private running = false
  private dirty = false
  private stopped = false
  private timer: NodeJS.Timeout | null = null
  private registeredGeneration: number | null = null
  constructor(
    private readonly core: Pick<CoreClient, 'request' | 'getSnapshot'>,
    private readonly ready: Promise<void>,
    private readonly preferencesRoot: () => string,
    private readonly apply: (plan: PortablePreferencePlan) => Promise<PreferenceApplyState | 'pending'>
  ) {}

  wake(): void {
    if (this.stopped) return
    this.dirty = true
    if (this.running || this.core.getSnapshot().fullCoreState !== 'ready') return
    this.running = true
    void this.drain().catch(() => {
      // 连接中断时只再领取/核对原 outbox，不重新生成导入业务命令。
      if (!this.stopped) this.dirty = true
    }).finally(() => {
      this.running = false
      if (this.dirty && !this.stopped && !this.timer) {
        this.timer = setTimeout(() => { this.timer = null; this.wake() }, 2000)
        this.timer.unref()
      }
    })
  }

  dispose(): void { this.stopped = true; if (this.timer) clearTimeout(this.timer); this.timer = null }

  private async drain(): Promise<void> {
    await this.ready
    this.dirty = false
    const generation = this.core.getSnapshot().generation
    if (this.registeredGeneration !== generation) {
      await this.core.request('configuration.desktopOwner.register', { preferencesRoot: this.preferencesRoot() })
      this.registeredGeneration = generation
    }
    let after: Cursor | null = null
    do {
      if (this.stopped || this.core.getSnapshot().fullCoreState !== 'ready' || this.core.getSnapshot().generation !== generation) return
      const page = await this.core.request<Page>('configuration.desktopOwner.pending', { after, limit: 4 })
      for (const plan of page.items) {
        if (this.stopped || this.core.getSnapshot().generation !== generation) return
        let state: PreferenceApplyState | 'pending' | 'failed'
        try { state = await this.apply(plan) } catch { state = 'failed' }
        if (state === 'pending') { this.dirty = true; continue }
        if (this.stopped || this.core.getSnapshot().fullCoreState !== 'ready' || this.core.getSnapshot().generation !== generation) {
          this.dirty = !this.stopped
          return
        }
        await this.core.request('configuration.desktopOwner.acknowledge', {
          commandId: plan.commandId, domain: plan.domain, targetId: plan.targetId,
          planDigest: plan.planDigest, state
        })
      }
      after = page.nextAfter
    } while (after)
  }
}
