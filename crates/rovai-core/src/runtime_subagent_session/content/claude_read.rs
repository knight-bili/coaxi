//! Core-only bridge from verified legacy agent bytes to the complete original
//! Claude projection. This does not create a User target or Session identity.
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::super::source::{DocumentKind, NativeDocuments};
use super::{
    NativeContent, ParseNotes, claude::*, codex::support, common, model::*, records::JsonRecords,
};

pub(super) struct ClaudeDetail {
    pub turns: Vec<MessageTurn>,
    pub session_stats: Option<SessionStats>,
    pub summary: NativeTranscriptSummary,
}

pub(super) fn parse(
    records: impl IntoIterator<Item = Result<Value>>,
    fallback: DateTime<Utc>,
    stop: &AtomicBool,
    child_transcript: &mut dyn FnMut(&str, &str) -> Option<SubagentTranscript>,
    child_usage: &BTreeMap<String, SubagentTranscript>,
    patch_source: &mut dyn FnMut(&str, Option<&str>) -> Option<Vec<String>>,
) -> Result<ClaudeDetail> {
    let mut acc = ClaudeRecordAccumulator::new(child_transcript, fallback);
    for record in records {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        acc.feed_value(record?);
    }
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    acc.finalize_background_lifecycle();
    let ClaudeRecordAccumulator {
        messages,
        cwd,
        git_branch,
        model,
        title,
        ai_title,
        custom_title,
        first_timestamp,
        last_timestamp,
        ..
    } = acc;
    let mut turns = group_into_turns(messages);
    common::relocate_orphaned_tool_results(&mut turns);
    common::structurize_read_tool_output(&mut turns);
    support::resolve_patch_line_numbers(&mut turns, cwd.as_deref(), &mut |path, cwd| {
        patch_source(path, cwd)
    });
    common::backfill_turn_durations(&mut turns, &[]);
    // The parent window is sampled before delegated spend is attributed.
    let used = latest_claude_context_window_used_tokens(&turns);
    let maximum = claude_context_window_max_tokens_for_model(model.as_deref());
    attribute_child_usage(child_usage, &mut turns, stop)?;
    let session_stats =
        merge_claude_context_window_stats(common::compute_session_stats(&turns), used, maximum);
    let summary = NativeTranscriptSummary {
        title: custom_title.or(ai_title).or(title),
        model,
        git_branch,
        started_at: first_timestamp.unwrap_or(fallback),
        ended_at: last_timestamp,
        message_count: support::message_turn_count(&turns),
    };
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    Ok(ClaudeDetail {
        turns,
        session_stats,
        summary,
    })
}

pub(in crate::runtime_subagent_session) fn read_agent(
    documents: &mut NativeDocuments,
    fallback: DateTime<Utc>,
    stop: &AtomicBool,
    child_transcript: &mut dyn FnMut(&str, &str) -> Option<SubagentTranscript>,
    child_usage: &BTreeMap<String, SubagentTranscript>,
    patch_source: &mut dyn FnMut(&str, Option<&str>) -> Option<Vec<String>>,
) -> Result<NativeContent> {
    let fingerprint = super::fingerprint(&documents.documents);
    let mut indexes = documents
        .documents
        .iter()
        .enumerate()
        .filter(|(_, d)| d.kind == DocumentKind::ClaudeAgent)
        .map(|(i, _)| i);
    let index = indexes.next().context("native_child.source_unavailable")?;
    ensure!(indexes.next().is_none(), "native_child.ambiguous_source");
    let mut notes = ParseNotes {
        unavailable_advisories: documents.unavailable_advisories,
        ..Default::default()
    };
    let detail = parse(
        JsonRecords::new(&mut documents.documents[index], stop, &mut notes),
        fallback,
        stop,
        child_transcript,
        child_usage,
        patch_source,
    )?;
    let context_tokens = detail
        .session_stats
        .as_ref()
        .and_then(|s| s.context_window_used_tokens);
    let mut digest = sha2::Sha256::new();
    use sha2::Digest;
    digest.update(fingerprint.as_bytes());
    // The resulting projection also freezes callback-derived stats/patches.
    super::hash_projection(
        &mut digest,
        &detail.turns,
        detail.session_stats.as_ref(),
        Some(&detail.summary),
    )?;
    Ok(NativeContent {
        turns: detail.turns,
        context_tokens,
        session_stats: detail.session_stats,
        summary: Some(detail.summary),
        snapshot_digest: format!("{:x}", digest.finalize()),
        notes,
        verified_children: Vec::new(),
    })
}

// Original claude.rs:3262-3312 attribution, with Core-verified sources in
// deterministic agent-name order instead of ambient directory discovery.
fn attribute_child_usage(
    children: &BTreeMap<String, SubagentTranscript>,
    turns: &mut [MessageTurn],
    stop: &AtomicBool,
) -> Result<()> {
    for (_, _, usage, started_at, _) in children.values() {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        let Some(extra) = usage else {
            continue;
        };
        let target = started_at
            .and_then(|start| {
                turns
                    .iter()
                    .rposition(|t| matches!(t.role, TurnRole::Assistant) && t.timestamp <= start)
            })
            .or_else(|| {
                turns
                    .iter()
                    .rposition(|t| matches!(t.role, TurnRole::Assistant))
            });
        let Some(launcher) = target.and_then(|i| turns.get_mut(i)) else {
            continue;
        };
        launcher.usage = Some(match launcher.usage {
            Some(ref own) => TurnUsage {
                input_tokens: own.input_tokens.saturating_add(extra.input_tokens),
                output_tokens: own.output_tokens.saturating_add(extra.output_tokens),
                cache_creation_input_tokens: own
                    .cache_creation_input_tokens
                    .saturating_add(extra.cache_creation_input_tokens),
                cache_read_input_tokens: own
                    .cache_read_input_tokens
                    .saturating_add(extra.cache_read_input_tokens),
            },
            None => extra.clone(),
        });
    }
    Ok(())
}
