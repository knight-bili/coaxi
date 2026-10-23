//! Read-only content projected from verified snapshots. No startup records,
//! arbitrary JSON, Native paths or model execution are exposed by this layer.
mod common;
mod grok;
pub(crate) mod model;
mod prompt;
pub(super) mod records;

use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

use super::{
    ObservedNativeSessionTarget,
    source::{DocumentKind, NativeDocument, NativeDocuments},
};
use model::MessageTurn;
pub(super) mod claude;
pub(super) mod claude_read;
mod codex;
mod grok_stats;
pub(super) mod legacy;
mod project;
pub(super) mod qoder;

pub(crate) use records::ParseNotes;
use records::{JsonRecords, json_document, text_document};

pub(crate) struct NativeContent {
    pub(crate) turns: Vec<MessageTurn>,
    pub(crate) context_tokens: Option<u64>,
    pub(crate) session_stats: Option<model::SessionStats>,
    pub(crate) summary: Option<model::NativeTranscriptSummary>,
    pub(crate) snapshot_digest: String,
    pub(crate) notes: ParseNotes,
    /// Observed Native ids are not UI grants. Later transport mints references
    /// only for entries whose full parent/child source was verified here.
    pub(crate) verified_children: Vec<String>,
}

/// Explicit User import is a distinct authority from a running native child.
/// Bytes are supplied by Core's selected-file importer; cwd, child ids and
/// file paths inside a transcript never authorize another filesystem read.
pub(crate) fn import_records(
    adapter: &str,
    bytes: &[u8],
    fallback: DateTime<Utc>,
    stop: &AtomicBool,
) -> Result<Value> {
    let mut rows = Vec::new();
    let mut malformed = 0_u64;
    let mut incomplete = false;
    let text = std::str::from_utf8(bytes)?;
    if text.trim_start().starts_with('[') {
        let value: Value = serde_json::from_str(text)?;
        rows = value
            .as_array()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("history.invalid_array"))?;
    } else {
        let lines = text.split_inclusive('\n').collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            ensure!(!stop.load(Ordering::Acquire), "history.cancelled");
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(line) {
                Ok(value) => rows.push(value),
                Err(_) => {
                    malformed += 1;
                    if index + 1 == lines.len() && !line.ends_with('\n') {
                        incomplete = true
                    }
                }
            }
        }
    }
    ensure!(!rows.is_empty(), "history.no_records");
    let unavailable = std::collections::BTreeMap::new();
    let (turns, stats, summary) = match adapter {
        "codex" => {
            let detail = codex::project(
                rows.into_iter().map(Ok),
                fallback,
                stop,
                &mut |_| None,
                &mut |_, _| None,
            )?;
            (detail.turns, detail.session_stats, detail.summary)
        }
        "claude" => {
            let detail = claude_read::parse(
                rows.into_iter().map(Ok),
                fallback,
                stop,
                &mut |_, _| None,
                &unavailable,
                &mut |_, _| None,
            )?;
            (detail.turns, detail.session_stats, detail.summary)
        }
        "qoder" => {
            let detail = qoder::parse(rows, fallback, stop, &Default::default(), &mut |_, _| None)?;
            (detail.turns, detail.session_stats, detail.summary)
        }
        "grok" => {
            let detail = grok::project(
                rows.into_iter().map(Ok),
                std::iter::empty(),
                fallback,
                &grok_stats::SummaryMeta::default(),
                None,
                None,
                &Default::default(),
                |_| Vec::new(),
            )?;
            (detail.turns, detail.session_stats, detail.summary)
        }
        _ => anyhow::bail!("history.unsupported_adapter"),
    };
    ensure!(!stop.load(Ordering::Acquire), "history.cancelled");
    Ok(
        serde_json::json!({"schema_version":1,"turns":turns,"context_tokens":null,"session_stats":stats,"summary":summary,"notes":{"malformed_records":malformed,"incomplete_tail":incomplete,"unavailable_children":0,"unavailable_advisories":0},"child_references":[]}),
    )
}

