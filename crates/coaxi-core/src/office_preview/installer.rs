use super::*;
use crate::managed_operation::run_bounded_command as execute;
use std::{fs, sync::Arc};
use tokio_util::sync::CancellationToken;
include!("installer_command.rs");

struct Active {
    root: PathBuf,
    cancel: CancellationToken,
    log: Arc<tokio::sync::Mutex<Vec<u8>>>,
}
static ACTIVE: LazyLock<tokio::sync::Mutex<HashMap<String, Active>>> =
    LazyLock::new(Default::default);
static MUTATION: LazyLock<Arc<tokio::sync::Mutex<()>>> =
    LazyLock::new(|| Arc::new(tokio::sync::Mutex::new(())));
fn receipt(root: &Path, id: &str) -> PathBuf {
    root.join("office-cli-operations")
        .join(format!("{id}.json"))
}
pub async fn probe(root: &Path) -> Result<Value> {
    let Some(binary) = resolve_officecli() else {
        return Ok(json!({"installed":false,"runnable":false,"manualUrl":OFFICECLI_MANUAL_URL}));
    };
    let digest = crate::backup::file_digest(&binary)?.1;
    let log = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let mut command = tokio_command(&binary);
    command.current_dir(root).arg("--version");
    let result = execute(
        command,
        "office-cli-probe",
        Duration::from_secs(15),
        CancellationToken::new(),
        log.clone(),
    )
    .await;
    let text = String::from_utf8_lossy(&log.lock().await).trim().to_owned();
    Ok(
        json!({"installed":true,"runnable":result.is_ok(),"version":if result.is_ok(){Some(text.clone())}else{None},"runtimeError":result.err().map(|error|format!("{error}: {text}")),"digest":digest,"manualUrl":OFFICECLI_MANUAL_URL}),
    )
}
fn shell_program(name: &str) -> Result<PathBuf> {
    #[cfg(windows)]
    if name == "powershell.exe" {
        let root = std::env::var_os("SystemRoot").context("office.system_root_missing")?;
        let path = PathBuf::from(root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        ensure!(path.is_absolute() && path.is_file(), "office.shell_missing");
        return Ok(path);
    }
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        if directory.is_absolute() {
            let path = directory.join(name);
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    anyhow::bail!("office.shell_missing")
}
pub async fn operation(root: &Path, value: Value) -> Result<Value> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Request {
        operation: String,
        command_id: Option<String>,
        #[serde(default)]
        confirmed: bool,
        expected_digest: Option<String>,
    }
    let request: Request = serde_json::from_value(value)?;
    if request.operation == "detect" {
        return probe(root).await;
    }
    let id = request.command_id.context("office.command_id_required")?;
    uuid::Uuid::parse_str(&id)?;
    let path = receipt(root, &id);
    if matches!(request.operation.as_str(), "status" | "cancel") {
        let active = ACTIVE.lock().await;
        if let Some(task) = active.get(&id) {
            ensure!(task.root == root, "office.owner_mismatch");
            if request.operation == "cancel" {
                task.cancel.cancel()
            }
            let log = String::from_utf8_lossy(&task.log.lock().await).into_owned();
            return Ok(json!({"commandId":id,"state":"running","log":log}));
        }
        if path.exists() {
            let mut status: Value = serde_json::from_slice(&fs::read(path)?)?;
            if status["state"] == "running" {
                status["state"] = json!("unknown");
                status["error"] = json!("操作中断，请先检测当前安装状态。原请求不会重新执行。")
            }
            return Ok(status);
        }
        return Ok(json!({"commandId":id,"state":"not_found"}));
    }
    ensure!(
        matches!(request.operation.as_str(), "install" | "uninstall") && request.confirmed,
        "office.confirmation_required"
    );
    if path.exists() {
        let status: Value = serde_json::from_slice(&fs::read(&path)?)?;
        ensure!(
            status["operation"] == request.operation
                && status["expectedDigest"] == json!(request.expected_digest),
            "office.command_identity_changed"
        );
        return Ok(status);
    }
    let guard = MUTATION
        .clone()
        .try_lock_owned()
        .map_err(|_| anyhow::anyhow!("office.operation_busy"))?;
    crate::platform::private_storage::prepare_private_directory(
        &root.join("office-cli-operations"),
    )?;
    let record = json!({"commandId":id,"operation":request.operation,"expectedDigest":request.expected_digest,"state":"running","startedAt":chrono::Utc::now().to_rfc3339()});
    crate::platform::private_storage::atomic_write_private_json(&path, &record)?;
    let cancel = CancellationToken::new();
    let log = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    ACTIVE.lock().await.insert(
        id.clone(),
        Active {
            root: root.to_path_buf(),
            cancel: cancel.clone(),
            log: log.clone(),
        },
    );
    let root = root.to_path_buf();
    let started = record.clone();
    tokio::spawn(async move {
        let _guard = guard;
        let result = async {
            if request.operation == "install" {
                let invocation = officecli_install_command(current_install_os());
                let program = shell_program(&invocation.program)?;
                let mut command = tokio_command(program);
                command.current_dir(&root).args(invocation.args);
                execute(
                    command,
                    &format!("office-install-{id}"),
                    OFFICECLI_INSTALL_TIMEOUT,
                    cancel,
                    log.clone(),
                )
                .await?;
                let info = probe(&root).await?;
                ensure!(
                    info["runnable"] == true,
                    "office.installed_but_unrunnable: {}",
                    info["runtimeError"]
                );
                Ok(info)
            } else {
                stop_all();
                #[cfg(windows)]
                let binary = PathBuf::from(
                    std::env::var_os("LOCALAPPDATA").context("office.install_root_missing")?,
                )
                .join("OfficeCLI/officecli.exe");
                #[cfg(not(windows))]
                let binary = dirs::home_dir()
                    .context("office.install_root_missing")?
                    .join(".local/bin/officecli");
                if binary.exists() {
                    let metadata = fs::symlink_metadata(&binary)?;
                    ensure!(
                        metadata.is_file() && !metadata.file_type().is_symlink(),
                        "office.linked_installation"
                    );
                    ensure!(
                        request.expected_digest.as_deref()
                            == Some(crate::backup::file_digest(&binary)?.1.as_str()),
                        "office.installation_changed"
                    );
                    fs::remove_file(binary)?;
                }
                probe(&root).await
            }
        }
        .await;
        let mut record = record;
        record["completedAt"] = json!(chrono::Utc::now().to_rfc3339());
        record["log"] = json!(String::from_utf8_lossy(&log.lock().await).into_owned());
        match result {
            Ok(info) => {
                record["state"] = json!("completed");
                record["info"] = info
            }
            Err(error) => {
                record["state"] = json!("failed");
                record["error"] = json!(error.to_string())
            }
        }
        let _ = crate::platform::private_storage::atomic_write_private_json(&path, &record);
        ACTIVE.lock().await.remove(&id);
    });
    Ok(started)
}
