//! Captures the native home and cwd from the final child launch snapshot. Never
//! serializes the environment or looks up today's configuration for old Runs.
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::{agent_profile::AdapterKind, db::Database, runtime::BindNativeSessionCommand};

pub(crate) const ORIGIN_EVENT: &str = "conversation.native_source.recorded";

#[cfg(test)]
#[path = "origin/capture_tests.rs"]
mod tests;

#[derive(Clone)]
pub struct CapturedNativeSource {
    adapter: AdapterKind,
    host: String,
    home: PathBuf,
    working_directory: PathBuf,
}

impl std::fmt::Debug for CapturedNativeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapturedNativeSource")
            .field("adapter", &self.adapter)
            .finish_non_exhaustive()
    }
}

impl CapturedNativeSource {
    /// 只比较 Core 已选择来源与该子进程最终环境中的原生目录；不把路径投影给客户端。
    pub(crate) fn contains_selected_root(&self,selected:&Path)->Result<bool>{
        let home=std::fs::canonicalize(&self.home)?;
        let selected=std::fs::canonicalize(selected)?;
        Ok(selected.starts_with(home))
    }
    /// The caller passes ManagedProcessLaunchSpec's FINAL environment and cwd,
    /// then retains this value on that successfully created Host across reuse.
    pub fn capture(
        adapter: AdapterKind,
        environment: &BTreeMap<OsString, OsString>,
        cwd: &Path,
        host: &str,
    ) -> Option<Self> {
        if !cwd.is_absolute() || !super::identity(host) {
            return None;
        }
        let directory = cwd.to_str()?;
        if directory.len() > 32768 || directory.contains('\0') {
            return None;
        }
        let raw = |name: &str| {
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
        };
        let get = |name: &str| raw(name).filter(|value| !value.is_empty());
        let os_home = || get(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
        let home = match adapter {
            AdapterKind::CodexCli => get("CODEX_HOME")
                .map(PathBuf::from)
                .or_else(|| os_home().map(|root| root.join(".codex")))?,
            AdapterKind::GrokBuild => get("GROK_HOME")
                .map(PathBuf::from)
                .or_else(|| os_home().map(|root| root.join(".grok")))?,
            AdapterKind::ClaudeCodeCli => get("CLAUDE_CONFIG_DIR")
                .map(PathBuf::from)
                .or_else(|| os_home().map(|root| root.join(".claude")))?,
            AdapterKind::QoderCli => {
                // Qoder native config behavior: official 1.1.45/1.1.68, Apache-2.0.
                // See integration/licenses/qoder-native-origin/LICENSE and source inventory.
                // Both reviewed publisher versions validate
                // the name first, use a nullish HOME fallback and normalize NFC.
                let configured_name = match get("QODER_CONFIG_DIR_NAME") {
                    Some(value) => Some(value.to_str()?),
                    None => None,
                };
                let name = qoder_directory_name(configured_name)?;
                if let Some(value) = get("QODER_CONFIG_DIR") {
                    let resolved = lexical_absolute(cwd, Path::new(value), environment)?;
                    PathBuf::from(resolved.to_str()?.nfc().collect::<String>())
                } else {
                    let root = raw("QODER_CLI_HOME")
                        .or_else(|| raw("GEMINI_CLI_HOME"))
                        .filter(|value| !value.is_empty())
                        .map(PathBuf::from)
                        .or_else(os_home)?;
                    let joined = root.join(name);
                    // A relative CLI_HOME is NFC-normalized before the OS uses
                    // cwd. Do not normalize an unrelated cwd along with it.
                    let normalized = PathBuf::from(joined.to_str()?.nfc().collect::<String>());
                    lexical_absolute(cwd, &normalized, environment)?
                }
            }
            _ => return None,
        };
        let home = if home.is_absolute() {
            home
        } else {
            cwd.join(home)
        };
        let text = home.to_str()?;
        if !home.is_absolute() || text.len() > 32768 || text.contains('\0') {
            return None;
        }
        Some(Self {
            adapter,
            host: host.into(),
            home,
            working_directory: cwd.to_path_buf(),
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
            working_directory: Some(self.working_directory.to_str().unwrap().into()),
        }
    }
}

// Qoder's Node resolver uses path.resolve/path.join before opening files.
// Collapse dots lexically, before any filesystem/symlink lookup; no host I/O.
fn lexical_absolute(
    cwd: &Path,
    path: &Path,
    environment: &BTreeMap<OsString, OsString>,
) -> Option<PathBuf> {
    use std::path::Component;
    let _ = environment;
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    #[cfg(windows)]
    let joined = if !path.has_root()
        && let Some(Component::Prefix(prefix)) = path.components().next()
        && let std::path::Prefix::Disk(drive) = prefix.kind()
    {
        let drive = drive.to_ascii_uppercase();
        let same_drive = |path: &Path| {
            matches!(path.components().next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(),
                std::path::Prefix::Disk(value) | std::path::Prefix::VerbatimDisk(value) if value.to_ascii_uppercase() == drive))
        };
        let key = format!("={}:", char::from(drive));
        let base = if same_drive(cwd) {
            cwd.to_path_buf()
        } else {
            environment
                .iter()
                .find(|(name, _)| {
                    name.to_str()
                        .is_some_and(|name| name.eq_ignore_ascii_case(&key))
                })
                .map(|(_, value)| PathBuf::from(value))
                .filter(|value| value.is_absolute() && same_drive(value))
                .unwrap_or_else(|| PathBuf::from(format!("{}:\\", char::from(drive))))
        };
        base.join(path.components().skip(1).collect::<PathBuf>())
    } else {
        joined
    };
    if !joined.is_absolute() {
        return None;
    }
    let mut result = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result.is_absolute().then_some(result)
}

fn qoder_directory_name(name: Option<&str>) -> Option<String> {
    use std::sync::OnceLock;
    static ALLOWED: OnceLock<regex::Regex> = OnceLock::new();
    static DEVICE: OnceLock<regex::Regex> = OnceLock::new();
    let name = name.unwrap_or(".qoder").nfc().collect::<String>();
    let length = name.chars().count();
    if !(1..=64).contains(&length)
        || name.starts_with(' ')
        || name.ends_with(' ')
        || name.ends_with('.')
        || matches!(name.as_str(), "." | "..")
        || !ALLOWED
            .get_or_init(|| regex::Regex::new(r"^[\p{L}\p{N}\p{M}._ -]+$").unwrap())
            .is_match(&name)
        || DEVICE
            .get_or_init(|| {
                regex::Regex::new(r"(?iu)^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)").unwrap()
            })
            .is_match(&name)
        || matches!(
            name.to_lowercase().as_str(),
            ".git"
                | ".svn"
                | ".hg"
                | ".vscode"
                | ".idea"
                | ".husky"
                | ".github"
                | "node_modules"
                | "bower_components"
        )
    {
        return None;
    }
    Some(name)
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) working_directory: Option<String>,
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
        ensure!(
            record.working_directory.as_deref().is_none_or(|directory| {
                directory.len() <= 32768
                    && !directory.contains('\0')
                    && Path::new(directory).is_absolute()
            }),
            "native_child.invalid_origin"
        );
        if let Some(previous) = &mut origin {
            // Re-observation of the identical Host is harmless; conflicting
            // origins cannot be chosen by recency or UI preference.
            let mut comparable = record.clone();
            comparable.working_directory = previous.working_directory.clone();
            ensure!(*previous == comparable, "native_child.ambiguous_origin");
            if let (Some(old), Some(new)) = (&previous.working_directory, &record.working_directory)
            {
                ensure!(old == new, "native_child.ambiguous_origin");
            }
            if previous.working_directory.is_none() {
                previous.working_directory = record.working_directory;
            }
        } else {
            origin = Some(record);
        }
    }
    Ok(origin)
}

