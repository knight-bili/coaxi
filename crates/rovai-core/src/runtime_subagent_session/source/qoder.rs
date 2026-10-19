//! Qoder agent files have their own scoped meta/task relationship. They are
//! not Claude acknowledgement documents or independent Native Sessions.
//! Codeg aff1e6c, qoder.rs:1090-1245, Apache-2.0 (integration/licenses/codeg).
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use super::{DocumentKind, NativeDocument, NativeDocuments, entries_with_stop, identity};
use crate::runtime_subagent_session::content::records::{JsonRecords, ParseNotes, json_document};

mod branch;

pub(in crate::runtime_subagent_session) struct QoderAgentLink<'a> {
    pub parent_session_id: &'a str,
    pub parent_tool_use_id: &'a str,
    pub agent_id: &'a str,
}

pub(in crate::runtime_subagent_session) fn open_agent(
    captured_home: &Path,
    link: &QoderAgentLink<'_>,
    stop: &AtomicBool,
) -> Result<NativeDocuments> {
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    ensure!(
        captured_home.is_absolute(),
        "native_child.invalid_source_root"
    );
    ensure!(
        identity(link.parent_session_id)
            && identity(link.parent_tool_use_id)
            && identity(link.agent_id),
        "native_child.invalid_reference"
    );
    let home = std::fs::canonicalize(captured_home).context("native_child.source_unavailable")?;
    let name = format!("{}.jsonl", link.parent_session_id);
    let parents: Vec<_> = entries_with_stop(&home, &home.join("projects"), 1, Some(stop))?
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|file| file == name.as_str()))
        .collect();
    ensure!(!parents.is_empty(), "native_child.source_unavailable");
    ensure!(parents.len() == 1, "native_child.ambiguous_parent_source");
    let path = &parents[0];
    let mut parent =
        NativeDocument::open_with_cancel(&home, path, DocumentKind::QoderParent, Some(stop))?;
    let mut notes = ParseNotes::default();
    let records: Vec<Value> =
        JsonRecords::new(&mut parent, stop, &mut notes).collect::<Result<_>>()?;
    // Display can fall back to extra history. A file relationship cannot use
    // missing/corrupt branch state as evidence that a call is still in view.
    ensure!(
        notes.malformed_records == 0 && !notes.incomplete_tail,
        "native_child.agent_link_unavailable"
    );
    for value in &records {
        if value["isSidechain"] != true
            && let Some(session) = value.get("sessionId").filter(|v| !v.is_null())
        {
            ensure!(
                session == link.parent_session_id,
                "native_child.parent_mismatch"
            );
        }
    }
    let active = verified_branch(&records, stop)?;
    let mut calls = 0;
    let mut result_found = false;
    for &index in &active {
        let value = &records[index];
        let Some(blocks) = value.pointer("/message/content").and_then(Value::as_array) else {
            continue;
        };
        for block in blocks {
            if value["type"] == "assistant"
                && block["type"] == "tool_use"
                && block["id"] == link.parent_tool_use_id
            {
                let tool = block["name"].as_str().unwrap_or_default();
                ensure!(
                    tool.eq_ignore_ascii_case("Agent") || tool.eq_ignore_ascii_case("Task"),
                    "native_child.parent_tool_mismatch"
                );
                calls += 1;
            }
            result_found |= value["type"] == "user"
                && block["type"] == "tool_result"
                && block["tool_use_id"] == link.parent_tool_use_id;
        }
    }
    ensure!(
        calls == 1 && result_found,
        "native_child.agent_link_unavailable"
    );
    let directory = path.with_extension("").join("subagents");
    let meta_path = directory.join(format!("agent-{}.meta.json", link.agent_id));
    let mut metadata = NativeDocument::open_with_cancel(
        &home,
        &meta_path,
        DocumentKind::QoderAgentMetadata,
        Some(stop),
    )?;
    let value = json_document(&mut metadata, stop)?;
    ensure!(
        value["toolUseId"] == link.parent_tool_use_id,
        "native_child.parent_tool_mismatch"
    );
    if let Some(agent) = value.get("agentId").filter(|v| !v.is_null()) {
        ensure!(agent == link.agent_id, "native_child.agent_mismatch");
    }
    let mut agent = NativeDocument::open_with_cancel(
        &home,
        &directory.join(format!("agent-{}.jsonl", link.agent_id)),
        DocumentKind::QoderAgent,
        Some(stop),
    )?;
    for record in JsonRecords::new(&mut agent, stop, &mut notes) {
        if let Some(id) = record?.get("agentId").filter(|v| !v.is_null()) {
            ensure!(id == link.agent_id, "native_child.agent_mismatch");
        }
    }
    parent.source_label = Some(link.parent_session_id.into());
    agent.source_label = Some(link.agent_id.into());
    metadata.source_label = Some(link.agent_id.into());
    let mut documents = vec![parent, metadata, agent];
    let mut unavailable_advisories = 0;
    let mut paths = entries_with_stop(&home, &directory, 0, Some(stop))?;
    paths.sort();
    for path in paths {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !(name.starts_with("task-") && name.ends_with(".json")) {
            continue;
        }
        let mut task =
            NativeDocument::open_with_cancel(&home, &path, DocumentKind::QoderTask, Some(stop))?;
        let value = match json_document(&mut task, stop) {
            Ok(value) => value,
            Err(error) if stop.load(Ordering::Acquire) => return Err(error),
            Err(_) => {
                unavailable_advisories += 1;
                continue;
            }
        };
        if value["parentToolUseId"] == link.parent_tool_use_id {
            task.source_label = Some(name.into());
            documents.push(task);
            // Original first-in-name-order status/result choice. A task is
            // advisory, never sufficient proof for opening an agent file.
            break;
        }
    }
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    Ok(NativeDocuments {
        documents,
        unavailable_advisories,
    })
}

