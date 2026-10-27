import {withHostProjectGroups} from '../shared/host-navigation-groups'
import { contextBridge, ipcRenderer, webUtils } from 'electron'
import { createCloseTabShortcutHandler } from '../shared/close-tab-shortcut'
import { createWindowCloseApi } from './window-close-api'
import {
  APP_PREPARE_QUIT_CHANNEL,
  type AppQuitPreparationResponse
} from '../shared/app-lifecycle'
import type {
  NewConversationDefaults,
  AppearancePreferences,
  AppearanceSnapshot,
  AppUpdateSnapshot,
  ChannelSettingsSnapshot,
  CoreEvent,
  CoreMethod,
  ExecutionWebSettingsSnapshot,
  ExecutionConsolePlacement,
  InterfaceLanguage,
  RestorableLocation,
  SettingsSection,
  StartupLocationMode,
  NavigationPin,
  NavigationPreferencesSnapshot,
  CoaxiRequestTransport,
  CoaxiApi,
  SupervisorSnapshot,
  ThemePreference
} from '@contracts'

let appQuitPreparationListener: (() => void | Promise<void>) | null = null

ipcRenderer.on(APP_PREPARE_QUIT_CHANNEL, (event) => {
  const port = event.ports[0]
  if (!port) return
  void (async () => {
    let response: AppQuitPreparationResponse
    try {
      await appQuitPreparationListener?.()
      response = { status: 'prepared' }
    } catch (error) {
      response = {
        status: 'failed',
        message: error instanceof Error ? error.message : String(error)
      }
    }
    try {
      port.postMessage(response)
    } finally {
      port.close()
    }
  })().catch(() => {
    port.close()
  })
})

async function invokeHostWeb<T>(operation: string, input?: unknown): Promise<T> {
  const transport = await ipcRenderer.invoke('coaxi:host-web', operation, input) as CoaxiRequestTransport<T>
  // Reuse the cloneable failure envelope: throwing in Main would prepend the
  // Electron remote-method exception and discard the domain error fields.
  if (transport.kind === 'failure') return Promise.reject(transport.failure)
  return transport.value
}

