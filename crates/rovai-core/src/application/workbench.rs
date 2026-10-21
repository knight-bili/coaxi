//! User workspace operations. Scope roots come from Core, never Renderer paths.
use super::*;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Component};

static MUTATIONS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Scope {
    thread_id: String,
    #[serde(default)]
    mission_id: Option<String>,
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
    async fn workbench_root(&self, scope: &Scope) -> Result<PathBuf> {
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
