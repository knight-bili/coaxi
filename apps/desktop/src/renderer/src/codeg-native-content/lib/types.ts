// Fixed Codeg aff1e6c; Apache-2.0, integration/licenses/codeg/LICENSE.
// Source: src/lib/types.ts; only module paths and explicitly recorded seams differ.

/** The fifteen agents codeg ships hand-written support for. */
export type BuiltinAgentType =
  | "claude_code"
  | "codex"
  | "open_code"
  | "gemini"
  | "open_claw"
  | "cline"
  | "hermes"
  | "code_buddy"
  | "kimi_code"
  | "pi"
  | "grok"
  | "cursor"
  | "deepseek"
  | "qoder"
  | "antigravity"


/**
 * Which agent backs a conversation.
 *
 * Open-ended on purpose: besides the built-ins, a user can register any ACP
 * agent, which arrives as `custom:<registry-id>` (mirrors Rust's
 * `AgentType::Custom`). The `(string & {})` arm keeps editor autocomplete for
 * the built-ins while accepting those ids.
 *
 * Never index a `Record` with this directly — use `getAgentLabel` /
 * `getAgentColor`, which fall back for custom agents.
 */
export type AgentType = BuiltinAgentType | (string & {})


export type MessageRole = "user" | "assistant" | "system" | "tool"


export interface AgentToolCall {
  tool_name: string
  input_preview?: string | null
  output_preview?: string | null
  is_error: boolean
}


export interface AgentExecutionStats {
  agent_type?: string | null
  status?: string | null
  total_duration_ms?: number | null
  total_tokens?: number | null
  total_tool_use_count?: number | null
  read_count?: number | null
  search_count?: number | null
  bash_count?: number | null
  edit_file_count?: number | null
  lines_added?: number | null
  lines_removed?: number | null
  other_tool_count?: number | null
  tool_calls?: AgentToolCall[]
  /** The child's thinking/text/tool activity in arrival order, from the same
   *  transcript pass that produced `tool_calls` (Rust mirror:
   *  `AgentExecutionStats.timeline`; a `tool_call` entry's `childIndex`
   *  anchors into `tool_calls` at the same ordinal). Lets the settled card
   *  replay the child's mixed timeline when the live transcript is gone. */
  timeline?: AgentTranscriptEntry[]
  /** What the child's task file filed as its outcome (Qoder): for a
   *  `status: "completed"` run, its final report, recovered after the parent
   *  turn died before the async result was delivered (the card's tool_result
   *  is then an error); other statuses file an error message or a
   *  placeholder. Rendered ONLY on an errored card whose `status` is
   *  `completed` — a normally completed async child delivered its result via
   *  the notification turn, so this field stays invisible there (display
   *  policy lives in the card, parsers attach it blindly). */
  recovered_result?: string
  /** The child's own session id, when the sub-agent ran as a standalone session
   *  on disk instead of as chunks folded into the parent (Grok: every
   *  `spawn_subagent` child). Drives the Agent card's "open the sub-agent's
   *  session" action — `getConversation` resolves it even though the session is
   *  hidden from the sidebar. Absent for every other agent. */
  child_session_id?: string | null
}


/**
 * One entry of a subagent transcript. Two data sources share this shape: the
 * LIVE channel (a running capsule's stream; entries arrive pre-merged — the
 * reducer/backend split blocks only at kind/attribution boundaries, so
 * consecutive same-kind chunks of one subagent are a single growing entry)
 * and history (`AgentExecutionStats.timeline`, parsed from the child's own
 * JSONL by the Rust parsers — one entry per thinking/text/tool block, in
 * arrival order).
 */
export type AgentTranscriptEntry =
  | { type: "text" | "thinking"; text: string }
  | {
      /**
       * A marker into the mixed timeline: the child's OWN tool call, keeping
       * its chronological position among the prose around it. `childIndex` is
       * the call's ordinal among this parent's children — `agent_stats.
       * tool_calls` carries no ids, and both lists are appended in the same
       * arrival pass, so the ordinal is exact.
       */
      type: "tool_call"
      childIndex: number
    }


/**
 * Image payload shared across `ContentBlock::Image` /
 * `ContentBlock::ImageGeneration` / ACP wire `ToolCallImageInfo`. Mirror of
 * Rust `models::message::ImageData`.
 */
export interface ImageData {
  data: string
  mime_type: string
  uri?: string | null
}


