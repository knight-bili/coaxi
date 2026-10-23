//! Per-open authority for Codeg's ref-counted OfficeCLI watch processes.
pub mod installer;
mod watch;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};
pub use watch::{acquire_sse_lease, note_proxy_request, release_sse_lease};
struct Lease {
    scope: String,
    root: String,
    path: String,
    port: u16,
    underlying: String,
    cap: String,
    editable: bool,
    last: Instant,
    owned_root: Option<PathBuf>,
}
static LEASES: LazyLock<Mutex<HashMap<String, Lease>>> = LazyLock::new(Default::default);
static START: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static SWEEP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn leases() -> std::sync::MutexGuard<'static, HashMap<String, Lease>> {
    LEASES.lock().unwrap_or_else(|error| error.into_inner())
}
fn view(id: &str, lease: &Lease) -> Value {
    json!({"holdId":id,"port":lease.port,"cap":lease.cap,"editable":lease.editable})
}
pub fn validate_watch_cap(port: u16, cap: &str) -> bool {
    leases().values().any(|lease| {
        lease.port == port
            && lease.cap == cap
            && lease.last.elapsed() < Duration::from_secs(180)
            && watch::validate_watch_cap(port, &lease.underlying)
    })
}
pub fn allows_edit(port: u16, cap: &str) -> bool {
    leases().values().any(|lease| {
        lease.port == port
            && lease.cap == cap
            && lease.editable
            && lease.last.elapsed() < Duration::from_secs(180)
            && watch::validate_watch_cap(port, &lease.underlying)
    })
}
pub fn detect() -> Value {
    json!({"installed":resolve_officecli().is_some(),"formats":["docx","xlsx","pptx"]})
}
pub async fn start(
    scope: &str,
    id: &str,
    root: &Path,
    path: &str,
    editable: bool,
) -> Result<Value> {
    uuid::Uuid::parse_str(id)?;
    let _start = START.lock().await;
    {
        let mut leases = leases();
        if let Some(lease) = leases.get_mut(id) {
            ensure!(
                lease.scope == scope
                    && lease.root == root.to_string_lossy()
                    && lease.path == path
                    && lease.editable == editable,
                "office.hold_identity_changed"
            );
            ensure!(
                watch::validate_watch_cap(lease.port, &lease.underlying),
                "office.original_watch_expired"
            );
            lease.last = Instant::now();
            return Ok(view(id, lease));
        }
        ensure!(leases.len() < 128, "office.lease_limit");
    }
    let _ = resolve_tree_path(root, path)?;
    let started = watch::start_office_watch_core(root.to_string_lossy().into_owned(), path.into())
        .await
        .map_err(|error| anyhow::anyhow!("office.{}: {error}", error.code()))?;
    let lease = Lease {
        scope: scope.into(),
        root: root.to_string_lossy().into_owned(),
        path: path.into(),
        port: started.port,
        underlying: started.cap,
        cap: uuid::Uuid::new_v4().simple().to_string(),
        editable,
        last: Instant::now(),
        owned_root: None,
    };
    let result = view(id, &lease);
    leases().insert(id.into(), lease);
    if !SWEEP.swap(true, std::sync::atomic::Ordering::AcqRel) {
        tokio::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let expired = {
                    let mut leases = leases();
                    let ids = leases
                        .iter()
                        .filter(|(_, lease)| lease.last.elapsed() >= Duration::from_secs(180))
                        .map(|(id, _)| id.clone())
                        .collect::<Vec<_>>();
                    ids.into_iter()
                        .filter_map(|id| leases.remove(&id))
                        .collect::<Vec<_>>()
                };
                for lease in expired {
                    let _ = watch::stop_office_watch_core(lease.root, lease.path).await;
                    if let Some(root) = lease.owned_root {
                        let _ = tokio::fs::remove_dir_all(root).await;
                    }
                }
                watch::sweep_office_watches(Duration::from_secs(300));
            }
        });
    }
    Ok(result)
}
pub fn heartbeat(scope: &str, id: &str) -> Result<Value> {
    let mut leases = leases();
    let lease = leases.get_mut(id).context("office.lease_expired")?;
    ensure!(
        lease.scope == scope && watch::validate_watch_cap(lease.port, &lease.underlying),
        "office.lease_expired"
    );
    lease.last = Instant::now();
    Ok(view(id, lease))
}
pub async fn release(scope: &str, id: &str) -> Result<Value> {
    let lease = {
        let mut leases = leases();
        if let Some(lease) = leases.get(id) {
            ensure!(lease.scope == scope, "office.hold_identity_changed")
        };
        leases.remove(id)
    };
    if let Some(lease) = lease {
        watch::stop_office_watch_core(lease.root, lease.path).await?;
        if let Some(root) = lease.owned_root {
            tokio::fs::remove_dir_all(root).await?;
        }
    }
    Ok(json!({"released":true}))
}
pub fn stop_all() {
    let roots = leases()
        .values()
        .filter_map(|lease| lease.owned_root.clone())
        .collect::<Vec<_>>();
    leases().clear();
    watch::stop_all_office_watches();
    for root in roots {
        let _ = std::fs::remove_dir_all(root);
    }
}
fn is_office_path(path: &Path) -> bool {
    path.extension()
        .and_then(|part| part.to_str())
        .is_some_and(|part| matches!(part.to_ascii_lowercase().as_str(), "docx" | "xlsx" | "pptx"))
}
fn resolve_tree_path(root: &Path, path: &str) -> Result<PathBuf> {
    ensure!(
        !path.is_empty() && !path.contains(['\0', ':', '\\']),
        "office.invalid_path"
    );
    let canonical = std::fs::canonicalize(root)?;
    let mut target = canonical.clone();
    for part in Path::new(path).components() {
        match part {
            Component::Normal(name) => {
                target.push(name);
                let metadata = std::fs::symlink_metadata(&target)?;
                ensure!(!metadata.file_type().is_symlink(), "office.linked_path");
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    ensure!(
                        metadata.file_attributes() & 0x400 == 0,
                        "office.linked_path"
                    );
                }
            }
            _ => anyhow::bail!("office.invalid_path"),
        }
    }
    let target = std::fs::canonicalize(target)?;
    ensure!(
        target.starts_with(canonical) && target.is_file(),
        "office.outside_root"
    );
    Ok(target)
}
pub(crate) fn resolve_officecli() -> Option<PathBuf> {
    let executable = if cfg!(windows) {
        "officecli.exe"
    } else {
        "officecli"
    };
    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            if path.is_absolute() {
                let candidate = path.join(executable);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    #[cfg(windows)]
    {
        if let Some(root) = std::env::var_os("LOCALAPPDATA") {
            let path = PathBuf::from(root).join("OfficeCLI").join(executable);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(root) = dirs::home_dir() {
            let path = root.join(".local/bin").join(executable);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}
pub(crate) fn tokio_command(program: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    command.env_clear();
    for key in [
        "PATH",
        "SystemRoot",
        "WINDIR",
        "HOME",
        "USERPROFILE",
        "TEMP",
        "TMP",
        "LOCALAPPDATA",
        "DOTNET_ROOT",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command.env("LANG", "C.UTF-8").env("LC_ALL", "C.UTF-8");
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    command
}

/// OfficeCLI sees one private read-only copy, never the attachment's siblings.
/// Core resolves the published owner before entering this function.
pub async fn start_attachment(
    scope: &str,
    id: &str,
    data_dir: &Path,
    source: &Path,
) -> Result<Value> {
    uuid::Uuid::parse_str(id)?;
    if let Some(lease) = leases().get(id) {
        ensure!(
            lease.scope == scope && lease.owned_root.is_some(),
            "office.hold_identity_changed"
        );
        return Ok(view(id, lease));
    }
    ensure!(is_office_path(source), "office.unsupported_attachment");
    let metadata = std::fs::symlink_metadata(source)?;
    ensure!(
        metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata.len() <= 256 * 1024 * 1024,
        "office.attachment_unavailable_or_too_large"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "office.linked_attachment"
        );
    }
    let base = data_dir.join("office-attachment-previews");
    crate::platform::private_storage::prepare_private_directory(&base)?;
    let owned = base.join(uuid::Uuid::new_v4().simple().to_string());
    crate::platform::private_storage::prepare_private_directory(&owned)?;
    let name = format!(
        "document.{}",
        source
            .extension()
            .and_then(|part| part.to_str())
            .context("office.unsupported_attachment")?
            .to_ascii_lowercase()
    );
    let destination = owned.join(&name);
    let copied = async {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut input = tokio::fs::File::open(source).await?;
        let observed = input.metadata().await?;
        ensure!(
            observed.is_file() && observed.len() == metadata.len(),
            "office.attachment_changed"
        );
        let mut output = tokio::fs::File::from_std(
            crate::platform::private_storage::create_private_new_file(&destination)?,
        );
        let count =
            tokio::io::copy(&mut (&mut input).take(256 * 1024 * 1024 + 1), &mut output).await?;
        ensure!(
            count == observed.len() && count <= 256 * 1024 * 1024,
            "office.attachment_changed"
        );
        output.flush().await?;
        output.sync_all().await?;
        Ok::<(), anyhow::Error>(())
    }
    .await;
    if let Err(error) = copied {
        let _ = tokio::fs::remove_dir_all(&owned).await;
        return Err(error);
    }
    match start(scope, id, &owned, &name, false).await {
        Ok(value) => {
            if let Some(lease) = leases().get_mut(id) {
                lease.owned_root = Some(owned);
            }
            Ok(value)
        }
        Err(error) => {
            let _ = tokio::fs::remove_dir_all(&owned).await;
            Err(error)
        }
    }
}
