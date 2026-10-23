//! Standalone channel transport. All business mutations still enter Core's
//! existing channel commands; the child owns SDK sessions and network I/O only.
use anyhow::{Context, Result, ensure};
use rovai_core::{
    application::CoreService,
    managed_process::{
        ManagedProcess, ManagedProcessLaunchSpec, ManagedProcessPurpose, ManagedStdinPolicy,
        ManagedWindowsArgvDialect,
    },
};
use rovai_web::{ChannelFuture, ChannelHost, ChannelRequest};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::{Mutex, mpsc, oneshot},
};
use tokio_util::sync::CancellationToken;

const MAX_FRAME: u64 = 8 * 1024 * 1024;
type Pending = Arc<StdMutex<HashMap<String, oneshot::Sender<Value>>>>;
struct Active {
    output: mpsc::Sender<Value>,
    pending: Pending,
    process: Arc<Mutex<ManagedProcess>>,
    cancel: CancellationToken,
    alive: Arc<AtomicBool>,
}
struct Inner {
    core: CoreService,
    active: Mutex<Option<Active>>,
    sequence: AtomicU64,
    closed: AtomicBool,
}
#[derive(Clone)]
pub struct ServerChannels {
    inner: Arc<Inner>,
}

impl ServerChannels {
    pub fn new(core: CoreService) -> Self {
        Self {
            inner: Arc::new(Inner {
                core,
                active: Mutex::new(None),
                sequence: AtomicU64::new(1),
                closed: AtomicBool::new(false),
            }),
        }
    }
    pub async fn start(&self) -> Result<()> {
        ensure!(
            !self.inner.closed.load(Ordering::Acquire),
            "channel_host_closed"
        );
        let mut active = self.inner.active.lock().await;
        if active
            .as_ref()
            .is_some_and(|value| value.alive.load(Ordering::Acquire))
        {
            return Ok(());
        }
        if let Some(previous) = active.take() {
            previous.cancel.cancel();
            let _ = previous.process.lock().await.force_terminate_tree();
        }
        let directory = std::fs::canonicalize(std::env::current_exe()?)?
            .parent()
            .context("channel_package_missing")?
            .to_owned();
        let manifest_path = directory.join("server-channels/manifest.json");
        ensure!(
            std::fs::metadata(&manifest_path)?.len() <= 1024 * 1024,
            "channel_manifest_invalid"
        );
        let manifest: Value = serde_json::from_slice(&std::fs::read(manifest_path)?)?;
        ensure!(
            manifest["schemaVersion"] == 1
                && manifest["protocol"] == 1
                && manifest["nodeMajor"] == 24,
            "channel_manifest_incompatible"
        );
        let bundle = checked_asset(
            &directory.join("server-channels"),
            &manifest["bundle"],
            64 * 1024 * 1024,
        )?;
        for asset in manifest["assets"]
            .as_array()
            .context("channel_manifest_invalid")?
        {
            checked_asset(&directory.join("server-channels"), asset, 16 * 1024 * 1024)?;
        }
        let program = checked_asset(&directory, &manifest["runtime"], 160 * 1024 * 1024)?;
        let mut command = tokio::process::Command::new(program);
        command.arg(bundle).current_dir(&directory).env_clear();
        for key in [
            "SystemRoot",
            "WINDIR",
            "TEMP",
            "TMP",
            "HOME",
            "USERPROFILE",
            "PATH",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "NO_PROXY",
            "SSL_CERT_FILE",
            "SSL_CERT_DIR",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        let spec = ManagedProcessLaunchSpec::capture(
            &command,
            ManagedProcessPurpose::RuntimeHost,
            ManagedStdinPolicy::Piped,
            ManagedWindowsArgvDialect::MicrosoftCrt,
            "standalone-channel-host",
        )?;
        let mut process = ManagedProcess::spawn(spec)?;
        let mut input = process.take_stdin().context("channel_pipe_unavailable")?;
        let stdout = process.take_stdout().context("channel_pipe_unavailable")?;
        let stderr = process.take_stderr().context("channel_pipe_unavailable")?;
        let process = Arc::new(Mutex::new(process));
        let alive = Arc::new(AtomicBool::new(true));
        let cancel = CancellationToken::new();
        let (output, mut outgoing) = mpsc::channel::<Value>(128);
        let pending: Pending = Default::default();
        let writer_cancel = cancel.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {_=writer_cancel.cancelled()=>break,value=outgoing.recv()=>{let Some(value)=value else{break};let Ok(mut bytes)=serde_json::to_vec(&value) else{break};if bytes.len() as u64>MAX_FRAME{break}bytes.push(b'\n');if input.write_all(&bytes).await.is_err(){break}}}
            }
        });
        // SDK logs remain outside the RPC pipe. Never relay cookie/credential-
        // bearing exception text to the public Web surface.
        tokio::spawn(async move {
            let mut error = stderr;
            let mut sink = tokio::io::sink();
            let _ = tokio::io::copy(&mut error, &mut sink).await;
        });
        let reader_cancel = cancel.clone();
        let replies = pending.clone();
        let sender = output.clone();
        let core = self.inner.core.clone();
        let living = alive.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                let mut limited = (&mut reader).take(MAX_FRAME + 1);
                let read = tokio::select! {_=reader_cancel.cancelled()=>break,value=limited.read_line(&mut line)=>value};
                let Ok(length) = read else { break };
                if length == 0 || length as u64 > MAX_FRAME || !line.ends_with('\n') {
                    break;
                }
                let Ok(frame) = serde_json::from_str::<Value>(&line) else {
                    break;
                };
                if let Some(id) = frame["id"].as_str() {
                    if let Ok(mut replies) = replies.lock() {
                        if let Some(reply) = replies.remove(id) {
                            let _ = reply.send(frame);
                        }
                    }
                    continue;
                }
                if let (Some(id), Some(method)) =
                    (frame["coreId"].as_str(), frame["method"].as_str())
                {
                    let (id, method, params) =
                        (id.to_owned(), method.to_owned(), frame["params"].clone());
                    let core = core.clone();
                    let sender = sender.clone();
                    tokio::spawn(async move {
                        let response = if allowed_core_method(&method) {
                            match core.request(&method, params).await {
                                Ok(reply) => {
                                    json!({"coreId":id,"result":reply.result,"error":reply.error})
                                }
                                Err(_) => {
                                    json!({"coreId":id,"error":{"message":"channel_core_result_unknown"}})
                                }
                            }
                        } else {
                            json!({"coreId":id,"error":{"message":"channel_operation_not_admitted"}})
                        };
                        let _ = sender.send(response).await;
                    });
                }
            }
            living.store(false, Ordering::Release);
            reader_cancel.cancel();
            if let Ok(mut replies) = replies.lock() {
                replies.clear();
            }
        });
        let mut events = self.inner.core.subscribe();
        let event_sender = output.clone();
        let event_cancel = cancel.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {_=event_cancel.cancelled()=>break,event=events.recv()=>match event{Ok(event)=>{if event_sender.send(json!({"event":event})).await.is_err(){break}},Err(tokio::sync::broadcast::error::RecvError::Lagged(_))=>{let _=event_sender.send(json!({"event":{"method":"core.resync_required","params":{}}})).await;},Err(_)=>break}}
            }
        });
        *active = Some(Active {
            output,
            pending,
            process,
            cancel,
            alive,
        });
        drop(active);
        self.exchange(json!({"operation":"start"}), Duration::from_secs(60))
            .await?;
        Ok(())
    }
    async fn exchange(&self, mut frame: Value, timeout: Duration) -> Result<Value> {
        let id = self
            .inner
            .sequence
            .fetch_add(1, Ordering::Relaxed)
            .to_string();
        let (reply, received) = oneshot::channel();
        let pending = {
            let active = self.inner.active.lock().await;
            let active = active.as_ref().context("channel_host_unavailable")?;
            ensure!(
                active.alive.load(Ordering::Acquire),
                "channel_host_unavailable"
            );
            let pending = active.pending.clone();
            {
                let mut slots = pending
                    .lock()
                    .map_err(|_| anyhow::anyhow!("channel_host_unavailable"))?;
                ensure!(slots.len() < 16, "channel_capacity");
                slots.insert(id.clone(), reply);
            }
            frame["id"] = json!(id);
            if active.output.send(frame).await.is_err() {
                if let Ok(mut slots) = pending.lock() {
                    slots.remove(&id);
                }
                anyhow::bail!("channel_host_unavailable")
            };
            pending
        };
        let result = tokio::time::timeout(timeout, received).await;
        if let Ok(mut slots) = pending.lock() {
            slots.remove(&id);
        }
        let value = result
            .context("channel_result_unknown")?
            .context("channel_result_unknown")?;
        ensure!(value["error"].is_null(), "channel_operation_failed");
        Ok(value["result"].clone())
    }
    pub async fn shutdown(&self) {
        self.inner.closed.store(true, Ordering::Release);
        let _ = self
            .exchange(json!({"operation":"stop"}), Duration::from_secs(5))
            .await;
        if let Some(active) = self.inner.active.lock().await.take() {
            active.cancel.cancel();
            if let Ok(mut pending) = active.pending.lock() {
                pending.clear();
            }
            let mut process = active.process.lock().await;
            let _ = process.force_terminate_tree();
            let _ = tokio::time::timeout(Duration::from_secs(3), process.wait()).await;
        }
    }
}

