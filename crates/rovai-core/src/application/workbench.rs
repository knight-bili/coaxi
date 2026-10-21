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
}
