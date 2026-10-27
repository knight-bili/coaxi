//! Owner-confirmed cleanup of this Host's completed configuration upload cache.
//! User-selected backups, incomplete transfers and remote objects stay outside it.
use super::*;
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Candidate {
    id: String,
    sha256: String,
    bytes: u64,
    at: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Plan {
    command_id: String,
    settings_revision: Option<String>,
    created_at: String,
    items: Vec<Candidate>,
    started: Vec<String>,
    removed: Vec<String>,
}
fn private_shape(path: &Path, directory: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink()
            && if directory {
                metadata.is_dir()
            } else {
                metadata.is_file()
            },
        "config_sync.cleanup_link_or_kind_changed"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "config_sync.cleanup_linked_path"
        );
    }
    Ok(())
}
fn cached(root: &Path, id: &str) -> Result<(PathBuf, PathBuf)> {
    uuid::Uuid::parse_str(id)?;
    let base = root.join("config-sync-cache");
    private_shape(&base, true)?;
    let directory = base.join(id);
    private_shape(&directory, true)?;
    let canonical_base = fs::canonicalize(&base)?;
    let canonical_directory = fs::canonicalize(&directory)?;
    ensure!(
        canonical_directory.parent() == Some(canonical_base.as_path()),
        "config_sync.cleanup_outside_cache"
    );
    let path = directory.join("configuration.coaxi-config");
    private_shape(&path, false)?;
    ensure!(
        fs::canonicalize(&path)?.parent() == Some(canonical_directory.as_path()),
        "config_sync.cleanup_outside_cache"
    );
    let children = fs::read_dir(&directory)?.collect::<std::io::Result<Vec<_>>>()?;
    ensure!(
        children.len() == 1 && children[0].file_name() == "configuration.coaxi-config",
        "config_sync.cleanup_unexpected_files"
    );
    Ok((directory, path))
}
fn completed(root: &Path, id: &str, path: &Path) -> Result<Candidate> {
    ensure!(
        !super::webdav_transfer::active(root, id),
        "config_sync.cleanup_transfer_active"
    );
    let receipt = root.join("webdav-operations").join(format!("{id}.json"));
    private_shape(&receipt, false)?;
    ensure!(
        fs::metadata(&receipt)?.len() <= 1024 * 1024,
        "config_sync.cleanup_receipt_limit"
    );
    let value: Value = serde_json::from_slice(&fs::read(receipt)?)?;
    ensure!(
        value["state"] == "completed"
            && value["request"]["operation"] == "push"
            && value["request"]["kind"] == "configuration"
            && value["request"]["path"] == path.to_string_lossy().as_ref(),
        "config_sync.cleanup_not_completed_cache"
    );
    let (bytes, sha256) = crate::backup::file_digest(path)?;
    ensure!(
        value["request"]["expectedDigest"] == sha256
            && value["result"]["snapshot"]["sha256"] == sha256,
        "config_sync.cleanup_cache_changed"
    );
    Ok(Candidate {
        id: id.into(),
        bytes,
        sha256,
        at: value["at"]
            .as_str()
            .context("config_sync.cleanup_receipt_invalid")?
            .into(),
    })
}
fn plan_path(root: &Path, id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(id)?;
    let directory = root.join("config-sync-cleanup");
    crate::platform::private_storage::prepare_private_directory(&directory)?;
    Ok(directory.join(format!("{id}.json")))
}
fn summary(plan: &Plan) -> Value {
    json!({"commandId":plan.command_id,"createdAt":plan.created_at,"items":plan.items,"removed":plan.removed,"bytes":plan.items.iter().map(|item|item.bytes).sum::<u64>(),"removedBytes":plan.items.iter().filter(|item|plan.removed.contains(&item.id)).map(|item|item.bytes).sum::<u64>()})
}

