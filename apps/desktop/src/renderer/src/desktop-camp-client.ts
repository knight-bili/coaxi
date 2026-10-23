import type { ThreadClient } from './camp-client'
import { dispatchShortcutAction } from './shortcut-action-driver'

const desktopBridge = () => typeof window === 'undefined' ? undefined : window.rovai

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
  get officeWorkspace() { return desktopBridge()?.officeWorkspace },
  get interactionComputer(){return desktopBridge()?.interactionComputer},
  get interactionWorkspace() { return desktopBridge()?.interactionWorkspace },
  get browserWorkspace() { return desktopBridge()?.browserWorkspace },
  get privateInputRecovery() { return desktopBridge()?.privateInputRecovery },
  exportMonitoring: async filter => { const path = await window.rovai.exportMonitoring(filter); return { exported: Boolean(path), ...(path ? { path } : {}) } },
  exportUsageActivityReport: async query => { const path = await window.rovai.exportUsageActivityReport?.(query); return {exported:Boolean(path),...(path?{path}:{})} },
  get usageCard() { return desktopBridge()?.usageCard },
  syncNativeHistoryUsage: input => window.rovai.request('history.usage.local',input),
  exportNativeHistoryUsage: async query => {const path=await window.rovai.exportNativeHistoryUsage?.(query);return {exported:Boolean(path),...(path?{path}:{})}},
  revealMonitoringExport: path => window.rovai.revealMonitoringExport(path),
  exportDiagnostics: async () => { const path = await window.rovai.exportDiagnostics(); return { exported: Boolean(path), ...(path ? { path } : {}) } },
  revealDiagnosticsExport: path => window.rovai.revealDiagnosticsExport(path),
  get memberAvatars() { return desktopBridge()?.memberAvatars },
  selectSkillImportDirectory: () => window.rovai.selectSkillImportDirectory(),
  selectRuntimeExecutable: () => window.rovai.selectRuntimeExecutable(),
  revealMcpConfig: () => window.rovai.revealMcpConfig(),
  channels: {
    get: () => window.rovai.channels.get(), onChanged: listener => window.rovai.channels.onChanged(listener),
    publishMemberBot: (agentId, kind) => window.rovai.channels.publishMemberBot(agentId, kind),
    retryMemberBot: (agentId, kind) => window.rovai.channels.retryMemberBot(agentId, kind),
    selectPublicationApprover: (agentId, userId, kind) => window.rovai.channels.selectPublicationApprover(agentId, userId, kind),
    native: {
      connect: kind => window.rovai.channels.connect(kind), disconnect: kind => window.rovai.channels.disconnect(kind),
      cancelQrAttempt: id => window.rovai.channels.cancelQrAttempt(id), refreshLoginQr: id => window.rovai.channels.refreshLoginQr(id)
    }
  },
  request: (method, params) => window.rovai.request(method, params),
  onEvent: (listener) => window.rovai.onEvent(listener),
  onClosePreviewRequested: listener => window.rovai.windowControls.onCloseTabRequested(()=>dispatchShortcutAction('close_current_tab')||listener()),
  get singleChatAttachments() { return desktopBridge()?.singleChatAttachments },
  get missionAttachments() { return desktopBridge()?.missionAttachments },
  get composerAttachments() { return desktopBridge()?.composerAttachments },
  attachmentLocation: async locator => {
    if (locator.owner === 'composer') {
      const path = await window.rovai.composerAttachments.location?.(locator)
      return path ? { path, location: 'local' } : null
    }
    const path = await window.rovai.request<string | null>('thread.attachments.location', locator)
    return path ? { path, location: 'local' } : null
  },
  attachments: { kind: 'native',
    open: locator => window.rovai.attachments.open(locator),
    reveal: locator => window.rovai.attachments.reveal(locator)
  },
  get platform() { return desktopBridge()?.platform }
}
