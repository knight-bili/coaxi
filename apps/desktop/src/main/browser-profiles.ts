import { app, session, type Session } from 'electron'
import { randomUUID } from 'node:crypto'
import { promises as fs } from 'node:fs'
import path from 'node:path'
import type { BrowserProfileView, BrowserWorkspaceApi } from '@contracts'

type Catalog = { revision: string; profiles: BrowserProfileView[] }
const validId = (id: string): boolean => id === 'default' || /^p-[0-9a-f-]{36}$/.test(id)
export class BrowserProfiles {
  private readonly file = path.join(app.getPath('userData'), 'browser-profiles.json')
  private catalog: Catalog = { revision: randomUUID(), profiles: [{ id: 'default', name: '默认浏览器', createdAt: Date.now() }] }
  private readonly occupancy = new Map<string, number>()
  private readonly ready: Promise<void>
  private tail: Promise<unknown> = Promise.resolve()
  constructor(private readonly hasDownloads: (value: Session) => boolean) { this.ready = this.load() }

  private async load(): Promise<void> {
    try {
      const stat = await fs.lstat(this.file)
      if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 131072) throw new Error('browser.invalid_profile_store')
      const value = JSON.parse(await fs.readFile(this.file, 'utf8')) as Catalog
      if (typeof value.revision !== 'string' || !Array.isArray(value.profiles) || value.profiles.length > 32 || !value.profiles.some(row => row.id === 'default')) throw new Error('browser.invalid_profile_store')
      const ids = new Set<string>()
      for (const row of value.profiles) {
        if (!row || !validId(row.id) || ids.has(row.id) || typeof row.name !== 'string' || !row.name.trim() || row.name.length > 80 || !Number.isFinite(row.createdAt)) throw new Error('browser.invalid_profile_store')
        ids.add(row.id)
      }
      this.catalog = { revision: value.revision, profiles: value.profiles.map(row => ({ id: row.id, name: row.name, createdAt: row.createdAt })) }
    } catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error }
  }

  private exclusive<T>(run: () => Promise<T>): Promise<T> {
    const result = this.tail.then(async () => { await this.ready; return run() })
    this.tail = result.catch(() => undefined); return result
  }

  async list(): Promise<Catalog> { await this.ready; return { revision: this.catalog.revision, profiles: this.catalog.profiles.map(row => ({ ...row })) } }
  async admit(profileId: string | undefined, tabId: string): Promise<{ profileId: string; session: Session; release: () => void }> {
    return this.exclusive(async () => {
      const id = profileId ?? 'temporary'
      if (id !== 'temporary' && !this.catalog.profiles.some(row => row.id === id)) throw new Error('browser.profile_not_found')
      const value = session.fromPartition(id === 'temporary' ? `browser-${tabId}` : `persist:codeg-rovai-browser-${id}`)
      this.occupancy.set(id, (this.occupancy.get(id) ?? 0) + 1)
      let released = false
      return { profileId: id, session: value, release: () => { if (!released) { released = true; this.occupancy.set(id, Math.max(0, (this.occupancy.get(id) ?? 1) - 1)) } } }
    })
  }

  async control(input: Parameters<BrowserWorkspaceApi['profileControl']>[0]): Promise<Catalog> {
    return this.exclusive(async () => {
      if (input.expectedRevision !== this.catalog.revision) throw new Error('browser.profiles_changed')
      const profiles = this.catalog.profiles.map(row => ({ ...row }))
      if (input.action === 'create') {
        if (typeof input.name !== 'string' || !input.name.trim() || input.name.trim().length > 80 || profiles.length >= 32) throw new Error('browser.invalid_profile_name')
        profiles.push({ id: `p-${randomUUID()}`, name: input.name.trim(), createdAt: Date.now() })
      } else {
        const row = profiles.find(row => row.id === input.id)
        if (!row) throw new Error('browser.profile_not_found')
        if (input.action === 'rename') {
          if (typeof input.name !== 'string' || !input.name.trim() || input.name.trim().length > 80) throw new Error('browser.invalid_profile_name')
          row.name = input.name.trim()
        } else if (input.action === 'clear' || input.action === 'remove') {
          if (!input.confirmed || input.action === 'remove' && row.id === 'default') throw new Error('browser.profile_confirmation_required')
          const value = session.fromPartition(`persist:codeg-rovai-browser-${row.id}`)
          if ((this.occupancy.get(row.id) ?? 0) > 0 || this.hasDownloads(value)) throw new Error('browser.profile_in_use_close_pages_and_downloads_first')
          await value.closeAllConnections(); await value.clearAuthCache(); await value.clearStorageData(); await value.clearCache()
          if (input.action === 'remove') profiles.splice(profiles.indexOf(row), 1)
        } else throw new Error('browser.invalid_profile_action')
      }
      const next = { revision: randomUUID(), profiles }, temporary = `${this.file}.${randomUUID()}.tmp`
      try { await fs.writeFile(temporary, JSON.stringify(next), { encoding: 'utf8', mode: 0o600, flag: 'wx' }); await fs.rename(temporary, this.file) }
      finally { await fs.unlink(temporary).catch(() => undefined) }
      this.catalog = next
      return this.list()
    })
  }
}
