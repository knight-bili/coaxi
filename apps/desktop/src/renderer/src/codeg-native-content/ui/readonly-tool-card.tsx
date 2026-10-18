import type { AdaptedToolCallPart } from '../lib/adapters/ai-elements-adapter'
import { normalizeToolName } from '../lib/tool-call-normalization'
import { contextCompactionSummary, isContextCompactionMeta } from '../lib/context-compaction'
import { isCodegMcpWorkbenchTool } from '../lib/codeg-mcp-tool'
import { ContextCompactionCard } from '../components/message/context-compaction-card'
import { FeedbackCheckResultCard } from '../components/message/feedback-check-result-card'
import { CodegMcpToolCard } from '../components/message/codeg-mcp-tool-card'
import { DelegationStatusCard } from '../components/message/delegation-status-card'
import { AskQuestionResultCard } from '../components/message/ask-question-result-card'

// Exact recognition order from fixed Codeg ToolCallPart. Other dedicated
// families retain their current route until their complete source UI is ported.
// resume_delegation belongs to ResumedDelegationCard, not this MCP fallback.
export function readonlyToolCardKind(part: AdaptedToolCallPart): 'compaction' | 'question' | 'feedback' | 'status' | 'cancel' | 'mcp' | null {
  if (isContextCompactionMeta(part.meta)) return 'compaction'
  const name = normalizeToolName(part.toolName).toLowerCase()
  if (name === 'get_delegation_status') return 'status'
  if (name === 'cancel_delegation') return 'cancel'
  if (name === 'check_user_feedback') return 'feedback'
  if (name === 'question') return 'question'
  if (name !== 'resume_delegation' && isCodegMcpWorkbenchTool(name)) return 'mcp'
  return null
}

export function ReadonlyToolCard({ part }: { part: AdaptedToolCallPart }): React.JSX.Element | null {
  const kind = readonlyToolCardKind(part)
  if (kind === 'compaction') return <ContextCompactionCard state={part.state} meta={part.meta} summary={contextCompactionSummary(part.meta, part.output)} />
  if (kind === 'feedback') return <FeedbackCheckResultCard state={part.state} output={part.output} errorText={part.errorText} />
  if (kind === 'question') return <AskQuestionResultCard state={part.state} input={part.input} output={part.output} errorText={part.errorText} />
  if (kind === 'status' || kind === 'cancel') return <DelegationStatusCard kind={kind} state={part.state} input={part.input} output={part.output} errorText={part.errorText} />
  const name = normalizeToolName(part.toolName).toLowerCase()
  if (kind === 'mcp' && isCodegMcpWorkbenchTool(name)) return <CodegMcpToolCard tool={name} state={part.state} input={part.input} output={part.output} errorText={part.errorText} />
  return null
}
