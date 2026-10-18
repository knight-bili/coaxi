import type { DelegationDetail } from '@contracts'
import { useRef } from 'react'
import { DelegatedSubThread } from './codeg-native-content/components/message/delegated-sub-thread'
import { ResumedDelegationCard } from './codeg-native-content/components/message/resumed-delegation-card'
import { SourceDelegationProvider } from './codeg-native-content/ui/source-delegation-host'
import { coreDelegationCardModel } from './core-delegation-card'

export function CoreDelegationCard({ detail, onOpen }: { detail: DelegationDetail; onOpen(trigger?: HTMLElement): void }): React.JSX.Element {
  const trigger = useRef<HTMLElement | undefined>(undefined)
  return <div className="codeg-native-rich" data-core-delegation-id={detail.delegationId} onClickCapture={event => {
    trigger.current = event.target instanceof Element ? event.target.closest('button') ?? undefined : undefined
  }}><SourceDelegationProvider value={{ sourceId: detail.delegationId, model: coreDelegationCardModel(detail), open: request => {
    if (request.source.parentToolUseId !== detail.delegationId) throw new Error('delegation.viewer_scope_mismatch')
    onOpen(trigger.current)
  } }}>
    {detail.currentAttemptNo > 1
      ? <ResumedDelegationCard toolCallId={detail.delegationId} input={JSON.stringify({ task_id: detail.delegationId })} />
      : <DelegatedSubThread parentToolUseId={detail.delegationId} />}
  </SourceDelegationProvider></div>
}
