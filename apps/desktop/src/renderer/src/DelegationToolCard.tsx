import { useContext, useEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import type { DelegationToolSourceView } from '@contracts'
import { CoreDelegationCard } from './CoreDelegationCard'
import { useThreadClient } from './camp-client'
import { DelegationViewerContext } from './delegation-viewer-context'
import { delegationChildrenController } from './delegation-children-controller'
import { DelegationToolSourceController, type DelegationToolSourceTarget } from './delegation-tool-source-controller'
import { CodegMcpToolCard } from './codeg-native-content/components/message/codeg-mcp-tool-card'
import { useUiText } from './interface-language'

type Props = { target: DelegationToolSourceTarget; revision: number; children: ReactNode }
/** No Viewer host means no source read or fake client. Static/review mounts
 * retain the existing complete tool record. The host outlives virtual rows. */
export function DelegationToolCard(props: Props): React.JSX.Element {
  const host = useContext(DelegationViewerContext)
  if (!host || host.environment.threadId !== props.target.threadId) return <>{props.children}</>
  return <DelegationToolCardContent {...props} />
}
function DelegationToolCardContent({ target, revision, children }: Props): React.JSX.Element {
  const client = useThreadClient(), host = useContext(DelegationViewerContext)!, t = useUiText()
  const controller = useMemo(() => new DelegationToolSourceController(client, { ...target }),
    [client, target.threadId, target.evidenceId, target.agentRunId, target.executionEpoch])
  const state = useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot)
  const previousEvidence = useRef({ controller, revision })
  useEffect(() => host.visible ? controller.retain() : undefined, [controller, host.visible])
  useEffect(() => {
    const previous = previousEvidence.current
    previousEvidence.current = { controller, revision }
    if (previous.controller === controller && previous.revision !== revision) controller.evidenceChanged()
  }, [controller, revision])
  const view = state.view
  if (!host.visible) return <>{children}</>
  return <div className="delegation-tool-source" data-delegation-tool-source={target.evidenceId}>
    {view?.state === 'bound' ? <BoundToolCard view={view} /> : <>
      {view?.state === 'refused' && (view.operation === 'delegation.resume' ? <div className="codeg-native-rich"><CodegMcpToolCard
        tool="resume_delegation"
        state="output-error" errorText={t(view.code === 'delegation.cleanup_pending'
          ? '原任务还在清理，请稍后再恢复。' : '这次委派或恢复请求未被接受。')} /></div>
        : <p className="delegation-read-error" role="status">{t('这次委派或恢复请求未被接受。')}</p>)}
      {state.error && <div className="delegation-read-error" role="alert">{t('工具来源暂时无法核对。')}<button type="button" onClick={() => void controller.refresh()}>{t('重试')}</button></div>}
      {view?.state === 'unresolved' && <div className="delegation-status">{t('工具来源尚未确认。')}<button type="button" className="quiet-button compact" onClick={() => void controller.refresh()}>{t('重新核对')}</button></div>}
    </>}
    {view?.state === 'bound' || view?.state === 'refused' ? <OriginalToolRecord>{children}</OriginalToolRecord> : children}
  </div>
}
function BoundToolCard({ view }: { view: Extract<DelegationToolSourceView, { state: 'bound' }> }): React.JSX.Element {
  const client = useThreadClient(), host = useContext(DelegationViewerContext)!, t = useUiText()
  const metadata = delegationChildrenController(client, view.source)
  const state = useSyncExternalStore(metadata.subscribe, metadata.getSnapshot, metadata.getSnapshot)
  useEffect(() => host.visible ? metadata.retain() : undefined, [metadata, host.visible])
  const detail = state.rows.find(row => row.delegationId === view.detail.delegationId)
  if (state.initialized && (!detail || detail.conversationId !== view.detail.conversationId || detail.ownerAgentId !== view.detail.ownerAgentId)) {
    return <div className="delegation-status">{t('工具来源暂时无法核对。')}<button type="button" className="quiet-button compact" onClick={() => void metadata.refresh()}>{t('重新核对')}</button></div>
  }
  const current = detail && detail.currentAttemptNo >= view.attemptNo ? detail : view.detail
  return <>
    <CoreDelegationCard detail={current} source={view} onOpen={trigger => host.open({
      threadId: view.source.threadId, parentConversationId: view.source.parentConversationId, delegationId: current.delegationId
    }, trigger)} />
    {(current.blockedOn?.kind === 'question' || current.blockedOn?.kind === 'plan_approval' || current.takenOver || current.cleanupPending) &&
      <small className="delegation-status">{t(current.cleanupPending ? '正在停止和清理原任务，完成后可继续聊天。'
        : current.takenOver ? '已接管' : current.blockedOn?.kind === 'question' ? '等待回答' : '等待计划确认')}</small>}
    {state.error && <small className="delegation-status" role="status">{t('帮手列表暂时无法读取，将重新核对。')}</small>}
  </>
}
function OriginalToolRecord({ children }: { children: ReactNode }): React.JSX.Element {
  const t = useUiText(), [expanded, setExpanded] = useState(false)
  return <details className="delegation-tool-original" open={expanded} onToggle={event => {
    if (event.target === event.currentTarget) setExpanded(event.currentTarget.open)
  }}><summary>{t('原工具记录')}</summary>{expanded && children}</details>
}
