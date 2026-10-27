import { uiAttribute } from './interface-language'
import { createContext, useContext, type ReactNode } from 'react'
import type { CoaxiApi } from '@contracts'
import { desktopThreadClient } from './desktop-camp-client'

/** Dependencies of the existing Thread UI, not a public HTTP operation allowlist.
 * Remote adapters must authorize each operation and resource at the Host boundary.
 * Native startup, credentials, window controls and supervisor are intentionally absent.
 */
export interface SavedDraftCopy { id: string; value: string; savedAt: number }
export interface EditingRecovery {
  get(identity: string): unknown
  set(identity: string, value: unknown): void
  draftCopies?(identity: string): SavedDraftCopy[]
}

export type ThreadClient = Pick<CoaxiApi,
  'request' | 'singleChatAttachments' | 'platform'
> & {
  missionAttachments: CoaxiApi['missionAttachments'] | null
  composerAttachments: Omit<CoaxiApi['composerAttachments'], 'preview'> & {
    preview(locator: import('@contracts').LocalAttachmentOwnerLocator): Promise<Omit<import('@contracts').AttachmentPreviewResult, 'preview'> & {
      preview: import('@contracts').AttachmentPreview | { blob: Blob } | null
    }>
  }
  nativePets?: CoaxiApi['nativePets']
  webdavWorkspace?: CoaxiApi['webdavWorkspace']
  nativeBackups?: CoaxiApi['nativeBackups']
  portableConfiguration?:CoaxiApi['portableConfiguration']
  keyboardShortcuts?:CoaxiApi['keyboardShortcuts']
  workbenchEditorRecovery?:CoaxiApi['workbenchEditorRecovery']
  nativeCustomSkills?: CoaxiApi['nativeCustomSkills']
  nativeHistory?: CoaxiApi['nativeHistory']
  projectLauncher?:CoaxiApi['projectLauncher']
  officeWorkspace?: CoaxiApi['officeWorkspace']
  interactionComputer?:CoaxiApi['interactionComputer']
  interactionWorkspace?: CoaxiApi['interactionWorkspace']
  browserWorkspace?: CoaxiApi['browserWorkspace']
  computerWorkspace?: CoaxiApi['computerWorkspace']
  editingRecovery?: EditingRecovery
  privateInputRecovery?: CoaxiApi['privateInputRecovery']
  retryPendingCommand?: (method: 'runtime.interactions.resolve' | import('../../shared/private-conversation-operations').PrivateInputMethod | import('../../shared/private-conversation-operations').DelegationActionMethod, params: unknown) => Promise<void>
  /** Authenticated Host/Owner identity for advisory presentation preferences; never a resource grant. */
  presentationScope?: string
  /** Local authentication lifetime for private read caches, never a Core grant. */
  readAuthority?: import('./thread-read-authority').ThreadReadAuthority
  /** Web uses a live, Core-verified editor identity instead of exposing the server data directory. */
  canvasCoreIdentity?: () => Promise<string>
  /** Explicit resource/platform dependencies of the shared management pages. */
  exportMonitoring: (filter: import('@contracts').MonitoringFilter) => Promise<{ exported: boolean; path?: string }>
  exportUsageActivityReport?: (query: import('@contracts').UsageActivityQuery) => Promise<{exported:boolean;path?:string}>
  usageCard?:import('@contracts').CoaxiApi['usageCard']
  syncNativeHistoryUsage?:(input:{operation:'sync';force:boolean;after?:string})=>Promise<import('@contracts').NativeHistoryUsageSync>
  exportNativeHistoryUsage?:(query:unknown)=>Promise<{exported:boolean;path?:string}>
  revealMonitoringExport: CoaxiApi['revealMonitoringExport'] | null
  exportDiagnostics: () => Promise<{ exported: boolean; path?: string }>
  revealDiagnosticsExport: CoaxiApi['revealDiagnosticsExport'] | null
  memberAvatars: CoaxiApi['memberAvatars']
  selectSkillImportDirectory: CoaxiApi['selectSkillImportDirectory']
  selectRuntimeExecutable: CoaxiApi['selectRuntimeExecutable'] | null
  revealMcpConfig: CoaxiApi['revealMcpConfig'] | null
  channels: (Pick<CoaxiApi['channels'], 'get' | 'onChanged' | 'publishMemberBot' | 'retryMemberBot' | 'selectPublicationApprover'> & {
    native: Pick<CoaxiApi['channels'], 'connect' | 'disconnect' | 'cancelQrAttempt' | 'refreshLoginQr'> | null
  }) | null
  onEvent?: CoaxiApi['onEvent']
  /** Authorized invalidation signal; it is deliberately not a CoreEvent. */
  onInvalidated?: (listener: (change?: import('@contracts').ThreadReadInvalidation) => void) => () => void
  /** Optional host shortcut; browsers retain their own tab/window shortcuts. */
  onClosePreviewRequested?: CoaxiApi['windowControls']['onCloseTabRequested']
  attachmentLocation?: (locator: import('@contracts').LocalAttachmentOwnerLocator) => Promise<{ path: string; location: 'local' | 'server' } | null>
  attachments: (CoaxiApi['attachments'] & { kind: 'native' }) | {
    kind: 'download'
    download: CoaxiApi['attachments']['open']
  }
}

const ThreadClientContext = createContext<ThreadClient | null>(null)

export function ThreadClientProvider({ client, children }: {
  // Stable for one Host/Owner/editor scope. Session renewal keeps the object and
  // mounted Composer; authentication loss/replacement retires private reads.
  client: ThreadClient
  children: ReactNode
}): React.JSX.Element {
  return <ThreadClientContext.Provider value={client}>{children}</ThreadClientContext.Provider>
}

export function useOptionalThreadClient():ThreadClient|null{return useContext(ThreadClientContext)}

export function useThreadClient(): ThreadClient {
  // Existing Desktop mounts retain their real bridge. Browser/review mounts must
  // inject a client; no global window.coaxi shim or empty-result fallback is created.
  const client = useOptionalThreadClient()
  if (client) return client
  if (typeof window === 'undefined' || window.coaxi) return desktopThreadClient
  throw new Error(uiAttribute('共享页面缺少 ThreadClientProvider；浏览器不能使用 Desktop 默认适配。'))
}

export function useEditingRecovery(): EditingRecovery | undefined { return useContext(ThreadClientContext)?.editingRecovery }
