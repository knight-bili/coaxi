import { app, shell, type DownloadItem, type Session, type WebContents } from 'electron'
import { randomUUID } from 'node:crypto'
import { promises as fs } from 'node:fs'
import path from 'node:path'
import type { BrowserDownloadView, BrowserWorkspaceApi } from '@contracts'

type Entry = { view: BrowserDownloadView; item?: DownloadItem; session?: Session; hidden: boolean }
const LIVE = new Set<BrowserDownloadView['state']>(['progressing', 'paused', 'interrupted'])
const HISTORY_LIMIT = 32
const ACTIVE_LIMIT = 16

// Codeg browser-downloads-store and BrowserDownloadBar: closing a tab does not
// erase a download; dismissing a record never deletes the downloaded file.
export class BrowserDownloads {
  private readonly entries = new Map<string, Entry>()
  private readonly file = path.join(app.getPath('userData'), 'browser-downloads.json')
  private readonly ready: Promise<void>
  private tail: Promise<void> = Promise.resolve()
  private disposed = false
  private readonly bindings = new Map<Session, { owners: Map<WebContents, {threadId: string; tabId: string}>; listener: (event: Electron.Event, item: DownloadItem, source: WebContents) => void }>()
  hasActive(value: Session): boolean { return [...this.entries.values()].some(entry => entry.item && entry.session === value) }

  constructor() { this.ready = this.load() }

