// Coaxi collab-agent-card / agent-tool-call: per-child Eye action and compact
// runtime identity; Coaxi's existing shared card tokens and one-slot host.
import { useContext, useEffect, useRef, useState } from 'react'
import { Eye } from 'lucide-react'
import type { AgentRunExecutionEvidenceView } from '@contracts'
import { NativeSessionViewerContext } from './native-session-viewer-context'
import { nativeSubagentEntries } from './native-subagent-entries'
import { LegacyAgentEntries } from './LegacyAgentEntries'
import { CoaxiRuntimeMark } from './CoaxiRuntimeMark'
import { useUiText } from './interface-language'
import { useThreadClient } from './camp-client'

export function NativeSubagentEntries({ threadId, runId, parentConversationId, evidence, parentLive = false }: {
  threadId: string; runId: string; parentConversationId: string; evidence: AgentRunExecutionEvidenceView[]; parentLive?: boolean
}): React.JSX.Element | null {
  const host = useContext(NativeSessionViewerContext), t = useUiText(), client = useThreadClient()
  const [full, setFull] = useState<{ client: typeof client; thread: string; run: string; values: Map<string, { stamp: string; observation: unknown }> }>({ client, thread: threadId, run: runId, values: new Map() })
  const [loading, setLoading] = useState(false), [error, setError] = useState(false), sequence = useRef(0), currentEvidence = useRef(evidence)
  const [expanded, setExpanded] = useState(false)
  currentEvidence.current = evidence
  useEffect(() => { ++sequence.current; setLoading(false); setError(false); return () => { ++sequence.current } }, [client, threadId, runId])
  const stamp = (item: AgentRunExecutionEvidenceView): string => JSON.stringify([item.executionEpoch, item.revision, item.changeSequence, item.contentBlobId])
  const values = full.client === client && full.thread === threadId && full.run === runId ? full.values : new Map<string, { stamp: string; observation: unknown }>()
  const shown = evidence.map(item => { const value = values.get(item.id); return value?.stamp === stamp(item) ? { ...item, payload: { nativeSubagents: value.observation } } : item })
  const entries = nativeSubagentEntries(threadId, runId, shown)
  // Generic evidence previews retain only 24 array entries. Read the complete
  // observation on explicit expansion, retaining original receiver indices.
  const missing = evidence.filter(item => item.agentRunId === runId && item.isTruncated
    && !!(item.payload as { nativeSubagents?: unknown } | null)?.nativeSubagents && values.get(item.id)?.stamp !== stamp(item))
  const loadComplete = async (): Promise<void> => {
    if (loading) return
    const request = ++sequence.current; setLoading(true); setError(false)
    try {
      const stamps = new Map(evidence.map(item => [item.id, stamp(item)]))
      const next = new Map([...values].filter(([id, value]) => stamps.get(id) === value.stamp))
      for (const item of missing) {
        const response = await client.request<{ payload: { nativeSubagents?: unknown } }>('agentRunEvidence.getContent', { threadId, evidenceId: item.id })
        if (request !== sequence.current) return
        const current = currentEvidence.current.find(candidate => candidate.id === item.id)
        if (current && stamp(current) === stamp(item) && response.payload?.nativeSubagents) next.set(item.id, { stamp: stamp(item), observation: response.payload.nativeSubagents })
      }
      if (request === sequence.current) setFull({ client, thread: threadId, run: runId, values: next })
    } catch { if (request === sequence.current) setError(true) }
    finally { if (request === sequence.current) setLoading(false) }
  }
  if (!host) return null
  const labels: Record<string, string> = { running: '正在执行', waiting: '等待继续', completed: '已完成', failed: '执行失败', cancelled: '已取消', interrupted: '已中断', shutdown: '已关闭', not_found: '未找到', unknown: '状态待核对' }
  return <><LegacyAgentEntries threadId={threadId} runId={runId} parentConversationId={parentConversationId} evidence={evidence} />{entries.length > 0 && <section className="delegation-children native-subagent-entries" aria-label={t('原生帮手')}>
    {(expanded ? entries : entries.slice(0, 3)).map(entry => <div className="delegation-card" key={entry.identity}>
      <span className="delegation-avatar"><CoaxiRuntimeMark target={entry.runtime} /></span>
      <div className="delegation-card-description"><div><strong>{entry.runtime === 'codex' ? 'Codex' : 'Grok'}</strong><code>{entry.label}</code><span className={`delegation-status is-${entry.status}`}>{!parentLive && ['running', 'waiting'].includes(entry.status) ? t('上次记录：{0}', t(labels[entry.status]!)) : t(labels[entry.status] ?? '状态待核对')}</span></div>
        {!entry.reference && <small>{t('运行器未提供独立会话。')}</small>}
      </div>
      {entry.reference && <button className="delegation-open" type="button" aria-label={t('查看原生帮手会话')} onClick={event => {
        if (entry.reference) host.open({ kind: 'native', identity: `${runId}:${entry.identity}`, threadId, parentConversationId,
          runtime: entry.runtime, subtitle: entry.label, source: { reference: entry.reference } }, event.currentTarget)
      }}><Eye size={14} /><span>{t('查看会话')}</span></button>}
    </div>)}
    {entries.length > 3 && <div className="delegation-list-controls"><button type="button" onClick={() => setExpanded(value => !value)}>{t(expanded ? '收起帮手列表' : '展开帮手列表')} ({entries.length})</button></div>}
    {missing.length > 0 && <div className="delegation-list-controls"><button type="button" disabled={loading} onClick={() => void loadComplete()}>{t(loading ? '正在读取完整列表…' : '读取完整帮手列表')}</button></div>}
    {error && <p className="delegation-read-error" role="alert">{t('帮手列表暂时无法读取，请重试。')}</p>}
  </section>}</>
}
