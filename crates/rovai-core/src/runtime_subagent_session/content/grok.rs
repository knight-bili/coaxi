// Adapted from fixed Codeg aff1e6c (Apache-2.0).
// Source ranges and changes: integration/inventory/native-grok-content-port.json.
//! Original Grok turn/tool/answer rules. Core uses complete content, with
//! transport windows applied later; preview helpers retain their donor names.
use super::{model::*, prompt::*};
use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;
use std::collections::HashMap;
const GROK_TOOL_OUTPUT_CAP: usize = 100_000;
const GROK_TOOL_INPUT_CAP: usize = 8_000;
const GROK_TASK_OUTPUT_CAP: usize = 48 * 1024;
const GROK_ASK_TOOL_NAME: &str = "ask_user_question";
const GROK_SPAWN_SUBAGENT_TOOL_NAME: &str = "spawn_subagent";
const GROK_SUBAGENT_RESULT_CAP: usize = 16_000;
const GROK_SUBAGENT_STARTED_ACK_PREFIX: &str = "Subagent started in background";
pub(crate) const BACKGROUND_TASK_MARKER: &str = "[[codeg-background-task]]";
#[derive(Default)]
struct ParsedUpdates {
    turns: Vec<MessageTurn>,
    first_ts: Option<DateTime<Utc>>,
    last_ts: Option<DateTime<Utc>>,
    content_events: u32,
    first_user_text: Option<String>,
    /// Model discovered in-stream (`user_message_chunk._meta.modelId`); a
    /// fallback when `summary.json` lacks `current_model_id`.
    model: Option<String>,
    /// `spawn_subagent` tool_call ids in encounter order, paired against
    /// `subagents` by `attach_subagent_stats`.
    spawn_call_ids: Vec<String>,
    /// Sub-agent lifecycles observed on the `_x.ai/session/update` stream
    /// (`subagent_spawned` opens one, `subagent_finished` completes it), in
    /// spawn order.
    subagents: Vec<GrokSubagentLifecycle>,
    /// Context-window occupancy: `params._meta.totalTokens` as of the last turn
    /// to state one. Feeds the context ring, and is kept apart from turn `usage`
    /// because the two measure different things — occupancy is what currently
    /// sits in the window, `usage` is what the turn spent getting there.
    context_tokens: Option<u64>,
}

/// One subagent's lifecycle, folded from the parent-stream extension
/// notifications. `subagent_finished` carries the child's FULL final output —
/// the only in-stream source for a background subagent the model never polled.
#[derive(Default)]
struct GrokSubagentLifecycle {
    subagent_id: String,
    /// Distinct from `subagent_id` in principle (`== subagent_id` in every
    /// observed capture); the child session directory is named by this.
    child_session_id: Option<String>,
    subagent_type: Option<String>,
    status: Option<String>,
    output: Option<String>,
    duration_ms: Option<u64>,
    tool_calls: Option<u32>,
    tokens_used: Option<u64>,
}

