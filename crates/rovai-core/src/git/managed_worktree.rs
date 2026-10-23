//! Shared admission for exact worktrees created by the engineering workspace.
use super::*;
use anyhow::ensure;
use serde_json::{Value, json};

fn regular(path: &Path, directory: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(!metadata.file_type().is_symlink() && metadata.is_dir() == directory, "workbench.worktree_owner_mismatch");
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        ensure!(metadata.file_attributes() & 0x400 == 0, "workbench.worktree_owner_mismatch");
    }
    if !directory { ensure!(metadata.is_file() && metadata.len() <= 65536, "workbench.worktree_owner_mismatch"); }
    Ok(())
}

fn text(path: &Path) -> Result<String> {
    regular(path, false)?;
    Ok(fs::read_to_string(path)?)
}

fn referenced(base: &Path, value: &str) -> Result<PathBuf> {
    let value = value.trim();
    ensure!(!value.is_empty() && !value.contains(['\0', '\n', '\r']), "workbench.worktree_owner_mismatch");
    let path = Path::new(value);
    Ok(fs::canonicalize(if path.is_absolute() { path.to_path_buf() } else { base.join(path) })?)
}

fn metadata_directory(marker: &Path) -> Result<PathBuf> {
    if fs::symlink_metadata(marker)?.is_dir() {
        regular(marker, true)?;
        return Ok(fs::canonicalize(marker)?);
    }
    let value = text(marker)?;
    referenced(marker.parent().context("workbench.worktree_owner_mismatch")?,
        value.trim().strip_prefix("gitdir: ").context("workbench.worktree_owner_mismatch")?)
}

pub(super) fn validate(tree: &Path, data: &Path) -> Result<()> {
    let trees = data.join("workspace-trees");
    regular(&trees, true)?;
    ensure!(fs::canonicalize(&trees)? == trees && tree.parent() == Some(trees.as_path()), "workbench.worktree_owner_mismatch");
    regular(tree, true)?;
    let key = tree.file_name().and_then(|name| name.to_str()).context("workbench.worktree_owner_mismatch")?;
    ensure!(key.len() == 64 && key.bytes().all(|byte| byte.is_ascii_hexdigit()), "workbench.worktree_owner_mismatch");
    let owner: Value = serde_json::from_str(&text(&trees.join(format!("{key}.json")))?)?;
    let source = owner["root"].as_str().context("workbench.worktree_owner_mismatch")?;
    let source_path = fs::canonicalize(source)?;
    // The source must remain an ordinary project; no recursive managed-data grant.
    ensure!(source_path.is_dir() && !source_path.starts_with(data) && source_path.parent().is_some()
        && persisted_workspace_path_matches_canonical(Path::new(source), &source_path), "workbench.worktree_owner_mismatch");
    let branch = owner["branch"].as_str().context("workbench.worktree_owner_mismatch")?;
    ensure!(!branch.is_empty() && !branch.starts_with('-') && !branch.contains(['\0', '\r', '\n']), "workbench.worktree_owner_mismatch");
    ensure!(crate::command::canonical_json_digest(&json!({"root":source,"branch":branch}))? == key, "workbench.worktree_owner_mismatch");
    let recorded = owner["path"].as_str().context("workbench.worktree_owner_mismatch")?;
    ensure!(fs::canonicalize(recorded)? == tree && persisted_workspace_path_matches_canonical(Path::new(recorded), tree), "workbench.worktree_owner_mismatch");
    let source_marker = nearest_git_marker(&source_path).context("workbench.worktree_owner_mismatch")?;
    let source_admin = metadata_directory(&source_marker)?;
    let common = if source_admin.join("commondir").exists() {
        referenced(&source_admin, &text(&source_admin.join("commondir"))?)?
    } else { source_admin.clone() };
    let marker = tree.join(".git");
    regular(&marker, false)?;
    let admin = metadata_directory(&marker)?;
    let registrations = common.join("worktrees");
    regular(&registrations, true)?;
    let registrations = fs::canonicalize(registrations)?;
    if fs::symlink_metadata(&source_marker)?.is_file() {
        ensure!(source_admin.parent() == Some(registrations.as_path())
            && referenced(&source_admin, &text(&source_admin.join("gitdir"))?)? == fs::canonicalize(source_marker)?, "workbench.worktree_owner_mismatch");
    }
    ensure!(admin.parent() == Some(registrations.as_path()), "workbench.worktree_owner_mismatch");
    regular(&admin, true)?;
    ensure!(referenced(&admin, &text(&admin.join("commondir"))?)? == common
        && referenced(&admin, &text(&admin.join("gitdir"))?)? == fs::canonicalize(marker)?, "workbench.worktree_owner_mismatch");
    Ok(())
}