  private async load(): Promise<void> {
    try {
      const stat = await fs.lstat(this.file)
      if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 262144) return
      const value: unknown = JSON.parse(await fs.readFile(this.file, 'utf8'))
      if (!Array.isArray(value)) return
      for (const item of value.slice(-HISTORY_LIMIT)) {
        if (!item || typeof item !== 'object') continue
        const row = item as BrowserDownloadView
        if (typeof row.id !== 'string' || row.id.length > 100 || typeof row.threadId !== 'string' || row.threadId.length > 200 || typeof row.tabId !== 'string' || typeof row.fileName !== 'string' || row.fileName.length > 1024 || typeof row.path !== 'string' || (row.path !== '' && !path.isAbsolute(row.path)) || !['completed', 'cancelled', 'failed'].includes(row.state) || !Number.isFinite(row.startedAt) || !Number.isFinite(row.updatedAt) || !Number.isSafeInteger(row.receivedBytes) || row.receivedBytes < 0 || !Number.isSafeInteger(row.totalBytes) || row.totalBytes < 0) continue
        if (!this.entries.has(row.id)) this.entries.set(row.id, { view: { id: row.id, threadId: row.threadId, tabId: row.tabId, fileName: row.fileName, path: row.path, state: row.state, startedAt: row.startedAt, updatedAt: row.updatedAt, receivedBytes: row.receivedBytes, totalBytes: row.totalBytes, canResume: false }, hidden: false })
      }
    } catch { /* Download history is optional; never affect saved files. */ }
  }

  private persist(): void {
    this.tail = this.tail.catch(() => undefined).then(async () => {
      await this.ready
      const records = [...this.entries.values()].filter(entry => !entry.item && !entry.hidden).map(entry => entry.view).sort((a, b) => a.updatedAt - b.updatedAt).slice(-HISTORY_LIMIT)
      const temporary = `${this.file}.${randomUUID()}.tmp`
      try { await fs.writeFile(temporary, JSON.stringify(records), { encoding: 'utf8', mode: 0o600, flag: 'wx' }); await fs.rename(temporary, this.file) }
      finally { await fs.unlink(temporary).catch(() => undefined) }
    }).catch(() => undefined)
  }

  private trim(): void {
    const history = [...this.entries.values()].filter(entry => !entry.item).sort((a, b) => b.view.updatedAt - a.view.updatedAt)
    for (const entry of history.slice(HISTORY_LIMIT)) this.entries.delete(entry.view.id)
  }

  bind(session: Session, contents: WebContents, threadId: string, tabId: string): () => void {
    const existing = this.bindings.get(session)
    if (existing) { existing.owners.set(contents, {threadId, tabId}); return () => { existing.owners.delete(contents); if (existing.owners.size === 0) { session.removeListener('will-download', existing.listener); this.bindings.delete(session) } } }
    const owners = new Map<WebContents, {threadId: string; tabId: string}>([[contents, {threadId, tabId}]])
    const listener = (event: Electron.Event, item: DownloadItem, source: WebContents): void => {
      const owner = owners.get(source)
      if (this.disposed || !owner || source.isDestroyed() || [...this.entries.values()].filter(entry => entry.item).length >= ACTIVE_LIMIT) { event.preventDefault(); return }
      try {
        const sourceOrigin = new URL(source.getURL()).origin
        for (const value of item.getURLChain()) {
          const url = new URL(value)
          if (url.username || url.password || (!['http:', 'https:'].includes(url.protocol) && !(url.protocol === 'blob:' && url.origin === sourceOrigin && /^https?:/.test(sourceOrigin)))) throw new Error('browser.invalid_download')
        }
      } catch { event.preventDefault(); return }
      // Synchronous will-download callback: Chromium displays the native Save
      // dialog. Renderer and web page cannot choose an arbitrary output path.
      item.setSaveDialogOptions({ title: '保存下载文件', buttonLabel: '保存' })
      const startedAt = Date.now(), id = randomUUID()
      const entry: Entry = { item, session, hidden: false, view: { id, threadId: owner.threadId, tabId: owner.tabId, fileName: item.getFilename().slice(0, 1024), path: '', startedAt, updatedAt: startedAt, state: 'progressing', receivedBytes: 0, totalBytes: Math.max(0, item.getTotalBytes()), canResume: false } }
      this.entries.set(id, entry)
      const update = (state: string): void => {
        entry.view = { ...entry.view, path: item.getSavePath(), receivedBytes: Math.max(0, item.getReceivedBytes()), totalBytes: Math.max(0, item.getTotalBytes()), state: state === 'interrupted' ? 'interrupted' : item.isPaused() ? 'paused' : 'progressing', canResume: item.canResume(), updatedAt: Date.now() }
      }
      item.on('updated', (_event, state) => update(state))
      item.once('done', (_event, state) => {
        update(state)
        entry.view.state = state === 'completed' ? 'completed' : state === 'cancelled' ? 'cancelled' : 'failed'
        entry.view.canResume = false
        delete entry.item
        delete entry.session
        if (entry.hidden) this.entries.delete(id)
        this.trim(); this.persist()
      })
    }
    session.on('will-download', listener)
    this.bindings.set(session, {owners, listener})
    return () => { owners.delete(contents); if (owners.size === 0) { session.removeListener('will-download', listener); this.bindings.delete(session) } }
  }

  async list(threadId: string): Promise<BrowserDownloadView[]> {
    await this.ready
    return [...this.entries.values()].filter(entry => !entry.hidden && entry.view.threadId === threadId).map(entry => ({ ...entry.view })).sort((a, b) => b.startedAt - a.startedAt)
  }

  async control(input: Parameters<BrowserWorkspaceApi['downloadControl']>[0]): Promise<void> {
    await this.ready
    const entry = this.entries.get(input.id)
    if (!entry || input.threadId !== entry.view.threadId || this.disposed) throw new Error('browser.download_not_found')
    switch (input.action) {
      case 'pause': if (!entry.item || entry.view.state !== 'progressing') throw new Error('browser.download_not_active'); entry.item.pause(); entry.view.state = 'paused'; break
      case 'resume': if (!entry.item || !LIVE.has(entry.view.state) || (entry.view.state !== 'paused' && !entry.item.canResume())) throw new Error('browser.download_cannot_resume'); entry.item.resume(); entry.view.state = 'progressing'; break
      case 'cancel': if (!entry.item) throw new Error('browser.download_not_active'); entry.item.cancel(); break
      case 'dismiss': entry.hidden = true; if (!entry.item) this.entries.delete(input.id); this.persist(); break
      case 'reveal': {
        if (entry.view.state !== 'completed' || !path.isAbsolute(entry.view.path)) throw new Error('browser.download_not_completed')
        const stat = await fs.stat(entry.view.path)
        if (!stat.isFile()) throw new Error('browser.download_missing')
        shell.showItemInFolder(entry.view.path); break
      }
      default: throw new Error('browser.invalid_download_action')
    }
  }

  close(): void { this.disposed = true; for (const entry of this.entries.values()) entry.item?.cancel() }
}
