import { useEffect, useState } from 'react'
import * as DropdownMenu from '@radix-ui/react-dropdown-menu'
import * as Popover from '@radix-ui/react-popover'
import { Bell, ChartNoAxesColumn, CircleAlert, FolderOpen, Grid2X2, Monitor, Plug, Rocket, Settings, Sparkles, Square } from 'lucide-react'
import type { AppUpdateSnapshot, NotificationActionView, NotificationEpisodeInbox, NotificationEpisodeView, SettingsSection } from '@contracts'
import { appUpdateBadgePresentation } from './app-update-presentation'
import { readErrorMessage } from './error-message'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { WorkspaceToolItems, requestWorkspaceTool } from './WorkspaceToolMenu'

type ComputerStatus = { enabled: boolean; installed: boolean; driverVersion: string; backend: { state: string }; shared: Array<{ targetId: string; level: string }> }
type ServiceStatus = { peer: unknown; pendingResults: number; computerAvailable: boolean; lastError: string | null }
export function CoaxiStatusBar({ threadId, sessionCount, disabled, updates, onSettings, onUpdates, onOpenFolder, onProjectLauncher, onNotification }: {
  threadId: string | null; sessionCount: number; disabled?: boolean; updates: AppUpdateSnapshot | null
  onSettings(section?: SettingsSection): void; onUpdates(): void; onOpenFolder(): void; onProjectLauncher(): void
  onNotification(episode: NotificationEpisodeView, action: NotificationActionView): Promise<unknown>
}): React.JSX.Element {
  const client = useThreadClient(), t = useUiText()
  const [panel, setPanel] = useState<'computer' | 'service' | 'alerts' | null>(null)
  const [computer, setComputer] = useState<ComputerStatus | null>(null), [service, setService] = useState<ServiceStatus | null>(null)
  const [inbox, setInbox] = useState<NotificationEpisodeInbox | null>(null), [error, setError] = useState(''), [busy, setBusy] = useState(false)
  const badge = appUpdateBadgePresentation(updates)
  useEffect(() => {
    if (!panel) return
    let current = true
    setError(''); setComputer(null); setService(null); setInbox(null); setBusy(true)
    const request = panel === 'computer'
      ? threadId && client.computerWorkspace ? client.computerWorkspace.request<ComputerStatus>(threadId, { operation: 'status' }).then(value => { if (current) setComputer(value) }) : Promise.reject(new Error(t('先打开一个对话，再选择共享窗口。')))
      : panel === 'service'
        ? client.interactionWorkspace ? client.interactionWorkspace.request<ServiceStatus>('status', {}).then(value => { if (current) setService(value) }) : Promise.reject(new Error(t('当前连接未提供本机工具服务。')))
        : client.request<NotificationEpisodeInbox>('notifications.inbox', { filter: 'unread', limit: 50 }).then(value => { if (current) setInbox(value) })
    void request.catch(failure => { if (current) setError(readErrorMessage(failure)) }).finally(() => { if (current) setBusy(false) })
    return () => { current = false }
  }, [panel, threadId, client])
  return <footer className="coaxi-statusbar" aria-label={t('底部工作条')}>
    <div className="coaxi-status-leading">
      <DropdownMenu.Root><DropdownMenu.Trigger asChild><button type="button" className="workspace-strip-icon" disabled={disabled} aria-label={t('工作区与应用菜单')} title={t('工作区与应用菜单')}><Grid2X2 size={15} /></button></DropdownMenu.Trigger>
        <DropdownMenu.Portal><DropdownMenu.Content className="workspace-menu" align="start" side="top" sideOffset={8}>
          <DropdownMenu.Label className="workspace-menu-label">{t('工作区')}</DropdownMenu.Label>
          <DropdownMenu.Item className="workspace-menu-item" onSelect={onOpenFolder}><FolderOpen size={16} />{t('打开工作目录')}</DropdownMenu.Item>
          <DropdownMenu.Item className="workspace-menu-item" onSelect={onProjectLauncher}><Rocket size={16} />{t('项目启动器')}</DropdownMenu.Item>
          <DropdownMenu.Separator className="workspace-menu-separator" />
          <DropdownMenu.Label className="workspace-menu-label">{t(threadId ? '当前对话' : '打开对话后可使用以下工具')}</DropdownMenu.Label>
          <WorkspaceToolItems disabled={!threadId} onSelect={tool => { if (threadId) requestWorkspaceTool(threadId, tool) }} />
          <DropdownMenu.Separator className="workspace-menu-separator" />
          <DropdownMenu.Item className="workspace-menu-item" onSelect={() => onSettings()}><Settings size={16} />{t('设置')}</DropdownMenu.Item>
        </DropdownMenu.Content></DropdownMenu.Portal>
      </DropdownMenu.Root>
      <span className="coaxi-status-stat"><ChartNoAxesColumn size={14} />{t('{0} 个会话', String(sessionCount))}</span>
    </div>
    <div className="coaxi-status-trailing">
      <button type="button" className={`coaxi-status-update${badge ? ' has-update' : ''}`} title={badge?.accessibleLabel ?? t('关于与更新')} aria-label={badge?.accessibleLabel ?? t('关于与更新')} onClick={onUpdates}><Sparkles size={15} />{badge && <span>{badge.label}</span>}</button>
      {([{ key: 'computer', Icon: Monitor, label: '电脑共享' }, { key: 'service', Icon: Plug, label: '本机工具连接' }, { key: 'alerts', Icon: Bell, label: '提醒' }] as const).map(({ key, Icon, label }) => <Popover.Root key={key} open={panel === key} onOpenChange={open => setPanel(open ? key : null)}>
        <Popover.Trigger asChild><button type="button" className="workspace-strip-icon" aria-label={t(label)} title={t(label)} disabled={disabled}><Icon size={15} /></button></Popover.Trigger>
        <Popover.Portal><Popover.Content className="coaxi-status-popover" side="top" align="end" sideOffset={8} collisionPadding={12}>
          <h2>{t(label)}</h2>
          {busy && <p role="status">{t('正在读取…')}</p>}
          {error && <p role="alert" className="workbench-error">{error}</p>}
          {key === 'computer' && computer && <><p>{t('驱动')} {computer.driverVersion} · {t(computer.installed ? '已安装' : '未安装')} · {computer.backend.state}</p><div className="coaxi-status-summary"><strong>{computer.shared.length ? t('已共享 {0} 个窗口', String(computer.shared.length)) : t('未共享窗口')}</strong><p>{t('只读取或操作你明确共享的窗口。')}</p></div></>}
          {key === 'computer' && <div className="coaxi-status-actions"><button className="primary-button" type="button" disabled={!threadId} onClick={() => { setPanel(null); if (threadId) requestWorkspaceTool(threadId, 'computer') }}><Monitor size={15} />{t('选择共享窗口')}</button>{!!computer?.shared.length && <button className="quiet-button" type="button" onClick={() => { if (threadId) void client.computerWorkspace?.request(threadId, { operation: 'stop' }).then(() => setPanel(null)).catch(failure => setError(readErrorMessage(failure))) }}><Square size={14} />{t('停止此会话共享')}</button>}</div>}
          {key === 'service' && service && <><div className="coaxi-status-summary"><strong>{t(service.lastError ? '连接需要处理' : service.peer ? '本机工具服务已连接' : '本机工具服务尚未连接')}</strong><p>{t('待核对结果：{0}', String(service.pendingResults))}</p>{service.lastError && <p className="workbench-error">{service.lastError}</p>}</div><button type="button" className="quiet-button" onClick={() => { setPanel(null); onSettings('toolbox') }}><Settings size={15} />{t('打开工具箱设置')}</button></>}
          {key === 'alerts' && inbox && <div className="coaxi-status-alerts">{inbox.items.length ? inbox.items.map(episode => <button key={episode.id} type="button" disabled={!episode.primaryAction.available} onClick={() => { void onNotification(episode, episode.primaryAction).then(() => setPanel(null)).catch(failure => setError(readErrorMessage(failure))) }}><CircleAlert size={15} /><span>{episode.thread.title}<small>{t(({approval_pending:'等待你批准',user_mention:'有人提及你',turn_completed:'执行已完成',turn_failed:'执行失败',turn_incomplete:'执行需要继续',round_completed:'本轮已完成',single_chat_reply:'收到单聊回复',mission_needs_you:'使命需要你处理',mission_status_changed:'使命状态更新',task_status_changed:'任务状态更新'})[episode.primarySemantic])}</small></span></button>) : <p>{t('暂无未读提醒')}</p>}{inbox.nextCursor&&<button type="button" onClick={()=>{setBusy(true);void client.request<NotificationEpisodeInbox>('notifications.inbox',{filter:'unread',limit:50,cursor:inbox.nextCursor}).then(value=>setInbox(previous=>({...value,items:[...(previous?.items??[]),...value.items]}))).catch(failure=>setError(readErrorMessage(failure))).finally(()=>setBusy(false))}}>{t('加载更多提醒')}</button>}</div>}
        </Popover.Content></Popover.Portal>
      </Popover.Root>)}
    </div>
  </footer>
}
