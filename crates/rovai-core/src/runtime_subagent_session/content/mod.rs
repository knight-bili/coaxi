//! Read-only content projected from verified snapshots. No startup records,
//! arbitrary JSON, Native paths or model execution are exposed by this layer.
mod common;
mod grok;
pub(crate) mod model;
mod prompt;
mod records;

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
pub(crate) use records::ParseNotes;
use records::{JsonRecords, json_document};

pub(crate) struct NativeContent {
    pub(crate) turns: Vec<MessageTurn>,
    pub(crate) context_tokens: Option<u64>,
    pub(crate) snapshot_digest: String,
    pub(crate) notes: ParseNotes,
    /// Observed Native ids are not UI grants. Later transport mints references
    /// only for entries whose full parent/child source was verified here.
    pub(crate) verified_children: Vec<String>,
}

fn merge_notes(into: &mut ParseNotes, other: ParseNotes) {
    into.malformed_records += other.malformed_records;
    into.incomplete_tail |= other.incomplete_tail;
    into.unavailable_children += other.unavailable_children;
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

pub(crate) fn grok_content(
    target: &ObservedNativeSessionTarget,
    fallback: DateTime<Utc>,
    stop: &AtomicBool,
) -> Result<NativeContent> {
    ensure!(
        target.adapter_kind == "grok-build",
        "native_child.unsupported_source"
    );
    let documents = NativeDocuments::open_recorded(target)?;
    let root_digest = fingerprint(&documents.documents);
    let mut digest = Sha256::new();
    digest.update(root_digest.as_bytes());
    digest.update(fallback.to_rfc3339().as_bytes());
    let mut updates = None;
    let mut history = None;
    let mut metadata = HashMap::new();
    let mut session_model = None;
    let mut notes = ParseNotes::default();
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
            DocumentKind::GrokSummary | DocumentKind::GrokSubagentMetadata => {
                let value = match json_document(&mut document, stop) {
                    Ok(value) => value,
                    Err(error) if error.is::<serde_json::Error>() => {
                        notes.malformed_records += 1;
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                if document.kind == DocumentKind::GrokSummary {
                    session_model = value["current_model_id"]
                        .as_str()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned);
                } else if value
                    .get("parent_session_id")
                    .is_none_or(|v| v == &target.child_native_session_id)
                    && let Some(label) = document.source_label
                {
                    metadata.insert(label, value);
                }
            }
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
    let mut cached = HashMap::<String, Vec<model::AgentToolCall>>::new();
    let parsed = grok::project(
        rows,
        answers,
        fallback,
        session_model.as_deref(),
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
                let mut documents = NativeDocuments::open_recorded(&child)?;
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
                digest.update(id.as_bytes());
                digest.update(source_digest.as_bytes());
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
    Ok(NativeContent {
        turns: parsed.turns,
        context_tokens: parsed.context_tokens,
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

#[cfg(all(test, feature = "extended-tests"))]
pub(crate) mod tests;
