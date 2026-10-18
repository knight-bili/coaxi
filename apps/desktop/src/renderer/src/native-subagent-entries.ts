import type { AgentRunExecutionEvidenceView, NativeChildReadReference } from '@contracts'

export interface NativeSubagentEntry {
  identity: string
  runtime: 'codex' | 'grok'
  label: string
  status: string
  reference: NativeChildReadReference | null
}
const object = (value: unknown): Record<string, unknown> | null => value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : null
const identity = (value: unknown): value is string => typeof value === 'string' && /^[a-zA-Z0-9_-]{1,256}$/.test(value)
/** Metadata only. Core will prove the actual source and parent chain on open;
 * a display ID, logical agentPath or Grok short ID is never a read parameter. */
export function nativeSubagentEntries(threadId: string, runId: string, evidence: AgentRunExecutionEvidenceView[]): NativeSubagentEntry[] {
  const rows = new Map<string, NativeSubagentEntry>()
  for (const item of [...evidence].sort((a, b) => a.executionEpoch - b.executionEpoch || a.sequence - b.sequence || (a.revision ?? 0) - (b.revision ?? 0))) {
    if (item.agentRunId !== runId) continue
    const value = object(object(item.payload)?.nativeSubagents)
    if (!value || value.schemaVersion !== 1 || !Array.isArray(value.children) || !value.children.length || value.children.length > 64) continue
    const codex = value.adapterKind === 'codex-cli' && (value.source === 'codex-app-server.collabAgentToolCall' && value.relationship === 'runtime_reported_receiver'
      || value.source === 'codex-app-server.subAgentActivity' && value.relationship === 'runtime_reported_child_activity')
    const grok = value.adapterKind === 'grok-build' && value.source === 'grok-acp.subagent_notification' && value.relationship === 'runtime_reported_session_child'
    if (!codex && !grok) continue
    const runtime = codex ? 'codex' : 'grok'
    value.children.forEach((raw, receiverIndex) => {
      const child = object(raw)
      if (!child || typeof child.status !== 'string') return
      const session = identity(child.nativeSessionId) && child.nativeSessionId !== value.parentNativeSessionId ? child.nativeSessionId : null
      const short = identity(child.nativeSubagentId) ? child.nativeSubagentId : null
      const key = `${runtime}:${session ?? short ?? `${item.id}:${receiverIndex}`}`
      const status = ['running', 'waiting', 'completed', 'failed', 'cancelled', 'interrupted', 'shutdown', 'not_found', 'unknown'].includes(child.status) ? child.status : 'unknown'
      rows.set(key, { identity: key, runtime, label: session ? `#${session.slice(-8)}` : short ? `#${short}` : '原生帮手', status,
        reference: session ? { threadId, agentRunId: runId, evidenceId: item.id, receiverIndex } : null })
    })
  }
  return [...rows.values()]
}
