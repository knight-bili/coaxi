//! User workspace operations. Scope roots come from Core, never Renderer paths.
use super::*;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Component};

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
    ensure!(text.len() <= 16 * 1024 * 1024 && !text.contains('\0'), "workbench.text_too_large");
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
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        if let Ok(metadata) = fs::metadata(path) { file.set_permissions(metadata.permissions())?; }
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        let current = if path.exists() { Some(digest(&fs::read(path)?)) } else { None };
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
                let replaced = unsafe { ReplaceFileW(destination.as_ptr(), source.as_ptr(), std::ptr::null(), 0, std::ptr::null(), std::ptr::null()) };
                ensure!(replaced != 0, "workbench.replace_failed: {}", std::io::Error::last_os_error());
            }
            #[cfg(not(windows))]
            fs::rename(&temporary, path)?;
        }
        #[cfg(not(windows))]
        fs::File::open(parent)?.sync_all()?;
        Ok(json!({"applied":true,"digest":digest(text.as_bytes())}))
    })();
    if result.is_err() { let _ = fs::remove_file(&temporary); }
    result
}

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GitRequest {
    scope: Scope,
    action: String,
    #[serde(default)] paths: Vec<String>,
    #[serde(default)] target: Option<String>,
    #[serde(default)] remote: Option<String>,
    #[serde(default)] message: Option<String>,
    #[serde(default)] mode: Option<String>,
    #[serde(default)] command_id: Option<String>,
    #[serde(default)] expected_revision: Option<String>,
    #[serde(default)] confirmed: bool,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Organizer {
    #[serde(default)] tags: Vec<String>,
    #[serde(default)] directory_aliases: Vec<DirectoryAlias>,
    #[serde(default)] quick_messages: Vec<QuickMessage>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectoryAlias { path: String, name: String, group: String }
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QuickMessage { id: String, title: String, text: String }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OrganizerRequest {
    scope: Scope,
    #[serde(default)] expected_digest: Option<String>,
    #[serde(default)] value: Option<Organizer>,
}

fn git_target(value: Option<&str>) -> Result<String> {
    let value = value.context("workbench.git_target_required")?;
    ensure!(!value.is_empty() && !value.starts_with('-') && !value.contains(['\0','\r','\n']), "workbench.invalid_git_target");
    Ok(value.into())
}

fn git_arguments(request: &GitRequest, write: bool) -> Result<Vec<String>> {
    let target = || git_target(request.target.as_deref());
    let remote = || git_target(Some(request.remote.as_deref().unwrap_or("origin")));
    let message = || request.message.as_deref().filter(|s| !s.trim().is_empty()).map(String::from).context("workbench.message_required");
    let args: Vec<String> = if !write {
        match request.action.as_str() {
            "status" => vec!["status", "--porcelain=v2", "--branch", "-z"].into_iter().map(String::from).collect(),
            "log" => vec!["log", "--all", "--date-order", "-n", "200", "--format=%H%x00%P%x00%an%x00%aI%x00%B%x00%x1e"].into_iter().map(String::from).collect(),
            "branches" => vec!["for-each-ref", "--format=%(refname)%00%(objectname)%00%(upstream:short)%00%(HEAD)%1e", "refs/heads", "refs/remotes"].into_iter().map(String::from).collect(),
            "stash" => vec!["stash", "list", "--format=%gd%x00%H%x00%gs%x1e"].into_iter().map(String::from).collect(),
            "remotes" => vec!["remote", "-v"].into_iter().map(String::from).collect(),
            "worktrees" => vec!["worktree", "list", "--porcelain", "-z"].into_iter().map(String::from).collect(),
            "conflicts" => vec!["ls-files", "-u", "-z"].into_iter().map(String::from).collect(),
            "conflict" => {
                let stage = request.mode.as_deref().context("workbench.conflict_stage_required")?;
                ensure!(["1", "2", "3"].contains(&stage) && request.paths.len() == 1, "workbench.invalid_conflict_stage");
                vec!["show".into(), format!(":{}:{}", stage, request.paths[0])]
            }
            "show" => vec!["show".into(), "--no-ext-diff".into(), "--no-textconv".into(), "--format=fuller".into(), target()?, "--".into()],
            "diff" | "diff_staged" => {
                let mut v = vec!["diff".into(), "--no-ext-diff".into(), "--no-textconv".into(), "--binary".into()];
                if request.action == "diff_staged" { v.push("--cached".into()); }
                v.push("--".into()); v
            }
            _ => anyhow::bail!("workbench.operation_not_admitted"),
        }
    } else {
        match request.action.as_str() {
            "init" => vec!["init".into()],
            "stage" => { ensure!(!request.paths.is_empty(), "workbench.paths_required"); vec!["add".into(), "--".into()] }
            "unstage" => { ensure!(!request.paths.is_empty(), "workbench.paths_required"); vec!["restore".into(), "--staged".into(), "--".into()] }
            "commit" => vec!["commit".into(), "-m".into(), message()?],
            "stash_save" => vec!["stash".into(), "push".into(), "--include-untracked".into(), "-m".into(), request.message.clone().unwrap_or_default()],
            "stash_apply" | "stash_pop" | "stash_drop" => {
                let value = target()?;
                ensure!(value.starts_with("stash@{") && value.ends_with('}') && value[7..value.len()-1].bytes().all(|b| b.is_ascii_digit()), "workbench.invalid_stash");
                vec!["stash".into(), request.action.trim_start_matches("stash_").into(), value]
            }
            "branch_create" => vec!["branch".into(), target()?],
            "checkout" => vec!["switch".into(), target()?],
            "reset" => {
                let mode = request.mode.as_deref().unwrap_or("mixed");
                ensure!(["soft", "mixed", "hard"].contains(&mode), "workbench.invalid_reset_mode");
                vec!["reset".into(), format!("--{mode}"), target()?, "--".into()]
            }
            "merge" => vec!["merge".into(), "--no-edit".into(), target()?],
            "rebase" => vec!["rebase".into(), target()?],
            "merge_abort" => vec!["merge".into(), "--abort".into()],
            "merge_continue" => vec!["-c".into(), "core.editor=true".into(), "merge".into(), "--continue".into()],
            "rebase_abort" => vec!["rebase".into(), "--abort".into()],
            "rebase_continue" => vec!["-c".into(), "core.editor=true".into(), "rebase".into(), "--continue".into()],
            "cherry_pick" => vec!["cherry-pick".into(), target()?],
            "cherry_pick_abort" => vec!["cherry-pick".into(), "--abort".into()],
            "cherry_pick_continue" => vec!["-c".into(), "core.editor=true".into(), "cherry-pick".into(), "--continue".into()],
            "revert" => vec!["revert".into(), "--no-edit".into(), target()?],
            "fetch" => vec!["fetch".into(), "--prune".into(), remote()?],
            "pull" => vec!["pull".into(), "--ff-only".into(), remote()?],
            "push" => { let mut v=vec!["push".into(), remote()?]; if request.target.is_some() {v.push(target()?)}; v }
            "remote_add" => vec!["remote".into(), "add".into(), remote()?, target()?],
            "remote_remove" => vec!["remote".into(), "remove".into(), remote()?],
            "worktree_remove" => { ensure!(request.paths.len()==1,"workbench.paths_required"); vec!["worktree".into(),"remove".into(),"--".into()] }
            "clean" => { ensure!(!request.paths.is_empty(),"workbench.paths_required"); vec!["clean".into(),"-f".into(),"-d".into(),"--".into()] }
            _ => anyhow::bail!("workbench.operation_not_admitted"),
        }
    };
    let mut args = args;
    if matches!(request.action.as_str(), "stage"|"unstage"|"diff"|"diff_staged"|"show"|"clean"|"worktree_remove") { args.extend(request.paths.clone()); }
    Ok(args)
}

fn relative(root: &Path, value: &str, allow_missing: bool) -> Result<PathBuf> {
    ensure!(!value.contains(['\0', '\r', '\n', ':', '\\']), "workbench.invalid_path");
    let path = Path::new(value);
    ensure!(!path.is_absolute(), "workbench.invalid_path");
    let mut current = root.to_path_buf();
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                ensure!(!name.to_string_lossy().eq_ignore_ascii_case(".git"), "workbench.git_metadata");
                current.push(name);
                match fs::symlink_metadata(&current) {
                    Ok(metadata) => {
                        ensure!(!metadata.file_type().is_symlink(), "workbench.link_not_allowed");
                        #[cfg(windows)]
                        {
                            use std::os::windows::fs::MetadataExt;
                            ensure!(metadata.file_attributes() & 0x400 == 0, "workbench.link_not_allowed");
                        }
                    }
                    Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => {}
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

impl Core {
    pub(super) async fn workbench_organizer(&self, method: &str, value: Value) -> Result<Value> {
        let request: OrganizerRequest = serde_json::from_value(value)?;
        let _write = MUTATIONS.lock().await;
        let root = self.workbench_root(&request.scope).await?;
        let directory = self.data_dir.join("workspace-metadata");
        let identity = crate::command::canonical_json_digest(&json!({"scope":request.scope,"root":root}))?;
        let path = directory.join(format!("{identity}.json"));
        let current: Organizer = if path.exists() { serde_json::from_slice(&fs::read(&path)?)? } else { Organizer::default() };
        let current_value = serde_json::to_value(&current)?;
        let current_digest = crate::command::canonical_json_digest(&current_value)?;
        if method == "workbench.organizer.get" { return Ok(json!({"value":current_value,"digest":current_digest})); }
        ensure!(method == "workbench.organizer.set", "workbench.operation_not_admitted");
        let mut next = request.value.context("workbench.metadata_required")?;
        ensure!(next.tags.len() <= 512 && next.directory_aliases.len() <= 512 && next.quick_messages.len() <= 512, "workbench.metadata_limit");
        let mut seen = std::collections::HashSet::new();
        for tag in &next.tags { ensure!(!tag.trim().is_empty() && tag.len() <= 256 && seen.insert(tag), "workbench.invalid_tag"); }
        let mut seen = std::collections::HashSet::new();
        for alias in &mut next.directory_aliases {
            let canonical = git::validate_workspace_directory(Path::new(&alias.path), &self.data_dir, false)?;
            alias.path = canonical.to_string_lossy().into_owned();
            ensure!(!alias.name.trim().is_empty() && alias.name.len() <= 256 && alias.group.len() <= 256 && seen.insert(alias.path.clone()), "workbench.invalid_alias");
        }
        let mut seen = std::collections::HashSet::new();
        for message in &next.quick_messages {
            uuid::Uuid::parse_str(&message.id).context("workbench.invalid_quick_message")?;
            ensure!(seen.insert(&message.id) && !message.title.trim().is_empty() && message.title.len() <= 512 && message.text.len() <= 1024 * 1024, "workbench.invalid_quick_message");
        }
        let value = serde_json::to_value(next)?;
        let next_digest = crate::command::canonical_json_digest(&value)?;
        if next_digest != current_digest {
            ensure!(request.expected_digest.as_deref() == Some(current_digest.as_str()), "workbench.metadata_changed");
            crate::platform::private_storage::prepare_private_directory(&directory)?;
            crate::platform::private_storage::atomic_write_private_json(&path, &value)?;
        }
        Ok(json!({"value":value,"digest":next_digest}))
    }

    async fn workbench_git_revision(&self, git: &crate::mission_workspace::MissionGit, root: &Path) -> Result<String> {
        let mut bytes = Vec::new();
        for args in [vec!["rev-parse", "--verify", "HEAD"], vec!["status", "--porcelain=v2", "--branch", "-z"], vec!["show-ref", "--head"], vec!["diff", "--no-ext-diff", "--no-textconv", "--binary"]] {
            let output = git.workbench_output(root, &args.into_iter().map(String::from).collect::<Vec<_>>()).await?;
            bytes.extend_from_slice(&output.status.code().unwrap_or(-1).to_le_bytes());
            bytes.extend_from_slice(&(output.stdout.bytes.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&output.stdout.bytes);
        }
        let output = git.workbench_output(root, &["rev-parse".into(), "--git-path".into(), "index".into()]).await?;
        if output.status.success() {
            let name = String::from_utf8(output.stdout.bytes)?.trim().to_owned();
            let index = Path::new(&name);
            let index = if index.is_absolute() { index.to_path_buf() } else { root.join(index) };
            if index.exists() { bytes.extend_from_slice(&fs::read(index)?); }
        }
        Ok(digest(&bytes))
    }

    pub(super) async fn workbench_git(&self, method: &str, value: Value) -> Result<Value> {
        let request: GitRequest = serde_json::from_value(value.clone())?;
        let root = self.workbench_root(&request.scope).await?;
        for path in &request.paths { ensure!(!path.is_empty(), "workbench.paths_required"); relative(&root, path, true)?; }
        let git = self.mission_git().await?;
        if method == "workbench.git.read" {
            let arguments = git_arguments(&request, false)?;
            let output = git.workbench_output(&root, &arguments).await?;
            return Ok(json!({"success":output.status.success(),"exitCode":output.status.code(),"output":String::from_utf8(output.stdout.bytes).context("workbench.git_non_utf8")?,"error":output.stderr.lossy_text()}));
        }
        let arguments = git_arguments(&request, true)?;
        let _write = MUTATIONS.lock().await;
        let revision = self.workbench_git_revision(&git, &root).await?;
        if method == "workbench.git.preview" {
            let clean = if request.action == "clean" {
                let mut preview = arguments.clone(); preview[1]="-n".into();
                Some(git.workbench_output(&root, &preview).await?.stdout.lossy_text())
            } else { None };
            return Ok(json!({"revision":revision,"action":request.action,"arguments":arguments,"cleanPreview":clean}));
        }
        ensure!(method == "workbench.git.execute" && request.confirmed, "workbench.confirmation_required");
        let id = request.command_id.as_deref().context("workbench.command_id_required")?;
        uuid::Uuid::parse_str(id).context("workbench.invalid_command_id")?;
        let identity = crate::command::canonical_json_digest(&json!({"root":root,"request":value}))?;
        let receipts = self.data_dir.join("workspace-operations");
        crate::platform::private_storage::prepare_private_directory(&receipts)?;
        let receipt = receipts.join(format!("{id}.json"));
        if receipt.exists() {
            let saved: Value = serde_json::from_slice(&fs::read(&receipt)?)?;
            ensure!(saved["identity"] == identity, "workbench.command_conflict");
            ensure!(saved["state"] == "finished", "workbench.operation_unknown_check_state");
            return Ok(saved["result"].clone());
        }
        ensure!(request.expected_revision.as_deref() == Some(revision.as_str()), "workbench.git_changed_refresh_preview");
        crate::platform::private_storage::atomic_write_private_json(&receipt, &json!({"identity":identity,"state":"started"}))?;
        let output = git.workbench_output(&root, &arguments).await?;
        let result = json!({"success":output.status.success(),"exitCode":output.status.code(),"output":output.stdout.lossy_text(),"error":output.stderr.lossy_text()});
        crate::platform::private_storage::atomic_write_private_json(&receipt, &json!({"identity":identity,"state":"finished","result":result}))?;
        Ok(result)
    }

    pub(super) async fn workbench_root(&self, scope: &Scope) -> Result<PathBuf> {
        let persisted: String = {
            let database = self.database.lock().await;
            let thread: String = database.connection().query_row(
                "SELECT project_path FROM camp WHERE id=?1", [&scope.thread_id], |row| row.get(0),
            ).context("workbench.thread_unavailable")?;
            if let Some(mission) = &scope.mission_id {
                database.connection().query_row(
                    "SELECT working_directory FROM mission_workspace WHERE mission_id=?1 AND camp_id=?2 AND state='ready'",
                    rusqlite::params![mission, scope.thread_id], |row| row.get(0),
                ).context("workbench.mission_unavailable")?
            } else { thread }
        };
        let root = git::validate_workspace_directory(Path::new(&persisted), &self.data_dir, false)?;
        ensure!(git::persisted_workspace_path_matches_canonical(Path::new(&persisted), &root), "workbench.workspace_changed");
        Ok(root)
    }

    pub(super) async fn workbench_files(&self, method: &str, value: Value) -> Result<Value> {
        let request: FileRequest = serde_json::from_value(value)?;
        let root = self.workbench_root(&request.scope).await?;
        let path = relative(&root, &request.path, false)?;
        match method {
            "workbench.files.list" => {
                ensure!(path.is_dir(), "workbench.directory_required");
                let mut entries = Vec::new();
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let name = entry.file_name().into_string().map_err(|_| anyhow::anyhow!("workbench.non_utf8_name"))?;
                    if name.eq_ignore_ascii_case(".git") { continue; }
                    let metadata = fs::symlink_metadata(entry.path())?;
                    let mut link = metadata.file_type().is_symlink();
                    #[cfg(windows)]
                    {
                        use std::os::windows::fs::MetadataExt;
                        link |= metadata.file_attributes() & 0x400 != 0;
                    }
                    let key = format!("{}:{}", if metadata.is_dir() { "0" } else { "1" }, name);
                    if request.after.as_ref().is_some_and(|after| key.as_str() <= after.as_str()) { continue; }
                    entries.push((key, json!({"name":name, "path":entry.path().strip_prefix(&root)?.to_string_lossy().replace('\\', "/"),
                        "directory":metadata.is_dir(), "link":link, "bytes":metadata.len()})));
                }
                entries.sort_by(|a,b| a.0.cmp(&b.0));
                let next = (entries.len() > 500).then(|| entries[499].0.clone());
                entries.truncate(500);
                Ok(json!({"entries":entries.into_iter().map(|(_,v)|v).collect::<Vec<_>>(), "next":next}))
            }
            "workbench.files.read" => {
                ensure!(path.is_file(), "workbench.file_required");
                ensure!(fs::metadata(&path)?.len() <= 16 * 1024 * 1024, "workbench.file_too_large");
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
        ensure!(!request.path.is_empty(), "workbench.root_immutable");
        let path = relative(&root, &request.path, true)?;
        ensure!(path != root, "workbench.root_immutable");
        let identity = crate::command::canonical_json_digest(&json!({"root":root,"request":value}))?;
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
        crate::platform::private_storage::atomic_write_private_json(&receipt, &json!({"identity":identity,"state":"started"}))?;
        let result = (|| -> Result<Value> {
            match request.action.as_str() {
                "save" => write_text(&path, request.text.as_deref().context("workbench.text_required")?, request.expected_digest.as_deref()),
                "create_directory" => { fs::create_dir(&path)?; Ok(json!({"applied":true})) }
                "rename" => {
                    let destination = relative(&root, request.destination.as_deref().context("workbench.destination_required")?, true)?;
                    ensure!(destination != root && !destination.exists(), "workbench.destination_exists");
                    ensure!(!destination.starts_with(&path), "workbench.invalid_destination");
                    if path.is_file() { ensure!(request.expected_digest.as_deref() == Some(digest(&fs::read(&path)?).as_str()), "workbench.file_changed"); }
                    fs::rename(&path, destination)?;
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
