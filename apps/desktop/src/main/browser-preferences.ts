import { app, type Session } from 'electron'
import { randomUUID } from 'node:crypto'
import { promises as fs } from 'node:fs'
import path from 'node:path'
import type { BrowserPreferences } from '@contracts'
import { isHostRule, matchHostRule, parseHostRulePattern, normalizeHostRulePattern } from '../shared/codeg-browser-host-rules'

const validate = (value: BrowserPreferences): BrowserPreferences => {
  if (!value || typeof value.revision !== 'string' || typeof value.devtools !== 'boolean' || !Array.isArray(value.hostRules) || value.hostRules.length > 200 || value.hostRules.some(rule => !isHostRule(rule) || !parseHostRulePattern(rule.pattern))) throw new Error('browser.invalid_preferences')
  let proxy: string | null = null
  if (value.proxy !== null) {
    if (typeof value.proxy !== 'string' || value.proxy.length > 1024) throw new Error('browser.invalid_proxy')
    const url = new URL(value.proxy)
    if (!['http:', 'socks5:'].includes(url.protocol) || url.username || url.password || url.pathname !== '/' && url.pathname !== '' || url.search || url.hash || !url.hostname || !url.port) throw new Error('browser.invalid_proxy')
    proxy = `${url.protocol}//${url.host}`
  }
  return { revision: value.revision, devtools: value.devtools, proxy, hostRules: value.hostRules.map(rule => ({pattern: normalizeHostRulePattern(rule.pattern), action: rule.action})) }
}

/** Native owner policy, separate from any model-visible browser tool catalog. */
export class BrowserPreferenceStore {
  private readonly file = path.join(app.getPath('userData'), 'browser-preferences.json')
  private value: BrowserPreferences = { revision: randomUUID(), hostRules: [], proxy: null, devtools: false }
  private readonly sessions = new Map<Session, number>()
  private readonly ready: Promise<void>
  private tail: Promise<unknown> = Promise.resolve()
  constructor(private readonly changed: (value: BrowserPreferences) => void) { this.ready = this.load() }
  private async load(): Promise<void> {
    try {
      const stat = await fs.lstat(this.file)
      if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 262144) throw new Error('browser.invalid_preferences')
      this.value = validate(JSON.parse(await fs.readFile(this.file, 'utf8')) as BrowserPreferences)
    } catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error }
  }
  private exclusive<T>(run: () => Promise<T>): Promise<T> { const result = this.tail.then(async () => { await this.ready; return run() }); this.tail = result.catch(() => undefined); return result }
  async get(): Promise<BrowserPreferences> { await this.ready; return structuredClone(this.value) }
  action(url: string): 'builtin' | 'system' | 'block' { try { return matchHostRule(this.value.hostRules, new URL(url))?.action ?? 'builtin' } catch { return 'block' } }
  private async proxy(value: Session, preferences: BrowserPreferences): Promise<void> {
    await value.setProxy(preferences.proxy ? { mode: 'fixed_servers', proxyRules: preferences.proxy, proxyBypassRules: '<-loopback>' } : { mode: 'system' })
    await value.closeAllConnections()
  }
  async attach(value: Session): Promise<{ devtools: boolean; release: () => void }> {
    return this.exclusive(async () => {
      if (!this.sessions.has(value)) {
        await this.proxy(value, this.value)
        value.webRequest.onBeforeRequest((details, callback) => {
          const action = this.action(details.url)
          const navigation = details.resourceType === 'mainFrame' || details.resourceType === 'subFrame'
          const protocol = new URL(details.url).protocol
          callback({ cancel: ['http:', 'https:', 'ws:', 'wss:'].includes(protocol) && (action === 'block' || navigation && action === 'system') })
        })
      }
      this.sessions.set(value, (this.sessions.get(value) ?? 0) + 1)
      let released = false
      return { devtools: this.value.devtools, release: () => { if (!released) { released = true; const count = Math.max(0, (this.sessions.get(value) ?? 1) - 1); if (count === 0) this.sessions.delete(value); else this.sessions.set(value, count) } } }
    })
  }
  async update(input: BrowserPreferences): Promise<BrowserPreferences> {
    return this.exclusive(async () => {
      if (input.revision !== this.value.revision) throw new Error('browser.preferences_changed')
      const next = { ...validate(input), revision: randomUUID() }, before = this.value
      const temporary = `${this.file}.${randomUUID()}.tmp`
      try {
        if (next.proxy !== before.proxy) for (const value of this.sessions.keys()) await this.proxy(value, next)
        await fs.writeFile(temporary, JSON.stringify(next), { encoding: 'utf8', mode: 0o600, flag: 'wx' }); await fs.rename(temporary, this.file)
      } catch (error) {
        if (next.proxy !== before.proxy) for (const value of this.sessions.keys()) { try { await this.proxy(value, before) } catch { /* Surface policy is revoked on failed transition below. */ } }
        this.changed(before)
        throw error
      } finally { await fs.unlink(temporary).catch(() => undefined) }
      this.value = next; this.changed(next)
      return this.get()
    })
  }
}