fn parse_updates(
    rows: impl IntoIterator<Item = Result<Value>>,
    fallback: DateTime<Utc>,
) -> Result<ParsedUpdates> {
    let mut out = ParsedUpdates::default();
    // The in-flight assistant turn, plus a `toolCallId → index-of-its-ToolResult`
    // map scoped to that turn (cleared on every turn boundary).
    let mut assistant: Option<MessageTurn> = None;
    let mut tool_result_idx: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    // Stats for the in-flight turn (tokens/timing/model), applied to the
    // assistant turn when it is finalized. Reset at each turn boundary.
    let mut turn_meta = GrokTurnMeta::default();
    // `promptIndex` of the currently-open user turn. Grok splits one prompt into
    // several `user_message_chunk`s (prose, image, …) sharing a `promptIndex`;
    // this lets consecutive same-prompt chunks merge into a single user turn
    // instead of each opening a new (often empty) one.
    let mut open_user_prompt_index: Option<i64> = None;

    for record in rows {
        let v = record?;
        let ts = v
            .get("timestamp")
            .and_then(Value::as_i64)
            .and_then(|secs| Utc.timestamp_opt(secs, 0).single());
        if let Some(t) = ts {
            if out.first_ts.is_none() {
                out.first_ts = Some(t);
            }
            out.last_ts = Some(t);
        }
        let now = ts.unwrap_or(fallback);

        let Some(update) = v.pointer("/params/update") else {
            continue;
        };
        let kind = update
            .get("sessionUpdate")
            .and_then(Value::as_str)
            .unwrap_or("");

        // Grok injects its own reminders (a background task finishing, …) as
        // `user_message_chunk`s and marks them `_meta.hideFromScrollback` — its
        // TUI never shows them. Honor the flag: rendered as a user bubble, such a
        // chunk splits one reply into two turns with a raw `<system-reminder>`
        // block wedged between them. Skipped BEFORE the turn-boundary logic below,
        // so a reminder injected mid-turn doesn't cut the open assistant turn
        // either.
        if kind == "user_message_chunk"
            && update
                .pointer("/_meta/hideFromScrollback")
                .and_then(Value::as_bool)
                == Some(true)
        {
            continue;
        }

        // Grok's per-turn stats live in the OUTER `params._meta` (token total +
        // timing) plus `update._meta.modelId`. Accumulate them into `turn_meta`
        // and apply at the turn boundary. A `user_message_chunk` that opens a NEW
        // prompt closes+resets the prior turn's accumulator; a continuation chunk
        // of the SAME prompt (see below) keeps accumulating.
        let params_meta = v.pointer("/params/_meta");
        let update_meta = update.get("_meta");
        // Grok emits each content piece of one prompt (prose, image, …) as its
        // own `user_message_chunk` sharing a `promptIndex`. Merge consecutive
        // user chunks of the same prompt into ONE user turn so a "text + image"
        // prompt renders as a single bubble (matching the live path) rather than
        // a trailing empty/image-only turn. A chunk continues the open user turn
        // when no assistant content has intervened and the `promptIndex` matches
        // (or is absent on either side).
        let user_chunk_continues = kind == "user_message_chunk"
            && assistant.is_none()
            && matches!(out.turns.last(), Some(t) if matches!(t.role, TurnRole::User))
            && update
                .pointer("/_meta/promptIndex")
                .and_then(Value::as_i64)
                .zip(open_user_prompt_index)
                .is_none_or(|(a, b)| a == b);
        if kind == "user_message_chunk" && !user_chunk_continues {
            if let Some(prev) = assistant.as_mut() {
                turn_meta.apply(prev, &mut out.context_tokens);
            }
            flush_assistant(&mut assistant, &mut out.turns, &mut tool_result_idx);
            turn_meta = GrokTurnMeta::default();
        }
        turn_meta.observe(params_meta, update_meta);

        match kind {
            "user_message_chunk" => {
                let input = user_chunk_to_input(update);
                let block = input.as_ref().map(user_turn_block);
                out.content_events += 1;
                // Title/first-prompt text comes only from PROSE chunks: an image
                // chunk carries no text, and an attachment projects to a
                // `[name](uri)` marker that would make a poor title.
                if let Some(PromptInputBlock::Text { text }) = &input {
                    if out.first_user_text.is_none() && !text.trim().is_empty() {
                        out.first_user_text = Some(text.clone());
                    }
                }
                if out.model.is_none() {
                    out.model = update
                        .pointer("/_meta/modelId")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                if user_chunk_continues {
                    // Same prompt: append the block to the open user turn.
                    if let (Some(b), Some(turn)) = (block, out.turns.last_mut()) {
                        turn.blocks.push(b);
                    }
                } else {
                    open_user_prompt_index =
                        update.pointer("/_meta/promptIndex").and_then(Value::as_i64);
                    out.turns.push(MessageTurn {
                        id: String::new(), // assigned in a final pass
                        role: TurnRole::User,
                        blocks: block.into_iter().collect(),
                        timestamp: now,
                        usage: None,
                        duration_ms: None,
                        model: None,
                        completed_at: None,
                        agent_message_id: None,
                    });
                }
            }
            "agent_message_chunk" => {
                out.content_events += 1;
                let text = update_text(update);
                append_text(ensure_assistant(&mut assistant, now), text);
            }
            "agent_thought_chunk" => {
                out.content_events += 1;
                let text = update_text(update);
                append_thinking(ensure_assistant(&mut assistant, now), text);
            }
            "tool_call" => {
                out.content_events += 1;
                let id = str_field(update, "toolCallId");
                // Grok wraps every MCP call in a `use_tool` envelope; peel it so
                // the call is classified/parsed as a direct MCP call (matches the
                // live path, connection.rs::unwrap_grok_use_tool). Native tools —
                // whose args are top-level — pass through unchanged.
                let raw_input = update.get("rawInput");
                let unwrapped = unwrap_use_tool(raw_input);
                let mut tool_name = match unwrapped {
                    Some((name, _)) => name.to_string(),
                    None => update
                        .get("_meta")
                        .and_then(|m| m.get("x.ai/tool"))
                        .and_then(|t| t.get("name"))
                        .and_then(Value::as_str)
                        .or_else(|| update.get("title").and_then(Value::as_str))
                        .unwrap_or("tool")
                        .to_string(),
                };
                // Native sub-agent launch → the dedicated Agent card (see
                // `GROK_SPAWN_SUBAGENT_TOOL_NAME`). Recorded for the
                // lifecycle/stats pairing pass; the MCP-unwrapped branch is
                // untouched (an MCP tool can't be grok's native launcher).
                if unwrapped.is_none() && tool_name == GROK_SPAWN_SUBAGENT_TOOL_NAME {
                    tool_name = "Agent".to_string();
                    if !id.is_empty() {
                        out.spawn_call_ids.push(id.clone());
                    }
                }
                let input_preview = match unwrapped {
                    // Valid-JSON-preserving cap so the delegation card can parse a
                    // long task; native inputs keep the opaque byte-truncation.
                    Some((_, input)) => grok_mcp_input_preview(input),
                    None => tool_input_preview(raw_input),
                };
                let turn = ensure_assistant(&mut assistant, now);
                turn.blocks.push(ContentBlock::ToolUse {
                    tool_use_id: Some(id.clone()),
                    tool_name,
                    input_preview,
                    // Grok is the one agent whose transcript is read WHILE it is
                    // being written (`SubagentSessionDialog` polls a running
                    // `spawn_subagent` child's file), so it is the one agent
                    // that has to say whether a call is still working. The
                    // paired placeholder result below cannot answer that: it
                    // stays `output_preview: None` for an empty completion too.
                    status: grok_line_status(&v, update),
                    meta: None,
                });
                turn.blocks.push(ContentBlock::ToolResult {
                    tool_use_id: Some(id.clone()),
                    output_preview: None,
                    is_error: false,
                    agent_stats: None,
                    images: Vec::new(),
                });
                if !id.is_empty() {
                    tool_result_idx.insert(id, turn.blocks.len() - 1);
                }
            }
            "tool_call_update" => {
                let id = str_field(update, "toolCallId");
                let output = update_tool_output(update);
                let status = grok_line_status(&v, update);
                let failed = status.as_deref() == Some("failed");
                // Cumulative updates: the latest STATED status wins, so a call
                // that settles overwrites its own `pending`; a content-only
                // update states nothing and leaves it standing.
                apply_tool_status(assistant.as_mut(), &tool_result_idx, &id, status.as_deref());
                apply_tool_result(assistant.as_mut(), &tool_result_idx, &id, output, failed);
            }
            "turn_completed" => {
                if let Some(mut turn) = assistant.take() {
                    // The one place Grok states real token spend, scoped to the
                    // prompt this update closes (see `prompt_usage`).
                    turn_meta.usage = update.get("usage").and_then(prompt_usage);
                    turn_meta.apply(&mut turn, &mut out.context_tokens);
                    turn.completed_at = Some(now);
                    out.turns.push(turn);
                }
                turn_meta = GrokTurnMeta::default();
                tool_result_idx.clear();
            }
            // Grok's auto-compaction (`/compact` or threshold-triggered) lands in
            // updates.jsonl on the namespaced `_x.ai/session/update` method as
            // `auto_compact_completed` {tokens_before, tokens_after}. Mirror the
            // live synthesis (connection.rs::map_grok_ext_notification): a completed
            // ToolUse tagged `meta.contextCompaction` so the history path renders the
            // shared ContextCompactionCard with the token delta instead of dropping
            // it. The paired ToolResult keeps the block well-formed (no orphan
            // tool_use). The `/compact` command itself is not recoverable — Grok
            // never persists slash commands as `user_message_chunk`s — so only the
            // compaction OUTCOME shows, not a "/compact" user bubble.
            "auto_compact_completed" => {
                out.content_events += 1;
                let mut meta = serde_json::Map::new();
                meta.insert("contextCompaction".to_string(), Value::Bool(true));
                if let Some(before) = update.get("tokens_before").and_then(Value::as_u64) {
                    meta.insert("tokensBefore".to_string(), before.into());
                }
                if let Some(after) = update.get("tokens_after").and_then(Value::as_u64) {
                    meta.insert("tokensAfter".to_string(), after.into());
                }
                // A stable id from the event id (deterministic across re-parses);
                // the paired blocks are self-contained, so no `tool_result_idx`
                // registration or later update is needed.
                let id = params_meta
                    .and_then(|m| m.get("eventId"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("grok-compaction-{}", out.content_events));
                let turn = ensure_assistant(&mut assistant, now);
                turn.blocks.push(ContentBlock::ToolUse {
                    tool_use_id: Some(id.clone()),
                    tool_name: "context_compaction".to_string(),
                    input_preview: None,
                    status: None,
                    meta: Some(Value::Object(meta)),
                });
                turn.blocks.push(ContentBlock::ToolResult {
                    tool_use_id: Some(id),
                    output_preview: None,
                    is_error: false,
                    agent_stats: None,
                    images: Vec::new(),
                });
            }
            // A failed auto-compaction: the same card in its failed state, as the
            // live mapper renders it (the two share `grok_failed_compaction_meta`,
            // whose `error` is the card's reason). `is_error` on the paired
            // result is what marks the card failed.
            "auto_compact_failed" => {
                out.content_events += 1;
                let id = params_meta
                    .and_then(|m| m.get("eventId"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("grok-compaction-{}", out.content_events));
                let turn = ensure_assistant(&mut assistant, now);
                turn.blocks.push(ContentBlock::ToolUse {
                    tool_use_id: Some(id.clone()),
                    tool_name: "context_compaction".to_string(),
                    input_preview: None,
                    status: None,
                    meta: Some(grok_failed_compaction_meta(update)),
                });
                turn.blocks.push(ContentBlock::ToolResult {
                    tool_use_id: Some(id),
                    output_preview: None,
                    is_error: true,
                    agent_stats: None,
                    images: Vec::new(),
                });
            }
            // Sub-agent lifecycle (namespaced `_x.ai/session/update` method).
            // Metadata, not content — deliberately NOT counted into
            // `content_events` and never a turn boundary. Collected in spawn
            // order for `attach_subagent_stats`.
            "subagent_spawned" => {
                let Some(subagent_id) = update
                    .get("subagent_id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                else {
                    continue;
                };
                out.subagents.push(GrokSubagentLifecycle {
                    subagent_id: subagent_id.to_string(),
                    child_session_id: update
                        .get("child_session_id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    subagent_type: update
                        .get("subagent_type")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    ..GrokSubagentLifecycle::default()
                });
            }
            "subagent_finished" => {
                let Some(subagent_id) = update.get("subagent_id").and_then(Value::as_str) else {
                    continue;
                };
                // Pair by id; a finished-without-spawned event (defensive —
                // not observed on real streams) still lands as its own entry
                // so its output is not lost.
                let lifecycle = match out
                    .subagents
                    .iter_mut()
                    .find(|l| l.subagent_id == subagent_id)
                {
                    Some(l) => l,
                    None => {
                        out.subagents.push(GrokSubagentLifecycle {
                            subagent_id: subagent_id.to_string(),
                            child_session_id: update
                                .get("child_session_id")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            ..GrokSubagentLifecycle::default()
                        });
                        out.subagents.last_mut().expect("just pushed")
                    }
                };
                lifecycle.status = update
                    .get("status")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                lifecycle.output = update
                    .get("output")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(|s| truncate_str(s, GROK_SUBAGENT_RESULT_CAP));
                lifecycle.duration_ms = update.get("duration_ms").and_then(Value::as_u64);
                lifecycle.tool_calls = update
                    .get("tool_calls")
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok());
                lifecycle.tokens_used = update.get("tokens_used").and_then(Value::as_u64);
            }
            // `task_backgrounded` / `task_completed` / plan / other extension
            // updates carry no distinct rendered content beyond what the tool
            // stream already has.
            //
            // In particular `task_completed`'s snapshot is deliberately NOT
            // applied to the launching tool call: Grok reports that CALL as
            // `completed` on the wire (it did start the task), the live path
            // never receives this ext notification at all, and the task's real
            // outcome — command, exit code, output — renders from the
            // `get_command_or_subagent_output` polls (see
            // `grok_task_output_envelope`). Writing the snapshot here would make
            // history contradict live for the same conversation.
            //
            // Known gap: a task the model never polls has its output ONLY in
            // this snapshot, so neither path surfaces it.
            _ => {}
        }
    }

    // A session that ends mid-turn (no trailing `turn_completed`) still gets its
    // accumulated stats. Its `usage` stays `None` — Grok has not stated the
    // spend yet — but the context occupancy it did state still reaches the ring.
    if let Some(prev) = assistant.as_mut() {
        turn_meta.apply(prev, &mut out.context_tokens);
    }
    flush_assistant(&mut assistant, &mut out.turns, &mut tool_result_idx);

    // Assign stable, unique, index-based ids (the transcript is append-only, so
    // positional ids are stable across re-parses).
    for (i, turn) in out.turns.iter_mut().enumerate() {
        turn.id = format!("grok-turn-{i}");
    }
    Ok(out)
}

fn update_text(update: &Value) -> String {
    update
        .pointer("/content/text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Classify a `user_message_chunk`'s `content` into a display block.
///
/// Grok sends prose as `{type:"text"}`. Current codeg prompts send a native
/// `{type:"image"}` chunk (so grok's describe sidecar runs). Older transcripts
/// still carry the embedded `{type:"resource", resource:{blob, mimeType, uri}}`
/// shape from when we followed grok's `image:false` advertisement.
///
/// All of it goes through [`user_turn_block_from_wire`], the
/// one projection every surface uses for a user's own message — so both image
/// carriages become a thumbnail, and an attached file becomes the same
/// `[name](uri)` marker a viewer saw live. This parser used to fold resources
/// itself and had no `resource_link` case at all, which silently dropped
/// plain file attachments from a reloaded grok turn.
///
/// Returns the block the chunk was SENT as, so the caller can both render it
/// (via [`user_turn_block`]) and tell prose from an attachment
/// — an attachment projects to a `Text` marker, and the conversation's title
/// must not latch onto `[report.pdf](…)` when the prose follows it.
///
/// `None` for a chunk with nothing to render (empty prose, a malformed
/// resource): the caller still opens the user turn, it just starts empty.
fn user_chunk_to_input(update: &Value) -> Option<PromptInputBlock> {
    prompt_block_from_wire(update.get("content")?)
}

// ---------------------------------------------------------------------------
// chat_history.jsonl — grok native ask_user_question answers
// ---------------------------------------------------------------------------

/// Inject the user's `ask_user_question` picks — recorded only in
/// `chat_history.jsonl`, never in `updates.jsonl` — into the matching ToolResult
/// so the `AskQuestionResultCard` renders the answer instead of "未选择". Mirrors
/// the live path (`connection.rs::handle_grok_ask_user_question`): both feed the
/// card the same `{answers, declined}` envelope with an empty `header`, so a
/// conversation renders identically live and after reload. No-op when there is no
/// ask or `chat_history.jsonl` is absent.
fn inject_grok_ask_answers(
    turns: &mut [MessageTurn],
    chat_history: impl IntoIterator<Item = Result<Value>>,
) -> Result<()> {
    // The native ask carries meta `x.ai/tool.kind == "ask_user"`, which the
    // tool_call arm mapped to this tool name; collect those call ids.
    let ask_ids: std::collections::HashSet<String> = turns
        .iter()
        .flat_map(|t| t.blocks.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolUse {
                tool_use_id: Some(id),
                tool_name,
                ..
            } if tool_name == GROK_ASK_TOOL_NAME => Some(id.clone()),
            _ => None,
        })
        .collect();
    if ask_ids.is_empty() {
        return Ok(());
    }
    let answers = read_grok_ask_answers(chat_history, &ask_ids)?;
    if answers.is_empty() {
        return Ok(());
    }
    for turn in turns.iter_mut() {
        for block in turn.blocks.iter_mut() {
            if let ContentBlock::ToolResult {
                tool_use_id: Some(id),
                output_preview,
                is_error,
                ..
            } = block
            {
                if let Some(env) = answers.get(id) {
                    *output_preview = Some(env.clone());
                    *is_error = false;
                }
            }
        }
    }
    Ok(())
}

/// Read `chat_history.jsonl` and, for each `tool_result` whose `tool_call_id` is a
/// known ask id, parse its content into the `{answers, declined}` envelope JSON.
/// `chat_history.jsonl` is grok's model-facing transcript; an ask result there is
/// `{type:"tool_result", tool_call_id, content}` and its id matches the
/// `updates.jsonl` call id verbatim. Empty map when the file is missing.
fn read_grok_ask_answers(
    chat_history: impl IntoIterator<Item = Result<Value>>,
    ask_ids: &std::collections::HashSet<String>,
) -> Result<std::collections::HashMap<String, String>> {
    let mut out = std::collections::HashMap::new();
    for record in chat_history {
        let v = record?;
        if v.get("type").and_then(Value::as_str) != Some("tool_result") {
            continue;
        }
        let Some(id) = v.get("tool_call_id").and_then(Value::as_str) else {
            continue;
        };
        if !ask_ids.contains(id) {
            continue;
        }
        let content = v.get("content").and_then(Value::as_str).unwrap_or("");
        if let Some(envelope) = grok_history_answer_to_envelope(content) {
            out.insert(id.to_string(), envelope.to_string());
        }
    }
    Ok(out)
}

/// Parse a grok `ask_user_question` `tool_result` content string into the codeg
/// `{answers, declined}` envelope (the shape `parseAskQuestionOutcome` reads).
///
/// Verified against grok-0.2.101. The accepted template is `User has answered
/// your questions: "Q"="A", "Q2"="B, C". You can now …` (a multi-select value is
/// joined with `, `); the declined / skip_interview template is `The user has
/// indicated they have provided enough answers …` / `(No answer provided)`.
///
/// `header` is emitted empty to match the header-less card input (grok's questions
/// carry no header). Returns `None` for anything that is not one of these shapes,
/// leaving the ToolResult untouched (today's behavior) — safe by construction.
fn grok_history_answer_to_envelope(content: &str) -> Option<Value> {
    let content = content.trim();
    // Declined / skip_interview: distinct template, no per-question picks to show.
    if content.starts_with("The user has indicated they have provided enough answers")
        || content.contains("(No answer provided)")
    {
        return Some(serde_json::json!({ "answers": [], "declined": true }));
    }
    // Accepted: only this exact prefix (English — grok's internal template, not
    // localized) carries `"Q"="A"` pairs.
    if !content.starts_with("User has answered your questions:") {
        return None;
    }
    // Split on the `"` delimiter. For `"Q1"="A1", "Q2"="A2". You can now …` the
    // tokens are ["…: ", Q1, "=", A1, ", ", Q2, "=", A2, ". You can now …"], so a
    // pair is (toks[i], toks[i+2]) with toks[i+1] == "=", advancing by 4. Trailing
    // prose after the last quote is ignored. Lossy only if a question or label
    // contains a literal `"` (then that pair's `=` guard fails and we stop) —
    // questions rarely do, matching the existing text-fallback's tolerance.
    let toks: Vec<&str> = content.split('"').collect();
    let mut answers: Vec<Value> = Vec::new();
    let mut i = 1;
    while i + 2 < toks.len() {
        if toks[i + 1] != "=" {
            break;
        }
        let question = toks[i];
        // Multi-select values are joined with ", "; split them back into the label
        // array the card partitions against the offered options.
        let selected: Vec<String> = toks[i + 2]
            .split(", ")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        answers.push(serde_json::json!({
            "header": "",
            "question": question,
            "selected": selected,
        }));
        i += 4;
    }
    if answers.is_empty() {
        return None;
    }
    Some(serde_json::json!({ "answers": answers, "declined": false }))
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// The call's status as this LINE states it, read from wherever the wire
/// actually put it.
///
/// Grok never sets `update.status` on the initial `tool_call` (486/486 in the
/// real transcripts on this machine) and only sets it on TERMINAL
/// `tool_call_update`s; the affirmative running value — `"Pending"` — lives in
/// the line-level `params._meta.updateParams.status`, a SIBLING of
/// `params.update`. Mid-run cumulative updates carry neither, which correctly
/// yields `None` ("this line states nothing") so the last stated value stands.
/// The inner field wins when both are present (terminal updates carry both);
/// the result is lowercased so the persisted value space stays the model's
/// documented `pending`/`in_progress`/`completed`/`failed` regardless of the
/// meta's capitalized spelling (`"Pending"`, `"Completed"`, …).
fn grok_line_status(line: &Value, update: &Value) -> Option<String> {
    update
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| {
            line.pointer("/params/_meta/updateParams/status")
                .and_then(Value::as_str)
        })
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
}

/// Peel Grok's `use_tool` MCP envelope (`{tool_name, tool_input}`) into its inner
/// `(tool_name, tool_input)`. Mirrors `connection.rs::unwrap_grok_use_tool` so the
/// history and live paths classify Grok's MCP calls identically. Native tools
/// (args at the top level, no such shape) return `None`.
fn unwrap_use_tool(raw_input: Option<&Value>) -> Option<(&str, &Value)> {
    let obj = raw_input?.as_object()?;
    let tool_name = obj
        .get("tool_name")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())?;
    let tool_input = obj.get("tool_input")?;
    Some((tool_name, tool_input))
}

/// Extract the readable text from a Grok MCP `rawOutput`
/// (`{"type":"MCP","output":{"OkayOutput":"…"}}`, an `*Output` error variant, or
/// a bare string `output`). Mirrors `connection.rs::grok_mcp_output_text`. The
/// result text is the first string value under `output`. Non-MCP → `None`.
fn grok_mcp_output_text(raw_output: &Value) -> Option<String> {
    if raw_output.get("type").and_then(Value::as_str) != Some("MCP") {
        return None;
    }
    let output = raw_output.get("output")?;
    if let Some(text) = output.as_str() {
        return (!text.is_empty()).then(|| text.to_string());
    }
    // First NON-EMPTY string value (the singleton `*Output` variant); filter
    // inside `find_map` so an earlier empty sibling can't shadow a later one.
    output
        .as_object()?
        .values()
        .find_map(|v| v.as_str().filter(|s| !s.is_empty()))
        .map(str::to_string)
}

/// Extract the tool output text from a `tool_call_update`. Prefers the ACP
/// `content[]` array (`{type:"content", content:{type:"text", text}}`), then
/// `rawOutput.output_for_prompt` (Bash/terminal), then a `TaskOutput` envelope
/// (background-task polls — see `grok_task_output_envelope`), then an MCP
/// `rawOutput`'s `output` text (`use_tool`). All are cumulative, so the last
/// update per call carries the full output.
fn update_tool_output(update: &Value) -> Option<String> {
    if let Some(items) = update.get("content").and_then(Value::as_array) {
        let mut buf = String::new();
        for item in items {
            if let Some(text) = item
                .get("content")
                .and_then(|c| c.get("text"))
                .and_then(Value::as_str)
            {
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(text);
            }
        }
        if !buf.is_empty() {
            return Some(truncate_str(&buf, GROK_TOOL_OUTPUT_CAP));
        }
    }
    if let Some(text) = update
        .get("rawOutput")
        .and_then(|r| r.get("output_for_prompt"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Some(truncate_str(text, GROK_TOOL_OUTPUT_CAP));
    }
    if let Some(envelope) = update.get("rawOutput").and_then(grok_task_output_envelope) {
        return Some(envelope);
    }
    update
        .get("rawOutput")
        .and_then(grok_mcp_output_text)
        .map(|s| truncate_str(&s, GROK_TOOL_OUTPUT_CAP))
}

fn tool_input_preview(raw: Option<&Value>) -> Option<String> {
    let raw = raw?;
    if raw.is_null() {
        return None;
    }
    let serialized = serde_json::to_string(raw).ok()?;
    Some(truncate_str(&serialized, GROK_TOOL_INPUT_CAP))
}

/// Full valid native MCP input. Core transport applies windows later.
fn grok_mcp_input_preview(input: &Value) -> Option<String> {
    if input.is_null() {
        return None;
    }
    cap_json_to_budget(input, GROK_TOOL_INPUT_CAP)
}

/// Preserve complete valid JSON; the donor helper name is retained for traceability.
fn cap_json_to_budget(value: &Value, _budget: usize) -> Option<String> {
    serde_json::to_string(value).ok()
}

/// Serialize a Grok `TaskOutput` / `SubagentCompleted` `rawOutput` — the result
/// of a `get_command_or_subagent_output` poll — for the frontend, which parses
/// it into a background-task card (`@/lib/background-task`). Returns `None` for
/// every other `rawOutput`, so the caller falls through to its normal paths.
///
/// The WHOLE envelope is passed through verbatim (untruncated in Core;
/// transport provides windows): its `type` discriminator is what lets the frontend
/// claim it without hijacking other JSON tool output, and passing it whole means
/// the variants Grok can put beside it (`Result`, `MultiResult`, `TaskNotFound`)
/// need no per-variant handling here. Without this the readable output — the
/// command, exit code and shell text all live under `Result` — is dropped
/// entirely: `content[]` is absent on these updates, and the `output_for_prompt`
/// / MCP paths don't match.
///
/// `SubagentCompleted` is the sibling `ToolOutput` variant grok 0.2.11x emits
/// when the SAME poll tool retrieves a background SUB-AGENT's result (flat
/// `{subagent_id, subagent_type, tool_calls, turns, duration_ms, …}` fields —
/// 0.2.9x instead folded subagents into a `TaskOutput.Result` whose `command`
/// reads `[subagent:<type>] <description>`). Passing it through the same
/// door keeps subagent polls from being the one shape both paths still drop.
///
/// Shared with the live path (`acp::connection::grok_live_tool_output`) so both
/// hand the frontend a byte-identical string.
pub(crate) fn grok_task_output_envelope(raw_output: &Value) -> Option<String> {
    if !matches!(
        raw_output.get("type").and_then(Value::as_str),
        Some("TaskOutput") | Some("SubagentCompleted")
    ) {
        return None;
    }
    cap_json_to_budget(raw_output, GROK_TASK_OUTPUT_CAP)
}

/// Truncate every string value in a JSON value to `cap` chars, preserving
/// structure so the result re-serializes to valid JSON.

/// Record the agent's own status on the `ToolUse` block correlated to `id`.
///
/// The `ToolUse` is pushed immediately before its placeholder `ToolResult` (see
/// the `tool_call` arm), so it sits one slot earlier — but the slot is only
/// written after its `tool_use_id` is confirmed to match, so a future change to
/// that ordering degrades to "no status" (unknown) rather than to a status
/// stamped on the wrong call. An absent status leaves the slot untouched: the
/// last value the wire actually stated wins.
fn apply_tool_status(
    turn: Option<&mut MessageTurn>,
    tool_result_idx: &std::collections::HashMap<String, usize>,
    id: &str,
    status: Option<&str>,
) {
    let Some(status) = status.filter(|s| !s.is_empty()) else {
        return;
    };
    let Some(turn) = turn else { return };
    let Some(&result_idx) = tool_result_idx.get(id) else {
        return;
    };
    let Some(use_idx) = result_idx.checked_sub(1) else {
        return;
    };
    if let Some(ContentBlock::ToolUse {
        tool_use_id: Some(existing),
        status: slot,
        ..
    }) = turn.blocks.get_mut(use_idx)
    {
        if existing == id {
            *slot = Some(status.to_string());
        }
    }
}

/// Update the `ToolResult` block correlated to `id` in the current turn. Grok's
/// `tool_call_update.content` is cumulative, and callers only pass `Some` output
/// when non-empty, so the last non-empty output wins; `failed` only ever sets
/// the error flag (never clears it). Ordering vs. `task_completed` is enforced
/// by the caller's `finalized_tools` gate, not here.
fn apply_tool_result(
    turn: Option<&mut MessageTurn>,
    tool_result_idx: &std::collections::HashMap<String, usize>,
    id: &str,
    output: Option<String>,
    failed: bool,
) {
    let Some(turn) = turn else { return };
    let Some(&idx) = tool_result_idx.get(id) else {
        return;
    };
    if let Some(ContentBlock::ToolResult {
        output_preview,
        is_error,
        ..
    }) = turn.blocks.get_mut(idx)
    {
        if let Some(text) = output {
            *output_preview = Some(text);
        }
        if failed {
            *is_error = true;
        }
    }
}

/// Per-turn stats accumulated from Grok's metadata and applied to the assistant
/// turn at its boundary. Grok exposes the numbers the message footer needs, but
/// in three sibling places the update loop otherwise ignores: context occupancy
/// and timing in the OUTER `params._meta` (`totalTokens`, `turnStartMs`,
/// `agentTimestampMs`), the model in `params.update._meta.modelId`, and the real
/// token split in `turn_completed`'s `update.usage` (see [`prompt_usage`]).
/// Duration is `end - start` in ms.
#[derive(Default)]
struct GrokTurnMeta {
    /// `params._meta.totalTokens` — the context-window OCCUPANCY, not a spend.
    /// It rides nearly every update and feeds the context ring; it is
    /// deliberately never folded into `usage`, which would bill a prompt's
    /// resident context as if it were freshly consumed input.
    total_tokens: Option<u64>,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    model: Option<String>,
    /// The prompt's real token spend, stated once by `turn_completed`. `None`
    /// for a turn that never completed.
    usage: Option<TurnUsage>,
}

impl GrokTurnMeta {
    /// Fold one update's metadata in. `params_meta` is `params._meta` (token
    /// total + timing); `update_meta` is `params.update._meta` (carries
    /// `modelId`). `totalTokens` climbs as the turn fills the window, so keep
    /// the max; `turnStartMs` is constant per turn (keep the min defensively);
    /// `agentTimestampMs` advances (keep the max as the turn end).
    fn observe(&mut self, params_meta: Option<&Value>, update_meta: Option<&Value>) {
        if let Some(pm) = params_meta {
            if let Some(tt) = pm.get("totalTokens").and_then(Value::as_u64) {
                self.total_tokens = Some(self.total_tokens.map_or(tt, |cur| cur.max(tt)));
            }
            if let Some(s) = pm.get("turnStartMs").and_then(Value::as_i64) {
                self.start_ms = Some(self.start_ms.map_or(s, |cur| cur.min(s)));
            }
            if let Some(e) = pm.get("agentTimestampMs").and_then(Value::as_i64) {
                self.end_ms = Some(self.end_ms.map_or(e, |cur| cur.max(e)));
            }
        }
        if self.model.is_none() {
            self.model = update_meta
                .and_then(|m| m.get("modelId"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
        }
    }

    /// Apply the accumulated stats to a finalized assistant turn, publishing the
    /// turn's context occupancy into the session-level `context_tokens` (the last
    /// turn to state one wins — that is the session's occupancy now). Never
    /// overwrites a turn field already set.
    fn apply(&self, turn: &mut MessageTurn, context_tokens: &mut Option<u64>) {
        if turn.model.is_none() {
            if let Some(model) = &self.model {
                turn.model = Some(model.clone());
            }
        }
        if let Some(tt) = self.total_tokens.filter(|t| *t > 0) {
            *context_tokens = Some(tt);
        }
        if turn.usage.is_none() {
            if let Some(usage) = &self.usage {
                turn.usage = Some(usage.clone());
            }
        }
        if turn.duration_ms.is_none() {
            if let (Some(start), Some(end)) = (self.start_ms, self.end_ms) {
                if end > start {
                    turn.duration_ms = Some((end - start) as u64);
                }
            }
        }
    }
}

/// Read the `usage` object Grok attaches to `turn_completed` into disjoint
/// `TurnUsage` buckets. `None` when it states nothing at all (every counter
/// absent or zero).
///
/// The figures are PER PROMPT, not running session totals — each
/// `turn_completed` carries its own `prompt_id` and, per Grok's docs, "`usage`
/// sums tokens for the prompt, including subagents that finished before turn
/// end" (`~/.grok/docs/user-guide/14-headless-mode.md`). So the snapshot is
/// attached to its own turn as-is and `compute_session_stats` sums the prompts.
/// The counters do climb from one `turn_completed` to the next in a captured
/// session, which reads as cumulative at a glance; it isn't. In `019f96d5` the
/// second prompt states 198457 input over 7 `modelCalls` — a shade under 28.4K
/// per call against that session's 31628-token peak occupancy — whereas reading
/// the pair as cumulative would charge its 2 additional calls 112283 tokens
/// (56K per call), which no call could have sent through a window that size.
///
/// Bucket semantics: this is the ACP `PromptUsage` flavour, whose `inputTokens`
/// is the WHOLE prompt side, with `cachedReadTokens` / `cacheCreationTokens`
/// naming cached SLICES OF IT rather than separate addends —
/// `inputTokens + outputTokens == totalTokens` holds in every capture, which it
/// could not if the cache counters sat outside `inputTokens`. `TurnUsage`
/// follows Anthropic's DISJOINT convention, where `input_tokens` excludes both
/// cache buckets (see `parsers::claude`), so the cached slices are subtracted
/// back out; the four buckets then re-sum to Grok's own `totalTokens`, which is
/// what the composer's breakdown adds up for its "total" row.
/// `reasoningTokens` is a subset of `outputTokens` and so is deliberately
/// dropped, mirroring `parsers::deepseek`.
///
/// Only the ACP spellings are read. Grok's headless projector publishes the same
/// quantities under `cacheReadInputTokens` / `input_tokens`, but with the cache
/// ALREADY subtracted out ("`usage.input_tokens` … are **uncached only**" —
/// same doc). Aliasing those names in here would silently double-subtract the
/// cached prefix the day a build emits them.
fn prompt_usage(usage: &Value) -> Option<TurnUsage> {
    let field = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let input = field("inputTokens");
    let output = field("outputTokens");
    // `cacheCreationTokens` reaches only newer Grok builds; older ones report
    // reads alone, and an absent counter reads as 0 either way.
    let cache_read = field("cachedReadTokens");
    let cache_create = field("cacheCreationTokens");
    if input == 0 && output == 0 && cache_read == 0 && cache_create == 0 {
        return None;
    }
    Some(TurnUsage {
        // Saturating rather than wrapping: a build that ever reports cache
        // counters exceeding `inputTokens` keeps its cache buckets and drops the
        // uncached remainder to zero, instead of inventing a colossal input.
        input_tokens: input
            .saturating_sub(cache_read)
            .saturating_sub(cache_create),
        output_tokens: output,
        cache_creation_input_tokens: cache_create,
        cache_read_input_tokens: cache_read,
    })
}

fn ensure_assistant(assistant: &mut Option<MessageTurn>, ts: DateTime<Utc>) -> &mut MessageTurn {
    if assistant.is_none() {
        *assistant = Some(MessageTurn {
            id: String::new(),
            role: TurnRole::Assistant,
            blocks: Vec::new(),
            timestamp: ts,
            usage: None,
            duration_ms: None,
            model: None,
            completed_at: None,
            agent_message_id: None,
        });
    }
    assistant.as_mut().expect("assistant just set")
}

fn flush_assistant(
    assistant: &mut Option<MessageTurn>,
    turns: &mut Vec<MessageTurn>,
    tool_result_idx: &mut std::collections::HashMap<String, usize>,
) {
    if let Some(turn) = assistant.take() {
        turns.push(turn);
    }
    tool_result_idx.clear();
}

/// Append assistant text, merging into the trailing `Text` block when adjacent
/// (streaming deltas concatenate; distinct segments separated by tools/thoughts
/// stay separate blocks).
fn append_text(turn: &mut MessageTurn, text: String) {
    if text.is_empty() {
        return;
    }
    if let Some(ContentBlock::Text { text: last }) = turn.blocks.last_mut() {
        last.push_str(&text);
    } else {
        turn.blocks.push(ContentBlock::Text { text });
    }
}

fn append_thinking(turn: &mut MessageTurn, text: String) {
    if text.is_empty() {
        return;
    }
    if let Some(ContentBlock::Thinking { text: last }) = turn.blocks.last_mut() {
        last.push('\n');
        last.push_str(&text);
    } else {
        turn.blocks.push(ContentBlock::Thinking { text });
    }
}

// ---------------------------------------------------------------------------
// spawn_subagent — child transcripts, lifecycle pairing, background marker
// ---------------------------------------------------------------------------

fn find_tool_result<'a>(turns: &'a [MessageTurn], call_id: &str) -> Option<&'a ContentBlock> {
    turns.iter().flat_map(|t| t.blocks.iter()).find(|b| {
        matches!(
            b,
            ContentBlock::ToolResult { tool_use_id: Some(id), .. } if id == call_id
        )
    })
}

/// Extract the `subagent_id: <uuid>` line from the background-launch ack
/// ("Subagent started in background.\nsubagent_id: 019f…\ntype: explore…").
/// `None` for any other text — notably a BLOCKING spawn's result, which is the
/// child's final output.
fn subagent_id_from_ack(output: &str) -> Option<String> {
    if !output
        .trim_start()
        .starts_with(GROK_SUBAGENT_STARTED_ACK_PREFIX)
    {
        return None;
    }
    output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("subagent_id:")
            .map(|id| id.trim().trim_matches('`').to_string())
            .filter(|id| !id.is_empty())
    })
}

/// Build the [`BACKGROUND_TASK_MARKER`] payload replacing a background
/// launch's ack text — the exact contract `parseBackgroundTaskMarker`
/// (`src/lib/background-agent.ts`) renders: settled status + folded result,
/// or "launched · result pending" while `status` is null. Byte-compatible
/// with `claude.rs::apply_background_lifecycle`'s shape. Shared with the live
/// `subagent_finished` settle (`acp::connection`) so a card flipped live and
/// one re-parsed from disk render identically. `output` is capped here so both
/// callers stay bounded.
pub(crate) fn grok_background_subagent_marker(
    subagent_id: &str,
    status: Option<&str>,
    output: Option<&str>,
) -> String {
    let payload = serde_json::json!({
        "task_id": subagent_id,
        "status": status,
        "summary": Value::Null,
        "result": output.map(|o| truncate_str(o, GROK_SUBAGENT_RESULT_CAP)),
    });
    format!("{BACKGROUND_TASK_MARKER}{payload}")
}

fn background_marker_payload(lifecycle: &GrokSubagentLifecycle) -> String {
    grok_background_subagent_marker(
        &lifecycle.subagent_id,
        lifecycle.status.as_deref(),
        lifecycle.output.as_deref(),
    )
}

/// Apply one paired subagent's enrichment to its spawn call blocks:
/// `agent_stats` onto the ToolResult, the background marker (when the recorded
/// output was the launch ack), and — for a blocking spawn cut short before its
/// completion frame — the child's final output. An errored spawn keeps its
/// error text untouched.
fn apply_subagent_result(
    turns: &mut [MessageTurn],
    call_id: &str,
    stats: Option<AgentExecutionStats>,
    marker: Option<String>,
    lifecycle: &GrokSubagentLifecycle,
) {
    for turn in turns.iter_mut() {
        for block in turn.blocks.iter_mut() {
            let ContentBlock::ToolResult {
                tool_use_id: Some(id),
                output_preview,
                is_error,
                agent_stats,
                ..
            } = block
            else {
                continue;
            };
            if id != call_id {
                continue;
            }
            if stats.is_some() {
                *agent_stats = stats.clone();
            }
            if let Some(marker) = &marker {
                *output_preview = Some(marker.clone());
            } else if !*is_error
                && output_preview
                    .as_deref()
                    .is_none_or(|o| o.trim().is_empty())
            {
                if let Some(final_output) = &lifecycle.output {
                    *output_preview = Some(final_output.clone());
                }
            }
            return;
        }
    }
}
pub(crate) fn grok_failed_compaction_meta(update: &serde_json::Value) -> serde_json::Value {
    let mut compaction = serde_json::Map::new();
    compaction.insert("version".to_string(), 1.into());
    if let Some(reason) = update
        .get("reason")
        .or_else(|| update.get("message"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
    {
        compaction.insert("error".to_string(), reason.into());
    }
    serde_json::json!({ "contextCompaction": compaction })
}

// Complete Core content: pagination belongs to the transport, not the parser.
fn truncate_str(value: &str, _cap: usize) -> String {
    value.to_owned()
}

pub(super) struct GrokContent {
    pub(super) turns: Vec<MessageTurn>,
    pub(super) context_tokens: Option<u64>,
}
pub(super) fn project(
    rows: impl IntoIterator<Item = Result<Value>>,
    answers: impl IntoIterator<Item = Result<Value>>,
    fallback: DateTime<Utc>,
    session_model: Option<&str>,
    metadata: &HashMap<String, Value>,
    mut child_tools: impl FnMut(&str) -> Vec<AgentToolCall>,
) -> Result<GrokContent> {
    let mut parsed = parse_updates(rows, fallback)?;
    super::common::relocate_orphaned_tool_results(&mut parsed.turns);
    super::common::structurize_read_tool_output(&mut parsed.turns);
    inject_grok_ask_answers(&mut parsed.turns, answers)?;
    enrich_subagents(&mut parsed, metadata, &mut child_tools);
    if let Some(model) = session_model.map(str::to_owned).or(parsed.model) {
        for turn in &mut parsed.turns {
            if matches!(turn.role, TurnRole::Assistant) && turn.model.is_none() {
                turn.model = Some(model.clone());
            }
        }
    }
    super::common::backfill_turn_durations(&mut parsed.turns, &[]);
    Ok(GrokContent {
        turns: parsed.turns,
        context_tokens: parsed.context_tokens,
    })
}

// Same call/lifecycle pairing as GrokParser::attach_subagent_stats. File access
// is replaced by the Core's verified-child loader. No short-id Session fallback.
fn enrich_subagents(
    parsed: &mut ParsedUpdates,
    metadata: &HashMap<String, Value>,
    child_tools: &mut impl FnMut(&str) -> Vec<AgentToolCall>,
) {
    let mut consumed = vec![false; parsed.subagents.len()];
    for call in &parsed.spawn_call_ids {
        let Some((output, is_error)) =
            find_tool_result(&parsed.turns, call).map(|block| match block {
                ContentBlock::ToolResult {
                    output_preview,
                    is_error,
                    ..
                } => (output_preview.clone(), *is_error),
                _ => (None, false),
            })
        else {
            continue;
        };
        let ack = output.as_deref().and_then(subagent_id_from_ack);
        let index = match &ack {
            Some(id) => parsed.subagents.iter().position(|l| &l.subagent_id == id),
            None if is_error => None,
            None => consumed.iter().position(|used| !*used),
        };
        let Some(index) = index else { continue };
        if consumed[index] {
            continue;
        }
        consumed[index] = true;
        let lifecycle = &parsed.subagents[index];
        let disk = metadata.get(&lifecycle.subagent_id);
        let child = lifecycle
            .child_session_id
            .as_deref()
            .or_else(|| disk?.get("child_session_id")?.as_str())
            .filter(|id| super::super::identity(id));
        let tools = child.map(&mut *child_tools).unwrap_or_default();
        let status = lifecycle
            .status
            .clone()
            .or_else(|| disk?.get("status")?.as_str().map(str::to_owned));
        let duration = lifecycle
            .duration_ms
            .or_else(|| disk?.get("duration_ms")?.as_u64());
        let count = lifecycle
            .tool_calls
            .or_else(|| u32::try_from(disk?.get("tool_calls")?.as_u64()?).ok())
            .or_else(|| u32::try_from(tools.len()).ok().filter(|n| *n > 0));
        let stats = if tools.is_empty() && status.is_none() && duration.is_none() && count.is_none()
        {
            None
        } else {
            Some(AgentExecutionStats {
                agent_type: lifecycle.subagent_type.clone(),
                status,
                total_duration_ms: duration,
                total_tokens: lifecycle.tokens_used,
                total_tool_use_count: count,
                tool_calls: tools,
                child_session_id: child.map(str::to_owned),
                ..AgentExecutionStats::default()
            })
        };
        let marker = output
            .as_deref()
            .is_some_and(|s| s.trim_start().starts_with(GROK_SUBAGENT_STARTED_ACK_PREFIX))
            .then(|| background_marker_payload(lifecycle));
        apply_subagent_result(&mut parsed.turns, call, stats, marker, lifecycle);
    }
}

pub(super) fn child_tool_previews(
    rows: impl IntoIterator<Item = Result<Value>>,
    fallback: DateTime<Utc>,
) -> Result<Vec<AgentToolCall>> {
    let parsed = parse_updates(rows, fallback)?;
    let preview = |s: &str| s.chars().take(500).collect::<String>();
    let mut out = Vec::new();
    for turn in parsed.turns {
        let mut pending = None;
        for block in turn.blocks {
            match block {
                ContentBlock::ToolUse {
                    tool_use_id,
                    tool_name,
                    input_preview,
                    ..
                } => {
                    pending = Some((tool_use_id, tool_name, input_preview.map(|s| preview(&s))));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    output_preview,
                    is_error,
                    ..
                } => {
                    if let Some((id, tool_name, input_preview)) = pending.take() {
                        if id == tool_use_id {
                            out.push(AgentToolCall {
                                tool_name,
                                input_preview,
                                output_preview: output_preview.map(|s| preview(&s)),
                                is_error,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(out)
}
