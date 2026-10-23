//! 仅由已经过本机选择的历史导入登记；日志中的路径不是文件读取权限。
use super::*;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};
const MAX_FILE: u64 = 128 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Location {
    pub root: PathBuf,
    pub relative: Option<String>,
}
pub(crate) fn no_link(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "native_lifecycle.linked_source"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "native_lifecycle.linked_source"
        );
    }
    Ok(())
}
fn root(path: &Path) -> Result<PathBuf> {
    ensure!(
        path.is_absolute()
            && !path
                .components()
                .any(|part| matches!(part, Component::CurDir | Component::ParentDir)),
        "native_lifecycle.source_path_invalid"
    );
    for ancestor in path.ancestors() {
        no_link(ancestor)?
    }
    let path = fs::canonicalize(path)?;
    ensure!(path.is_dir(), "native_lifecycle.source_directory_required");
    Ok(path)
}
fn relative(base: &Path, name: &str) -> Result<PathBuf> {
    ensure!(
        !name.is_empty() && name.len() <= 8192 && !name.contains('\0') && !name.contains(':'),
        "native_lifecycle.source_path_invalid"
    );
    let mut path = base.to_path_buf();
    for part in Path::new(name).components() {
        match part {
            Component::Normal(value) => {
                path.push(value);
                no_link(&path)?
            }
            _ => anyhow::bail!("native_lifecycle.source_path_invalid"),
        }
    }
    ensure!(
        fs::canonicalize(&path)?.starts_with(base),
        "native_lifecycle.source_outside_root"
    );
    Ok(path)
}
fn hash_file(path: &Path) -> Result<(u64, String)> {
    no_link(path)?;
    let metadata = fs::metadata(path)?;
    ensure!(
        metadata.is_file() && metadata.len() <= MAX_FILE,
        "native_lifecycle.source_file_invalid"
    );
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        ensure!(total <= MAX_FILE, "native_lifecycle.source_file_too_large");
        hash.update(&buffer[..count]);
    }
    ensure!(total == metadata.len(), "native_lifecycle.source_changed");
    Ok((total, format!("sha256:{:x}", hash.finalize())))
}
fn excluded(path: &Path) -> bool {
    path.components().any(|part| {
        matches!(
            part.as_os_str()
                .to_string_lossy()
                .to_ascii_lowercase()
                .as_str(),
            ".git"
                | "node_modules"
                | "target"
                | "dist"
                | ".ssh"
                | ".aws"
                | "cache"
                | ".cache"
                | "bin"
                | "locks"
                | "credentials"
                | "oauth"
        )
    }) || path.file_name().is_some_and(|name| {
        let name = name.to_string_lossy().to_ascii_lowercase();
        name == ".env"
            || name.starts_with(".env.")
            || matches!(
                name.as_str(),
                "auth.json"
                    | "credentials.json"
                    | "oauth_creds.json"
                    | "acp_token.json"
                    | "acp_business_token.json"
                    | "secrets.json"
                    | "config.toml"
                    | "settings.json"
            )
    })
}
/// 目录源也包含 SQLite 的现有 WAL 字节；检测变化，不修改或 checkpoint 原数据库。
pub(crate) fn revision(location: &Location) -> Result<String> {
    let base = root(&location.root)?;
    if let Some(name) = &location.relative {
        return Ok(hash_file(&relative(&base, name)?)?.1);
    }
    let mut pending = vec![(base.clone(), 0usize)];
    let mut entries = Vec::new();
    let mut visited = 0usize;
    let mut bytes = 0u64;
    while let Some((directory, depth)) = pending.pop() {
        ensure!(depth <= 24, "native_lifecycle.source_depth_limit");
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            visited += 1;
            ensure!(visited <= 50000, "native_lifecycle.source_entry_limit");
            let path = entry.path();
            let name = path.strip_prefix(&base)?;
            if excluded(name) {
                continue;
            }
            no_link(&path)?;
            let metadata = fs::metadata(&path)?;
            if metadata.is_dir() {
                pending.push((path, depth + 1));
                continue;
            }
            ensure!(metadata.is_file(), "native_lifecycle.source_type_invalid");
            let file = name.file_name().unwrap_or_default().to_string_lossy();
            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let compressed_log = file.starts_with("session") && file.ends_with(".jsonl.zstd");
            if !compressed_log
                && ![
                    "json", "jsonl", "db", "sqlite", "sqlite3", "vscdb", "pb", "meta", "idx",
                ]
                .contains(&extension.as_str())
                && !file.ends_with("-wal")
                && !file.ends_with("-shm")
            {
                continue;
            }
            let (size, digest) = hash_file(&path)?;
            bytes = bytes
                .checked_add(size)
                .context("native_lifecycle.source_size_limit")?;
            ensure!(
                bytes <= 512 * 1024 * 1024,
                "native_lifecycle.source_size_limit"
            );
            entries.push((name.to_string_lossy().replace('\\', "/"), size, digest));
        }
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    digest(&json!(entries))
}
pub(crate) fn native_id(provider: &str, bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes)?;
    let mut ids = std::collections::BTreeSet::<String>::new();
    for line in text.lines() {
        let Ok(row) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let value: Option<&str> = match provider {
            "codex" if row["type"] == "session_meta" => {
                row.pointer("/payload/id").and_then(Value::as_str)
            }
            "claude" | "qoder" | "grok" => row["sessionId"]
                .as_str()
                .or_else(|| row["session_id"].as_str()),
            _ => None,
        };
        if let Some(id) = value {
            ensure!(
                !id.is_empty() && id.len() <= 4096 && !id.contains('\0'),
                "native_lifecycle.native_id_invalid"
            );
            ids.insert(id.to_owned());
            ensure!(ids.len() <= 1, "native_lifecycle.ambiguous_native_id");
        }
    }
    ids.into_iter()
        .next()
        .context("native_lifecycle.native_id_unavailable")
}
pub(crate) fn register(
    connection: &Connection,
    owner: &str,
    import: &str,
    import_revision: &str,
    provider: &str,
    native_id: &str,
    location: &Location,
    source_revision: &str,
) -> Result<NativeSourceReferenceV1> {
    let reference = NativeSourceReferenceV1 {
        schema_version: 1,
        import_id: import.into(),
        import_revision: import_revision.into(),
        provider: provider.into(),
        native_session_id: native_id.into(),
        source_handle_id: uuid::Uuid::new_v4().to_string(),
        source_revision: source_revision.into(),
    };
    validate_shape(
        "NativeSourceReferenceV1",
        &serde_json::to_value(&reference)?,
    )?;
    connection.execute("INSERT INTO native_lifecycle_source_handle(id,owner_thread_id,import_id,import_revision,provider,native_session_id,location_json,source_revision,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![reference.source_handle_id,owner,import,import_revision,provider,native_id,serde_json::to_string(location)?,source_revision,chrono::Utc::now().to_rfc3339()])?;
    Ok(reference)
}
pub(crate) fn latest(
    connection: &Connection,
    owner: &str,
    import: &str,
    import_revision: &str,
) -> Result<Option<NativeSourceReferenceV1>> {
    connection.query_row("SELECT id,provider,native_session_id,source_revision FROM native_lifecycle_source_handle WHERE owner_thread_id=?1 AND import_id=?2 AND import_revision=?3 AND revoked=0 ORDER BY created_at DESC,id DESC LIMIT 1",params![owner,import,import_revision],|row|Ok(NativeSourceReferenceV1{schema_version:1,import_id:import.into(),import_revision:import_revision.into(),provider:row.get(1)?,native_session_id:row.get(2)?,source_handle_id:row.get(0)?,source_revision:row.get(3)?})).optional().map_err(Into::into)
}
pub(crate) fn resolve(
    connection: &Connection,
    owner: &str,
    reference: &NativeSourceReferenceV1,
) -> Result<Location> {
    validate_shape("NativeSourceReferenceV1", &serde_json::to_value(reference)?)?;
    let raw:String=connection.query_row("SELECT location_json FROM native_lifecycle_source_handle WHERE id=?1 AND owner_thread_id=?2 AND import_id=?3 AND import_revision=?4 AND provider=?5 AND native_session_id=?6 AND source_revision=?7 AND revoked=0",params![reference.source_handle_id,owner,reference.import_id,reference.import_revision,reference.provider,reference.native_session_id,reference.source_revision],|row|row.get(0)).context("native_lifecycle.source_reselection_required")?;
    Ok(serde_json::from_str(&raw)?)
}
