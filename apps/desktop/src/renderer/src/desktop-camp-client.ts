import type { ThreadClient } from './camp-client'
import { dispatchShortcutAction } from './shortcut-action-driver'

const desktopBridge = () => typeof window === 'undefined' ? undefined : window.coaxi

/** Lazy Desktop-only compatibility adapter; importing it never accesses Electron. */
export const desktopThreadClient: ThreadClient = {
  presentationScope: 'desktop',
  get computerWorkspace() { return desktopBridge()?.computerWorkspace },
  get nativePets() { return desktopBridge()?.nativePets },
  get webdavWorkspace() { return desktopBridge()?.webdavWorkspace },
  get nativeBackups() { return desktopBridge()?.nativeBackups },
  get portableConfiguration() { return desktopBridge()?.portableConfiguration },
  get keyboardShortcuts(){return desktopBridge()?.keyboardShortcuts},
  get workbenchEditorRecovery(){return desktopBridge()?.workbenchEditorRecovery},
  get nativeCustomSkills() { return desktopBridge()?.nativeCustomSkills },
  get nativeHistory() { return desktopBridge()?.nativeHistory },
  get projectLauncher(){return desktopBridge()?.projectLauncher},
  get officeWorkspace() { return desktopBridge()?.officeWorkspace },
  get interactionComputer(){return desktopBridge()?.interactionComputer},
  get interactionWorkspace() { return desktopBridge()?.interactionWorkspace },
  get browserWorkspace() { return desktopBridge()?.browserWorkspace },
  get privateInputRecovery() { return desktopBridge()?.privateInputRecovery },
  exportMonitoring: async filter => { const path = await window.coaxi.exportMonitoring(filter); return { exported: Boolean(path), ...(path ? { path } : {}) } },
  exportUsageActivityReport: async query => { const path = await window.coaxi.exportUsageActivityReport?.(query); return {exported:Boolean(path),...(path?{path}:{})} },
  get usageCard() { return desktopBridge()?.usageCard },
  syncNativeHistoryUsage: input => window.coaxi.request('history.usage.local',input),
  exportNativeHistoryUsage: async query => {const path=await window.coaxi.exportNativeHistoryUsage?.(query);return {exported:Boolean(path),...(path?{path}:{})}},
  revealMonitoringExport: path => window.coaxi.revealMonitoringExport(path),
  exportDiagnostics: async () => { const path = await window.coaxi.exportDiagnostics(); return { exported: Boolean(path), ...(path ? { path } : {}) } },
  revealDiagnosticsExport: path => window.coaxi.revealDiagnosticsExport(path),
  get memberAvatars() { return window.coaxi.memberAvatars },
  selectSkillImportDirectory: () => window.coaxi.selectSkillImportDirectory(),
  selectRuntimeExecutable: () => window.coaxi.selectRuntimeExecutable(),
  revealMcpConfig: () => window.coaxi.revealMcpConfig(),
  channels: {
    get: () => window.coaxi.channels.get(), onChanged: listener => window.coaxi.channels.onChanged(listener),
    publishMemberBot: (agentId, kind) => window.coaxi.channels.publishMemberBot(agentId, kind),
    retryMemberBot: (agentId, kind) => window.coaxi.channels.retryMemberBot(agentId, kind),
    selectPublicationApprover: (agentId, userId, kind) => window.coaxi.channels.selectPublicationApprover(agentId, userId, kind),
    native: {
      connect: kind => window.coaxi.channels.connect(kind), disconnect: kind => window.coaxi.channels.disconnect(kind),
      cancelQrAttempt: id => window.coaxi.channels.cancelQrAttempt(id), refreshLoginQr: id => window.coaxi.channels.refreshLoginQr(id)
    }
  },
  request: (method, params) => window.coaxi.request(method, params),
  onEvent: (listener) => window.coaxi.onEvent(listener),
  onClosePreviewRequested: listener => window.coaxi.windowControls.onCloseTabRequested(()=>dispatchShortcutAction('close_current_tab')||listener()),
  get singleChatAttachments() { return window.coaxi.singleChatAttachments },
  get missionAttachments() { return desktopBridge()?.missionAttachments ?? null },
  get composerAttachments() { return window.coaxi.composerAttachments },
  attachmentLocation: async locator => {
    if (locator.owner === 'composer') {
      const path = await window.coaxi.composerAttachments.location?.(locator)
      return path ? { path, location: 'local' } : null
    }
    const path = await window.coaxi.request<string | null>('thread.attachments.location', locator)
    return path ? { path, location: 'local' } : null
  },
  attachments: { kind: 'native',
    open: locator => window.coaxi.attachments.open(locator),
    reveal: locator => window.coaxi.attachments.reveal(locator)
  },
  get platform() { return window.coaxi.platform }
}
