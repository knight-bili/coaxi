//! Complete Qoder record rules from Codeg aff1e6c, Apache-2.0.
//! Core supplies verified records/children. Native environment and discovery
//! are not parser authority. See native-qoder-content-port.json.
use super::super::source::qoder::replay_branch as active_branch;
use super::{
    claude::{
        ClaudeRecordAccumulator, api_error_text, capture_title_record, extract_assistant_content,
        extract_usage, extract_user_content, extract_user_text, group_into_turns,
        is_interrupt_marker, is_meta_message, is_synthetic_assistant,
        latest_turn_prompt_usage_tokens, slash_command_display,
    },
    codex::support,
    common::{infer_context_window_max_tokens, merge_context_window_stats, title_from_user_text},
    model::*,
};
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::atomic::{AtomicBool, Ordering},
};

#[cfg(all(test, feature = "extended-tests"))]
mod reference_tests;

#[cfg(all(test, feature = "extended-tests"))]
pub(in crate::runtime_subagent_session) fn assert_original_cases(root: &std::path::Path) {
    reference_tests::assert_original_cases(root);
}

/// Inputs already correlated by Core. These are display facts, never Native
/// Session addresses or a grant to read another file.
pub(in crate::runtime_subagent_session) struct QoderChild {
    pub parent_tool_use_id: Option<String>,
    pub agent_type: Option<String>,
    pub status: Option<String>,
    pub result: Option<String>,
    pub transcript: super::claude::SubagentTranscript,
    pub usage_calls: Vec<(Option<DateTime<Utc>>, TurnUsage)>,
}

pub(super) struct QoderDetail {
    pub summary: NativeTranscriptSummary,
    pub turns: Vec<MessageTurn>,
    pub session_stats: Option<SessionStats>,
}

pub(super) fn parse(
    records: Vec<Value>,
    fallback: DateTime<Utc>,
    stop: &AtomicBool,
    children: &BTreeMap<String, QoderChild>,
    patch_source: &mut dyn FnMut(&str, Option<&str>) -> Option<Vec<String>>,
) -> Result<QoderDetail> {
    let transcript = parse_transcript(records, stop)?;
    let message_count = transcript.message_count();
    let mut turns = group_into_turns(transcript.messages);
    super::common::relocate_orphaned_tool_results(&mut turns);
    super::common::structurize_read_tool_output(&mut turns);
    support::resolve_patch_line_numbers(&mut turns, transcript.cwd.as_deref(), &mut |path, cwd| {
        patch_source(path, cwd)
    });
    super::common::backfill_turn_durations(&mut turns, &[]);
    attach_child_stats(&mut turns, children, stop)?;
    let used = transcript
        .reported_prompt_tokens
        .or_else(|| latest_turn_prompt_usage_tokens(&turns));
    let maximum = transcript
        .reported_context_window
        .or_else(|| {
            transcript
                .reported_context
                .and_then(ReportedContext::window)
        })
        .or_else(|| infer_context_window_max_tokens(transcript.model.as_deref()));
    // Each child's spend is attributed after this parent's occupancy is read.
    attribute_child_usage(&mut turns, children, stop)?;
    let session_stats = with_reported_context_percent(
        merge_context_window_stats(super::common::compute_session_stats(&turns), used, maximum),
        transcript.reported_context.map(ReportedContext::percent),
    );
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    Ok(QoderDetail {
        summary: NativeTranscriptSummary {
            title: transcript.title,
            model: transcript.model,
            git_branch: transcript.git_branch,
            started_at: transcript.first_ts.unwrap_or(fallback),
            ended_at: transcript.last_ts,
            message_count,
        },
        turns,
        session_stats,
    })
}

pub(in crate::runtime_subagent_session) struct QoderAgentContent {
    pub content: super::NativeContent,
    pub agent_stats: AgentExecutionStats,
}

