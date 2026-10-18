//! Captures only the native home from the final child launch snapshot. Never
//! serializes the environment or looks up today's configuration for old Runs.
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{agent_profile::AdapterKind, db::Database, runtime::BindNativeSessionCommand};

pub(crate) const ORIGIN_EVENT: &str = "conversation.native_source.recorded";

#[derive(Clone)]
pub struct CapturedNativeSource {
    adapter: AdapterKind,
    host: String,
    home: PathBuf,
}

impl std::fmt::Debug for CapturedNativeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapturedNativeSource")
            .field("adapter", &self.adapter)
            .finish_non_exhaustive()
    }
}

impl CapturedNativeSource {
    /// The caller passes ManagedProcessLaunchSpec's FINAL environment and cwd,
    /// then retains this value on that successfully created Host across reuse.
    pub fn capture(
        adapter: AdapterKind,
        environment: &BTreeMap<OsString, OsString>,
        cwd: &Path,
        host: &str,
    ) -> Option<Self> {
        let (variable, default) = match adapter {
            AdapterKind::CodexCli => ("CODEX_HOME", ".codex"),
            AdapterKind::GrokBuild => ("GROK_HOME", ".grok"),
            _ => return None,
        };
        if !cwd.is_absolute() || !super::identity(host) {
            return None;
        }
        let get = |name: &str| {
            environment
                .iter()
                .find(|(key, _)| {
                    key.to_str().is_some_and(|key| {
                        if cfg!(windows) {
                            key.eq_ignore_ascii_case(name)
                        } else {
                            key == name
                        }
                    })
                })
                .map(|(_, value)| value)
                .filter(|value| !value.is_empty())
        };
        let home = if let Some(value) = get(variable) {
            PathBuf::from(value)
        } else {
            PathBuf::from(get(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?).join(default)
        };
        let home = if home.is_absolute() {
            home
        } else {
            cwd.join(home)
        };
        let text = home.to_str()?;
        if text.len() > 32768 || text.contains('\0') {
            return None;
        }
        Some(Self {
            adapter,
            host: host.into(),
            home,
        })
    }

    pub(crate) fn adapter_kind(&self) -> AdapterKind {
        self.adapter
    }

    pub(crate) fn binding_record(
        &self,
        command: &BindNativeSessionCommand,
        binding: &str,
        generation: i64,
    ) -> OriginRecord {
        OriginRecord {
            schema_version: 1,
            conversation_id: command.conversation_id.clone(),
            agent_run_id: command.agent_run_id.clone(),
            execution_epoch: command.expected_execution_epoch,
            adapter_kind: self.adapter.as_str().into(),
            installation_id: command.adapter_installation_id.clone(),
            binding_id: binding.into(),
            binding_generation: generation,
            compatibility_digest: command.binding_compatibility_digest.clone(),
            parent_native_session_id: command.native_session_id.clone(),
            host_instance_id: self.host.clone(),
            home_path: self.home.to_str().unwrap().into(),
        }
    }
}

/// Stored only in Core's System binding provenance event, never in Context,
/// formatter input, builtin catalogs, launch arguments or model responses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OriginRecord {
    pub(crate) schema_version: u32,
    pub(crate) conversation_id: String,
    pub(crate) agent_run_id: String,
    pub(crate) execution_epoch: i64,
    pub(crate) adapter_kind: String,
    pub(crate) installation_id: String,
    pub(crate) binding_id: String,
    pub(crate) binding_generation: i64,
    pub(crate) compatibility_digest: String,
    pub(crate) parent_native_session_id: String,
    pub(crate) host_instance_id: String,
    pub(crate) home_path: String,
}

pub(crate) fn lookup(
    database: &Database,
    thread: &str,
    conversation: &str,
    run: &str,
    epoch: i64,
    adapter: &str,
    parent: &str,
) -> Result<Option<OriginRecord>> {
    let mut query = database.connection().prepare(
        "SELECT payload_json FROM event_log WHERE event_type=?1 AND camp_id=?2
         AND entity_type='conversation' AND entity_id=?3 AND actor_type='system' AND actor_id=?4
         AND execution_epoch=?5 AND json_extract(payload_json,'$.agentRunId')=?6
         AND json_extract(payload_json,'$.parentNativeSessionId')=?7 ORDER BY rowid",
    )?;
    let rows = query.query_map(
        params![
            ORIGIN_EVENT,
            thread,
            conversation,
            format!("runtime-adapter:{adapter}"),
            epoch,
            run,
            parent
        ],
        |row| row.get::<_, String>(0),
    )?;
    let mut origin: Option<OriginRecord> = None;
    for row in rows {
        let record: OriginRecord =
            serde_json::from_str(&row?).context("native_child.invalid_origin")?;
        ensure!(
            record.schema_version == 1
                && record.conversation_id == conversation
                && record.agent_run_id == run
                && record.execution_epoch == epoch
                && record.adapter_kind == adapter
                && record.parent_native_session_id == parent
                && record.binding_generation >= 1
                && uuid::Uuid::parse_str(&record.binding_id).is_ok()
                && super::identity(&record.host_instance_id)
                && Path::new(&record.home_path).is_absolute(),
            "native_child.invalid_origin"
        );
        if let Some(previous) = &origin {
            // Re-observation of the identical Host is harmless; conflicting
            // origins cannot be chosen by recency or UI preference.
            ensure!(previous == &record, "native_child.ambiguous_origin");
        } else {
            origin = Some(record);
        }
    }
    Ok(origin)
}
