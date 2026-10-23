//! User workspace operations. Scope roots come from Core, never Renderer paths.
use super::*;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, io::Write, path::Component};

pub(super) static MUTATIONS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Scope {
    pub(super) thread_id: String,
    #[serde(default)]
    pub(super) mission_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileRequest {
    scope: Scope,
    #[serde(default)]
    path: String,
    #[serde(default)]
    after: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileMutation {
    command_id: String,
    scope: Scope,
    action: String,
    path: String,
    #[serde(default)]
    destination: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    expected_digest: Option<String>,
}

fn write_text(path: &Path, text: &str, expected: Option<&str>) -> Result<Value> {
    ensure!(
        text.len() <= 16 * 1024 * 1024 && !text.contains('\0'),
        "workbench.text_too_large"
    );
    let parent = path.parent().context("workbench.invalid_path")?;
    ensure!(parent.is_dir(), "workbench.parent_missing");
    let before = match fs::read(path) {
        Ok(bytes) => Some(digest(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    ensure!(before.as_deref() == expected, "workbench.file_changed");
    let temporary = parent.join(format!(".rovai-edit-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<Value> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        if let Ok(metadata) = fs::metadata(path) {
            file.set_permissions(metadata.permissions())?;
        }
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        let current = if path.exists() {
            Some(digest(&fs::read(path)?))
        } else {
            None
        };
        ensure!(current == before, "workbench.file_changed");
        if before.is_none() {
            fs::hard_link(&temporary, path).context("workbench.destination_exists")?;
            fs::remove_file(&temporary)?;
        } else {
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;
                let destination: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
                let source: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
                let replaced = unsafe {
                    ReplaceFileW(
                        destination.as_ptr(),
                        source.as_ptr(),
                        std::ptr::null(),
                        0,
                        std::ptr::null(),
                        std::ptr::null(),
                    )
                };
                ensure!(
                    replaced != 0,
                    "workbench.replace_failed: {}",
                    std::io::Error::last_os_error()
                );
            }
            #[cfg(not(windows))]
            fs::rename(&temporary, path)?;
        }
        #[cfg(not(windows))]
        fs::File::open(parent)?.sync_all()?;
        Ok(json!({"applied":true,"digest":digest(text.as_bytes())}))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn is_link(metadata: &fs::Metadata) -> bool {
    let mut linked = metadata.file_type().is_symlink();
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        linked |= metadata.file_attributes() & 0x400 != 0;
    }
    linked
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GitRequest {
    scope: Scope,
    action: String,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    remote: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    command_id: Option<String>,
    #[serde(default)]
    expected_revision: Option<String>,
    #[serde(default)]
    confirmed: bool,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Organizer {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    directory_aliases: Vec<DirectoryAlias>,
    #[serde(default)]
    quick_messages: Vec<QuickMessage>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectoryAlias {
    path: String,
    name: String,
    group: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QuickMessage {
    id: String,
    title: String,
    text: String,
}
fn organizer_thread_level() -> String {
    "thread".into()
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OrganizerRequest {
    #[serde(default)]
    scope: Option<Scope>,
    #[serde(default = "organizer_thread_level")]
    level: String,
    #[serde(default)]
    expected_digest: Option<String>,
    #[serde(default)]
    value: Option<Organizer>,
}

fn organizer_metadata_path(
    directory: &Path,
    level: &str,
    scope: &Scope,
    root: &Path,
) -> Result<PathBuf> {
    if level == "global" {
        return Ok(directory.join("global-v1.json"));
    }
    ensure!(
        level == "project" || level == "thread",
        "workbench.invalid_organizer_level"
    );
    let identity = crate::command::canonical_json_digest(&if level == "project" {
        json!({"root":root,"level":"project","version":1})
    } else {
        json!({"scope":scope,"root":root})
    })?;
    Ok(directory.join(format!("{identity}.json")))
}

fn read_organizer(path: &Path) -> Result<Organizer> {
    if path.exists() {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    } else {
        Ok(Organizer::default())
    }
}

fn git_target(value: Option<&str>) -> Result<String> {
    let value = value.context("workbench.git_target_required")?;
    ensure!(
        !value.is_empty() && !value.starts_with('-') && !value.contains(['\0', '\r', '\n']),
        "workbench.invalid_git_target"
    );
    Ok(value.into())
}

#[derive(Debug, PartialEq, Eq)]
struct GitFileMetadata {
    path: String,
    original_path: Option<String>,
    kind: &'static str,
    binary: bool,
}

fn git_nul_fields(bytes: &[u8]) -> Result<Vec<&[u8]>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    ensure!(
        bytes.last() == Some(&0),
        "workbench.git_files_metadata_invalid"
    );
    Ok(bytes[..bytes.len() - 1]
        .split(|byte| *byte == 0)
        .collect())
}

fn git_metadata_path(bytes: &[u8]) -> Result<String> {
    ensure!(!bytes.is_empty(), "workbench.git_files_metadata_invalid");
    String::from_utf8(bytes.to_vec()).context("workbench.git_files_metadata_non_utf8")
}

fn parse_git_name_status(bytes: &[u8]) -> Result<Vec<GitFileMetadata>> {
    let fields = git_nul_fields(bytes)?;
    let mut changes = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let status = fields[index];
        index += 1;
        let code = *status
            .first()
            .context("workbench.git_files_metadata_invalid")?;
        let (kind, original_path) = match code {
            b'A' if status.len() == 1 => ("added", None),
            b'D' if status.len() == 1 => ("deleted", None),
            b'M' | b'T' if status.len() == 1 => ("modified", None),
            b'R' if status.len() > 1 && status[1..].iter().all(|byte| byte.is_ascii_digit()) => {
                ensure!(
                    index + 1 < fields.len(),
                    "workbench.git_files_metadata_invalid"
                );
                let original_path = git_metadata_path(fields[index])?;
                index += 1;
                ("renamed", Some(original_path))
            }
            _ => anyhow::bail!("workbench.git_files_metadata_invalid"),
        };
        ensure!(index < fields.len(), "workbench.git_files_metadata_invalid");
        let path = git_metadata_path(fields[index])?;
        index += 1;
        changes.push(GitFileMetadata {
            path,
            original_path,
            kind,
            binary: false,
        });
    }
    Ok(changes)
}

fn parse_git_numstat(bytes: &[u8]) -> Result<HashMap<String, bool>> {
    let fields = git_nul_fields(bytes)?;
    let mut binaries = HashMap::new();
    let mut index = 0;
    while index < fields.len() {
        let record = fields[index];
        index += 1;
        let added_end = record
            .iter()
            .position(|byte| *byte == b'\t')
            .context("workbench.git_files_metadata_invalid")?;
        let deleted_start = added_end + 1;
        let deleted_end = deleted_start
            + record[deleted_start..]
                .iter()
                .position(|byte| *byte == b'\t')
                .context("workbench.git_files_metadata_invalid")?;
        let added = &record[..added_end];
        let deleted = &record[deleted_start..deleted_end];
        let path = &record[deleted_end + 1..];
        let valid_count = |value: &[u8]| {
            value == b"-" || (!value.is_empty() && value.iter().all(|byte| byte.is_ascii_digit()))
        };
        ensure!(
            valid_count(added)
                && valid_count(deleted)
                && (added == b"-") == (deleted == b"-"),
            "workbench.git_files_metadata_invalid"
        );
        let path = if path.is_empty() {
            ensure!(
                index + 1 < fields.len(),
                "workbench.git_files_metadata_invalid"
            );
            git_metadata_path(fields[index])?;
            let path = git_metadata_path(fields[index + 1])?;
            index += 2;
            path
        } else {
            git_metadata_path(path)?
        };
        ensure!(
            binaries.insert(path, added == b"-").is_none(),
            "workbench.git_files_metadata_invalid"
        );
    }
    Ok(binaries)
}

fn parse_git_file_metadata(
    name_status: &crate::runtime_probe_process::BoundedCapture,
    numstat: &crate::runtime_probe_process::BoundedCapture,
) -> Result<Vec<GitFileMetadata>> {
    ensure!(
        !name_status.truncated && !numstat.truncated,
        "workbench.git_files_metadata_truncated"
    );
    let mut binaries = parse_git_numstat(&numstat.bytes)?;
    let mut files = parse_git_name_status(&name_status.bytes)?;
    for file in &mut files {
        file.binary = binaries
            .remove(&file.path)
            .context("workbench.git_files_metadata_mismatch")?;
    }
    ensure!(
        binaries.is_empty(),
        "workbench.git_files_metadata_mismatch"
    );
    Ok(files)
}

fn git_file_metadata_arguments(request: &GitRequest, numstat: bool) -> Result<Vec<String>> {
    let format = if numstat { "--numstat" } else { "--name-status" };
    let target = git_target(request.target.as_deref())?;
    let arguments = match request.action.as_str() {
        "show" => vec![
            "show".into(),
            "--no-ext-diff".into(),
            "--no-textconv".into(),
            "--format=".into(),
            format.into(),
            "-z".into(),
            "-M".into(),
            "--diff-merges=first-parent".into(),
            target,
            "--".into(),
        ],
        "diff_compare" => {
            let mut arguments = vec![
                "diff".into(),
                "--no-ext-diff".into(),
                "--no-textconv".into(),
                format.into(),
                "-z".into(),
                "-M".into(),
                target,
                "--".into(),
            ];
            arguments.extend(request.paths.iter().cloned());
            arguments
        }
        _ => anyhow::bail!("workbench.git_files_metadata_invalid"),
    };
    Ok(arguments)
}

fn git_arguments(request: &GitRequest, write: bool) -> Result<Vec<String>> {
    let target = || git_target(request.target.as_deref());
    let remote = || git_target(Some(request.remote.as_deref().unwrap_or("origin")));
    let message = || {
        request
            .message
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(String::from)
            .context("workbench.message_required")
    };
    let args: Vec<String> = if !write {
        match request.action.as_str() {
            "status" => vec![
                "status",
                "--porcelain=v2",
                "--branch",
                "--untracked-files=all",
                "-z",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            "log" => vec![
                "log",
                "--all",
                "--date-order",
                "-n",
                "200",
                "--format=%H%x00%P%x00%an%x00%aI%x00%B%x00%x1e",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            "reflog" => vec!["reflog", "-n", "200", "--format=%H%x00%gd%x00%gs%x1e"]
                .into_iter()
                .map(String::from)
                .collect(),
            "tags" => vec![
                "tag",
                "--list",
                "--format=%(refname:short)%00%(objectname)%00%(subject)%1e",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            "stash_diff" => vec![
                "stash".into(),
                "show".into(),
                "--no-ext-diff".into(),
                "--no-textconv".into(),
                "-p".into(),
                "--include-untracked".into(),
                target()?,
            ],
            "file_at" => {
                ensure!(request.paths.len() == 1, "workbench.paths_required");
                vec![
                    "cat-file".into(),
                    "blob".into(),
                    format!("{}:{}", target()?, request.paths[0]),
                ]
            }
            "diff_compare" => {
                let mut args = vec![
                    "diff".into(),
                    "--no-ext-diff".into(),
                    "--no-textconv".into(),
                    "--binary".into(),
                    target()?,
                    "--".into(),
                ];
                args.extend(request.paths.iter().cloned());
                args
            }
            "branches" => vec![
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00%(upstream:short)%00%(HEAD)%1e",
                "refs/heads",
                "refs/remotes",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            "stash" => vec!["stash", "list", "--format=%gd%x00%H%x00%gs%x1e"]
                .into_iter()
                .map(String::from)
                .collect(),
            "remotes" => vec!["remote", "-v"].into_iter().map(String::from).collect(),
            "worktrees" => vec!["worktree", "list", "--porcelain", "-z"]
                .into_iter()
                .map(String::from)
                .collect(),
            "conflicts" => vec!["ls-files", "-u", "-z"]
                .into_iter()
                .map(String::from)
                .collect(),
            "conflict" => {
                let stage = request
                    .mode
                    .as_deref()
                    .context("workbench.conflict_stage_required")?;
                ensure!(
                    ["1", "2", "3"].contains(&stage) && request.paths.len() == 1,
                    "workbench.invalid_conflict_stage"
                );
                vec!["show".into(), format!(":{}:{}", stage, request.paths[0])]
            }
            "show" => vec![
                "show".into(),
                "--no-ext-diff".into(),
                "--no-textconv".into(),
                "--format=fuller".into(),
                target()?,
                "--".into(),
            ],
            "diff" | "diff_staged" => {
                let mut v = vec![
                    "diff".into(),
                    "--no-ext-diff".into(),
                    "--no-textconv".into(),
                    "--binary".into(),
                ];
                if request.action == "diff_staged" {
                    v.push("--cached".into());
                }
                v.push("--".into());
                v
            }
            _ => anyhow::bail!("workbench.operation_not_admitted"),
        }
    } else {
        match request.action.as_str() {
            "init" => vec!["init".into()],
            "stage" => {
                ensure!(!request.paths.is_empty(), "workbench.paths_required");
                vec!["add".into(), "--".into()]
            }
            "unstage" => {
                ensure!(!request.paths.is_empty(), "workbench.paths_required");
                vec!["restore".into(), "--staged".into(), "--".into()]
            }
            "commit" => vec!["commit".into(), "-m".into(), message()?],
            "stash_save" => vec![
                "stash".into(),
                "push".into(),
                "--include-untracked".into(),
                "-m".into(),
                request.message.clone().unwrap_or_default(),
            ],
            "stash_apply" | "stash_pop" | "stash_drop" => {
                let value = target()?;
                ensure!(
                    value.starts_with("stash@{")
                        && value.ends_with('}')
                        && value[7..value.len() - 1]
                            .bytes()
                            .all(|b| b.is_ascii_digit()),
                    "workbench.invalid_stash"
                );
                vec![
                    "stash".into(),
                    request.action.trim_start_matches("stash_").into(),
                    value,
                ]
            }
            "branch_delete" => vec!["branch".into(), "-d".into(), target()?],
            "branch_rename" => vec![
                "branch".into(),
                "-m".into(),
                target()?,
                git_target(request.message.as_deref())?,
            ],
            "tag_create" => vec!["tag".into(), target()?],
            "tag_delete" => vec!["tag".into(), "-d".into(), target()?],
            "discard" => {
                ensure!(!request.paths.is_empty(), "workbench.paths_required");
                vec!["restore".into(), "--worktree".into(), "--".into()]
            }
            "clone" => {
                ensure!(request.paths.len() == 1, "workbench.paths_required");
                let source = target()?;
                ensure!(
                    (source.starts_with("https://")
                        || source.starts_with("ssh://")
                        || (source.contains('@') && source.contains(':')))
                        && !source.contains(['\0', '\r', '\n']),
                    "workbench.invalid_remote_url"
                );
                vec![
                    "clone".into(),
                    "--no-local".into(),
                    "--".into(),
                    source,
                    request.paths[0].clone(),
                ]
            }
            "worktree_add" => vec!["worktree".into(), "add".into()],
            "branch_create" => vec!["branch".into(), target()?],
            "checkout" => vec!["switch".into(), target()?],
            "reset" => {
                let mode = request.mode.as_deref().unwrap_or("mixed");
                ensure!(
                    ["soft", "mixed", "hard"].contains(&mode),
                    "workbench.invalid_reset_mode"
                );
                vec!["reset".into(), format!("--{mode}"), target()?, "--".into()]
            }
            "merge" => vec!["merge".into(), "--no-edit".into(), target()?],
            "rebase" => vec!["rebase".into(), target()?],
            "merge_abort" => vec!["merge".into(), "--abort".into()],
            "merge_continue" => vec![
                "-c".into(),
                "core.editor=true".into(),
                "merge".into(),
                "--continue".into(),
            ],
            "rebase_abort" => vec!["rebase".into(), "--abort".into()],
            "rebase_continue" => vec![
                "-c".into(),
                "core.editor=true".into(),
                "rebase".into(),
                "--continue".into(),
            ],
            "cherry_pick" => vec!["cherry-pick".into(), target()?],
            "cherry_pick_abort" => vec!["cherry-pick".into(), "--abort".into()],
            "cherry_pick_continue" => vec![
                "-c".into(),
                "core.editor=true".into(),
                "cherry-pick".into(),
                "--continue".into(),
            ],
            "revert" => vec!["revert".into(), "--no-edit".into(), target()?],
            "fetch" => vec!["fetch".into(), "--prune".into(), remote()?],
            "pull" => vec!["pull".into(), "--ff-only".into(), remote()?],
            "push" => {
                let mut v = vec!["push".into(), remote()?];
                if request.target.is_some() {
                    v.push(target()?)
                };
                v
            }
            "remote_add" => vec!["remote".into(), "add".into(), remote()?, target()?],
            "remote_remove" => vec!["remote".into(), "remove".into(), remote()?],
            "worktree_remove" => vec!["worktree".into(), "remove".into()],
            "clean" => {
                ensure!(!request.paths.is_empty(), "workbench.paths_required");
                vec!["clean".into(), "-f".into(), "-d".into(), "--".into()]
            }
            _ => anyhow::bail!("workbench.operation_not_admitted"),
        }
    };
    let mut args = args;
    if matches!(
        request.action.as_str(),
        "stage" | "unstage" | "diff" | "diff_staged" | "show" | "clean" | "discard"
    ) {
        args.extend(request.paths.iter().cloned());
    }
    Ok(args)
}

pub(super) fn relative(root: &Path, value: &str, allow_missing: bool) -> Result<PathBuf> {
    ensure!(
        !value.contains(['\0', '\r', '\n', '\\']) && (!cfg!(windows) || !value.contains(':')),
        "workbench.invalid_path"
    );
    let path = Path::new(value);
    ensure!(!path.is_absolute(), "workbench.invalid_path");
    let mut current = root.to_path_buf();
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                ensure!(
                    !name.to_string_lossy().eq_ignore_ascii_case(".git"),
                    "workbench.git_metadata"
                );
                current.push(name);
                match fs::symlink_metadata(&current) {
                    Ok(metadata) => {
                        ensure!(
                            !metadata.file_type().is_symlink(),
                            "workbench.link_not_allowed"
                        );
                        #[cfg(windows)]
                        {
                            use std::os::windows::fs::MetadataExt;
                            ensure!(
                                metadata.file_attributes() & 0x400 == 0,
                                "workbench.link_not_allowed"
                            );
                        }
                    }
                    Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => {
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            Component::CurDir => {}
            _ => anyhow::bail!("workbench.invalid_path"),
        }
    }
    ensure!(current.starts_with(root), "workbench.path_outside_root");
    Ok(current)
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManagedDirectoryLink {
    name: String,
    target: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectoryLinkManifest {
    root: String,
    links: Vec<ManagedDirectoryLink>,
}

pub(super) struct WorkspaceLinks {
    manifest: DirectoryLinkManifest,
}
pub(super) struct ResolvedWorkspacePath {
    pub(super) path: PathBuf,
    pub(super) boundary: PathBuf,
    pub(super) alias_root: bool,
}

fn valid_link_name(name: &str) -> Result<()> {
    ensure!(
        !name.trim().is_empty()
            && name.len() <= 255
            && !name.contains(['/', '\\', ':', '\0', '\r', '\n']),
        "workbench.invalid_alias"
    );
    let mut components = Path::new(name).components();
    ensure!(
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none(),
        "workbench.invalid_alias"
    );
    ensure!(
        !name.eq_ignore_ascii_case(".git"),
        "workbench.invalid_alias"
    );
    Ok(())
}

fn alias_name_matches(managed: &str, requested: &str) -> bool {
    #[cfg(windows)]
    {
        managed.eq_ignore_ascii_case(requested)
    }
    #[cfg(not(windows))]
    {
        managed == requested
    }
}

fn link_manifest_path(data_dir: &Path, root: &Path) -> Result<PathBuf> {
    let identity =
        crate::command::canonical_json_digest(&json!({"root":root.to_string_lossy(),"version":1}))?;
    Ok(data_dir
        .join("workspace-directory-links")
        .join(format!("{identity}.json")))
}

fn directory_link_target(value: &str) -> Result<(PathBuf, bool)> {
    let target = PathBuf::from(value);
    ensure!(
        target.is_absolute(),
        "workbench.directory_link_manifest_invalid"
    );
    for component in target.components() {
        ensure!(
            !matches!(component, Component::CurDir | Component::ParentDir),
            "workbench.directory_link_manifest_invalid"
        );
    }
    let mut current = target.as_path();
    let mut missing = Vec::new();
    loop {
        match fs::canonicalize(current) {
            Ok(base) => {
                let existed = missing.is_empty();
                let mut reconstructed = base;
                for component in missing.iter().rev() {
                    reconstructed.push(component);
                }
                ensure!(
                    reconstructed.to_string_lossy().as_ref() == value,
                    "workbench.directory_link_manifest_invalid"
                );
                if existed {
                    ensure!(
                        reconstructed.is_dir(),
                        "workbench.directory_link_manifest_invalid"
                    );
                }
                return Ok((reconstructed, existed));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = current
                    .file_name()
                    .context("workbench.directory_link_manifest_invalid")?;
                missing.push(name.to_os_string());
                current = current
                    .parent()
                    .context("workbench.directory_link_manifest_invalid")?;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn load_workspace_links(root: &Path, data_dir: &Path) -> Result<WorkspaceLinks> {
    let root = fs::canonicalize(root)?;
    let path = link_manifest_path(data_dir, &root)?;
    let manifest_directory = path
        .parent()
        .context("workbench.directory_link_manifest_invalid")?;
    match fs::symlink_metadata(manifest_directory) {
        Ok(metadata) => ensure!(
            !is_link(&metadata) && metadata.is_dir(),
            "workbench.directory_link_manifest_invalid"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let manifest = match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            ensure!(
                !is_link(&metadata) && metadata.is_file(),
                "workbench.directory_link_manifest_invalid"
            );
            serde_json::from_slice::<DirectoryLinkManifest>(&fs::read(path)?)?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DirectoryLinkManifest {
            root: root.to_string_lossy().into_owned(),
            links: Vec::new(),
        },
        Err(error) => return Err(error.into()),
    };
    ensure!(
        manifest.root.as_str() == root.to_string_lossy().as_ref(),
        "workbench.directory_link_manifest_invalid"
    );
    let mut names = std::collections::HashSet::new();
    for link in &manifest.links {
        valid_link_name(&link.name)?;
        ensure!(
            names.insert(link.name.to_lowercase()),
            "workbench.directory_link_manifest_invalid"
        );
        let (target, _) = directory_link_target(&link.target)?;
        let private_root = fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf());
        ensure!(
            !target.starts_with(private_root)
                && !target.starts_with(&root)
                && !root.starts_with(&target),
            "workbench.directory_link_manifest_invalid"
        );
    }
    Ok(WorkspaceLinks { manifest })
}

fn verify_directory_link(root: &Path, link: &ManagedDirectoryLink) -> Result<bool> {
    let path = root.join(&link.name);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    ensure!(is_link(&metadata), "workbench.directory_link_changed");
    let actual = fs::read_link(&path)?;
    ensure!(
        actual.is_absolute()
            && git::persisted_workspace_path_matches_canonical(&actual, Path::new(&link.target)),
        "workbench.directory_link_changed"
    );
    match fs::canonicalize(&path) {
        Ok(target) => {
            ensure!(
                target.to_string_lossy().as_ref() == link.target.as_str()
                    && fs::metadata(&path)?.is_dir(),
                "workbench.directory_link_changed"
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(true)
}

fn create_directory_link(target: &Path, path: &Path) -> Result<()> {
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(target, path)
        .context("workbench.directory_link_create_failed")?;
    #[cfg(not(windows))]
    std::os::unix::fs::symlink(target, path).context("workbench.directory_link_create_failed")?;
    Ok(())
}

fn remove_directory_link(path: &Path) -> Result<()> {
    #[cfg(windows)]
    fs::remove_dir(path)?;
    #[cfg(not(windows))]
    fs::remove_file(path)?;
    Ok(())
}

fn rollback_directory_links(
    root: &Path,
    created: &[ManagedDirectoryLink],
    removed: &[ManagedDirectoryLink],
) -> Result<()> {
    for link in created.iter().rev() {
        if verify_directory_link(root, link)? {
            remove_directory_link(&root.join(&link.name))?;
        }
    }
    for link in removed.iter().rev() {
        create_directory_link(Path::new(&link.target), &root.join(&link.name))?;
    }
    Ok(())
}

fn sync_directory_links(
    root: &Path,
    data_dir: &Path,
    desired: Vec<ManagedDirectoryLink>,
) -> Result<(
    PathBuf,
    Option<Vec<u8>>,
    Vec<ManagedDirectoryLink>,
    Vec<ManagedDirectoryLink>,
)> {
    let root = fs::canonicalize(root)?;
    let directory = data_dir.join("workspace-directory-links");
    let manifest_path = link_manifest_path(data_dir, &root)?;
    let current = load_workspace_links(&root, data_dir)?.manifest;
    let previous_bytes = match fs::symlink_metadata(&manifest_path) {
        Ok(metadata) => {
            ensure!(
                !is_link(&metadata) && metadata.is_file(),
                "workbench.directory_link_manifest_invalid"
            );
            Some(fs::read(&manifest_path)?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let mut desired_by_name = std::collections::HashMap::new();
    for link in desired {
        valid_link_name(&link.name)?;
        ensure!(
            desired_by_name
                .insert(link.name.to_lowercase(), link)
                .is_none(),
            "workbench.directory_alias_conflict"
        );
    }
    let current_by_name = current
        .links
        .iter()
        .map(|link| (link.name.to_lowercase(), link))
        .collect::<std::collections::HashMap<_, _>>();
    for (key, next) in &desired_by_name {
        if let Some(previous) = current_by_name.get(key) {
            let _ = verify_directory_link(&root, previous)?;
        } else {
            let candidate = root.join(&next.name);
            match fs::symlink_metadata(candidate) {
                Ok(_) => anyhow::bail!("workbench.directory_alias_name_taken"),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let target =
            fs::canonicalize(&next.target).context("workbench.directory_link_target_missing")?;
        let private_root = fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf());
        ensure!(
            target.is_dir()
                && target.to_string_lossy().as_ref() == next.target
                && !target.starts_with(private_root)
                && !target.starts_with(&root)
                && !root.starts_with(&target),
            "workbench.directory_link_manifest_invalid"
        );
    }
    for link in &current.links {
        if !desired_by_name.contains_key(&link.name.to_lowercase()) {
            let _ = verify_directory_link(&root, link)?;
        }
    }
    crate::platform::private_storage::prepare_private_directory(&directory)?;

    let mut created = Vec::new();
    let mut removed = Vec::new();
    let apply = (|| -> Result<()> {
        for link in &current.links {
            let unchanged = desired_by_name
                .get(&link.name.to_lowercase())
                .is_some_and(|next| next.name == link.name && next.target == link.target);
            if !unchanged && verify_directory_link(&root, link)? {
                remove_directory_link(&root.join(&link.name))?;
                removed.push(link.clone());
            }
        }
        for link in desired_by_name.values() {
            let previous = current_by_name.get(&link.name.to_lowercase());
            if previous.is_some_and(|previous| {
                previous.name == link.name && previous.target == link.target
            }) && verify_directory_link(&root, previous.unwrap())?
            {
                continue;
            }
            create_directory_link(Path::new(&link.target), &root.join(&link.name))?;
            created.push(link.clone());
        }
        let mut links = desired_by_name.into_values().collect::<Vec<_>>();
        links.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        let manifest = DirectoryLinkManifest {
            root: root.to_string_lossy().into_owned(),
            links,
        };
        crate::platform::private_storage::atomic_write_private_json(
            &manifest_path,
            &json!(manifest),
        )?;
        Ok(())
    })();
    if let Err(error) = apply {
        let rollback = rollback_directory_links(&root, &created, &removed);
        if let Some(bytes) = &previous_bytes {
            let restore = crate::platform::private_storage::atomic_write_private_json(
                &manifest_path,
                &serde_json::from_slice::<Value>(bytes)?,
            );
            restore.context("workbench.directory_link_rollback_failed")?;
        } else if manifest_path.exists() {
            fs::remove_file(&manifest_path).context("workbench.directory_link_rollback_failed")?;
        }
        rollback.context("workbench.directory_link_rollback_failed")?;
        return Err(error);
    }
    Ok((manifest_path, previous_bytes, created, removed))
}

fn rollback_directory_link_sync(
    root: &Path,
    undo: (
        PathBuf,
        Option<Vec<u8>>,
        Vec<ManagedDirectoryLink>,
        Vec<ManagedDirectoryLink>,
    ),
) -> Result<()> {
    let (manifest_path, previous_bytes, created, removed) = undo;
    rollback_directory_links(root, &created, &removed)?;
    if let Some(bytes) = previous_bytes {
        let value: Value = serde_json::from_slice(&bytes)?;
        crate::platform::private_storage::atomic_write_private_json(&manifest_path, &value)?;
    } else if manifest_path.exists() {
        fs::remove_file(&manifest_path)?;
    }
    Ok(())
}

pub(super) fn workspace_links(root: &Path, data_dir: &Path) -> Result<WorkspaceLinks> {
    load_workspace_links(root, data_dir)
}

pub(super) fn is_directory_alias(root: &Path, links: &WorkspaceLinks, name: &str) -> Result<bool> {
    if let Some(link) = links
        .manifest
        .links
        .iter()
        .find(|link| alias_name_matches(&link.name, name))
    {
        if !verify_directory_link(root, link)? {
            return Ok(false);
        }
        Ok(fs::metadata(root.join(&link.name)).is_ok_and(|metadata| metadata.is_dir()))
    } else {
        Ok(false)
    }
}

pub(super) fn normalized_workspace_path(value: &str) -> Result<String> {
    ensure!(
        !value.contains(['\0', '\r', '\n', '\\']) && (!cfg!(windows) || !value.contains(':')),
        "workbench.invalid_path"
    );
    let path = Path::new(value);
    ensure!(!path.is_absolute(), "workbench.invalid_path");
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => names.push(name.to_str().context("workbench.invalid_path")?),
            Component::CurDir => {}
            _ => anyhow::bail!("workbench.invalid_path"),
        }
    }
    Ok(names.join("/"))
}

pub(super) fn resolve_workspace_path(
    root: &Path,
    links: &WorkspaceLinks,
    value: &str,
    allow_missing: bool,
) -> Result<ResolvedWorkspacePath> {
    ensure!(
        !value.contains(['\0', '\r', '\n', ':', '\\']),
        "workbench.invalid_path"
    );
    let path = Path::new(value);
    ensure!(!path.is_absolute(), "workbench.invalid_path");
    let components = path.components().collect::<Vec<_>>();
    if let Some(Component::Normal(first)) = components.first() {
        if let Some(link) = links
            .manifest
            .links
            .iter()
            .find(|link| alias_name_matches(&link.name, &first.to_string_lossy()))
        {
            ensure!(
                verify_directory_link(root, link)?,
                "workbench.directory_link_missing"
            );
            ensure!(
                fs::metadata(root.join(&link.name)).is_ok_and(|metadata| metadata.is_dir()),
                "workbench.directory_link_target_missing"
            );
            let rest = components
                .iter()
                .skip(1)
                .map(|part| match part {
                    Component::Normal(name) => name.to_str().context("workbench.invalid_path"),
                    Component::CurDir => Ok("."),
                    _ => anyhow::bail!("workbench.invalid_path"),
                })
                .collect::<Result<Vec<_>>>()?
                .join("/");
            let target = PathBuf::from(&link.target);
            let resolved = relative(&target, &rest, allow_missing)?;
            return Ok(ResolvedWorkspacePath {
                path: resolved,
                boundary: target,
                alias_root: components.len() == 1,
            });
        }
    }
    Ok(ResolvedWorkspacePath {
        path: relative(root, value, allow_missing)?,
        boundary: root.to_path_buf(),
        alias_root: false,
    })
}

impl Core {
    pub(super) async fn workbench_organizer(&self, method: &str, value: Value) -> Result<Value> {
        let request: OrganizerRequest = serde_json::from_value(value)?;
        let _write = MUTATIONS.lock().await;
        ensure!(
            ["global", "project", "thread"].contains(&request.level.as_str()),
            "workbench.invalid_organizer_level"
        );
        let directory = self.data_dir.join("workspace-metadata");
        let scope_root = if request.level == "global" && method == "workbench.organizer.get" {
            None
        } else {
            match request.scope.as_ref() {
                Some(scope) => Some(self.workbench_root(scope).await?),
                None => None,
            }
        };
        let path = if request.level == "global" {
            directory.join("global-v1.json")
        } else {
            organizer_metadata_path(
                &directory,
                &request.level,
                request.scope.as_ref().context("workbench.scope_required")?,
                scope_root.as_deref().context("workbench.scope_required")?,
            )?
        };
        let current = read_organizer(&path)?;
        let current_value = serde_json::to_value(&current)?;
        let current_digest = crate::command::canonical_json_digest(&current_value)?;
        if method == "workbench.organizer.get" {
            return Ok(json!({"value":current_value,"digest":current_digest}));
        }
        ensure!(
            method == "workbench.organizer.set",
            "workbench.operation_not_admitted"
        );
        let scope = request.scope.as_ref().context("workbench.scope_required")?;
        let root = scope_root.as_deref().context("workbench.scope_required")?;
        let mut next = request.value.context("workbench.metadata_required")?;
        ensure!(
            next.tags.len() <= 512
                && next.directory_aliases.len() <= 512
                && next.quick_messages.len() <= 512,
            "workbench.metadata_limit"
        );
        let mut seen = std::collections::HashSet::new();
        for tag in &next.tags {
            ensure!(
                !tag.trim().is_empty() && tag.len() <= 256 && seen.insert(tag),
                "workbench.invalid_tag"
            );
        }
        let mut seen_paths = std::collections::HashSet::new();
        let mut seen_names = std::collections::HashSet::new();
        for alias in &mut next.directory_aliases {
            let canonical =
                git::validate_workspace_directory(Path::new(&alias.path), &self.data_dir, false)?;
            alias.path = canonical.to_string_lossy().into_owned();
            valid_link_name(&alias.name)?;
            ensure!(
                !canonical.starts_with(root) && !root.starts_with(&canonical),
                "workbench.invalid_alias"
            );
            ensure!(
                alias.group.len() <= 256
                    && seen_paths.insert(alias.path.clone())
                    && seen_names.insert(alias.name.to_lowercase()),
                "workbench.invalid_alias"
            );
        }
        let mut seen = std::collections::HashSet::new();
        for message in &next.quick_messages {
            uuid::Uuid::parse_str(&message.id).context("workbench.invalid_quick_message")?;
            ensure!(
                seen.insert(&message.id)
                    && !message.title.trim().is_empty()
                    && message.title.len() <= 512
                    && message.text.len() <= 1024 * 1024,
                "workbench.invalid_quick_message"
            );
        }
        let value = serde_json::to_value(next)?;
        let next_digest = crate::command::canonical_json_digest(&value)?;
        ensure!(
            request.expected_digest.as_deref() == Some(current_digest.as_str()),
            "workbench.metadata_changed"
        );

        let mut catalogs = std::collections::HashMap::new();
        for level in ["global", "project", "thread"] {
            let catalog_path = organizer_metadata_path(&directory, level, scope, root)?;
            let catalog = if level == request.level {
                serde_json::from_value(value.clone())?
            } else {
                read_organizer(&catalog_path)?
            };
            catalogs.insert(level, catalog);
        }
        let mut desired = std::collections::HashMap::<String, ManagedDirectoryLink>::new();
        for level in ["global", "project", "thread"] {
            for alias in &catalogs[level].directory_aliases {
                let canonical = git::validate_workspace_directory(
                    Path::new(&alias.path),
                    &self.data_dir,
                    false,
                )?;
                ensure!(
                    !canonical.starts_with(root) && !root.starts_with(&canonical),
                    "workbench.invalid_alias"
                );
                let key = alias.name.to_lowercase();
                let link = ManagedDirectoryLink {
                    name: alias.name.clone(),
                    target: canonical.to_string_lossy().into_owned(),
                };
                if let Some(previous) = desired.get(&key) {
                    ensure!(
                        previous.target == link.target,
                        "workbench.directory_alias_conflict"
                    );
                } else {
                    desired.insert(key, link);
                }
            }
        }
        let undo = sync_directory_links(root, &self.data_dir, desired.into_values().collect())?;
        if next_digest != current_digest {
            crate::platform::private_storage::prepare_private_directory(&directory)?;
            if let Err(error) =
                crate::platform::private_storage::atomic_write_private_json(&path, &value)
            {
                rollback_directory_link_sync(root, undo)
                    .context("workbench.directory_link_rollback_failed")?;
                return Err(error);
            }
        }
        Ok(json!({"value":value,"digest":next_digest}))
    }

    async fn workbench_git_revision(
        &self,
        git: &crate::mission_workspace::MissionGit,
        root: &Path,
    ) -> Result<String> {
        let mut bytes = Vec::new();
        for args in [
            vec!["rev-parse", "--verify", "HEAD"],
            vec!["status", "--porcelain=v2", "--branch", "-z"],
            vec!["show-ref", "--head"],
            vec!["diff", "--no-ext-diff", "--no-textconv", "--binary"],
        ] {
            let output = git
                .workbench_output(
                    root,
                    &args.into_iter().map(String::from).collect::<Vec<_>>(),
                )
                .await?;
            bytes.extend_from_slice(&output.status.code().unwrap_or(-1).to_le_bytes());
            bytes.extend_from_slice(&(output.stdout.bytes.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&output.stdout.bytes);
        }
        let output = git
            .workbench_output(
                root,
                &["rev-parse".into(), "--git-path".into(), "index".into()],
            )
            .await?;
        if output.status.success() {
            let name = String::from_utf8(output.stdout.bytes)?.trim().to_owned();
            let index = Path::new(&name);
            let index = if index.is_absolute() {
                index.to_path_buf()
            } else {
                root.join(index)
            };
            if index.exists() {
                bytes.extend_from_slice(&fs::read(index)?);
            }
        }
        let untracked = git
            .workbench_output(
                root,
                &[
                    "ls-files".into(),
                    "--others".into(),
                    "--exclude-standard".into(),
                    "-z".into(),
                ],
            )
            .await?;
        ensure!(
            untracked.status.success(),
            "workbench.git_untracked_unavailable"
        );
        let mut total = 0u64;
        for name in untracked
            .stdout
            .bytes
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            let name = std::str::from_utf8(name)?;
            let path = relative(root, name, false)?;
            let metadata = fs::metadata(&path)?;
            if !metadata.is_file() {
                continue;
            }
            total += metadata.len();
            ensure!(
                total <= 512 * 1024 * 1024,
                "workbench.git_untracked_size_limit"
            );
            let (size, hash) = crate::backup::file_digest(&path)?;
            bytes.extend_from_slice(&(name.len() as u64).to_le_bytes());
            bytes.extend_from_slice(name.as_bytes());
            bytes.extend_from_slice(&size.to_le_bytes());
            bytes.extend_from_slice(hash.as_bytes());
        }
        Ok(digest(&bytes))
    }

    pub(super) async fn workbench_git(&self, method: &str, value: Value) -> Result<Value> {
        let request: GitRequest = serde_json::from_value(value.clone())?;
        let root = self.workbench_root(&request.scope).await?;
        let git = self.mission_git().await?;
        if method == "workbench.git.read" {
            for path in &request.paths {
                ensure!(!path.is_empty(), "workbench.paths_required");
                relative(&root, path, true)?;
            }
            let arguments = git_arguments(&request, false)?;
            let output = git.workbench_output(&root, &arguments).await?;
            let success = output.status.success();
            let exit_code = output.status.code();
            let stdout = String::from_utf8(output.stdout.bytes)
                .context("workbench.git_non_utf8")?;
            let error = output.stderr.lossy_text();
            let mut result = json!({
                "success": success,
                "exitCode": exit_code,
                "output": stdout,
                "error": error
            });
            if success && matches!(request.action.as_str(), "show" | "diff_compare") {
                match self
                    .workbench_git_file_metadata(&git, &root, &request)
                    .await
                {
                    Ok(files) => result["files"] = json!(files),
                    Err(error) => result["filesError"] = json!(error.to_string()),
                }
            }
            return Ok(result);
        }
        let execute = method == "workbench.git.execute";
        if execute {
            ensure!(request.confirmed, "workbench.confirmation_required");
        } else {
            ensure!(
                method == "workbench.git.preview",
                "workbench.operation_not_admitted"
            );
        }
        let mut arguments = git_arguments(&request, true)?;
        let _write = if execute {
            Some(MUTATIONS.lock().await)
        } else {
            None
        };
        let receipt = if execute {
            let id = request
                .command_id
                .as_deref()
                .context("workbench.command_id_required")?;
            uuid::Uuid::parse_str(id).context("workbench.invalid_command_id")?;
            let identity =
                crate::command::canonical_json_digest(&json!({"root":root,"request":value}))?;
            let receipts = self.data_dir.join("workspace-operations");
            crate::platform::private_storage::prepare_private_directory(&receipts)?;
            let receipt = receipts.join(format!("{id}.json"));
            if receipt.exists() {
                let saved: Value = serde_json::from_slice(&fs::read(&receipt)?)?;
                ensure!(saved["identity"] == identity, "workbench.command_conflict");
                ensure!(
                    saved["state"] == "finished",
                    "workbench.operation_unknown_check_state"
                );
                return Ok(saved["result"].clone());
            }
            Some((receipt, identity))
        } else {
            None
        };
        for path in &request.paths {
            ensure!(!path.is_empty(), "workbench.paths_required");
            relative(&root, path, true)?;
        }
        let mut managed_tree = None;
        if matches!(request.action.as_str(), "worktree_add" | "worktree_remove") {
            let branch = git_target(request.target.as_deref())?;
            let key = crate::command::canonical_json_digest(&json!({"root":root,"branch":branch}))?;
            let directory = self.data_dir.join("workspace-trees");
            crate::platform::private_storage::prepare_private_directory(&directory)?;
            let tree = directory.join(&key);
            let owner = directory.join(format!("{key}.json"));
            if request.action == "worktree_add" {
                ensure!(!tree.exists(), "workbench.worktree_exists");
                arguments.extend([tree.to_string_lossy().into_owned(), branch.clone()]);
            } else {
                let record: Value = serde_json::from_slice(&fs::read(&owner)?)?;
                ensure!(
                    record["root"] == json!(root) && record["branch"] == branch,
                    "workbench.worktree_owner_mismatch"
                );
                arguments.extend(["--".into(), tree.to_string_lossy().into_owned()]);
            }
            managed_tree = Some((tree, owner, branch));
        }
        if request.action == "clone" {
            let destination = relative(&root, &request.paths[0], true)?;
            ensure!(!destination.exists(), "workbench.destination_exists");
        }
        let revision = if matches!(request.action.as_str(), "init" | "clone") {
            crate::command::canonical_json_digest(
                &json!({"root":root,"destination":request.paths,"exists":request.paths.iter().map(|path|root.join(path).exists()).collect::<Vec<_>>()}),
            )?
        } else {
            self.workbench_git_revision(&git, &root).await?
        };
        if !execute {
            let clean = if request.action == "clean" {
                let mut preview = arguments.clone();
                preview[1] = "-n".into();
                Some(
                    git.workbench_output(&root, &preview)
                        .await?
                        .stdout
                        .lossy_text(),
                )
            } else {
                None
            };
            return Ok(
                json!({"revision":revision,"action":request.action,"arguments":arguments,"cleanPreview":clean}),
            );
        }
        ensure!(
            request.expected_revision.as_deref() == Some(revision.as_str()),
            "workbench.git_changed_refresh_preview"
        );
        let (receipt, identity) = receipt.context("workbench.command_receipt_required")?;
        if let Some((tree, owner, branch)) = &managed_tree {
            if request.action == "worktree_add" {
                crate::platform::private_storage::atomic_write_private_json(
                    owner,
                    &json!({"root":root,"branch":branch,"path":tree,"createdAt":Utc::now().to_rfc3339()}),
                )?;
            }
        }
        crate::platform::private_storage::atomic_write_private_json(
            &receipt,
            &json!({"identity":identity,"state":"started"}),
        )?;
        let output = git.workbench_output(&root, &arguments).await?;
        if output.status.success() {
            if let Some((_, owner, _)) = &managed_tree {
                if request.action == "worktree_remove" {
                    let _ = fs::remove_file(owner);
                }
            }
        }
        let result = json!({"success":output.status.success(),"exitCode":output.status.code(),"output":output.stdout.lossy_text(),"error":output.stderr.lossy_text(),"workingDirectory":managed_tree.as_ref().map(|(tree,_,_)|tree)});
        crate::platform::private_storage::atomic_write_private_json(
            &receipt,
            &json!({"identity":identity,"state":"finished","result":result}),
        )?;
        Ok(result)
    }

    async fn workbench_git_file_metadata(
        &self,
        git: &crate::mission_workspace::MissionGit,
        root: &Path,
        request: &GitRequest,
    ) -> Result<Vec<Value>> {
        let name_status_arguments = git_file_metadata_arguments(request, false)?;
        let name_status = git
            .workbench_output(root, &name_status_arguments)
            .await
            .context("workbench.git_files_metadata_command_failed")?;
        ensure!(
            name_status.status.success(),
            "workbench.git_files_metadata_command_failed"
        );
        let numstat_arguments = git_file_metadata_arguments(request, true)?;
        let numstat = git
            .workbench_output(root, &numstat_arguments)
            .await
            .context("workbench.git_files_metadata_command_failed")?;
        ensure!(
            numstat.status.success(),
            "workbench.git_files_metadata_command_failed"
        );
        parse_git_file_metadata(&name_status.stdout, &numstat.stdout).map(|files| {
            files
                .into_iter()
                .map(|file| {
                    let mut value = json!({"path":file.path,"kind":file.kind,"binary":file.binary});
                    if let Some(original_path) = file.original_path {
                        value["originalPath"] = json!(original_path);
                    }
                    value
                })
                .collect()
        })
    }

    pub(super) async fn workbench_root(&self, scope: &Scope) -> Result<PathBuf> {
        let persisted = self.workbench_directory(scope).await?;
        let path = Path::new(&persisted);
        let root = git::validate_workspace_directory(path, &self.data_dir, true)?;
        ensure!(
            git::persisted_workspace_path_matches_canonical(Path::new(&persisted), &root),
            "workbench.workspace_changed"
        );
        Ok(root)
    }

    pub(super) async fn workbench_files(&self, method: &str, value: Value) -> Result<Value> {
        let request: FileRequest = serde_json::from_value(value)?;
        let root = self.workbench_root(&request.scope).await?;
        let links = workspace_links(&root, &self.data_dir)?;
        let resolved = resolve_workspace_path(&root, &links, &request.path, false)?;
        let path = resolved.path;
        match method {
            "workbench.files.list" => {
                ensure!(path.is_dir(), "workbench.directory_required");
                let prefix = normalized_workspace_path(&request.path)?;
                let mut entries = Vec::new();
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let name = entry
                        .file_name()
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("workbench.non_utf8_name"))?;
                    if name.eq_ignore_ascii_case(".git") {
                        continue;
                    }
                    let metadata = fs::symlink_metadata(entry.path())?;
                    let alias = prefix.is_empty() && is_directory_alias(&root, &links, &name)?;
                    let link = is_link(&metadata);
                    let child_path = if prefix.is_empty() {
                        name.clone()
                    } else {
                        format!("{prefix}/{name}")
                    };
                    let key = format!(
                        "{}:{}",
                        if metadata.is_dir() || alias { "0" } else { "1" },
                        name
                    );
                    if request
                        .after
                        .as_ref()
                        .is_some_and(|after| key.as_str() <= after.as_str())
                    {
                        continue;
                    }
                    entries.push((key, json!({"name":name, "path":child_path,
                        "directory":metadata.is_dir() || alias, "link":link, "managedAlias":alias, "bytes":metadata.len()})));
                }
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                let next = (entries.len() > 500).then(|| entries[499].0.clone());
                entries.truncate(500);
                Ok(
                    json!({"entries":entries.into_iter().map(|(_,v)|v).collect::<Vec<_>>(), "next":next, "workspaceRoot":root}),
                )
            }
            "workbench.files.read" => {
                ensure!(path.is_file(), "workbench.file_required");
                ensure!(
                    fs::metadata(&path)?.len() <= 16 * 1024 * 1024,
                    "workbench.file_too_large"
                );
                let bytes = fs::read(&path)?;
                let version = digest(&bytes);
                let text = String::from_utf8(bytes).context("workbench.text_required")?;
                ensure!(!text.contains('\0'), "workbench.text_required");
                Ok(json!({"path":request.path,"text":text,"digest":version}))
            }
            _ => anyhow::bail!("workbench.operation_not_admitted"),
        }
    }

    pub(super) async fn workbench_file_mutate(&self, value: Value) -> Result<Value> {
        let request: FileMutation = serde_json::from_value(value.clone())?;
        uuid::Uuid::parse_str(&request.command_id).context("workbench.invalid_command_id")?;
        let _write = MUTATIONS.lock().await;
        let root = self.workbench_root(&request.scope).await?;
        let links = workspace_links(&root, &self.data_dir)?;
        ensure!(!request.path.is_empty(), "workbench.root_immutable");
        let resolved = resolve_workspace_path(&root, &links, &request.path, true)?;
        let path = resolved.path;
        ensure!(
            path != root && !resolved.alias_root,
            "workbench.root_immutable"
        );
        let identity =
            crate::command::canonical_json_digest(&json!({"root":root,"request":value}))?;
        let receipts = self.data_dir.join("workspace-operations");
        crate::platform::private_storage::prepare_private_directory(&receipts)?;
        let receipt = receipts.join(format!("{}.json", request.command_id));
        if receipt.exists() {
            let saved: Value = serde_json::from_slice(&fs::read(&receipt)?)?;
            ensure!(saved["identity"] == identity, "workbench.command_conflict");
            return match saved["state"].as_str() {
                Some("applied") => Ok(saved["result"].clone()),
                Some("failed") => anyhow::bail!("workbench.operation_failed: {}", saved["error"]),
                _ => anyhow::bail!("workbench.operation_unknown_check_state"),
            };
        }
        crate::platform::private_storage::atomic_write_private_json(
            &receipt,
            &json!({"identity":identity,"state":"started"}),
        )?;
        let result = (|| -> Result<Value> {
            match request.action.as_str() {
                "save" => write_text(
                    &path,
                    request.text.as_deref().context("workbench.text_required")?,
                    request.expected_digest.as_deref(),
                ),
                "create_directory" => {
                    fs::create_dir(&path)?;
                    Ok(json!({"applied":true}))
                }
                "rename" => {
                    let destination = resolve_workspace_path(
                        &root,
                        &links,
                        request
                            .destination
                            .as_deref()
                            .context("workbench.destination_required")?,
                        true,
                    )?;
                    ensure!(
                        destination.path != root
                            && !destination.alias_root
                            && destination.boundary == resolved.boundary,
                        "workbench.invalid_destination"
                    );
                    ensure!(!destination.path.exists(), "workbench.destination_exists");
                    ensure!(
                        !destination.path.starts_with(&path),
                        "workbench.invalid_destination"
                    );
                    if path.is_file() {
                        ensure!(
                            request.expected_digest.as_deref()
                                == Some(digest(&fs::read(&path)?).as_str()),
                            "workbench.file_changed"
                        );
                    }
                    fs::rename(&path, destination.path)?;
                    Ok(json!({"applied":true}))
                }
                _ => anyhow::bail!("workbench.operation_not_admitted"),
            }
        })();
        let saved = match &result {
            Ok(output) => json!({"identity":identity,"state":"applied","result":output}),
            Err(error) => json!({"identity":identity,"state":"failed","error":error.to_string()}),
        };
        crate::platform::private_storage::atomic_write_private_json(&receipt, &saved)?;
        result
    }
}

#[cfg(test)]
mod git_file_metadata_tests {
    use super::{parse_git_file_metadata, GitFileMetadata};
    use crate::runtime_probe_process::BoundedCapture;

    fn capture(bytes: &[u8], truncated: bool) -> BoundedCapture {
        BoundedCapture {
            bytes: bytes.to_vec(),
            truncated,
        }
    }

    #[test]
    fn parses_space_unicode_and_tab_paths_from_nul_fields() {
        let name_status = [
            b"M\0space name.txt\0".as_slice(),
            "A\0目录/带\t制表符.txt\0".as_bytes(),
        ]
        .concat();
        let numstat = [
            b"2\t1\tspace name.txt\0".as_slice(),
            "0\t0\t目录/带\t制表符.txt\0".as_bytes(),
        ]
        .concat();

        let files = parse_git_file_metadata(
            &capture(&name_status, false),
            &capture(&numstat, false),
        )
        .unwrap();

        assert_eq!(
            files,
            vec![
                GitFileMetadata {
                    path: "space name.txt".into(),
                    original_path: None,
                    kind: "modified",
                    binary: false,
                },
                GitFileMetadata {
                    path: "目录/带\t制表符.txt".into(),
                    original_path: None,
                    kind: "added",
                    binary: false,
                },
            ]
        );
    }

    #[test]
    fn parses_rename_paths_with_spaces_and_tabs() {
        let name_status = [
            b"R100\0old\t name [record].txt\0".as_slice(),
            "new\t 名 [记录].txt\0".as_bytes(),
        ]
        .concat();
        let numstat = [
            b"3\t1\t\0old\t name [record].txt\0".as_slice(),
            "new\t 名 [记录].txt\0".as_bytes(),
        ]
        .concat();

        let files = parse_git_file_metadata(
            &capture(&name_status, false),
            &capture(&numstat, false),
        )
        .unwrap();

        assert_eq!(
            files,
            vec![GitFileMetadata {
                path: "new\t 名 [记录].txt".into(),
                original_path: Some("old\t name [record].txt".into()),
                kind: "renamed",
                binary: false,
            }]
        );
    }

    #[test]
    fn parses_binary_deletions() {
        let files = parse_git_file_metadata(
            &capture(b"D\0deleted binary.bin\0", false),
            &capture(b"-\t-\tdeleted binary.bin\0", false),
        )
        .unwrap();

        assert_eq!(
            files,
            vec![GitFileMetadata {
                path: "deleted binary.bin".into(),
                original_path: None,
                kind: "deleted",
                binary: true,
            }]
        );
    }

    #[test]
    fn empty_metadata_returns_no_files() {
        assert!(parse_git_file_metadata(&capture(b"", false), &capture(b"", false))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn truncated_metadata_is_rejected() {
        assert!(parse_git_file_metadata(
            &capture(b"M\0partial.txt\0", true),
            &capture(b"1\t0\tpartial.txt\0", false),
        )
        .is_err());
        assert!(parse_git_file_metadata(
            &capture(b"M\0partial.txt\0", false),
            &capture(b"1\t0\tpartial.txt\0", true),
        )
        .is_err());
    }

    #[test]
    fn non_utf8_paths_are_rejected() {
        assert!(parse_git_file_metadata(
            &capture(b"M\0\xff\0", false),
            &capture(b"1\t0\t\xff\0", false),
        )
        .is_err());
    }
}