pub(in crate::runtime_subagent_session) fn read_agent(
    documents: &mut super::super::source::NativeDocuments,
    fallback: DateTime<Utc>,
    stop: &AtomicBool,
    children: &BTreeMap<String, QoderChild>,
    patch_source: &mut dyn FnMut(&str, Option<&str>) -> Option<Vec<String>>,
) -> Result<QoderAgentContent> {
    use super::super::source::DocumentKind;
    use super::records::{JsonRecords, ParseNotes, json_document};
    use anyhow::Context;
    use sha2::{Digest, Sha256};
    let fingerprint = super::fingerprint(&documents.documents);
    let indexes: Vec<_> = documents
        .documents
        .iter()
        .enumerate()
        .filter(|(_, d)| d.kind == DocumentKind::QoderAgent)
        .map(|(i, _)| i)
        .collect();
    ensure!(indexes.len() == 1, "native_child.ambiguous_source");
    let agent = &mut documents.documents[indexes[0]];
    let id = agent
        .source_label
        .as_ref()
        .context("native_child.agent_link_unavailable")?
        .clone();
    let mut notes = ParseNotes {
        unavailable_advisories: documents.unavailable_advisories,
        ..Default::default()
    };
    let mut records: Vec<Value> =
        JsonRecords::new(agent, stop, &mut notes).collect::<Result<_>>()?;
    // A verified agent document is this view's subject. Its original sidechain
    // flag must not hide its own content; normal Session replay remains intact.
    let mut last = records
        .iter()
        .filter(|r| is_content_record(r))
        .find_map(record_timestamp)
        .unwrap_or(fallback);
    for record in &mut records {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        if let Some(agent) = record.get("agentId").filter(|v| !v.is_null()) {
            ensure!(agent == &id, "native_child.agent_mismatch");
        }
        if is_content_record(record) {
            if let Some(at) = record_timestamp(record) {
                last = at;
            } else {
                record["timestamp"] = Value::String(last.to_rfc3339());
            }
        }
        if let Some(value) = record.as_object_mut() {
            value.remove("isSidechain");
        }
    }
    let (calls, timeline, _, _, duration) =
        super::claude::parse_subagent_tool_calls(records.iter().cloned().map(Ok), stop)?;
    let detail = parse(records, fallback, stop, children, patch_source)?;
    let mut agent_type = None;
    let mut status = None;
    let mut result = None;
    for document in &mut documents.documents {
        match document.kind {
            DocumentKind::QoderAgentMetadata => {
                let value = json_document(document, stop)?;
                agent_type = value["agentType"].as_str().map(str::to_string);
            }
            DocumentKind::QoderTask => {
                let value = json_document(document, stop)?;
                status = value["status"].as_str().map(str::to_string);
                result = value["result"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_string);
            }
            _ => {}
        }
    }
    let agent_stats = AgentExecutionStats {
        agent_type,
        status,
        total_duration_ms: duration,
        total_tool_use_count: Some(calls.len() as u32),
        tool_calls: calls,
        timeline,
        recovered_result: result,
        ..Default::default()
    };
    let context_tokens = detail
        .session_stats
        .as_ref()
        .and_then(|s| s.context_window_used_tokens);
    let mut digest = Sha256::new();
    digest.update(fingerprint.as_bytes());
    super::hash_projection(
        &mut digest,
        &detail.turns,
        detail.session_stats.as_ref(),
        Some(&detail.summary),
    )?;
    // Private metadata also participates in refresh identity, not just text.
    digest.update(serde_json::to_vec(&agent_stats)?);
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    Ok(QoderAgentContent {
        content: super::NativeContent {
            turns: detail.turns,
            context_tokens,
            session_stats: detail.session_stats,
            summary: Some(detail.summary),
            snapshot_digest: format!("{:x}", digest.finalize()),
            notes,
            verified_children: Vec::new(),
        },
        agent_stats,
    })
}

