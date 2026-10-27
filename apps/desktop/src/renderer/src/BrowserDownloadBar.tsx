import { useEffect, useState } from 'react'
import { AlertTriangle, Check, Download, FolderOpen, Pause, Play, X } from 'lucide-react'
import type { BrowserDownloadView, BrowserWorkspaceApi } from '@contracts'
import { useUiText } from './interface-language'

const bytes = (value: number): string => value >= 1048576 ? `${(value / 1048576).toFixed(1)} MB` : value >= 1024 ? `${(value / 1024).toFixed(0)} KB` : `${value} B`

/** Coaxi BrowserDownloadBar: compact rows above content; no auto-open. */
export function BrowserDownloadBar({ api, threadId, tabId }: { api: BrowserWorkspaceApi; threadId: string; tabId: string | null }): React.JSX.Element | null {
  const t = useUiText()
  const [downloads, setDownloads] = useState<BrowserDownloadView[]>([])
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [all, setAll] = useState(false)
  useEffect(() => {
    let cancelled = false
    setDownloads([]); setError(null); setBusy(null)
    const refresh = (): void => { void api.downloads(threadId).then(value => { if (!cancelled) setDownloads(value) }).catch(failure => { if (!cancelled) setError(String(failure)) }) }
    refresh(); const timer = window.setInterval(refresh, 1000)
    return () => { cancelled = true; window.clearInterval(timer) }
  }, [api, threadId])
  const act = async (id: string, action: Parameters<BrowserWorkspaceApi['downloadControl']>[0]['action']): Promise<void> => {
    if (busy) return
    setBusy(id); setError(null)
    try { await api.downloadControl({ threadId, id, action }); setDownloads(await api.downloads(threadId)) }
    catch (failure) { setError(String(failure)) }
    finally { setBusy(null) }
  }
  const rows = downloads.filter(item => all || tabId === null || item.tabId === tabId)
  if (!downloads.length && !error) return null
  return <section className="browser-downloads" aria-label={t('浏览器下载')}>
    <div className="browser-downloads-heading"><Download size={14}/><span>{t('下载')}</span><button type="button" aria-pressed={all} onClick={() => setAll(value => !value)}>{t(all ? '当前页面' : '本会话所有下载')}{!all && downloads.length !== rows.length ? ` (${downloads.length})` : ''}</button></div>
    {error && <p className="workbench-error" role="alert">{error}</p>}
    {rows.map(item => {
      const live = ['progressing', 'paused', 'interrupted'].includes(item.state)
      const label = item.state === 'completed' ? '已完成' : item.state === 'cancelled' ? '已取消' : item.state === 'failed' ? '下载失败' : item.state === 'paused' ? '已暂停' : item.state === 'interrupted' ? '连接中断' : '正在下载'
      return <div className="browser-download-row" key={item.id}>
        {item.state === 'completed' ? <Check size={14}/> : item.state === 'failed' || item.state === 'interrupted' ? <AlertTriangle size={14}/> : <Download size={14}/>}
        <span className="browser-download-name" title={item.path || item.fileName}>{item.fileName}</span>
        <span className="browser-download-status">{t(label)}{live ? ` · ${bytes(item.receivedBytes)}${item.totalBytes > 0 ? ' / ' + bytes(item.totalBytes) : ''}` : ''}</span>
        {live && item.totalBytes > 0 && <progress aria-label={t('下载进度')} value={item.receivedBytes} max={item.totalBytes}/>}
        {item.state === 'progressing' && <button type="button" disabled={busy !== null} aria-label={t('暂停下载')} onClick={() => void act(item.id, 'pause')}><Pause size={14}/></button>}
        {(item.state === 'paused' || item.state === 'interrupted' && item.canResume) && <button type="button" disabled={busy !== null} aria-label={t('继续下载')} title={t('继续下载；服务器不支持断点时可能重新下载')} onClick={() => void act(item.id, 'resume')}><Play size={14}/></button>}
        {live && <button type="button" disabled={busy !== null} onClick={() => void act(item.id, 'cancel')}>{t('取消')}</button>}
        {item.state === 'completed' && <button type="button" disabled={busy !== null} onClick={() => void act(item.id, 'reveal')}><FolderOpen size={14}/>{t('在文件夹中显示')}</button>}
        <button type="button" disabled={busy !== null} aria-label={t('隐藏下载记录')} title={t('只隐藏记录，保留文件和正在进行的下载')} onClick={() => void act(item.id, 'dismiss')}><X size={14}/></button>
      </div>
    })}
  </section>
}