pub(super) fn preview(
    root: &Path,
    id: &str,
    revision: Option<String>,
    active: Option<&str>,
) -> Result<Value> {
    let file = plan_path(root, id)?;
    ensure!(!file.exists(), "config_sync.cleanup_command_already_used");
    let cache = root.join("config-sync-cache");
    let mut candidates = Vec::new();
    if cache.exists() {
        private_shape(&cache, true)?;
        let directories = fs::read_dir(&cache)?
            .take(10001)
            .collect::<std::io::Result<Vec<_>>>()?;
        ensure!(directories.len() <= 10000, "config_sync.cleanup_scan_limit");
        for entry in directories {
            let id = entry.file_name().to_string_lossy().into_owned();
            if uuid::Uuid::parse_str(&id).is_err() || active == Some(id.as_str()) {
                continue;
            }
            let Ok((_, path)) = cached(root, &id) else {
                continue;
            };
            if let Ok(candidate) = completed(root, &id, &path) {
                candidates.push(candidate)
            }
        }
    }
    candidates.sort_by(|a, b| b.at.cmp(&a.at));
    let kept = candidates.len().min(10);
    let remaining = candidates.len().saturating_sub(kept);
    let items = candidates.into_iter().skip(10).take(20).collect();
    let plan = Plan {
        command_id: id.into(),
        settings_revision: revision,
        created_at: Utc::now().to_rfc3339(),
        items,
        started: Vec::new(),
        removed: Vec::new(),
    };
    crate::platform::private_storage::atomic_write_private_json(&file, &plan)?;
    let mut result = summary(&plan);
    result["kept"] = json!(kept);
    result["remaining"] = json!(remaining);
    Ok(result)
}
pub(super) fn apply(
    root: &Path,
    id: &str,
    revision: Option<&str>,
    active: Option<&str>,
    confirmed: bool,
    status_only: bool,
) -> Result<Value> {
    let file = plan_path(root, id)?;
    private_shape(&file, false)?;
    ensure!(
        fs::metadata(&file)?.len() <= 262144,
        "config_sync.cleanup_plan_limit"
    );
    let mut plan: Plan = serde_json::from_slice(&fs::read(&file)?)?;
    ensure!(
        plan.command_id == id
            && plan.settings_revision.as_deref() == revision
            && plan.items.len() <= 20,
        "config_sync.cleanup_plan_changed"
    );
    if status_only {
        return Ok(summary(&plan));
    }
    ensure!(confirmed, "config_sync.cleanup_confirmation_required");
    let created = chrono::DateTime::parse_from_rfc3339(&plan.created_at)?;
    let age = (Utc::now() - created.with_timezone(&Utc)).num_seconds();
    ensure!((0..900).contains(&age), "config_sync.cleanup_plan_expired");
    for item in plan.items.clone() {
        if plan.removed.contains(&item.id) {
            continue;
        }
        ensure!(
            active != Some(item.id.as_str()) && !super::webdav_transfer::active(root, &item.id),
            "config_sync.cleanup_transfer_active"
        );
        let directory = root.join("config-sync-cache").join(&item.id);
        uuid::Uuid::parse_str(&item.id)?;
        if plan.started.contains(&item.id) && !directory.join("configuration.coaxi-config").exists()
        {
            if directory.exists() {
                private_shape(&directory, true)?;
                ensure!(
                    fs::read_dir(&directory)?.next().is_none(),
                    "config_sync.cleanup_unexpected_files"
                );
                fs::remove_dir(&directory)?;
            }
            plan.removed.push(item.id.clone());
            crate::platform::private_storage::atomic_write_private_json(&file, &plan)?;
            continue;
        }
        let (directory, path) = cached(root, &item.id)?;
        let current = completed(root, &item.id, &path)?;
        ensure!(
            current.sha256 == item.sha256 && current.bytes == item.bytes,
            "config_sync.cleanup_cache_changed"
        );
        if !plan.started.contains(&item.id) {
            plan.started.push(item.id.clone());
            crate::platform::private_storage::atomic_write_private_json(&file, &plan)?;
        }
        fs::remove_file(path)?;
        fs::remove_dir(directory)?;
        plan.removed.push(item.id);
        crate::platform::private_storage::atomic_write_private_json(&file, &plan)?;
    }
    Ok(summary(&plan))
}