fn attach_child_stats(
    turns: &mut [MessageTurn],
    children: &BTreeMap<String, QoderChild>,
    stop: &AtomicBool,
) -> Result<()> {
    let mut pending: HashSet<String> = turns
        .iter()
        .flat_map(|t| t.blocks.iter())
        .filter_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id: Some(id),
                agent_stats: None,
                ..
            } => Some(id.clone()),
            _ => None,
        })
        .collect();
    // Core supplies one source per agent in the same stable name order as the
    // original directory pass. Task metadata alone cannot provide a child.
    for child in children.values() {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        let Some(parent) = child.parent_tool_use_id.as_ref() else {
            continue;
        };
        if !pending.remove(parent) {
            continue;
        }
        let (calls, timeline, _, _, duration) = &child.transcript;
        let stats = AgentExecutionStats {
            agent_type: child.agent_type.clone(),
            status: child.status.clone(),
            total_duration_ms: *duration,
            total_tool_use_count: Some(calls.len() as u32),
            tool_calls: calls.clone(),
            timeline: timeline.clone(),
            recovered_result: child.result.clone(),
            ..Default::default()
        };
        if let Some(slot) = turns
            .iter_mut()
            .flat_map(|t| t.blocks.iter_mut())
            .find_map(|block| match block {
                ContentBlock::ToolResult {
                    tool_use_id: Some(id),
                    agent_stats,
                    ..
                } if id == parent && agent_stats.is_none() => Some(agent_stats),
                _ => None,
            })
        {
            *slot = Some(stats);
        }
    }
    Ok(())
}

fn attribute_child_usage(
    turns: &mut [MessageTurn],
    children: &BTreeMap<String, QoderChild>,
    stop: &AtomicBool,
) -> Result<()> {
    for child in children.values() {
        for (recorded_at, usage) in &child.usage_calls {
            ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
            let target = recorded_at
                .and_then(|at| {
                    turns
                        .iter()
                        .rposition(|t| matches!(t.role, TurnRole::Assistant) && t.timestamp <= at)
                })
                .or_else(|| {
                    turns
                        .iter()
                        .rposition(|t| matches!(t.role, TurnRole::Assistant))
                });
            let Some(turn) = target.and_then(|i| turns.get_mut(i)) else {
                continue;
            };
            turn.usage = Some(match turn.usage.take() {
                Some(own) => TurnUsage {
                    input_tokens: own.input_tokens.saturating_add(usage.input_tokens),
                    output_tokens: own.output_tokens.saturating_add(usage.output_tokens),
                    cache_creation_input_tokens: own
                        .cache_creation_input_tokens
                        .saturating_add(usage.cache_creation_input_tokens),
                    cache_read_input_tokens: own
                        .cache_read_input_tokens
                        .saturating_add(usage.cache_read_input_tokens),
                },
                None => usage.clone(),
            });
        }
    }
    Ok(())
}

struct Transcript {
    messages: Vec<UnifiedMessage>,
    session_id: Option<String>,
    cwd: Option<String>,
    git_branch: Option<String>,
    model: Option<String>,
    title: Option<String>,
    first_ts: Option<DateTime<Utc>>,
    last_ts: Option<DateTime<Utc>>,
    /// Prompt tokens Qoder itself reported (`token-stats`), when it did.
    reported_prompt_tokens: Option<u64>,
    /// Context window Qoder itself reported (`runtime-config.contextWindow`).
    reported_context_window: Option<u64>,
    /// Occupancy from the newest response that stated one, if any.
    reported_context: Option<ReportedContext>,
}