pub(super) fn read(
    target: &ObservedNativeSessionTarget,
    fallback: DateTime<Utc>,
    stop: &AtomicBool,
) -> Result<NativeContent> {
    match target.adapter_kind.as_str() {
        "codex-cli" => {
            let mut files =
                project::ProjectFiles::new(target.recorded_working_directory.as_deref());
            codex_content(target, fallback, stop, &mut |path, cwd| {
                files.lines(path, cwd, stop)
            })
        }
        "grok-build" => grok_content(target, fallback, stop),
        _ => anyhow::bail!("native_child.unsupported_source"),
    }
}

fn merge_notes(into: &mut ParseNotes, other: ParseNotes) {
    into.malformed_records += other.malformed_records;
    into.incomplete_tail |= other.incomplete_tail;
    into.unavailable_children += other.unavailable_children;
    into.unavailable_advisories += other.unavailable_advisories;
}

fn fingerprint(documents: &[NativeDocument]) -> String {
    let mut items: Vec<_> = documents
        .iter()
        .map(|d| {
            (
                format!("{:?}", d.kind),
                d.source_label.clone(),
                d.snapshot_bytes,
                d.snapshot_digest.clone(),
            )
        })
        .collect();
    items.sort();
    let mut digest = Sha256::new();
    for item in items {
        digest.update(serde_json::to_vec(&item).unwrap());
    }
    format!("{:x}", digest.finalize())
}

fn hash_projection(
    digest: &mut Sha256,
    turns: &[MessageTurn],
    stats: Option<&model::SessionStats>,
    summary: Option<&model::NativeTranscriptSummary>,
) -> Result<()> {
    struct DigestWriter<'a>(&'a mut Sha256);
    impl std::io::Write for DigestWriter<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(DigestWriter(digest), &(turns, stats, summary))?;
    Ok(())
}

fn hash_child_sources(digest: &mut Sha256, sources: &mut [(String, String)]) {
    sources.sort();
    for (id, source_digest) in sources {
        digest.update(id.as_bytes());
        digest.update(source_digest.as_bytes());
    }
}