export type ContentBlock =
  | { type: "text"; text: string }
  | {
      type: "image"
      data: string
      mime_type: string
      uri?: string | null
    }
  | {
      /**
       * codex-acp v0.14+ image generation. Distinct from `image` because
       * codex-acp positions image generation as a first-class
       * `ToolCall(title="Image generation")` carrying revised_prompt + image.
       * Rendered with the dedicated `<GeneratedImagesBlock>` component, not
       * mixed with regular tool-call cards.
       *
       * Singular `image` (not array): codex-acp emits exactly one image per
       * `ToolCall`. Multi-image turns produce N separate ToolCalls. `null`
       * during the in-flight placeholder window between
       * `ImageGenerationBegin` and `ImageGenerationEnd`.
       *
       * `status` mirrors the underlying ToolCallStatus during live streaming
       * so the renderer can distinguish in-flight vs. failed when no image
       * arrives. Absent on Rust-emitted blocks (JSONL replay only emits
       * blocks with a present image, so absence is treated as success).
       */
      type: "image_generation"
      revised_prompt?: string | null
      image?: ImageData | null
      status?: ToolCallStatus | null
      /** Real tool/page name when this card is not Codex image generation. */
      label?: string | null
    }
  | {
      type: "tool_use"
      tool_use_id: string | null
      tool_name: string
      input_preview: string | null
      /**
       * ACP tool-call status when known. Live and promoted turns forward it
       * from `ToolCallInfo.status` in `buildStreamingTurnsFromLiveMessage`;
       * DB-persisted rows omit it (`undefined`). Lets the render layer tell a
       * still-unsettled orphan (interrupted/retried arg-less call promoted into
       * `localTurns`) from a completed no-op. See `dropEmptyInFlightToolCalls`.
       */
      status?: string | null
      /**
       * ACP extensibility metadata for this tool call. Opaque pass-through
       * — both the live snapshot (`ToolCallState.meta`) and the persisted
       * message-row variant carry the same shape. Delegation writes
       * `meta["codeg.delegation"] = { status, child_connection_id,
       * child_conversation_id, error_code? }` here.
       */
      meta?: Record<string, unknown> | null
    }
  | {
      type: "tool_result"
      tool_use_id: string | null
      output_preview: string | null
      is_error: boolean
      agent_stats?: AgentExecutionStats | null
      /**
       * Images returned in a tool result (e.g. Claude Code's `Read` of a
       * PNG/JPEG, or a multi-page PDF read returning one image per page).
       * Mirror of Rust `ContentBlock::ToolResult.images`. The adapter renders
       * these in-position as `generated-image` cards so the historical (JSONL
       * replay) path matches the live ACP stream — which surfaces the same
       * bytes via `ToolCallInfo.images` and an `image_generation` block.
       * Absent/empty for the common text-only tool result.
       */
      images?: ImageData[] | null
      /**
       * LIVE-stream data: the in-flight transcript of a native subagent —
       * a marker per child tool call, plus (Claude Code only) the text/
       * thinking chunks attributed to this Agent tool call via
       * `_meta.claudeCode.parentToolUseId` (claude-agent-acp ≥0.63) —
       * rendered inside the live Agent capsule. Still attached at settle so
       * the finished card keeps the interleaved order it showed live; once
       * history reloads from disk, the parsed `agent_stats.timeline` takes
       * over as the card's timeline source.
       */
      agent_transcript?: AgentTranscriptEntry[] | null
    }
  | { type: "thinking"; text: string }
  /**
   * The turn FAILED, in the agent's own words. Mirror of Rust
   * `ContentBlock::TurnError`: a parser puts it alone in a `system` turn
   * closing the round, and the live stream does the same with the failure the
   * adapter reported (`buildStreamingTurnsFromLiveMessage`). Drawn as one muted
   * line, never as a reply.
   */
  | { type: "turn_error"; message: string }
  /**
   * Frontend-only, LIVE-stream synthetic block. It is NEVER persisted and
   * NEVER emitted by the Rust JSONL parsers — the persisted plan path is a
   * `TodoWrite` tool_use block. It exists purely so a live plan can survive
   * `buildStreamingTurnsFromLiveMessage` → `adaptContentBlock` without being
   * down-converted into a `thinking`/reasoning block. Mirrors the reducer's
   * `LiveContentBlock` plan variant in `acp-connections-context.tsx` (NOT the
   * `kind`-tagged snapshot type lower in this file). Because it is live-only,
   * persistence/export switches over `ContentBlock` never receive it.
   */
  | { type: "plan"; entries: PlanEntryInfo[] }