fn verified_branch(records: &[Value], stop: &AtomicBool) -> Result<Vec<usize>> {
    let mut by_uuid = HashMap::new();
    let mut has_links = false;
    for (index, record) in records.iter().enumerate() {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        if record["isSidechain"] == true {
            continue;
        }
        if let Some(id) = record["uuid"].as_str() {
            if let Some(previous) = by_uuid.get(id) {
                ensure!(
                    records[*previous] == *record,
                    "native_child.ambiguous_parent_record"
                );
            } else {
                by_uuid.insert(id, index);
            }
        }
        has_links |= parent_id(record).is_some();
    }
    if let Some(marker) = records.iter().rfind(|r| r["type"] == "active-leaf") {
        match marker.get("leafUuid") {
            Some(Value::Null) => {}
            Some(Value::String(id)) if by_uuid.contains_key(id.as_str()) => {}
            _ => anyhow::bail!("native_child.invalid_parent_branch"),
        }
    }
    let active = branch::active_branch(records);
    if !has_links {
        return Ok(active);
    }
    let mut verified = HashSet::new();
    for &index in &active {
        let mut current = index;
        let mut visiting = HashSet::new();
        while !verified.contains(&current) {
            ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
            ensure!(
                visiting.insert(current),
                "native_child.invalid_parent_branch"
            );
            ensure!(
                records[current]["uuid"].is_string(),
                "native_child.invalid_parent_branch"
            );
            let Some(parent) = parent_id(&records[current]) else {
                break;
            };
            current = *by_uuid
                .get(parent)
                .context("native_child.invalid_parent_branch")?;
        }
        verified.extend(visiting);
    }
    Ok(active)
}

fn parent_id(record: &Value) -> Option<&str> {
    record["parentUuid"]
        .as_str()
        .filter(|id| !id.is_empty())
        .or_else(|| {
            record["logicalParentUuid"]
                .as_str()
                .filter(|id| !id.is_empty())
        })
}