pub(crate) fn grok_content(
    target: &ObservedNativeSessionTarget,
    fallback: DateTime<Utc>,
    stop: &AtomicBool,
) -> Result<NativeContent> {
    ensure!(
        target.adapter_kind == "grok-build",
        "native_child.unsupported_source"
    );
    let documents = NativeDocuments::open_recorded_cancellable(target, stop)?;
    let root_digest = fingerprint(&documents.documents);
    let mut digest = Sha256::new();
    digest.update(root_digest.as_bytes());
    digest.update(fallback.to_rfc3339().as_bytes());
    let mut updates = None;
    let mut history = None;
    let mut metadata = HashMap::new();
    let mut summary_meta = grok_stats::SummaryMeta::default();
    let mut model_cache = None;
    let mut model_config = None;
    let mut notes = ParseNotes {
        unavailable_advisories: documents.unavailable_advisories,
        ..Default::default()
    };
    for mut document in documents.documents {
        match document.kind {
            DocumentKind::GrokUpdates => {
                ensure!(updates.is_none(), "native_child.ambiguous_source");
                updates = Some(document);
            }
            DocumentKind::GrokChatHistory => {
                ensure!(history.is_none(), "native_child.ambiguous_source");
                history = Some(document);
            }
            DocumentKind::GrokSummary
            | DocumentKind::GrokSubagentMetadata
            | DocumentKind::GrokModelsCache => {
                let value = match json_document(&mut document, stop) {
                    Ok(value) => value,
                    Err(error) if error.is::<serde_json::Error>() => {
                        notes.malformed_records += 1;
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                if document.kind == DocumentKind::GrokSummary {
                    summary_meta = grok_stats::summary_meta(&value);
                } else if document.kind == DocumentKind::GrokModelsCache {
                    model_cache = Some(value);
                } else if value
                    .get("parent_session_id")
                    .is_none_or(|v| v == &target.child_native_session_id)
                    && let Some(label) = document.source_label
                {
                    metadata.insert(label, value);
                }
            }
            DocumentKind::GrokConfig => match text_document(&mut document, stop) {
                Ok(value) => model_config = Some(value),
                Err(error) if error.is::<std::string::FromUtf8Error>() => {
                    notes.malformed_records += 1
                }
                Err(error) => return Err(error),
            },
            _ => anyhow::bail!("native_child.invalid_document"),
        }
    }
    let mut updates = updates.ok_or_else(|| anyhow::anyhow!("native_child.source_unavailable"))?;
    let mut update_notes = ParseNotes::default();
    let mut answer_notes = ParseNotes::default();
    let rows = JsonRecords::new(&mut updates, stop, &mut update_notes);
    let answers: Box<dyn Iterator<Item = Result<Value>> + '_> = match history.as_mut() {
        Some(document) => Box::new(JsonRecords::new(document, stop, &mut answer_notes)),
        None => Box::new(std::iter::empty()),
    };
    let mut child_notes = ParseNotes::default();
    let mut verified = Vec::new();
    let mut child_sources = Vec::new();
    let mut cached = HashMap::<String, Vec<model::AgentToolCall>>::new();
    let parsed = grok::project(
        rows,
        answers,
        fallback,
        &summary_meta,
        model_cache.as_ref(),
        model_config.as_deref(),
        &metadata,
        |id| {
            if let Some(tools) = cached.get(id) {
                return tools.clone();
            }
            let read = (|| -> Result<_> {
                ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
                let mut child = target.clone();
                child.parent_native_session_id = target.child_native_session_id.clone();
                child.child_native_session_id = id.into();
                let mut documents = NativeDocuments::open_recorded_cancellable(&child, stop)?;
                let source_digest = fingerprint(&documents.documents);
                let document = documents
                    .documents
                    .iter_mut()
                    .find(|d| d.kind == DocumentKind::GrokUpdates)
                    .ok_or_else(|| anyhow::anyhow!("native_child.source_unavailable"))?;
                let tools = grok::child_tool_previews(
                    JsonRecords::new(document, stop, &mut child_notes),
                    fallback,
                )?;
                child_sources.push((id.to_owned(), source_digest));
                verified.push(id.into());
                Ok(tools)
            })();
            let tools = match read {
                Ok(tools) => tools,
                Err(_) => {
                    child_notes.unavailable_children += 1;
                    Vec::new()
                }
            };
            cached.insert(id.to_owned(), tools.clone());
            tools
        },
    )?;
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    merge_notes(&mut notes, update_notes);
    merge_notes(&mut notes, answer_notes);
    merge_notes(&mut notes, child_notes);
    hash_child_sources(&mut digest, &mut child_sources);
    verified.sort();
    verified.dedup();
    hash_projection(
        &mut digest,
        &parsed.turns,
        parsed.session_stats.as_ref(),
        Some(&parsed.summary),
    )?;
    Ok(NativeContent {
        turns: parsed.turns,
        context_tokens: parsed.context_tokens,
        session_stats: parsed.session_stats,
        summary: Some(parsed.summary),
        snapshot_digest: format!("{:x}", digest.finalize()),
        notes,
        verified_children: verified,
    })
}

/// Exact Codeg history splice over already verified snapshot documents. This
/// preserves the child's own header, trims declared replay seeds, and retains
/// ordinal-less records; it does not yet substitute for Codex's content parser.
pub(crate) fn codex_history_records(
    documents: &mut NativeDocuments,
    stop: &AtomicBool,
    notes: &mut ParseNotes,
) -> Result<Vec<Value>> {
    ensure!(
        !documents.documents.is_empty(),
        "native_child.source_unavailable"
    );
    let mut parts = Vec::new();
    for document in &mut documents.documents {
        ensure!(
            document.kind == DocumentKind::CodexRollout,
            "native_child.invalid_document"
        );
        let mut rows = JsonRecords::new(document, stop, notes).collect::<Result<Vec<_>>>()?;
        if let Some(cut) = document.seed_start_ordinal
            && let Some(header) = rows
                .iter()
                .take(4)
                .position(|v| v["type"] == "session_meta")
        {
            let boundary = rows
                .iter()
                .position(|v| v["ordinal"].as_u64().is_none_or(|n| n >= cut))
                .unwrap_or(rows.len());
            if boundary > header {
                let mut kept = vec![rows[header].clone()];
                kept.extend(rows.into_iter().skip(boundary));
                rows = kept;
            }
        }
        parts.push(rows);
    }
    let mut assembled = parts.pop().unwrap();
    for index in (0..parts.len()).rev() {
        let mut own = std::mem::take(&mut parts[index]);
        let link = documents.documents[index]
            .history_base
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("native_child.invalid_history_link"))?;
        ensure!(
            link.document_index == index + 1,
            "native_child.invalid_history_link"
        );
        let header = own
            .iter()
            .take(4)
            .position(|v| v["type"] == "session_meta")
            .ok_or_else(|| anyhow::anyhow!("native_child.header_unavailable"))?;
        let mut spliced = vec![own.remove(header)];
        spliced.extend(assembled.into_iter().filter(|v| {
            v["ordinal"]
                .as_u64()
                .is_none_or(|n| n < link.end_ordinal_exclusive)
        }));
        spliced.extend(own.into_iter().filter(|v| {
            v["ordinal"]
                .as_u64()
                .is_none_or(|n| n >= link.end_ordinal_exclusive)
        }));
        assembled = spliced;
    }
    Ok(assembled)
}

