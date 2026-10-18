//! Verified legacy Claude agent documents, not Native Session addresses.
//! Layout/correlation source: fixed Codeg aff1e6c, parsers/claude.rs:2117-2201.
//! This filesystem seam grants no User/API authority. Its home and link must
//! come from a future Core-correlated Claude producer, never raw UI paths/ids.
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result, ensure};

use super::{DocumentKind, NativeDocument, NativeDocuments, entries_with_stop, identity};
use crate::runtime_subagent_session::content::records::{JsonRecords, ParseNotes};

pub(in crate::runtime_subagent_session) struct ClaudeAgentLink<'a> {
    pub parent_session_id: &'a str,
    pub parent_tool_use_id: &'a str,
    pub agent_id: &'a str,
}

pub(in crate::runtime_subagent_session) fn open_agent(
    captured_home: &Path,
    link: &ClaudeAgentLink<'_>,
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
        NativeDocument::open_with_cancel(&home, path, DocumentKind::ClaudeParent, Some(stop))?;
    let mut notes = ParseNotes::default();
    let mut call_found = false;
    let mut acknowledgement_found = false;
    for record in JsonRecords::new(&mut parent, stop, &mut notes) {
        let value = record?;
        if let Some(session) = value.get("sessionId").filter(|value| !value.is_null()) {
            ensure!(
                session == link.parent_session_id,
                "native_child.parent_mismatch"
            );
        }
        let Some(blocks) = value
            .pointer("/message/content")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        if value["type"] == "assistant" {
            for block in blocks {
                if block["type"] == "tool_use" && block["id"] == link.parent_tool_use_id {
                    let tool = block["name"].as_str().unwrap_or_default();
                    ensure!(
                        tool.eq_ignore_ascii_case("Agent") || tool.eq_ignore_ascii_case("Task"),
                        "native_child.parent_tool_mismatch"
                    );
                    call_found = true;
                }
            }
        }
        if value["type"] == "user" {
            let results: Vec<_> = blocks
                .iter()
                .filter(|block| block["type"] == "tool_result")
                .collect();
            if results
                .iter()
                .any(|block| block["tool_use_id"] == link.parent_tool_use_id)
            {
                ensure!(results.len() == 1, "native_child.ambiguous_parent_tool");
                let Some(agent) = value
                    .pointer("/toolUseResult/agentId")
                    .and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                ensure!(agent == link.agent_id, "native_child.agent_mismatch");
                acknowledgement_found = true;
            }
        }
    }
    ensure!(
        call_found && acknowledgement_found,
        "native_child.agent_link_unavailable"
    );
    let agent_path = path
        .with_extension("")
        .join("subagents")
        .join(format!("agent-{}.jsonl", link.agent_id));
    let mut agent = NativeDocument::open_with_cancel(
        &home,
        &agent_path,
        DocumentKind::ClaudeAgent,
        Some(stop),
    )?;
    for record in JsonRecords::new(&mut agent, stop, &mut notes) {
        let value = record?;
        if let Some(id) = value.get("agentId").filter(|value| !value.is_null()) {
            ensure!(id == link.agent_id, "native_child.agent_mismatch");
        }
        // `parentUuid` addresses a message; `sessionId` in an agent log is not
        // an independent Session grant. The exact parent tool acknowledgement
        // plus its scoped directory supplies this legacy relationship.
    }
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    parent.source_label = Some(link.parent_session_id.into());
    agent.source_label = Some(link.agent_id.into());
    Ok(NativeDocuments {
        documents: vec![parent, agent],
        unavailable_advisories: 0,
    })
}
