import { randomUUID } from 'node:crypto'
import { WebContentsView, type BrowserWindow, type BrowserWindowConstructorOptions } from 'electron'

/** Host-owned sandboxed page. Visibility and bounds come only from the local renderer. */
export class BrowserNativeSurface {
  readonly native: WebContentsView
  readonly webContents: Electron.WebContents
  private visible = false
  private bounds: { x: number; y: number; width: number; height: number } | null = null
  constructor(options: BrowserWindowConstructorOptions, private readonly owner: BrowserWindow) {
    this.native = new WebContentsView({ webPreferences: options.webPreferences })
    this.webContents = this.native.webContents
    this.native.setBackgroundColor('#ffffff')
    this.native.setBounds({ x: 0, y: 0, width: 1100, height: 780 })
    owner.contentView.addChildView(this.native)
    this.native.setVisible(false)
    const hide = (): void => this.present(null)
    owner.on('hide', hide); owner.on('minimize', hide)
    this.webContents.once('destroyed', () => {
      owner.removeListener('hide', hide); owner.removeListener('minimize', hide)
      if (!owner.isDestroyed()) owner.contentView.removeChildView(this.native)
    })
  }
  isDestroyed(): boolean { return this.webContents.isDestroyed() }
  loadURL(url: string): Promise<void> { return this.webContents.loadURL(url) }
  // A page never raises itself over an application menu or another tab.
  show(): void {}
  focus(): void { if (this.visible && !this.isDestroyed()) this.webContents.focus() }
  once(event: 'closed' | 'ready-to-show', listener: () => void): void {
    if (event === 'closed') this.webContents.once('destroyed', listener)
  }
  close(): void { this.present(null); if (!this.isDestroyed()) this.webContents.close() }
  destroy(): void { this.close() }
  present(bounds: { x: number; y: number; width: number; height: number } | null): void {
    if (this.isDestroyed() || this.owner.isDestroyed()) return
    if (!bounds || !this.owner.isVisible() || this.owner.isMinimized()) {
      this.setVisible(false)
      return
    }
    const scale = this.owner.webContents.getZoomFactor(), available = this.owner.getContentBounds()
    const x = Math.max(0, Math.round(bounds.x * scale)), y = Math.max(0, Math.round(bounds.y * scale))
    const width = Math.min(Math.round(bounds.width * scale), available.width - x)
    const height = Math.min(Math.round(bounds.height * scale), available.height - y)
    if (width < 2 || height < 2) { this.setVisible(false); return }
    if (!this.bounds || this.bounds.x !== x || this.bounds.y !== y || this.bounds.width !== width || this.bounds.height !== height) {
      this.bounds = { x, y, width, height }
      this.native.setBounds(this.bounds)
    }
    this.setVisible(true)
  }
  private setVisible(visible: boolean): void {
    if (this.visible === visible) return
    this.visible = visible
    this.native.setVisible(visible)
  }
}

const WORLD = 998
const picker = `(() => {
  globalThis.__coaxiPicker?.cancel?.();
  const state = globalThis.__coaxiPicker = { result: null, cancelled: false };
  const box = document.createElement('div');
  Object.assign(box.style, {position:'fixed',pointerEvents:'none',zIndex:'2147483647',border:'2px solid #9865ef',background:'rgba(152,101,239,.08)',boxSizing:'border-box'});
  document.documentElement.appendChild(box);
  const move = event => {
    const target = event.target; if (!(target instanceof Element)) return;
    const rect = target.getBoundingClientRect();
    Object.assign(box.style,{left:rect.left+'px',top:rect.top+'px',width:rect.width+'px',height:rect.height+'px'});
  };
  const cleanup = () => { box.remove(); document.removeEventListener('pointermove',move,true); document.removeEventListener('click',select,true); document.removeEventListener('keydown',key,true); };
  const select = event => {
    event.preventDefault(); event.stopImmediatePropagation();
    const target = event.target;
    if(target instanceof Element) { const r=target.getBoundingClientRect();state.result = {tag:target.tagName.toLowerCase(),text:(target.textContent||'').trim().slice(0,3000),html:target.outerHTML.slice(0,6000),rect:{x:Math.max(0,Math.floor(r.x)),y:Math.max(0,Math.floor(r.y)),width:Math.max(0,Math.ceil(Math.min(r.right,innerWidth)-Math.max(0,r.x))),height:Math.max(0,Math.ceil(Math.min(r.bottom,innerHeight)-Math.max(0,r.y)))}}; }
    cleanup();
  };
  const key = event => { if(event.key==='Escape'){event.preventDefault();state.cancelled=true;cleanup();} };
  state.cancel = () => { state.cancelled=true;cleanup(); };
  document.addEventListener('pointermove',move,true);document.addEventListener('click',select,true);document.addEventListener('keydown',key,true);
  return true;
})()`

export async function inspectBrowserPage(contents: Electron.WebContents, action: string, expectedDocument?: string): Promise<Record<string, unknown>> {
  const url = contents.getURL()
  const read = (code: string): Promise<unknown> => contents.executeJavaScriptInIsolatedWorld(WORLD, [{ code }], true)
  const identity = ():Promise<unknown> => read('globalThis.__coaxiDocumentIdentity ??= '+JSON.stringify(randomUUID()))
  if (!/^https?:/.test(url)) throw new Error('browser.document_changed')
  const documentId = String(await identity())
  if (expectedDocument && expectedDocument !== documentId) throw new Error('browser.document_changed')
  let value: Record<string, unknown>
  if (action === 'screenshot') value = { mime: 'image/png', data: (await contents.capturePage()).toPNG().toString('base64') }
  else if (action === 'page') value = { text: await read("(document.body?.innerText || '').slice(0,16000)") }
  else if (action === 'pick_start') { await read(picker); value = { started: true } }
  else if (action === 'pick_read') {
    value = await read("({result:globalThis.__coaxiPicker?.result??null,cancelled:!globalThis.__coaxiPicker||globalThis.__coaxiPicker.cancelled})") as Record<string, unknown>
    const result = value.result as { rect?: { x: number; y: number; width: number; height: number } } | null
    if (result?.rect && result.rect.width > 0 && result.rect.height > 0) value.data = (await contents.capturePage(result.rect)).toPNG().toString('base64')
  }
  else if (action === 'pick_cancel') { await read('globalThis.__coaxiPicker?.cancel?.()'); value = { cancelled: true } }
  else throw new Error('browser.operation_not_admitted')
  if (contents.isDestroyed() || contents.getURL() !== url || String(await identity()) !== documentId) throw new Error('browser.document_changed')
  return { ...value, url, documentId, title: contents.getTitle() }
}