/// Pure Codex parsing over the same captured provenance and checked files as
/// Grok. The caller supplies an authorized project-file reader for optional
/// patch geometry; a native transcript's cwd never grants filesystem access.
pub(crate) fn codex_content(
    target: &ObservedNativeSessionTarget,
    fallback: DateTime<Utc>,
    stop: &AtomicBool,
    patch_lines: &mut impl FnMut(&str, Option<&str>) -> Option<Vec<String>>,
) -> Result<NativeContent> {
    ensure!(
        target.adapter_kind == "codex-cli",
        "native_child.invalid_adapter"
    );
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    let mut documents = NativeDocuments::open_recorded_cancellable(target, stop)?;
    let mut digest = Sha256::new();
    digest.update(fingerprint(&documents.documents).as_bytes());
    digest.update(fallback.to_rfc3339().as_bytes());
    let mut notes = ParseNotes::default();
    let rows = codex_history_records(&mut documents, stop, &mut notes)?;
    let mut child_notes = ParseNotes::default();
    let mut verified = Vec::new();
    let mut child_sources = Vec::new();
    let mut cached = HashMap::<String, Option<model::AgentExecutionStats>>::new();
    let detail = codex::project(
        rows.into_iter().map(Ok),
        fallback,
        stop,
        &mut |id| {
            if let Some(stats) = cached.get(id) {
                return stats.clone();
            }
            let read = (|| -> Result<_> {
                ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
                let mut child = target.clone();
                child.parent_native_session_id = target.child_native_session_id.clone();
                child.child_native_session_id = id.to_owned();
                let mut documents = NativeDocuments::open_recorded_cancellable(&child, stop)?;
                let source_digest = fingerprint(&documents.documents);
                let document = documents
                    .documents
                    .first_mut()
                    .ok_or_else(|| anyhow::anyhow!("native_child.source_unavailable"))?;
                let stats = codex::subagent_stats(
                    JsonRecords::new(document, stop, &mut child_notes),
                    stop,
                )?;
                child_sources.push((id.to_owned(), source_digest));
                verified.push(id.to_owned());
                Ok(stats)
            })();
            let stats = match read {
                Ok(stats) => stats,
                Err(_) => {
                    child_notes.unavailable_children += 1;
                    None
                }
            };
            cached.insert(id.to_owned(), stats.clone());
            stats
        },
        patch_lines,
    )?;
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    merge_notes(&mut notes, child_notes);
    hash_child_sources(&mut digest, &mut child_sources);
    verified.sort();
    verified.dedup();
    hash_projection(
        &mut digest,
        &detail.turns,
        detail.session_stats.as_ref(),
        Some(&detail.summary),
    )?;
    Ok(NativeContent {
        turns: detail.turns,
        context_tokens: detail
            .session_stats
            .as_ref()
            .and_then(|s| s.context_window_used_tokens),
        session_stats: detail.session_stats,
        summary: Some(detail.summary),
        snapshot_digest: format!("{:x}", digest.finalize()),
        notes,
        verified_children: verified,
    })
}

#[cfg(all(test, feature = "extended-tests"))]
pub(crate) mod tests;

#[cfg(all(test, feature = "extended-tests"))]
mod reference_tests;
