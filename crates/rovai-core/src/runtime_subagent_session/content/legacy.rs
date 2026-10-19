//! Complete legacy Agent content on the shared read worker, never under SQLite.
use super::{
    NativeContent,
    model::AgentExecutionStats,
    records::{JsonRecords, ParseNotes},
};
use crate::{
    agent_profile::AdapterKind,
    runtime_subagent_session::{
        legacy::{NativeAgentReadPlan, ObservedNativeAgentTarget},
        source::DocumentKind,
    },
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

pub(in crate::runtime_subagent_session) fn read(
    plan: &NativeAgentReadPlan,
    fallback: DateTime<Utc>,
    stop: &AtomicBool,
) -> Result<(
    ObservedNativeAgentTarget,
    NativeContent,
    AgentExecutionStats,
)> {
    let (target, mut documents) = plan.open(stop)?;
    let mut project = super::project::ProjectFiles::new(
        target.origin.working_directory.as_deref().map(Path::new),
    );
    let mut patch = |path: &str, cwd: Option<&str>| project.lines(path, cwd, stop);
    let (content, stats) = match target.adapter_kind {
        AdapterKind::ClaudeCodeCli => {
            let agent = documents
                .documents
                .iter_mut()
                .find(|d| d.kind == DocumentKind::ClaudeAgent)
                .context("native_child.source_unavailable")?;
            let mut notes = ParseNotes::default();
            let (calls, timeline, _, _, duration) = super::claude::parse_subagent_tool_calls(
                JsonRecords::new(agent, stop, &mut notes),
                stop,
            )?;
            let stats = AgentExecutionStats {
                total_duration_ms: duration,
                total_tool_use_count: Some(calls.len() as u32),
                tool_calls: calls,
                timeline,
                ..Default::default()
            };
            let content = super::claude_read::read_agent(
                &mut documents,
                fallback,
                stop,
                &mut |_, _| None,
                &BTreeMap::new(),
                &mut patch,
            )?;
            (content, stats)
        }
        AdapterKind::QoderCli => {
            let read = super::qoder::read_agent(
                &mut documents,
                fallback,
                stop,
                &BTreeMap::new(),
                &mut patch,
            )?;
            (read.content, read.agent_stats)
        }
        _ => anyhow::bail!("native_agent.unsupported_source"),
    };
    ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
    Ok((target, content, stats))
}