export type TurnRole = "user" | "assistant" | "system"


export interface TurnUsage {
  input_tokens: number
  output_tokens: number
  cache_creation_input_tokens: number
  cache_read_input_tokens: number
}


export interface MessageTurn {
  id: string
  role: TurnRole
  blocks: ContentBlock[]
  timestamp: string
  usage?: TurnUsage | null
  duration_ms?: number | null
  model?: string | null
  /** Wall-clock completion time (ISO). Each Rust parser sets this to its
   * own end-marker (e.g. OpenCode's `time.completed`, or just the event-log
   * `timestamp` for agents that log post-generation). Notably this is NOT
   * `timestamp + duration_ms` — those two fields encode unrelated spans in
   * most parsers. */
  completed_at?: string | null
  /** The id the AGENT knows this turn's message by, when codeg can name it the
   * same way the agent does — `id` above is positional (`turn-3`) and names
   * nothing an agent could look up.
   *
   * Present only where the turn can be a fork point ("fork from here"), which
   * today means Claude and DeepSeek assistant turns. Its absence does NOT mean
   * the turn cannot be forked at: codex forks by content fingerprint instead,
   * and DeepSeek falls back to one — all resolved entirely in the backend, so
   * never gate the fork affordance on this. */
  agent_message_id?: string | null
  /** CLIENT-ONLY, never on the wire. The id the PARSER gave this turn.
   *
   * A turn produced in the current session is named
   * `live-<conversationId>-<liveMessageId>` by `buildStreamingTurnsFromLiveMessage`
   * and keeps that name after it settles into `localTurns`. The backend has
   * never heard of it — it resolves turns against a fresh parse, whose turns are
   * `turn-N` — so asking the backend to act on "this turn" by its live id
   * silently finds nothing. "Fork from here" hit exactly that: it degraded to a
   * tail fork, and the forked session came out identical to its parent.
   *
   * Backfilled by the post-turn reparse (`computeTurnMetadataPatches`), which
   * already aligns parsed turns onto local ones to fill in usage/duration.
   * Absent on turns that came from the parser to begin with — those already ARE
   * `turn-N` — so read it as `turn.source_turn_id ?? turn.id`. */
  source_turn_id?: string | null
}


export const ALL_AGENT_TYPES: BuiltinAgentType[] = [
  "claude_code",
  "codex",
  "open_code",
  "gemini",
  "open_claw",
  "cline",
  "hermes",
  "code_buddy",
  "kimi_code",
  "pi",
  "grok",
  "cursor",
  "deepseek",
  "qoder",
  "antigravity",
]


export interface PlanEntryInfo {
  content: string
  priority: string
  status: string
}


// --- LiveSessionSnapshot wire types (mirror src-tauri/src/acp/session_state.rs) ---

export type ToolCallStatus = "pending" | "in_progress" | "completed" | "failed"


// --- ask_user_question (mirror of Rust `crate::acp::question`) ---

/** One selectable choice in an `ask_user_question` (mirror of `QuestionOption`). */
export interface QuestionOption {
  label: string
  description: string
}


/** A single multiple-choice question (mirror of Rust `QuestionSpec`). `id` is
 *  the backend-minted correlation key the answer is submitted against. Empty
 *  `options` means free-text: the card renders only its "Other" input (codex
 *  elicitation / MCP-server forms ask open questions this way). `is_secret`
 *  masks that input (absent on the wire for non-secret sources). */
export interface QuestionSpec {
  id: string
  question: string
  header: string
  multi_select: boolean
  options: QuestionOption[]
  is_secret?: boolean
}


/** Awaiting-answer question set on the session (mirror of `PendingQuestionState`). */
export interface PendingQuestionState {
  question_id: string
  questions: QuestionSpec[]
  created_at: string
}


/** One question's answer submitted to `acp_answer_question`. `labels` carries
 *  the selected option labels plus any free-text "Other" the user typed. */
export interface QuestionAnswerItem {
  questionId: string
  labels: string[]
}


/** The full submission to `acp_answer_question`. `declined` is set when the
 *  user dismissed the card without choosing. */
export interface QuestionAnswer {
  answers: QuestionAnswerItem[]
  declined: boolean
}
