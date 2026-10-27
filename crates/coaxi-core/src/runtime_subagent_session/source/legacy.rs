//! Join a Core-observed ACP tool and captured parent with its native agent.
//! Native ids are discovered from verified records, never supplied by the UI.
use std::{
    collections::HashSet,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use super::{DocumentKind, NativeDocument, NativeDocuments, entries_with_stop, identity};
use crate::runtime_subagent_session::content::records::{JsonRecords, ParseNotes, json_document};
use crate::{agent_profile::AdapterKind, command::canonical_json_digest};
use anyhow::{Context, Result, ensure};
use serde_json::Value;

pub(in crate::runtime_subagent_session) struct DiscoveredAgent {
    pub agent_id: String,
    pub documents: NativeDocuments,
}

pub(in crate::runtime_subagent_session) fn open_observed(
    adapter: AdapterKind,
    home: &Path,
    parent: &str,
    tool: &str,
    input_digest: &str,
    stop: &AtomicBool,
) -> Result<Option<DiscoveredAgent>> {
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    ensure!(
        home.is_absolute() && identity(parent) && identity(tool),
        "native_agent.invalid_reference"
    );
    ensure!(
        matches!(adapter, AdapterKind::ClaudeCodeCli | AdapterKind::QoderCli),
        "native_agent.unsupported_source"
    );
    if !home.try_exists()? {
        return Ok(None);
    }
    let home = std::fs::canonicalize(home).context("native_child.source_unavailable")?;
    let name = format!("{parent}.jsonl");
    let paths: Vec<_> = entries_with_stop(&home, &home.join("projects"), 1, Some(stop))?
        .into_iter()
        .filter(|p| p.file_name().is_some_and(|n| n == name.as_str()))
        .collect();
    if paths.is_empty() {
        return Ok(None);
    }
    ensure!(paths.len() == 1, "native_child.ambiguous_parent_source");
    let path = &paths[0];
    let kind = if adapter == AdapterKind::ClaudeCodeCli {
        DocumentKind::ClaudeParent
    } else {
        DocumentKind::QoderParent
    };
    let mut document = NativeDocument::open_with_cancel(&home, path, kind, Some(stop))?;
    let records = parent_records(&mut document, stop)?;
    if !verify_tool(adapter, &records, parent, tool, input_digest, stop)? {
        return Ok(None);
    }
    let agent = if adapter == AdapterKind::ClaudeCodeCli {
        let mut agents = HashSet::new();
        for record in &records {
            ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
            if record["type"] != "user" || record["isSidechain"] == true {
                continue;
            }
            let Some(blocks) = record.pointer("/message/content").and_then(Value::as_array) else {
                continue;
            };
            let results: Vec<_> = blocks
                .iter()
                .filter(|b| b["type"] == "tool_result")
                .collect();
            if results.iter().any(|b| b["tool_use_id"] == tool) {
                ensure!(results.len() == 1, "native_child.ambiguous_parent_tool");
                if let Some(id) = record
                    .pointer("/toolUseResult/agentId")
                    .and_then(Value::as_str)
                {
                    ensure!(identity(id), "native_child.invalid_reference");
                    agents.insert(id.to_owned());
                }
            }
        }
        ensure!(agents.len() <= 1, "native_child.agent_mismatch");
        let Some(agent) = agents.into_iter().next() else {
            return Ok(None);
        };
        agent
    } else {
        let directory = path.with_extension("").join("subagents");
        let mut paths = entries_with_stop(&home, &directory, 0, Some(stop))?;
        paths.sort();
        let mut found = None;
        for path in paths {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some(id) = name
                .strip_prefix("agent-")
                .and_then(|n| n.strip_suffix(".meta.json"))
            else {
                continue;
            };
            if !identity(id) {
                continue;
            }
            let mut metadata = NativeDocument::open_with_cancel(
                &home,
                &path,
                DocumentKind::QoderAgentMetadata,
                Some(stop),
            )?;
            let value = match json_document(&mut metadata, stop) {
                Ok(value) => value,
                Err(error) if error.is::<serde_json::Error>() => continue,
                Err(error) => return Err(error),
            };
            if value["toolUseId"] == tool {
                if !path
                    .with_file_name(format!("agent-{id}.jsonl"))
                    .try_exists()?
                {
                    continue;
                }
                found = Some(id.to_owned());
                break;
            }
        }
        let Some(agent) = found else {
            return Ok(None);
        };
        agent
    };
    let mut documents = if adapter == AdapterKind::ClaudeCodeCli {
        super::claude::open_agent(
            &home,
            &super::claude::ClaudeAgentLink {
                parent_session_id: parent,
                parent_tool_use_id: tool,
                agent_id: &agent,
            },
            stop,
        )?
    } else {
        super::qoder::open_agent(
            &home,
            &super::qoder::QoderAgentLink {
                parent_session_id: parent,
                parent_tool_use_id: tool,
                agent_id: &agent,
            },
            stop,
        )?
    };
    // Discovery and the final open can observe different file generations.
    // Re-check the exact parent snapshot that is returned with the child.
    let parent_doc = documents
        .documents
        .iter_mut()
        .find(|d| d.kind == kind)
        .context("native_child.source_unavailable")?;
    let returned = parent_records(parent_doc, stop)?;
    ensure!(
        verify_tool(adapter, &returned, parent, tool, input_digest, stop)?,
        "native_agent.tool_changed"
    );
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    Ok(Some(DiscoveredAgent {
        agent_id: agent,
        documents,
    }))
}

fn parent_records(document: &mut NativeDocument, stop: &AtomicBool) -> Result<Vec<Value>> {
    let mut notes = ParseNotes::default();
    let records = JsonRecords::new(document, stop, &mut notes).collect::<Result<_>>()?;
    ensure!(
        notes.malformed_records == 0 && !notes.incomplete_tail,
        "native_child.agent_link_unavailable"
    );
    Ok(records)
}

fn verify_tool(
    adapter: AdapterKind,
    records: &[Value],
    parent: &str,
    tool: &str,
    digest: &str,
    stop: &AtomicBool,
) -> Result<bool> {
    let indexes = if adapter == AdapterKind::QoderCli {
        super::qoder::verified_branch(records, stop)?
    } else {
        (0..records.len()).collect()
    };
    let mut found = false;
    for index in indexes {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        let record = &records[index];
        if record["isSidechain"] == true {
            continue;
        }
        if let Some(id) = record.get("sessionId").filter(|v| !v.is_null()) {
            ensure!(id == parent, "native_child.parent_mismatch");
        }
        if record["type"] != "assistant" {
            continue;
        }
        let Some(blocks) = record.pointer("/message/content").and_then(Value::as_array) else {
            continue;
        };
        for block in blocks {
            if block["type"] != "tool_use" || block["id"] != tool {
                continue;
            }
            let name = block["name"].as_str().unwrap_or_default();
            ensure!(
                name.eq_ignore_ascii_case("Agent") || name.eq_ignore_ascii_case("Task"),
                "native_child.parent_tool_mismatch"
            );
            ensure!(
                canonical_json_digest(block.get("input").unwrap_or(&Value::Null))? == digest,
                "native_agent.input_mismatch"
            );
            found = true;
        }
    }
    Ok(found)
}