/// Evidence carries the provider tool id, not an assumed native Session id.
/// Enumerate only this exact Run/epoch's System-captured sources, then reuse
/// the existing full origin validation; no current settings or latest-wins.
pub(super) fn for_run(
    database: &Database,
    thread: &str,
    conversation: &str,
    run: &str,
    epoch: i64,
    adapter: &str,
) -> Result<Vec<OriginRecord>> {
    let mut query = database.connection().prepare(
        "SELECT DISTINCT json_extract(payload_json,'$.parentNativeSessionId')
         FROM event_log WHERE event_type=?1 AND camp_id=?2 AND entity_type='conversation'
         AND entity_id=?3 AND actor_type='system' AND actor_id=?4 AND execution_epoch=?5
         AND json_extract(payload_json,'$.agentRunId')=?6 ORDER BY 1 LIMIT 65",
    )?;
    let parents = query
        .query_map(
            params![
                ORIGIN_EVENT,
                thread,
                conversation,
                format!("runtime-adapter:{adapter}"),
                epoch,
                run
            ],
            |row| row.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(parents.len() <= 64, "native_child.ambiguous_origin");
    let mut records = Vec::new();
    for parent in parents {
        ensure!(super::identity(&parent), "native_child.invalid_origin");
        if let Some(record) = lookup(database, thread, conversation, run, epoch, adapter, &parent)?
        {
            records.push(record);
        }
    }
    Ok(records)
}
