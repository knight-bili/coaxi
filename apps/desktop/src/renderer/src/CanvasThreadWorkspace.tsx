import { useCallback, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import type { AgentProfile, AdapterInstallation, FilePreviewApi, ResolvedTheme, ThreadComposerDraftView } from '@contracts'
import { useThreadClient } from './camp-client'
import { ThreadWorkspace, type ThreadInspectorTab, type ThreadRuntimeRecovery } from './ThreadWorkspace'
import { FilePreviewProvider } from './FilePreviewContext'
import { ThreadProjectionStore, ThreadProjectionSupersededError } from './thread-projection-store'
import { threadCommands } from './thread-commands'
import { threadWorkspaceActions, type ThreadOptimisticEntry, type ThreadActionToast } from './thread-workspace-actions'
import { effectiveCancellingRunIds, reconcileRunCancellationIds } from './thread-run-cancellation'
import type { CanvasConversationInstance, CanvasConversationsController } from './canvas-conversations-controller'
import type { NavigationWindowReader } from './navigation-window-reader'
import type { LiveRuntimeEvent } from './ui-model'
import type { CanvasNotificationVisibility } from './canvas-notification-visibility'
import type { VisibleNotificationSources } from './ThreadWorkspace'
import { UiText, uiAttribute } from './interface-language'
import { errorMessage, type ThreadSurfaceSnapshot } from './thread-workspace-data'

export interface CanvasThreadWorkspaceProps {
  instance: CanvasConversationInstance
  controller: CanvasConversationsController
  visibility: CanvasNotificationVisibility
  reads: ThreadProjectionStore
  files: FilePreviewApi
  theme: ResolvedTheme
  agents: AgentProfile[]
  installations: AdapterInstallation[]
  liveRuntimeEvents: LiveRuntimeEvent[]
  navigation: Pick<NavigationWindowReader, 'refreshThreads' | 'invalidate'>
  onPendingLeave(draft: ThreadComposerDraftView): Promise<void>
  onForgetMemberCreation(threadId: string): void
  onConfigureRuntime(agentId: string): void
  onOpenMotionSettings(): void
  onNotify(message: string): void
  onNotifyError(message: string): void
}

export function CanvasThreadWorkspace(props: CanvasThreadWorkspaceProps): React.JSX.Element {
  return <FilePreviewProvider api={props.files} threadId={props.instance.target.reference.threadId} resolvedTheme={props.theme} contained>
    <CanvasThreadWorkspaceContent {...props} />
  </FilePreviewProvider>
}

function CanvasThreadWorkspaceContent({ instance, controller, visibility, reads, agents, installations, liveRuntimeEvents, navigation,
  onPendingLeave, onForgetMemberCreation, onConfigureRuntime, onOpenMotionSettings, onNotify, onNotifyError }: CanvasThreadWorkspaceProps): React.JSX.Element {
  const client = useThreadClient(), threadId = instance.target.reference.threadId
  const visibilityLease = useRef<ReturnType<CanvasNotificationVisibility['register']> | null>(null)
  useLayoutEffect(() => {
    const lease = visibility.register(instance.key, threadId)
    visibilityLease.current = lease
    return () => { lease.release(); if (visibilityLease.current === lease) visibilityLease.current = null }
  }, [visibility, instance.key, threadId])
  const publicVisible = useCallback((source: VisibleNotificationSources) => visibilityLease.current?.publish('public', source), [])
  const privateVisible = useCallback((source: VisibleNotificationSources) => visibilityLease.current?.publish('private', source), [])
  const subscribe = useCallback((listener: () => void) => reads.subscribe(threadId, listener), [reads, threadId])
  const getSnapshot = useCallback(() => reads.getSnapshot(threadId), [reads, threadId])
  const projection = useSyncExternalStore(subscribe, getSnapshot, getSnapshot)
  const lastSnapshot = useRef<ThreadSurfaceSnapshot | null>(null)
  if (projection.snapshot) lastSnapshot.current = projection.snapshot
  const campSnapshot = projection.snapshot ?? lastSnapshot.current
  const campSnapshotRef = useRef(campSnapshot); campSnapshotRef.current = campSnapshot
  const activeThreadId = threadId, activeThreadIdRef = useRef<string | null>(threadId)
  const activeThreadCommands = useMemo(() => threadCommands(client, threadId), [client, threadId])
  const campSelectionGeneration = useRef(0), threadClientOwner = useRef(client), campEventSequenceMarker = useRef(0)
  const [busy, setBusy] = useState<string | null>(null), [error, setError] = useState<string | null>(null)
  const [, setToast] = useState<ThreadActionToast | null>(null)
  const [runtimeRecovery, setRuntimeRecovery] = useState<ThreadRuntimeRecovery | null>(null)
  const [optimisticThreadMessages, setOptimisticThreadMessages] = useState<ThreadOptimisticEntry[]>([])
  const [cancellingRunIds, setCancellingRunIds] = useState<Set<string>>(() => new Set())
  const [confirmingRunIds, setConfirmingRunIds] = useState<Set<string>>(() => new Set())
  const [singleChatVisible, setSingleChatVisible] = useState(false)
  const [inspectorVisible, setInspectorVisible] = useState(false), [inspectorTab, setInspectorTab] = useState<ThreadInspectorTab>('tasks')
  useLayoutEffect(() => {
    const release = reads.retain(threadId)
    activeThreadIdRef.current = threadId
    return () => { activeThreadIdRef.current = null; campSelectionGeneration.current += 1; release() }
  }, [reads, threadId])
  const setThreadSnapshot = (snapshot: ThreadSurfaceSnapshot | null) => { if (snapshot) reads.remember(snapshot) }
  const refreshActiveThreadSnapshot = useCallback(async (id: string) => {
    try { await reads.refresh(id) } catch (error) { if (!(error instanceof ThreadProjectionSupersededError)) throw error }
  }, [reads])
  const requestThreadProjection = async (id: string, mode: 'open' | 'enter') => {
    if (mode !== 'open') throw new Error(uiAttribute('画布恢复只能读取已有会话。'))
    return reads.open(id)
  }
  const actions = threadWorkspaceActions({ client, activeThreadId, activeThreadCommands, campSnapshot, campSnapshotRef, activeThreadIdRef,
    campSelectionGeneration, threadClientOwner, campEventSequenceMarker, setThreadSnapshot, setBusy, setError, setToast, setRuntimeRecovery,
    setOptimisticThreadMessages, setCancellingRunIds, setConfirmingRunIds, refreshActiveThreadSnapshot, requestThreadProjection,
    navigationRefreshCoordinator: navigation, forgetMemberCreationDraft: onForgetMemberCreation, notify: onNotify })
  const bindLeave = useCallback((_id: string, guard: import('./ThreadWorkspace').ThreadLeaveGuard | null) => controller.register(instance.key, guard), [controller, instance.key])
  const issue = error ?? (projection.error && errorMessage(projection.error))
  const notifyError = (message: string) => { setError(message); onNotifyError(message) }
  if (!campSnapshot) return <div className="canvas-notice" role={issue ? 'alert' : 'status'}>
    {issue || uiAttribute('正在读取对话…')}<button type="button" disabled={projection.loading} onClick={() => { void reads.refresh(threadId).catch(error => setError(errorMessage(error))) }}><UiText zh="重新读取" /></button>
  </div>
  return <>
    {issue && <div className="canvas-notice" role="alert">{issue}<button type="button" disabled={projection.loading} onClick={() => { setError(null); void reads.refresh(threadId).catch(error => setError(errorMessage(error))) }}><UiText zh="重新读取" /></button></div>}
    <ThreadWorkspace snapshot={campSnapshot} composerContext={instance.target.draftContext} projectName={null} previewTabsInPane
      agents={agents} installations={installations} liveRuntimeEvents={liveRuntimeEvents} openCoverage={campSnapshot.openCoverage}
      messageHistory={campSnapshot.openCoverage?.messages} onLoadEarlierMessages={async () => { await reads.loadEarlier(threadId) }}
      busy={Boolean(busy || !projection.snapshot)} optimisticMessages={optimisticThreadMessages.map(entry => entry.message)}
      onSend={actions.sendThreadMessage} onWithdrawMessage={actions.withdrawThreadMessage}
      onVisibleNotificationSources={publicVisible} onVisibleSingleChatSources={privateVisible}
      onPendingDraftPersisted={() => { void navigation.invalidate({ scope: 'group', threadId }).catch(() => undefined) }}
      onPendingThreadLeave={onPendingLeave} onThreadLeaveGuardChange={bindLeave}
      onChangeLead={actions.changeDefaultLead} onAddMembers={actions.addThreadMembers} onPreviewMemberRemoval={actions.previewThreadMemberRemoval}
      onRemoveMember={actions.removeThreadMember} onTasksChanged={() => refreshActiveThreadSnapshot(threadId)}
      onResolveApproval={(approval, optionId) => { void actions.resolveActionApproval(approval, optionId) }}
      cancellingRunIds={effectiveCancellingRunIds(cancellingRunIds, campSnapshot)} confirmingRunIds={reconcileRunCancellationIds(confirmingRunIds, campSnapshot)}
      onCancelAgentRun={actions.cancelAgentRun} stopping={false} executionPlacement="inspector" worldMapEnabled={false}
      workspaceEntrySnapshotReady={Boolean(projection.snapshot)} inspectorVisible={inspectorVisible} inspectorTab={inspectorTab}
      onOpenInspector={tab => { setInspectorTab(tab); setInspectorVisible(true); setSingleChatVisible(false) }}
      onCloseInspector={() => setInspectorVisible(false)} onInspectorTabChange={setInspectorTab}
      singleChatVisible={Boolean(instance.conversation || singleChatVisible)}
      singleChatBoundConversationId={instance.conversation?.id}
      singleChatTarget={instance.conversation ? { conversationId: instance.conversation.id, agentId: instance.conversation.agentId,
        agentDisplayName: agents.find(item => item.agentId === instance.conversation?.agentId)?.displayName ?? uiAttribute('队员'), requestId: 1 } : null}
      onOpenSingleChat={() => { setSingleChatVisible(true); setInspectorVisible(false) }}
      onCloseSingleChat={() => { if (instance.conversation) { void controller.close([instance.key]) } else setSingleChatVisible(false) }}
      runtimeRecovery={runtimeRecovery} onDismissRuntimeRecovery={() => setRuntimeRecovery(null)}
      onConfigureRuntime={onConfigureRuntime} onOpenMotionSettings={onOpenMotionSettings} onNotify={onNotify} onNotifyError={notifyError} />
  </>
}
