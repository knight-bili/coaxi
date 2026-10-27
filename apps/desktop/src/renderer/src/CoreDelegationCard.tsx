import type { DelegationDetail, DelegationToolSourceView } from '@contracts'
import { useRef } from 'react'
import { DelegatedSubThread } from './coaxi-native-content/components/message/delegated-sub-thread'
import { ResumedDelegationCard } from './coaxi-native-content/components/message/resumed-delegation-card'
import { SourceDelegationProvider } from './coaxi-native-content/ui/source-delegation-host'
import { coreDelegationCardModel } from './core-delegation-card'

export function CoreDelegationCard({ detail, source, onOpen }: { detail: DelegationDetail; source?: Extract<DelegationToolSourceView, { state: 'bound' }>; onOpen(trigger?: HTMLElement): void }): React.JSX.Element {
  const trigger = useRef<HTMLElement | undefined>(undefined)
  const sourceId = source?.source.evidenceId ?? detail.delegationId
  const model = coreDelegationCardModel(detail)
  if (source) model.task = source.task
  const resumed = source ? source.operation === 'delegation.resume' : detail.currentAttemptNo > 1
  return <div className="coaxi-native-rich" data-core-delegation-id={detail.delegationId} data-core-delegation-source={source?.source.evidenceId} onClickCapture={event => {
    trigger.current = event.target instanceof Element ? event.target.closest('button') ?? undefined : undefined
  }}><SourceDelegationProvider value={{ sourceId, model, open: request => {
    if (request.source.parentToolUseId !== sourceId) throw new Error('delegation.viewer_scope_mismatch')
    onOpen(trigger.current)
  } }}>
    {resumed
      ? <ResumedDelegationCard toolCallId={sourceId} input={JSON.stringify({ task_id: detail.delegationId, ...(source?.reason == null ? {} : { reason: source.reason }) })} />
      : <DelegatedSubThread parentToolUseId={sourceId} />}
  </SourceDelegationProvider></div>
}