const api: CoaxiApi = {
  deepLinks:{
    peek:()=>ipcRenderer.invoke('coaxi:deep-links','peek'),
    acknowledge:requestId=>ipcRenderer.invoke('coaxi:deep-links','acknowledge',requestId),
    onAvailable(listener){const handler=():void=>listener();ipcRenderer.on('coaxi:deep-link-available',handler);return()=>ipcRenderer.removeListener('coaxi:deep-link-available',handler)}
  },
  portableConfiguration:{finishMigrationFiles:commandId=>ipcRenderer.invoke('coaxi:portable-configuration-private','filesFinish',{commandId}),templates:input=>ipcRenderer.invoke('coaxi:portable-configuration-private','templates',input),logging:input=>ipcRenderer.invoke('coaxi:portable-configuration-private','logging',input),sync:input=>ipcRenderer.invoke('coaxi:portable-configuration-private','sync',input),prepareExport:input=>ipcRenderer.invoke('coaxi:portable-configuration-private','exportPrepare',input),exportReceipt:operationId=>ipcRenderer.invoke('coaxi:portable-configuration-private','exportReceipt',{operationId}),saveExport:operationId=>ipcRenderer.invoke('coaxi:portable-configuration-private','exportSave',{operationId}),providerForEdit:reference=>ipcRenderer.invoke('coaxi:portable-configuration-private','edit',reference),prepareSecret:input=>ipcRenderer.invoke('coaxi:portable-configuration-private','secret',input),archiveReceipt:operationId=>ipcRenderer.invoke('coaxi:portable-configuration-private','receipt',{operationId}),selectArchive:()=>ipcRenderer.invoke('coaxi:portable-configuration-private','select',{}),importSelected:input=>ipcRenderer.invoke('coaxi:portable-configuration-private','import',input)},
  nativePets:{importDirectory:()=>ipcRenderer.invoke('coaxi:pets-import'),synchronize:()=>ipcRenderer.invoke('coaxi:pets-synchronize'),onChanged:listener=>{const receive=():void=>listener();ipcRenderer.on('coaxi:pets-changed',receive);return()=>ipcRenderer.removeListener('coaxi:pets-changed',receive)}},
  webdavWorkspace:{request:input=>ipcRenderer.invoke('coaxi:webdav-workspace',input)},
  nativeBackups: {
    restoreEnvironment:()=>ipcRenderer.invoke('coaxi:backup-workspace','restore_environment',{}),
    inspectLegacyData:()=>ipcRenderer.invoke('coaxi:backup-workspace','legacy_inspect',{passphrase:''}),
    importLegacyData:selectionId=>ipcRenderer.invoke('coaxi:backup-workspace','legacy_stage',{passphrase:'',selectionId}),
    exportConfiguration:passphrase=>ipcRenderer.invoke('coaxi:backup-workspace','config_export',{passphrase}),
    inspectConfiguration:passphrase=>ipcRenderer.invoke('coaxi:backup-workspace','config_inspect',{passphrase}),
    applyConfiguration:(selectionId,passphrase)=>ipcRenderer.invoke('coaxi:backup-workspace','config_apply',{selectionId,passphrase}),
    create:passphrase=>ipcRenderer.invoke('coaxi:backup-workspace','create',{passphrase}),
    inspect:passphrase=>ipcRenderer.invoke('coaxi:backup-workspace','inspect',{passphrase}),
    restore:(archiveId,passphrase)=>ipcRenderer.invoke('coaxi:backup-workspace','restore',{archiveId,passphrase}),
    onProgress:listener=>{const receive=(_event:Electron.IpcRendererEvent,value:{phase:string;done:number;total:number}):void=>listener(value);ipcRenderer.on('coaxi:backup-progress',receive);return()=>ipcRenderer.removeListener('coaxi:backup-progress',receive)}
  },
  keyboardShortcuts:{get:()=>ipcRenderer.invoke('coaxi:keyboard-shortcuts','get'),save:input=>ipcRenderer.invoke('coaxi:keyboard-shortcuts','save',input),recording:value=>ipcRenderer.invoke('coaxi:keyboard-shortcuts','recording',value),onNativeInput:listener=>{const receive=(_event:Electron.IpcRendererEvent,input:import('@contracts').NativeShortcutInput):void=>listener(input);ipcRenderer.on('coaxi:keyboard-input',receive);return()=>ipcRenderer.removeListener('coaxi:keyboard-input',receive)}},
  nativeCustomSkills: {importDirectory:input=>ipcRenderer.invoke('coaxi:custom-skill-import',input)},
  nativeHistory: {export:(threadId,importId,expectedRevision)=>ipcRenderer.invoke('coaxi:native-history','export',{threadId,importId,expectedRevision}),scan:(threadId,provider)=>ipcRenderer.invoke('coaxi:native-history','scan',{threadId,provider}),request:(threadId,input)=>ipcRenderer.invoke('coaxi:native-history','request',{threadId,input})},
  projectLauncher: {
    open:()=>ipcRenderer.invoke('coaxi:project-launcher','open'),
    peek:()=>ipcRenderer.invoke('coaxi:project-launcher','peek'),
    acknowledge:id=>ipcRenderer.invoke('coaxi:project-launcher','acknowledge',id),
    onAvailable:listener=>{const handler=():void=>listener();ipcRenderer.on('coaxi:project-launcher-available',handler);return()=>ipcRenderer.removeListener('coaxi:project-launcher-available',handler)}
  },
  officeWorkspace: {open:(threadId,path,editable,embedded)=>ipcRenderer.invoke('coaxi:office-workspace',{threadId,path,editable,embedded}),present:input=>ipcRenderer.invoke('coaxi:office-workspace',{...input,operation:'present'}),inspect:input=>ipcRenderer.invoke('coaxi:office-workspace',{...input,operation:'inspect'}),close:id=>ipcRenderer.invoke('coaxi:office-workspace',{id,operation:'close'})},
  computerWorkspace: { request: (threadId,input)=>ipcRenderer.invoke('coaxi:computer-workspace',{threadId,input,commandId:globalThis.crypto.randomUUID()}) },
  interactionComputer:{request:(method,input)=>ipcRenderer.invoke('coaxi:interaction-computer',method,input)},
  interactionWorkspace: {request:(action,payload)=>ipcRenderer.invoke('coaxi:interaction-workspace',action,payload)},
  workbenchEditorRecovery:{get:key=>ipcRenderer.invoke('coaxi:workbench-editor-recovery','get',{key}),put:value=>ipcRenderer.invoke('coaxi:workbench-editor-recovery','put',value),copies:(key,after)=>ipcRenderer.invoke('coaxi:workbench-editor-recovery','copies',{key,after})},
  browserWorkspace: {
    present: input => ipcRenderer.invoke('coaxi:browser-workspace','present',input),
    inspect: input => ipcRenderer.invoke('coaxi:browser-workspace','inspect',input),
    preferences: () => ipcRenderer.invoke('coaxi:browser-workspace','preferences'),
    updatePreferences: input => ipcRenderer.invoke('coaxi:browser-workspace','updatePreferences',input),
    profiles: () => ipcRenderer.invoke('coaxi:browser-workspace','profiles'),
    profileControl: input => ipcRenderer.invoke('coaxi:browser-workspace','profileControl',input),
    downloads: threadId => ipcRenderer.invoke('coaxi:browser-workspace','downloads',threadId),
    downloadControl: input => ipcRenderer.invoke('coaxi:browser-workspace','downloadControl',input),
    list: threadId => ipcRenderer.invoke('coaxi:browser-workspace','list',threadId),
    open: (threadId,url,profileId) => ipcRenderer.invoke('coaxi:browser-workspace','open',{threadId,url,profileId}),
    control: input => ipcRenderer.invoke('coaxi:browser-workspace','control',input),
    perform: input => ipcRenderer.invoke('coaxi:browser-workspace','perform',input)
  },
  privateInputRecovery: {
    get(identity) {
      const reply = ipcRenderer.sendSync('coaxi:private-input-recovery', 'get', identity)
      if (!reply?.ok) throw new Error(reply?.error ?? '私聊输入无法读取。')
      return reply.value
    },
    set(identity, value) {
      const reply = ipcRenderer.sendSync('coaxi:private-input-recovery', 'set', identity, value)
      if (!reply?.ok) throw new Error(reply?.error ?? '私聊输入未可靠保存。')
    }
  },
  windowClose: createWindowCloseApi(ipcRenderer, process.platform),
  hostWeb: {
    loginTicket: () => invokeHostWeb('loginTicket'),
    token: () => invokeHostWeb('token'),
    status: () => invokeHostWeb('status'),
    start: (input) => invokeHostWeb('start', input),
    stop: () => invokeHostWeb('stop'),
    rotate: () => invokeHostWeb('rotate')
  },
  async request<T>(method: CoreMethod, params?: unknown): Promise<T> {
    const transport = await ipcRenderer.invoke(
      'coaxi:request',
      method,
      params
    ) as CoaxiRequestTransport<T>
    if (transport.kind === 'failure') {
      // contextBridge drops custom Error properties; keep the rejection cloneable.
      // Any Error instance must be constructed after the Renderer receives it.
      return Promise.reject(transport.failure)
    }
    return transport.value
  },
  onEvent(listener: (event: CoreEvent) => void): () => void {
    const handler = (_event: Electron.IpcRendererEvent, value: CoreEvent): void => listener(value)
    ipcRenderer.on('coaxi:event', handler)
    return () => ipcRenderer.removeListener('coaxi:event', handler)
  },
  appLifecycle: {
    onPrepareQuit(listener) {
      appQuitPreparationListener = listener
      return () => {
        if (appQuitPreparationListener === listener) appQuitPreparationListener = null
      }
    }
  },
  supervisor: {
    getSnapshot() {
      return ipcRenderer.invoke('coaxi:supervisor-get-snapshot') as Promise<SupervisorSnapshot>
    },
    retryFullCore() {
      return ipcRenderer.invoke('coaxi:supervisor-retry') as Promise<SupervisorSnapshot>
    },
    onChanged(listener: (snapshot: SupervisorSnapshot) => void): () => void {
      const handler = (
        _event: Electron.IpcRendererEvent,
        snapshot: SupervisorSnapshot
      ): void => listener(snapshot)
      ipcRenderer.on('coaxi:supervisor-changed', handler)
      return () => ipcRenderer.removeListener('coaxi:supervisor-changed', handler)
    }
  },
  userAutomation: {
    onOpenThread(listener) {
      const handler = (
        _event: Electron.IpcRendererEvent,
        request: { threadId: string }
      ): void => listener(request)
      ipcRenderer.on('coaxi:user-automation-open-camp', handler)
      return () => ipcRenderer.removeListener('coaxi:user-automation-open-camp', handler)
    }
  },
  appearance: {
    get() {
      return ipcRenderer.invoke('coaxi:appearance-get') as Promise<AppearanceSnapshot>
    },
    setPreference(preference: ThemePreference) {
      return ipcRenderer.invoke('coaxi:appearance-set', preference) as Promise<AppearanceSnapshot>
    },
    updatePreferences(preferences: Partial<AppearancePreferences>) {
      return ipcRenderer.invoke('coaxi:appearance-update', preferences) as Promise<AppearanceSnapshot>
    },
    onChanged(listener: (snapshot: AppearanceSnapshot) => void): () => void {
      const handler = (
        _event: Electron.IpcRendererEvent,
        value: AppearanceSnapshot
      ): void => listener(value)
      ipcRenderer.on('coaxi:appearance-changed', handler)
      return () => ipcRenderer.removeListener('coaxi:appearance-changed', handler)
    }
  },
  appUpdates: {
    get() {
      return ipcRenderer.invoke('coaxi:app-updates-get')
    },
    check() {
      return ipcRenderer.invoke('coaxi:app-updates-check')
    },
    download() {
      return ipcRenderer.invoke('coaxi:app-updates-download')
    },
    install() {
      return ipcRenderer.invoke('coaxi:app-updates-install')
    },
    dismissPrompt(promptId: string) {
      return ipcRenderer.invoke('coaxi:app-updates-dismiss-prompt', promptId)
    },
    onChanged(listener: (snapshot: AppUpdateSnapshot) => void): () => void {
      const handler = (
        _event: Electron.IpcRendererEvent,
        value: AppUpdateSnapshot
      ): void => listener(value)
      ipcRenderer.on('coaxi:app-updates-changed', handler)
      return () => ipcRenderer.removeListener('coaxi:app-updates-changed', handler)
    }
  },
  desktopSession: {
    getStartupSnapshot() {
      return ipcRenderer.invoke('coaxi:desktop-session-get-startup')
    },
    getInterfaceLanguage() {
      return ipcRenderer.invoke('coaxi:desktop-session-get-interface-language')
    },
    commitRestorableLocation(location: RestorableLocation) {
      return ipcRenderer.invoke('coaxi:desktop-session-commit-location', location)
    }
  },
  currentUserProfile: {
    get: () => ipcRenderer.invoke('coaxi:current-user-profile-get'),
    save: (profile) => ipcRenderer.invoke('coaxi:current-user-profile-save', profile)
  },
  generalPreferences: {
    onChanged(listener) {
      const receive = (_event: Electron.IpcRendererEvent, snapshot: import('@contracts').GeneralPreferencesSnapshot): void => listener(snapshot)
      ipcRenderer.on('coaxi:general-preferences-changed', receive)
      return () => ipcRenderer.removeListener('coaxi:general-preferences-changed', receive)
    },
    get() {
      return ipcRenderer.invoke('coaxi:general-preferences-get')
    },
    setInterfaceLanguage(language: InterfaceLanguage) {
      return ipcRenderer.invoke('coaxi:general-preferences-set-language', language)
    },
    setInterfaceLanguageMode(mode: 'manual' | 'system') {
      return ipcRenderer.invoke('coaxi:general-preferences-set-language-mode', mode)
    },
    setStartupLocationMode(mode: StartupLocationMode) {
      return ipcRenderer.invoke('coaxi:general-preferences-set-startup', mode)
    },
    setLastSettingsSection(section: SettingsSection) {
      return ipcRenderer.invoke('coaxi:general-preferences-set-section', section)
    },
    setExecutionConsolePlacement(placement: ExecutionConsolePlacement) {
      return ipcRenderer.invoke('coaxi:general-preferences-set-execution-placement', placement)
    },
    setNewConversationDefaults(defaults, enableOneClick = false) {
      return ipcRenderer.invoke('coaxi:general-preferences-set-new-conversation-defaults', defaults, enableOneClick)
    },
    setOneClickNewConversationEnabled(enabled: boolean) {
      return ipcRenderer.invoke('coaxi:general-preferences-set-one-click-new-conversation', enabled)
    },
    setWorldMapEnabled(enabled: boolean) {
      return ipcRenderer.invoke('coaxi:general-preferences-set-world-map', enabled)
    },
    invalidateNewConversationDefaults(expectedDefaults?: NewConversationDefaults | null) {
      return ipcRenderer.invoke('coaxi:general-preferences-invalidate-new-conversation-defaults', expectedDefaults)
    }
  },
  channels: {
    get() {
      return ipcRenderer.invoke('coaxi:channels-get') as Promise<ChannelSettingsSnapshot>
    },
    getExecutionWebSettings() {
      return ipcRenderer.invoke('coaxi:execution-web-settings-get') as Promise<ExecutionWebSettingsSnapshot>
    },
    setExecutionWebSettings(settings) {
      return ipcRenderer.invoke('coaxi:execution-web-settings-set', settings)
    },
    connect(kind) {
      return ipcRenderer.invoke('coaxi:channels-connect', kind)
    },
    disconnect(kind) {
      return ipcRenderer.invoke('coaxi:channels-disconnect', kind)
    },
    publishMemberBot(agentId, kind) {
      return ipcRenderer.invoke('coaxi:channels-publish-member-bot', agentId, kind)
    },
    retryMemberBot(agentId, kind) {
      return ipcRenderer.invoke('coaxi:channels-retry-member-bot', agentId, kind)
    },
    selectPublicationApprover(agentId, userId, kind) {
      return ipcRenderer.invoke(
        'coaxi:channels-select-publication-approver',
        agentId,
        userId,
        kind
      )
    },
    cancelQrAttempt(attemptId) {
      return ipcRenderer.invoke('coaxi:channels-cancel-qr', attemptId)
    },
    setLoginViewBounds(attemptId, bounds) {
      return ipcRenderer.invoke('coaxi:channels-login-view-bounds', attemptId, bounds)
    },
    refreshLoginQr(attemptId) {
      return ipcRenderer.invoke('coaxi:channels-refresh-login-qr', attemptId)
    },
    onChanged(listener) {
      const handler = (
        _event: Electron.IpcRendererEvent,
        value: ChannelSettingsSnapshot
      ): void => listener(value)
      ipcRenderer.on('coaxi:channels-changed', handler)
      return () => ipcRenderer.removeListener('coaxi:channels-changed', handler)
    },
    onExecutionWebSettingsChanged(listener) {
      const handler = (
        _event: Electron.IpcRendererEvent,
        value: ExecutionWebSettingsSnapshot
      ): void => listener(value)
      ipcRenderer.on('coaxi:execution-web-settings-changed', handler)
      return () => ipcRenderer.removeListener('coaxi:execution-web-settings-changed', handler)
    }
  },
  onboarding: {
    get() {
      return ipcRenderer.invoke('coaxi:onboarding-get')
    },
    showWelcome() {
      return ipcRenderer.invoke('coaxi:onboarding-show-welcome')
    },
    completeWelcome() {
      return ipcRenderer.invoke('coaxi:onboarding-complete-welcome')
    },
    selectMember(role) {
      return ipcRenderer.invoke('coaxi:onboarding-select-member', role)
    },
    showMemberSelection() {
      return ipcRenderer.invoke('coaxi:onboarding-show-member-selection')
    },
    completeMemberSelection() {
      return ipcRenderer.invoke('coaxi:onboarding-complete-member-selection')
    },
    setRuntimeSelection(selection) {
      return ipcRenderer.invoke('coaxi:onboarding-set-runtime-selection', selection)
    },
    deferRuntimeSetup() {
      return ipcRenderer.invoke('coaxi:onboarding-defer-runtime')
    },
    beginProvisioning(selection, runtimePermissions) {
      return ipcRenderer.invoke(
        'coaxi:onboarding-begin-provisioning',
        selection,
        runtimePermissions
      )
    },
    recordProvisionedMember(agentId, version) {
      return ipcRenderer.invoke('coaxi:onboarding-record-member', agentId, version)
    },
    recordProvisionedRuntime(version) {
      return ipcRenderer.invoke('coaxi:onboarding-record-runtime', version)
    },
    prepareRuntimeCopies(targets) {
      return ipcRenderer.invoke('coaxi:onboarding-prepare-runtime-copies', targets)
    },
    recordRuntimeCopy(agentId, commandId, outcome) {
      return ipcRenderer.invoke('coaxi:onboarding-record-runtime-copy', agentId, commandId, outcome)
    },
    recordProvisionedThread(threadId) {
      return ipcRenderer.invoke('coaxi:onboarding-record-camp', threadId)
    },
    complete() {
      return ipcRenderer.invoke('coaxi:onboarding-complete')
    }
  },
  windowControls: {
    onNavigationRequested(listener) {
      const handler = (_event: Electron.IpcRendererEvent, direction: unknown): void => {
        if (direction === 'back' || direction === 'forward') listener(direction)
      }
      ipcRenderer.on('coaxi:navigation-requested', handler)
      return () => ipcRenderer.removeListener('coaxi:navigation-requested', handler)
    },
    onCloseTabRequested: createCloseTabShortcutHandler(ipcRenderer),
    getResetCapability() {
      return ipcRenderer.invoke('coaxi:window-reset-capability')
    },
    resetBounds() {
      return ipcRenderer.invoke('coaxi:window-reset-bounds')
    },
    popupApplicationMenu(request) {
      return ipcRenderer.invoke('coaxi:window-application-menu-popup', request) as Promise<boolean>
    },
    onPageZoomChanged(listener) {
      const handler = (_event: Electron.IpcRendererEvent, percentage: unknown): void => {
        if (typeof percentage === 'number' && Number.isFinite(percentage)) listener(percentage)
      }
      ipcRenderer.on('coaxi:page-zoom-changed', handler)
      return () => ipcRenderer.removeListener('coaxi:page-zoom-changed', handler)
    }
  },
  navigationPreferences: {
    applyProjectLayout(expectedRevision, mutation) {
      return ipcRenderer.invoke('coaxi:navigation-preferences-apply-project-layout', expectedRevision, mutation)
    },
    registerProjects(paths, completeLegacy = false) {
      return ipcRenderer.invoke('coaxi:navigation-preferences-register-projects', paths, completeLegacy)
    },
    setThreadReadState(threadId, state) {
      return ipcRenderer.invoke('coaxi:navigation-preferences-set-thread-read-state', threadId, state)
    },
    onChanged(listener) {
      const handler = (_event: Electron.IpcRendererEvent, snapshot: NavigationPreferencesSnapshot): void => listener(snapshot)
      ipcRenderer.on('coaxi:navigation-preferences-changed', handler)
      return () => ipcRenderer.removeListener('coaxi:navigation-preferences-changed', handler)
    },
    get() {
      return ipcRenderer.invoke('coaxi:navigation-preferences-get') as Promise<NavigationPreferencesSnapshot>
    },
    replacePins(pins: NavigationPin[]) {
      return ipcRenderer.invoke('coaxi:navigation-preferences-replace-pins', pins) as Promise<NavigationPreferencesSnapshot>
    },
    setProjectName(targetKey: string, name: string | null) {
      return ipcRenderer.invoke(
        'coaxi:navigation-preferences-set-project-name',
        targetKey,
        name
      ) as Promise<NavigationPreferencesSnapshot>
    },
    removeProject(targetKey: string, relatedThreadIds: string[]) {
      return ipcRenderer.invoke(
        'coaxi:navigation-preferences-remove-project',
        targetKey,
        relatedThreadIds
      ) as Promise<NavigationPreferencesSnapshot>
    },
    restoreProject(targetKey: string) {
      return ipcRenderer.invoke(
        'coaxi:navigation-preferences-restore-project',
        targetKey
      ) as Promise<NavigationPreferencesSnapshot>
    }
  },
  memberAvatars: {
    selectSource() {
      return ipcRenderer.invoke('coaxi:member-avatar-select-source')
    },
    save(input) {
      return ipcRenderer.invoke('coaxi:member-avatar-save', input)
    },
    read(avatarRef, rendition) {
      return ipcRenderer.invoke('coaxi:member-avatar-read', avatarRef, rendition)
    }
  },
  composerAttachments: {
    async prepare(threadId, expectedRevision, file) {
      const sourcePath = webUtils.getPathForFile(file)
      if (sourcePath) {
        return ipcRenderer.invoke(
          'coaxi:composer-attachment-prepare-path',
          threadId,
          expectedRevision,
          sourcePath,
          file.name,
          file.type || null
        )
      }
      const bytes = new Uint8Array(await file.arrayBuffer())
      return ipcRenderer.invoke(
        'coaxi:composer-attachment-prepare-bytes',
        threadId,
        expectedRevision,
        file.name,
        file.type || null,
        bytes
      )
    },
    preview(locator) {
      return ipcRenderer.invoke('coaxi:composer-attachment-preview', locator)
    },
    restore(threadId, attachments) {
      return ipcRenderer.invoke('coaxi:composer-attachment-restore', threadId, attachments)
    },
    discard(threadId, attachmentRefIds) {
      return ipcRenderer.invoke(
        'coaxi:composer-attachment-discard',
        threadId,
        attachmentRefIds
      )
    },
    location(locator) {
      return ipcRenderer.invoke('coaxi:composer-attachment-location', locator)
    }
  },
  missionAttachments: {
    async create(commandId, command, attachments) {
      return ipcRenderer.invoke(
        'coaxi:mission-create-with-attachments',
        commandId,
        command,
        await Promise.all(attachments.map(async ({ id, file }) => {
          const sourcePath = webUtils.getPathForFile(file)
          return sourcePath
            ? { id, sourcePath, displayName: file.name, mediaType: file.type || null }
            : { id, bytes: new Uint8Array(await file.arrayBuffer()), displayName: file.name, mediaType: file.type || null }
        }))
      )
    },
    async update(commandId, command, keepAttachmentIds, attachments) {
      return ipcRenderer.invoke(
        'coaxi:mission-update-with-attachments',
        commandId,
        command,
        keepAttachmentIds,
        await Promise.all(attachments.map(async ({ id, file }) => {
          const sourcePath = webUtils.getPathForFile(file)
          return sourcePath
            ? { id, sourcePath, displayName: file.name, mediaType: file.type || null }
            : { id, bytes: new Uint8Array(await file.arrayBuffer()), displayName: file.name, mediaType: file.type || null }
        }))
      )
    }
  },
  singleChatAttachments: {
    async prepareInput(request, file) {
      const frozen = structuredClone(request)
      const sourcePath = webUtils.getPathForFile(file)
      const metadata = { displayName: file.name, mediaType: file.type || null }
      if (sourcePath) return ipcRenderer.invoke('coaxi:private-attachment-prepare', frozen, { ...metadata, sourcePath })
      if (file.size > 25 * 1024 * 1024) throw new Error('附件超过 25 MiB。')
      const bytes = new Uint8Array(await file.arrayBuffer())
      return ipcRenderer.invoke('coaxi:private-attachment-prepare', frozen, { ...metadata, bytes })
    },
    reconcileInput: request => ipcRenderer.invoke('coaxi:private-attachment-reconcile', request),
    retryInput: request => ipcRenderer.invoke('coaxi:private-attachment-retry', request),
    finishInput: request => ipcRenderer.invoke('coaxi:private-attachment-finish', request),
    pendingPreparations: owner => ipcRenderer.invoke('coaxi:private-attachment-list', owner),
    async prepare(conversationId, expectedDraftRevision, file) {
      const sourcePath = webUtils.getPathForFile(file)
      if (sourcePath) {
        return ipcRenderer.invoke(
          'coaxi:single-chat-attachment-prepare-path',
          conversationId,
          expectedDraftRevision,
          sourcePath,
          file.name,
          file.type || null
        )
      }
      const bytes = new Uint8Array(await file.arrayBuffer())
      return ipcRenderer.invoke(
        'coaxi:single-chat-attachment-prepare-bytes',
        conversationId,
        expectedDraftRevision,
        file.name,
        file.type || null,
        bytes
      )
    },
    async preparePending(input, file) {
      const sourcePath = webUtils.getPathForFile(file)
      if (sourcePath) {
        return ipcRenderer.invoke(
          'coaxi:single-chat-pending-attachment-prepare-path',
          input,
          sourcePath,
          file.name,
          file.type || null
        )
      }
      const bytes = new Uint8Array(await file.arrayBuffer())
      return ipcRenderer.invoke(
        'coaxi:single-chat-pending-attachment-prepare-bytes',
        input,
        file.name,
        file.type || null,
        bytes
      )
    },
    remove(conversationId, expectedDraftRevision, attachmentRefId) {
      return ipcRenderer.invoke(
        'coaxi:single-chat-attachment-remove',
        conversationId,
        expectedDraftRevision,
        attachmentRefId
      )
    }
  },
  attachments: {
    open(locator) {
      return ipcRenderer.invoke('coaxi:attachment-open', locator)
    },
    reveal(locator) {
      return ipcRenderer.invoke('coaxi:attachment-reveal', locator)
    }
  },
  filePreview: {
    nativeReference(request) { return ipcRenderer.invoke('coaxi:file-preview-native-reference', request) },
    updateRetention(state) { return ipcRenderer.invoke('coaxi:file-preview-retention', state) },
    onResourcesReleased(listener) {
      const handler = (_event: Electron.IpcRendererEvent, value: { handleIds: string[] }): void => listener(value)
      ipcRenderer.on('coaxi:file-preview-resources-released', handler)
      return () => ipcRenderer.removeListener('coaxi:file-preview-resources-released', handler)
    },
    bindThread(threadId) {
      return ipcRenderer.invoke('coaxi:file-preview-bind-camp', threadId)
    },
    open(request) {
      return ipcRenderer.invoke('coaxi:file-preview-open', request)
    },
    restore(request) {
      return ipcRenderer.invoke('coaxi:file-preview-restore', request)
    },
    reopen(request) {
      return ipcRenderer.invoke('coaxi:file-preview-reopen', request)
    },
    readText(request) {
      return ipcRenderer.invoke('coaxi:file-preview-read-text', request)
    },
    readPage(request) {
      return ipcRenderer.invoke('coaxi:file-preview-read-page', request)
    },
    resolveLine(request) {
      return ipcRenderer.invoke('coaxi:file-preview-resolve-line', request)
    },
    readBinary(request) {
      return ipcRenderer.invoke('coaxi:file-preview-read-binary', request)
    },
    prepareHtmlSite(request) {
      return ipcRenderer.invoke('coaxi:file-preview-prepare-html-site', request)
    },
    releaseHtmlSite(request) {
      return ipcRenderer.invoke('coaxi:file-preview-release-html-site', request)
    },
    prepareHtml(request) {
      return ipcRenderer.invoke('coaxi:file-preview-prepare-html', request)
    },
    reload(request) {
      return ipcRenderer.invoke('coaxi:file-preview-reload', request)
    },
    release(request) {
      return ipcRenderer.invoke('coaxi:file-preview-release', request)
    },
    openInSystem(request) {
      return ipcRenderer.invoke('coaxi:file-preview-open-in-system', request)
    },
    revealInFolder(request) {
      return ipcRenderer.invoke('coaxi:file-preview-reveal', request)
    },
    copyPath(request) {
      return ipcRenderer.invoke('coaxi:file-preview-copy-path', request)
    },
    chooseAuthorizedRoot(request) {
      return ipcRenderer.invoke('coaxi:file-preview-choose-root', request)
    },
    onExternalUpdate(listener) {
      const handler = (
        _event: Electron.IpcRendererEvent,
        value: Parameters<typeof listener>[0]
      ): void => listener(value)
      ipcRenderer.on('coaxi:file-preview-external-update', handler)
      return () => ipcRenderer.removeListener('coaxi:file-preview-external-update', handler)
    }
  },
  clipboard: {
    write(input) {
      return ipcRenderer.invoke('coaxi:clipboard-write', input)
    }
  },
  selectWorkspaceDirectory() {
    return ipcRenderer.invoke('coaxi:select-workspace-directory')
  },
  selectRuntimeExecutable() {
    return ipcRenderer.invoke('coaxi:select-runtime-executable')
  },
  selectSkillImportDirectory() {
    return ipcRenderer.invoke('coaxi:select-skill-import-directory')
  },
  revealProjectDirectory(projectPath) {
    return ipcRenderer.invoke('coaxi:reveal-project-directory', projectPath)
  },
  revealSkill(skillId: string) {
    return ipcRenderer.invoke('coaxi:reveal-skill', skillId)
  },
  revealMcpConfig() {
    return ipcRenderer.invoke('coaxi:reveal-mcp-config')
  },
  exportMemory() {
    return ipcRenderer.invoke('coaxi:export-memory')
  },
  exportDiagnostics() {
    return ipcRenderer.invoke('coaxi:export-diagnostics')
  },
  revealDiagnosticsExport(path: string) {
    return ipcRenderer.invoke('coaxi:reveal-diagnostics-export', path)
  },
  exportMonitoring(filter) {
    return ipcRenderer.invoke('coaxi:export-monitoring', filter)
  },
  exportUsageActivityReport(query) {
    return ipcRenderer.invoke('coaxi:export-usage-activity', query)
  },
  usageCard: {
    save: dataUrl => ipcRenderer.invoke('coaxi:usage-card-save',dataUrl),
    copy: dataUrl => ipcRenderer.invoke('coaxi:usage-card-copy',dataUrl),
    saveEstimate: snapshotJson => ipcRenderer.invoke('coaxi:usage-estimate-save',snapshotJson)
  },
  exportNativeHistoryUsage: query => ipcRenderer.invoke('coaxi:export-native-history-usage',query),
  revealMonitoringExport(path: string) {
    return ipcRenderer.invoke('coaxi:reveal-monitoring-export', path)
  },
  platform: process.platform
}

api.navigationPreferences = withHostProjectGroups(api.navigationPreferences, (method, params) => api.request(method, params))
contextBridge.exposeInMainWorld('coaxi', api)