fn allowed_core_method(method: &str) -> bool {
    // This is a packaged, trusted worker seam, never a user-supplied Core RPC.
    method.starts_with("channels.")
        || matches!(
            method,
            "members.get"
                | "members.list"
                | "memberAvatars.read"
                | "thread.attachments.desktopOpenTarget"
        )
}
fn checked_asset(root: &Path, entry: &Value, limit: u64) -> Result<PathBuf> {
    let name = entry["path"].as_str().context("channel_manifest_invalid")?;
    let path = Path::new(name);
    ensure!(
        !path.is_absolute()
            && path
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_))),
        "channel_asset_path_invalid"
    );
    let mut target = root.to_owned();
    for part in path.components() {
        target.push(part);
        ensure!(
            !std::fs::symlink_metadata(&target)?.file_type().is_symlink(),
            "channel_asset_link_rejected"
        );
    }
    let metadata = std::fs::metadata(&target)?;
    ensure!(
        metadata.is_file() && metadata.len() <= limit,
        "channel_asset_invalid"
    );
    let digest = format!("{:x}", Sha256::digest(std::fs::read(&target)?));
    ensure!(
        entry["sha256"].as_str() == Some(digest.as_str()),
        "channel_asset_digest_mismatch"
    );
    Ok(target)
}
impl ChannelHost for ServerChannels {
    fn deployment(&self) -> &'static str {
        "server"
    }
    fn call(&self, request: ChannelRequest) -> ChannelFuture<'_> {
        Box::pin(async move {
            self.start().await.map_err(|_| "channel_host_unavailable")?;
            let response = self
                .exchange(json!({"request":request}), Duration::from_secs(60))
                .await
                .map_err(|_| "channel_result_unavailable")?;
            if let Some(error) = response["error"].as_str() {
                return Err(match error {
                    "channel_session_expired" => "channel_session_expired",
                    "channel_native_interaction" => "channel_native_interaction",
                    _ => "channel_operation_failed",
                });
            }
            let snapshot = &response["result"];
            if snapshot["schemaVersion"] != 4 || !snapshot["channels"].is_array() {
                return Err("channel_result_unavailable");
            }
            Ok(snapshot.clone())
        })
    }
}