/// Qoder's own context-occupancy report, attached to every response's `usage`
/// as `context_usage_ratio`.
///
/// Worth reading even though the same object carries token counters, because
/// for Qoder's hosted models those counters are REDACTED to zero before they
/// reach the transcript (the ratio is computed from the real figures first and
/// copied onto the redacted usage, so it survives — see
/// `acp::registry`'s `QODER_EXPOSE_TOKEN_USAGE` note). For a session recorded
/// without that env this is the only occupancy signal in the file.
///
/// It also names the window, which nothing else here does: `token-stats` gives
/// prompt tokens only, `runtime-config.contextWindow` is null even when Qoder
/// writes the record, and the model ids are account-internal keys that
/// [`infer_context_window_max_tokens`] cannot match.
#[derive(Debug, Clone, Copy)]
struct ReportedContext {
    /// `0.0..=1.0`, as Qoder clamps it.
    ratio: f64,
    /// Raw `input_tokens` from the SAME usage object, i.e. the numerator of
    /// Qoder's own `min(1, max(0, input_tokens / window))`. Zero when the
    /// counters were redacted.
    ///
    /// This is the WHOLE prompt, cached prefix included (see
    /// [`qoder_turn_usage`]), so after that normalization it equals the turn's
    /// [`latest_turn_prompt_usage_tokens`] — which is what keeps the gauge's
    /// percentage and its used/max pair from contradicting each other.
    input_tokens: u64,
}

impl ReportedContext {
    fn percent(self) -> f64 {
        self.ratio * 100.0
    }

    /// The model's context window, recovered by inverting Qoder's own formula.
    ///
    /// Exact in practice — the ratio is written as a full-precision float, so
    /// the division lands on the integer (verified against a live transcript:
    /// `0.015572222222222222 × 180000 = 2803`, and `2803 / ratio` rounds back
    /// to 180000).
    ///
    /// `None` unless the inversion is trustworthy. A redacted (zero) numerator
    /// carries no information, and a ratio Qoder clamped to 1.0 means "full or
    /// overflowing", where the quotient is the occupancy rather than the
    /// window. Both would otherwise invent a number and show it as fact.
    fn window(self) -> Option<u64> {
        if self.input_tokens == 0 || self.ratio <= 0.0 || self.ratio >= 1.0 {
            return None;
        }
        let window = (self.input_tokens as f64 / self.ratio).round();
        if !window.is_finite() || window < self.input_tokens as f64 {
            return None;
        }
        // Past 2^53 an f64 no longer holds every integer, and past `u64::MAX`
        // the cast below SATURATES rather than failing — so a preposterous
        // ratio (1 token at 5e-20) would otherwise be reported as a real
        // window instead of no window at all. 2^53 is the boundary where the
        // arithmetic itself stops being exact, which is the honest place to
        // stop trusting it; every real context window is nine orders of
        // magnitude below it.
        const MAX_EXACT_INTEGER: f64 = 9_007_199_254_740_992.0; // 2^53
        if window > MAX_EXACT_INTEGER {
            return None;
        }
        Some(window as u64)
    }
}

/// Qoder's `usage` as ANTHROPIC-shaped counters.
///
/// The field NAMES are Anthropic's, but the semantics behind them are OpenAI's
/// — Qoder's own `model_config` says `"format":"openai"`, and its gateway fills
/// `input_tokens` with the whole prompt, of which `cache_read_input_tokens` is
/// a SUBSET rather than a second helping. Summing the two (which every shared
/// helper does, because for Claude they really are disjoint) counts the cached
/// prefix twice: a real session measured 20513 input / 18871 cache-read, so the
/// gauge read 39384 against a 200000 window — 19.7% — while Qoder itself
/// reported 10.26% for the same turn.
///
/// Splitting the cached part out restores the invariant the rest of the
/// pipeline assumes, and makes `input + cache_read + cache_creation` come back
/// out as the original `input_tokens` — the exact numerator of
/// [`ReportedContext`]'s ratio, so the percentage and the used/max pair agree
/// by construction.
///
/// Same correction `parsers::codex::codex_usage_counters` applies for the same
/// reason. This is what `FACT_SCHEMA_VERSION` 3 exists for: sessions with real
/// counters are not only the ones codeg launched with the usage launch env —
/// a custom/BYO model has always exposed them, and the parser reads every
/// session under `~/.qoder/projects` — so stored rows need a full rebuild
/// under the new rule.
fn qoder_turn_usage(value: &Value) -> Option<TurnUsage> {
    let mut usage = extract_usage(value)?;
    let cached = usage
        .cache_read_input_tokens
        .saturating_add(usage.cache_creation_input_tokens);
    // `saturating_sub` rather than an assertion: a payload where the cached
    // part exceeds the prompt is nonsense, and clamping to zero degrades to
    // "the prompt was entirely cache" instead of wrapping to ~1.8e19.
    usage.input_tokens = usage.input_tokens.saturating_sub(cached);
    Some(usage)
}

