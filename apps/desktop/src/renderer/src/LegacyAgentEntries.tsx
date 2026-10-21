import { useContext, useEffect, useMemo, useRef, useState } from 'react'
import { Eye, Loader2 } from 'lucide-react'
import type { AgentRunDiagnosticView, AgentRunExecutionEvidenceView } from '@contracts'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { NativeSessionViewerContext } from './native-session-viewer-context'

const record = (value: unknown): Record<string, unknown> | null => value !== null && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : null

/** Codeg Agent/Task eye action; Core keeps the actual file and provider authority. */
export function LegacyAgentEntries({ threadId, runId, parentConversationId, evidence }: {
  threadId: string; runId: string; parentConversationId: string; evidence: AgentRunExecutionEvidenceView[]
}): React.JSX.Element | null {
  const client = useThreadClient(), host = useContext(NativeSessionViewerContext), t = useUiText()
  const [opening, setOpening] = useState<string | null>(null), [error, setError] = useState(false)
  const generation = useRef(0)
  useEffect(() => { ++generation.current; setOpening(null); setError(false); return () => { ++generation.current } }, [client, threadId, runId])
  const entries = useMemo(() => {
    const values = new Map<string, AgentRunExecutionEvidenceView>()
    for (const item of [...evidence].sort((a, b) => a.executionEpoch - b.executionEpoch || a.sequence - b.sequence)) {
      const payload = record(item.payload)
      if (item.agentRunId !== runId || item.eventType !== 'runtime.action' || !payload || payload.sourceAuthority === 'core' || payload.canonicalTool != null) continue
      if (typeof payload.toolName !== 'string' || !['agent', 'task'].includes(payload.toolName.toLowerCase())
        || typeof payload.toolCallId !== 'string' || typeof payload.rawInputDigest !== 'string' || !/^[a-f0-9]{64}$/.test(payload.rawInputDigest)) continue
      values.set(`${item.executionEpoch}:${payload.toolCallId}`, item)
    }
    return [...values.entries()]
  }, [evidence, runId])
  if (!host || !entries.length) return null
  const open = async (item: AgentRunExecutionEvidenceView, trigger: HTMLElement): Promise<void> => {
    const current = ++generation.current
    setOpening(item.id); setError(false)
    try {
      const diagnostic = await client.request<AgentRunDiagnosticView>('agentRuns.diagnostic.get', { agentRunId: runId })
      if (current !== generation.current) return
      const runtime = diagnostic.runtime.adapterKind === 'claude-code-cli' ? 'claude' : diagnostic.runtime.adapterKind === 'qoder-cli' ? 'qoder' : null
      if (!runtime || diagnostic.agentRunId !== runId || diagnostic.threadId !== threadId) throw new Error('native_agent.scope_changed')
      host.open({ kind: 'native', identity: `native-agent:${runId}:${item.id}`, threadId, parentConversationId, runtime,
        subtitle: String(record(item.payload)?.toolName ?? 'Agent'),
        source: { kind: 'agent', reference: { threadId, agentRunId: runId, evidenceId: item.id } } }, trigger)
    } catch { if (current === generation.current) setError(true) }
    finally { if (current === generation.current) setOpening(null) }
  }
  return <section className="delegation-children native-subagent-entries" aria-label={t('原生帮手')}>
    {entries.map(([key, item]) => <div className="delegation-card" key={key}>
      <span className="delegation-avatar"><Eye size={16} /></span>
      <div className="delegation-card-description"><strong>{t('原生帮手')}</strong><code>{String(record(item.payload)?.toolCallId ?? '')}</code></div>
      <button type="button" className="delegation-open" disabled={opening !== null} onClick={event => void open(item, event.currentTarget)}>
        {opening === item.id ? <Loader2 size={14} className="native-session-spinner" /> : <Eye size={14} />}<span>{t('查看会话')}</span>
      </button>
    </div>)}
    {error && <p className="delegation-read-error" role="alert">{t('帮手会话暂时无法读取，请重试。')}</p>}
  </section>
}