/// Read `message.usage.context_usage_ratio` off an assistant record.
///
/// Absent on every record Qoder wrote before it started reporting one, and on
/// responses that never reached the model, so a missing field is ordinary.
fn extract_reported_context(value: &Value) -> Option<ReportedContext> {
    let usage = value.get("message")?.get("usage")?;
    let ratio = usage.get("context_usage_ratio").and_then(Value::as_f64)?;
    if !ratio.is_finite() || ratio < 0.0 {
        return None;
    }
    Some(ReportedContext {
        ratio: ratio.min(1.0),
        input_tokens: usage
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

impl Transcript {
    /// Messages, not raw records: the count the sessions list shows should be
    /// what the conversation actually renders, so rewound branches, `isMeta`
    /// injections and `<synthetic>` placeholders are already gone. A failed
    /// turn's line is drawn but is nobody's message, so it does not count —
    /// as Claude Code's own does not (`parsers::claude`).
    fn message_count(&self) -> u32 {
        self.messages
            .iter()
            .filter(|m| !matches!(m.content.as_slice(), [ContentBlock::TurnError { .. }]))
            .count() as u32
    }
}
fn is_content_record(value: &Value) -> bool {
    matches!(
        value.get("type").and_then(Value::as_str),
        Some("user" | "assistant")
    )
}

fn is_sidechain(value: &Value) -> bool {
    value
        .get("isSidechain")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// ISO-8601 `timestamp` string on a content record.
fn record_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    value
        .get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.with_timezone(&Utc))
}

/// The assistant records that are bookkeeping rather than an answer.
///
/// Beyond Claude's `<synthetic>` placeholder, Qoder writes a FAILED turn as a
/// real assistant record: `isApiErrorMessage: true`, `model: "<synthetic>"`,
/// and a `content` whose text is the raw provider payload (e.g.
/// `{"pricingUrl":"https://qoder.com/pricing?client=qoder"}` for an
/// unentitled account). Rendering that as the model's reply puts an internal
/// error string in the assistant's mouth — and it is the FIRST thing an
/// unauthenticated user would see, since every turn fails that way. So it is
/// drawn as the turn's failure line instead (`claude::api_error_text`).
fn is_non_conversational_assistant(value: &Value) -> bool {
    is_synthetic_assistant(value)
        || value
            .get("isApiErrorMessage")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}
fn parse_transcript(records: Vec<Value>, stop: &AtomicBool) -> Result<Transcript> {
    let mut session_id = None;
    let mut cwd = None;
    let mut git_branch = None;
    let mut launch_model = None;
    let mut custom_title = None;
    let mut ai_title = None;
    let mut reported_prompt_tokens = None;
    let mut reported_context_window = None;

    // Metadata sweep over EVERY record, including the tail repeats: these are
    // snapshots, so the newest value of each wins. Deliberately independent of
    // the branch walk — a title or a context window stays true no matter which
    // branch is live.
    for value in &records {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        let record_type = value.get("type").and_then(Value::as_str).unwrap_or("");
        if is_sidechain(value) {
            continue;
        }
        capture_title_record(value, record_type, &mut custom_title, &mut ai_title);
        if session_id.is_none() {
            session_id = value
                .get("sessionId")
                .and_then(Value::as_str)
                .map(String::from);
        }
        if cwd.is_none() {
            cwd = value.get("cwd").and_then(Value::as_str).map(String::from);
        }
        if git_branch.is_none() {
            git_branch = value
                .get("gitBranch")
                .and_then(Value::as_str)
                .map(String::from);
        }
        match record_type {
            "runtime-config" => {
                if let Some(model) = value.get("model").and_then(Value::as_str) {
                    launch_model = Some(model.to_string());
                }
                // The CLI's own window for the launch model. Authoritative where
                // it exists: `infer_context_window_max_tokens` cannot size a
                // Qoder model id (`qmodel_38max` and friends carry no lane
                // marker), so without this the gauge simply has no maximum.
                if let Some(window) = value
                    .get("contextWindow")
                    .and_then(Value::as_u64)
                    .filter(|w| *w > 0)
                {
                    reported_context_window = Some(window);
                }
            }
            "token-stats" => {
                if let Some(prompt_tokens) = value
                    .get("promptTokenCount")
                    .and_then(Value::as_u64)
                    .filter(|t| *t > 0)
                {
                    reported_prompt_tokens = Some(prompt_tokens);
                }
            }
            _ => {}
        }
    }

    let mut messages: Vec<UnifiedMessage> = Vec::new();
    let mut usage_owner_by_message_id: HashMap<String, usize> = HashMap::new();
    // `message.id` of the assistant response currently accumulating; `None`
    // whenever the previous message was not one of its fragments.
    let mut pending_assistant_chat_id: Option<String> = None;
    let mut model: Option<String> = None;
    let mut first_prompt_title: Option<String> = None;
    let mut first_ts: Option<DateTime<Utc>> = None;
    let mut last_ts: Option<DateTime<Utc>> = None;
    // Newest wins, and only over the ACTIVE branch: occupancy is a statement
    // about the context the next prompt will carry, so a figure left behind by
    // a rewound branch describes history that no longer exists.
    let mut reported_context: Option<ReportedContext> = None;

    for index in active_branch(&records) {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        let value = &records[index];
        // Addressed to the MODEL, not spoken by anyone: hook injections, the
        // max-output-token retry prompt and slash-command expansions all arrive
        // as `isMeta` records that ALSO carry `origin.kind: "human"`, so origin
        // alone cannot tell them apart from a real prompt.
        if is_meta_message(value) || is_interrupt_marker(value) {
            continue;
        }
        let record_type = value.get("type").and_then(Value::as_str).unwrap_or("");
        if record_type == "assistant" && is_non_conversational_assistant(value) {
            // A FAILED turn's record still closes its round, as the failure
            // line rather than as the model's reply.
            if let Some(failure) = api_error_text(value).and_then(|text| {
                let uuid = value.get("uuid").and_then(Value::as_str).unwrap_or("");
                let timestamp = record_timestamp(value).or(last_ts)?;
                support::turn_error_message(uuid.to_string(), &text, timestamp)
            }) {
                pending_assistant_chat_id = None;
                messages.push(failure);
            }
            continue;
        }
        let Some(timestamp) = record_timestamp(value).or(last_ts) else {
            continue;
        };
        let uuid = value.get("uuid").and_then(Value::as_str).map(String::from);

        match record_type {
            "user" => {
                // A slash command is persisted as the tag soup Claude Code
                // invented and Qoder copied verbatim (`command-name` /
                // `command-message` / `command-args` are literal string
                // constants in the 1.1.23 bundle). `strip_system_tags` erases
                // ALL of them, so without reconstructing the invocation first
                // the record renders empty and is dropped — the `/command` the
                // user typed just disappears from the transcript.
                let slash = raw_user_text(value)
                    .as_deref()
                    .and_then(slash_command_display);
                // Otherwise: both content shapes — a bare string, and the block
                // array Qoder writes for every ACP-entrypoint prompt and for any
                // prompt carrying an attachment. Tool results (which Qoder, like
                // Claude Code, delivers as `user` records) come back as
                // `ToolResult` blocks and are folded into the assistant turn by
                // `group_into_turns`.
                let content = match &slash {
                    Some(display) => vec![ContentBlock::Text {
                        text: display.clone(),
                    }],
                    None => extract_user_content(value),
                };
                if content.is_empty() {
                    continue;
                }
                let furniture = is_transcript_furniture(value);
                if !furniture && first_prompt_title.is_none() && is_human_prompt(value) {
                    if let Some(text) = slash.or_else(|| extract_user_text(value)) {
                        first_prompt_title = Some(title_from_user_text(&text));
                    }
                }
                messages.push(UnifiedMessage {
                    id: uuid.unwrap_or_else(|| format!("q-user-{}", messages.len())),
                    role: if furniture {
                        MessageRole::System
                    } else {
                        MessageRole::User
                    },
                    content,
                    timestamp,
                    usage: None,
                    duration_ms: None,
                    model: None,
                    completed_at: Some(timestamp),
                    agent_message_id: None,
                });
                pending_assistant_chat_id = None;
            }
            "assistant" => {
                let content = extract_assistant_content(value);
                if content.is_empty() {
                    continue;
                }
                // A blank `model` names nothing, so it is dropped here, once,
                // for every use below: stored, it would overwrite the last valid
                // model, block the launch-model fallback, and give its bubble
                // (and turn) an empty name.
                let entry_model = value
                    .pointer("/message/model")
                    .and_then(Value::as_str)
                    .filter(|m| !m.trim().is_empty())
                    .map(String::from);
                // Last NON-EMPTY model wins: a mid-session switch has to land
                // in `summary.model` (the details dialog reads it ahead of the
                // turns) and in the context-window fallback below. Pinning the
                // FIRST model here left both on the session's opening model —
                // a switch to Qwen3.8-Flash still displayed DeepSeek-Flash
                // next to the new model's 200K window.
                if entry_model.is_some() {
                    model.clone_from(&entry_model);
                }
                let message_id = value
                    .pointer("/message/id")
                    .and_then(Value::as_str)
                    .map(String::from);

                // One API response streams as SEVERAL records (thinking,
                // tool_use, final text) sharing one `message.id`; adjacent
                // fragments merge into a single bubble. 1.1.65 repeats the
                // response's full usage on every one of them, 1.1.67 writes it
                // on the last one only; either way the claim below keeps it on
                // exactly one message — including across the turn boundary a
                // tool result opens, where the trailing fragment lands in a
                // different bubble than its siblings.
                let merges_into_pending = message_id.is_some()
                    && pending_assistant_chat_id == message_id
                    && matches!(
                        messages.last(),
                        Some(UnifiedMessage {
                            role: MessageRole::Assistant,
                            ..
                        })
                    );
                let owner_index = if merges_into_pending {
                    messages.len() - 1
                } else {
                    messages.len()
                };
                let usage = ClaudeRecordAccumulator::claim_assistant_usage(
                    &mut messages,
                    &mut usage_owner_by_message_id,
                    message_id.as_deref(),
                    qoder_turn_usage(value),
                    owner_index,
                );
                // Read off the RAW record rather than the claimed usage: the
                // claim exists to stop one response's counters being added up
                // once per fragment, but occupancy is a level, not a sum, and
                // every fragment that carries usage carries the same one. Taking
                // it here keeps the figure even when the claim handed the usage
                // to an earlier bubble.
                if let Some(reported) = extract_reported_context(value) {
                    reported_context = Some(reported);
                }

                if merges_into_pending {
                    let last = messages.last_mut().expect("checked non-empty");
                    last.content.extend(content);
                    last.completed_at = Some(timestamp);
                    if usage.is_some() {
                        last.usage = usage;
                    }
                    if entry_model.is_some() {
                        last.model = entry_model;
                    }
                } else {
                    messages.push(UnifiedMessage {
                        id: uuid.unwrap_or_else(|| format!("q-assistant-{}", messages.len())),
                        role: MessageRole::Assistant,
                        content,
                        timestamp,
                        usage,
                        duration_ms: None,
                        model: entry_model,
                        completed_at: Some(timestamp),
                        agent_message_id: None,
                    });
                }
                pending_assistant_chat_id = message_id;
            }
            _ => continue,
        }

        first_ts.get_or_insert(timestamp);
        last_ts = Some(timestamp);
    }

    Ok(Transcript {
        messages,
        session_id,
        cwd,
        git_branch,
        // A real reply names the model; `runtime-config` only says what the
        // session was LAUNCHED with, which is also all an error-only session
        // (every turn `<synthetic>`) can offer.
        model: model.or(launch_model),
        // The user's own name for the session, then Qoder's generated one, then
        // the first prompt. Same precedence Qoder's session picker applies.
        title: custom_title.or(ai_title).or(first_prompt_title),
        first_ts,
        last_ts,
        reported_prompt_tokens,
        reported_context_window,
        reported_context,
    })
}

fn is_transcript_furniture(value: &Value) -> bool {
    ["isCompactSummary", "isVisibleInTranscriptOnly", "isVirtual"]
        .iter()
        .any(|flag| value.get(*flag).and_then(Value::as_bool).unwrap_or(false))
}

/// A user record's text BEFORE any tag stripping, across both content shapes.
///
/// Only [`slash_command_display`] wants this: everything else goes through
/// Claude's extractors, which strip the system tags on the way out.
fn raw_user_text(value: &Value) -> Option<String> {
    match value.pointer("/message/content")? {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let texts: Vec<&str> = items
                .iter()
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .collect();
            (!texts.is_empty()).then(|| texts.join("\n"))
        }
        _ => None,
    }
}

/// Whether a `user` record is a prompt a HUMAN typed, as opposed to a tool
/// result or a synthetic continuation.
///
/// Qoder marks real prompts with `origin.kind == "human"`. A record with no
/// `origin` at all is treated as human — the field is a newer addition, and a
/// missing marker must not cost an old session its title.
fn is_human_prompt(value: &Value) -> bool {
    match value.pointer("/origin/kind").and_then(Value::as_str) {
        Some(kind) => kind == "human",
        None => true,
    }
}
pub(super) fn subagent_call_usage(
    records: impl IntoIterator<Item = Result<Value>>,
    stop: &AtomicBool,
) -> Result<Vec<(Option<DateTime<Utc>>, TurnUsage)>> {
    let mut per_call: HashMap<String, (Option<DateTime<Utc>>, TurnUsage)> = HashMap::new();
    for record in records {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        let value = record?;
        if value.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(usage) = qoder_turn_usage(&value) else {
            continue;
        };
        // A record with no `message.id` cannot be grouped with anything, so it
        // gets a key of its own rather than being dropped — the parent
        // transcript and claude's sub-agent reader treat the same shape so.
        let key = value
            .pointer("/message/id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map(String::from)
            .unwrap_or_else(|| format!("__ungrouped-{}__", per_call.len()));
        let billable = |u: &TurnUsage| {
            u.input_tokens
                .saturating_add(u.output_tokens)
                .saturating_add(u.cache_creation_input_tokens)
                .saturating_add(u.cache_read_input_tokens)
        };
        match per_call.get_mut(&key) {
            Some(slot) => {
                if billable(&usage) > billable(&slot.1) {
                    slot.1 = usage;
                }
                if slot.0.is_none() {
                    slot.0 = record_timestamp(&value);
                }
            }
            None => {
                per_call.insert(key, (record_timestamp(&value), usage));
            }
        }
    }
    Ok(per_call.into_values().collect())
}

pub fn with_reported_context_percent(
    stats: Option<SessionStats>,
    percent: Option<f64>,
) -> Option<SessionStats> {
    let Some(percent) = percent.filter(|p| p.is_finite()) else {
        return stats;
    };
    let percent = percent.clamp(0.0, 100.0);
    match stats {
        Some(mut s) => {
            s.context_window_usage_percent = Some(percent);
            Some(s)
        }
        None => Some(SessionStats {
            total_usage: None,
            total_tokens: None,
            total_duration_ms: 0,
            context_window_used_tokens: None,
            context_window_max_tokens: None,
            context_window_usage_percent: Some(percent),
        }),
    }
}
