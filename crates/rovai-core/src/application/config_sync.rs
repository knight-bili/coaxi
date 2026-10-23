//! Portable configuration upload policy. A tick never applies remote settings.
use super::*;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    sync::{LazyLock, Mutex as SyncMutex},
    time::Instant,
};
use tokio_util::sync::CancellationToken;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Settings {
    revision: String,
    key_revision: String,
    enabled: bool,
    auto_sync: bool,
    interval_minutes: u32,
    passphrase: String,
    #[serde(default)]
    full_configuration: bool,
    #[serde(default)]
    include_secrets: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    #[serde(default)]
    active_command_id: Option<String>,
    #[serde(default)]
    pending_digest: Option<String>,
    #[serde(default)]
    last_digest: Option<String>,
    #[serde(default)]
    last_sync_at: Option<String>,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    failures: u32,
    #[serde(default)]
    next_at: i64,
    #[serde(default)]
    phase: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    operation: String,
    expected_revision: Option<String>,
    enabled: Option<bool>,
    auto_sync: Option<bool>,
    interval_minutes: Option<u32>,
    #[serde(default)]
    full_configuration: Option<bool>,
    #[serde(default)]
    include_secrets: Option<bool>,
    passphrase: Option<String>,
    #[serde(default)]
    clear_passphrase: bool,
    command_id: Option<String>,
    #[serde(default)]
    confirmed: bool,
}
static PREPARING: LazyLock<SyncMutex<HashMap<String, CancellationToken>>> =
    LazyLock::new(Default::default);
static STARTED: LazyLock<SyncMutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);
fn key(root: &Path, id: &str) -> String {
    format!("{}:{id}", root.display())
}
fn settings(root: &Path) -> Result<Option<Settings>> {
    let path = root.join("config-sync-settings-v1.json");
    if !path.exists() {
        return Ok(None);
    }
    ensure!(
        fs::metadata(&path)?.len() <= 65536,
        "config_sync.settings_limit"
    );
    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}
fn state(root: &Path) -> Result<State> {
    let path = root.join("config-sync-state-v1.json");
    if !path.exists() {
        return Ok(State::default());
    }
    ensure!(
        fs::metadata(&path)?.len() <= 65536,
        "config_sync.state_limit"
    );
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn save_state(root: &Path, value: &State) -> Result<()> {
    crate::platform::private_storage::atomic_write_private_json(
        &root.join("config-sync-state-v1.json"),
        value,
    )
}
fn view(settings: Option<&Settings>, state: &State) -> Value {
    json!({"revision":settings.map(|value|&value.revision),"enabled":settings.is_some_and(|value|value.enabled),"autoSync":settings.is_some_and(|value|value.auto_sync),"intervalMinutes":settings.map(|value|value.interval_minutes).unwrap_or(5),"passphraseSet":settings.is_some_and(|value|!value.passphrase.is_empty()),"fullConfiguration":settings.is_some_and(|value|value.full_configuration),"includeSecrets":settings.is_some_and(|value|value.full_configuration&&value.include_secrets),"state":state})
}
fn fail(root: &Path, id: &str, error: &str, interval: u32) {
    if let Ok(mut value) = state(root) {
        if value.active_command_id.as_deref() == Some(id) {
            value.last_error = Some(error.into());
            value.failures = value.failures.saturating_add(1);
            value.phase = "unknown".into();
            value.next_at = Utc::now().timestamp()
                + i64::from(interval.max(1)) * 60 * (1i64 << value.failures.min(4));
            let _ = save_state(root, &value);
        }
    }
}
fn reconcile(root: &Path, value: &mut State) -> Result<()> {
    let Some(id) = value.active_command_id.clone() else {
        return Ok(());
    };
    let preparing = PREPARING
        .lock()
        .map(|jobs| jobs.contains_key(&key(root, &id)))
        .unwrap_or(false);
    if preparing {
        return Ok(());
    }
    let receipt = root.join("webdav-operations").join(format!("{id}.json"));
    if !receipt.exists() {
        if !super::webdav_transfer::active(root, &id) {
            value.phase = "stopped".into();
            value.last_error = Some("配置准备已中断，尚未开始远端传输；请重新上传。".into());
            value.active_command_id = None;
            value.next_at = i64::MAX;
            save_state(root, value)?;
        }
        return Ok(());
    }
    let result = super::webdav_transfer::status(root, &id)?;
    if result["state"] == "completed" {
        value.last_digest = value.pending_digest.take();
        value.active_command_id = None;
        value.last_sync_at = Some(Utc::now().to_rfc3339());
        value.last_error = None;
        value.failures = 0;
        value.phase = "idle".into();
        save_state(root, value)?;
    } else if result["state"] == "unknown" {
        value.phase = "unknown".into();
        value.last_error = result["message"].as_str().map(str::to_owned);
        save_state(root, value)?;
    }
    Ok(())
}
pub(super) fn original_receipt(root: &Path, id: &str) -> Result<Value> {
    uuid::Uuid::parse_str(id)?;
    if root
        .join("webdav-operations")
        .join(format!("{id}.json"))
        .exists()
    {
        let result = super::webdav_transfer::status(root, id)?;
        return Ok(
            json!({"commandId":id,"state":result["state"],"operation":result["operation"],"snapshot":result["result"]["snapshot"]}),
        );
    }
    let value = state(root)?;
    Ok(
        json!({"commandId":id,"state":if value.active_command_id.as_deref()==Some(id)&&value.phase=="preparing"{"preparing"}else{"unknown"}}),
    )
}
fn begin(
    root: &Path,
    current: Settings,
    connection: crate::backup::webdav::Config,
    account_revision: String,
    id: String,
    force: bool,
    full: Option<(Value, String)>,
) -> Result<Value> {
    uuid::Uuid::parse_str(&id)?;
    ensure!(
        !root
            .join("webdav-operations")
            .join(format!("{id}.json"))
            .exists(),
        "config_sync.command_identity_already_used"
    );
    let mut previous = state(root)?;
    ensure!(
        previous.active_command_id.is_none(),
        "config_sync.original_upload_needs_reconciliation"
    );
    let configured = crate::backup::owned_roots::configured(root)?;
    let preferences = configured
        .iter()
        .find(|value| value.id == "desktop_preferences" && value.kind == "preferences")
        .map(|value| value.path.clone())
        .unwrap_or_else(|| root.into());
    let token = CancellationToken::new();
    previous.active_command_id = Some(id.clone());
    previous.pending_digest = None;
    previous.phase = "preparing".into();
    previous.last_error = None;
    previous.next_at = Utc::now().timestamp() + i64::from(current.interval_minutes) * 60;
    save_state(root, &previous)?;
    PREPARING
        .lock()
        .map_err(|_| anyhow::anyhow!("config_sync.preparation_unavailable"))?
        .insert(key(root, &id), token.clone());
    let root = root.to_path_buf();
    let response_id = id.clone();
    tokio::spawn(async move {
        let result=async{
            enum FrozenSnapshot{Light(crate::backup::light_config::Snapshot),Full(Value)}
            let(frozen,content)=if let Some((snapshot,digest))=full{(FrozenSnapshot::Full(snapshot),json!({"fullConfigurationDigest":digest}))}else{
                let snapshot=tokio::task::spawn_blocking({let root=root.clone();let preferences=preferences.clone();move||crate::backup::light_config::collect(&root,&preferences)}).await??;
                let content=serde_json::to_value(&snapshot.0.domains)?;(FrozenSnapshot::Light(snapshot.0),content)
            };
            let digest=crate::command::canonical_json_digest(&json!({"domains":content,"keyRevision":current.key_revision,"accountRevision":account_revision,"fullConfiguration":current.full_configuration,"includeSecrets":current.include_secrets}))?;
            if !force&&previous.last_digest.as_deref()==Some(digest.as_str()){let _lock=workbench::MUTATIONS.lock().await;let mut value=state(&root)?;if value.active_command_id.as_deref()==Some(id.as_str()){value.active_command_id=None;value.phase="idle".into();save_state(&root,&value)?;}return Ok::<_,anyhow::Error>(())}
            let path=tokio::task::spawn_blocking({let root=root.clone();let id=id.clone();let secret=current.passphrase.clone();let token=token.clone();move||match frozen{FrozenSnapshot::Light(snapshot)=>crate::backup::light_config::cache_snapshot(&root,&snapshot,&id,&secret,&token),FrozenSnapshot::Full(snapshot)=>crate::portable_configuration::export::cache_for_sync(&root,&snapshot,&id,&secret,&token)}}).await??;
            let source_digest=crate::backup::file_digest(&path)?.1;let _lock=workbench::MUTATIONS.lock().await;ensure!(settings(&root)?.as_ref().is_some_and(|value|value.revision==current.revision),"config_sync.settings_changed");let mut value=state(&root)?;ensure!(value.active_command_id.as_deref()==Some(id.as_str()),"config_sync.operation_changed");value.pending_digest=Some(digest);value.phase="transferring".into();save_state(&root,&value)?;
            let request=json!({"operation":"push","commandId":id,"path":path,"expectedDigest":source_digest,"expectedRevision":account_revision,"kind":"configuration"});let identity=crate::command::canonical_json_digest(&request)?;super::webdav_transfer::start_with_token(&root,connection,request,identity,token.clone())?;Ok(())
        }.await;
        if let Err(error) = result {
            let _lock = workbench::MUTATIONS.lock().await;
            fail(&root, &id, &format!("{error:#}"), current.interval_minutes);
        }
        if let Ok(mut jobs) = PREPARING.lock() {
            jobs.remove(&key(&root, &id));
        }
    });
    Ok(json!({"state":"preparing","commandId":response_id}))
}
impl Core {
    pub(super) async fn config_sync_access(&self, value: Value) -> Result<Value> {
        let request: Request = serde_json::from_value(value)?;
        let _lock = workbench::MUTATIONS.lock().await;
        let mut status = state(&self.data_dir)?;
        reconcile(&self.data_dir, &mut status)?;
        let old = settings(&self.data_dir)?;
        if request.operation == "get" {
            return Ok(view(old.as_ref(), &status));
        }
        if matches!(
            request.operation.as_str(),
            "cache_preview" | "cache_remove" | "cache_status"
        ) {
            ensure!(
                old.as_ref().map(|value| value.revision.as_str())
                    == request.expected_revision.as_deref(),
                "config_sync.settings_changed"
            );
            let id = request
                .command_id
                .context("config_sync.cleanup_command_required")?;
            if request.operation == "cache_preview" {
                return super::config_sync_cleanup::preview(
                    &self.data_dir,
                    &id,
                    request.expected_revision,
                    status.active_command_id.as_deref(),
                );
            }
            return super::config_sync_cleanup::apply(
                &self.data_dir,
                &id,
                request.expected_revision.as_deref(),
                status.active_command_id.as_deref(),
                request.confirmed,
                request.operation == "cache_status",
            );
        }
        if request.operation == "set" {
            ensure!(
                status.active_command_id.is_none(),
                "config_sync.original_upload_needs_reconciliation"
            );
            ensure!(
                old.as_ref().map(|value| value.revision.as_str())
                    == request.expected_revision.as_deref(),
                "config_sync.settings_changed"
            );
            let clear = request.clear_passphrase;
            let changed = request
                .passphrase
                .as_ref()
                .is_some_and(|secret| !secret.is_empty())
                || clear;
            let secret = if clear {
                String::new()
            } else {
                request
                    .passphrase
                    .filter(|secret| !secret.is_empty())
                    .or_else(|| old.as_ref().map(|value| value.passphrase.clone()))
                    .unwrap_or_default()
            };
            let enabled = request.enabled.unwrap_or(false);
            let interval = request.interval_minutes.unwrap_or(5);
            ensure!(
                (1..=1440).contains(&interval)
                    && secret.len() <= 16000
                    && (!enabled || secret.chars().count() >= 8),
                "config_sync.invalid_settings"
            );
            let next = Settings {
                revision: uuid::Uuid::new_v4().to_string(),
                key_revision: if changed {
                    uuid::Uuid::new_v4().to_string()
                } else {
                    old.as_ref()
                        .map(|value| value.key_revision.clone())
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
                },
                enabled,
                auto_sync: request.auto_sync.unwrap_or(false),
                interval_minutes: interval,
                passphrase: secret,
                full_configuration: request.full_configuration.unwrap_or(false),
                include_secrets: request.full_configuration.unwrap_or(false)
                    && request.include_secrets.unwrap_or(false),
            };
            crate::platform::private_storage::atomic_write_private_json(
                &self.data_dir.join("config-sync-settings-v1.json"),
                &next,
            )?;
            status.next_at = Utc::now().timestamp() + 60;
            status.failures = 0;
            save_state(&self.data_dir, &status)?;
            return Ok(view(Some(&next), &status));
        }
        let Some(current) = old else {
            return if request.operation == "tick" {
                Ok(json!({"skipped":true}))
            } else {
                Err(anyhow::anyhow!("config_sync.not_configured"))
            };
        };
        if request.operation == "tick" && !current.enabled {
            return Ok(json!({"skipped":true}));
        }
        if ["upload", "cancel", "resume", "archive"].contains(&request.operation.as_str()) {
            ensure!(
                request.expected_revision.as_deref() == Some(current.revision.as_str()),
                "config_sync.settings_changed"
            );
        }
        if ["cancel", "resume", "archive"].contains(&request.operation.as_str()) {
            ensure!(
                request.command_id.as_deref() == status.active_command_id.as_deref()
                    && status.active_command_id.is_some(),
                "config_sync.active_operation_changed"
            );
        }
        if request.operation == "archive" {
            ensure!(request.confirmed, "config_sync.confirmation_required");
            let id = status
                .active_command_id
                .clone()
                .context("config_sync.no_active_upload")?;
            ensure!(
                !super::webdav_transfer::active(&self.data_dir, &id)
                    && !PREPARING
                        .lock()
                        .map_err(|_| anyhow::anyhow!("config_sync.preparation_unavailable"))?
                        .contains_key(&key(&self.data_dir, &id)),
                "config_sync.original_upload_running"
            );
            let directory = crate::platform::prepare_private_directory(
                &self.data_dir.join("config-sync-archived"),
            )?;
            crate::platform::private_storage::atomic_write_private_json(
                &directory.join(format!("{id}.json")),
                &json!({"state":status,"resolution":"user_archived_unknown","at":Utc::now().to_rfc3339()}),
            )?;
            status.active_command_id = None;
            status.pending_digest = None;
            status.phase = "idle".into();
            status.last_error = None;
            status.next_at = Utc::now().timestamp() + i64::from(current.interval_minutes) * 60;
            save_state(&self.data_dir, &status)?;
            return Ok(view(Some(&current), &status));
        }
        if request.operation == "cancel" {
            let id = status
                .active_command_id
                .as_deref()
                .context("config_sync.no_active_upload")?;
            if let Ok(jobs) = PREPARING.lock() {
                if let Some(token) = jobs.get(&key(&self.data_dir, id)) {
                    token.cancel();
                }
            }
            return super::webdav_transfer::cancel(&self.data_dir, id);
        }
        if request.operation == "resume" {
            let id = status
                .active_command_id
                .as_deref()
                .context("config_sync.no_active_upload")?;
            ensure!(
                !super::webdav_transfer::active(&self.data_dir, id),
                "config_sync.original_upload_running"
            );
            let path = self
                .data_dir
                .join("webdav-operations")
                .join(format!("{id}.json"));
            let receipt: Value = serde_json::from_slice(&fs::read(path)?)?;
            let original = receipt["request"].clone();
            let (connection, revision) = super::webdav_access::saved_connection(&self.data_dir)?;
            ensure!(
                original["expectedRevision"] == revision && original["kind"] == "configuration",
                "config_sync.account_changed"
            );
            let digest = crate::command::canonical_json_digest(&original)?;
            ensure!(
                receipt["identity"] == digest,
                "config_sync.command_identity_changed"
            );
            return super::webdav_transfer::start(&self.data_dir, connection, original, digest);
        }
        ensure!(
            ["tick", "upload"].contains(&request.operation.as_str()),
            "config_sync.operation_not_admitted"
        );
        ensure!(
            current.enabled && !current.passphrase.is_empty(),
            "config_sync.disabled"
        );
        if request.operation == "tick" {
            let mut started = STARTED
                .lock()
                .map_err(|_| anyhow::anyhow!("config_sync.scheduler_unavailable"))?;
            let elapsed = started
                .entry(self.data_dir.to_string_lossy().into_owned())
                .or_insert_with(Instant::now)
                .elapsed();
            if !current.auto_sync
                || elapsed.as_secs() < 60
                || status.active_command_id.is_some()
                || Utc::now().timestamp() < status.next_at
            {
                return Ok(json!({"skipped":true}));
            }
        }
        let (connection, revision) = match super::webdav_access::saved_connection(&self.data_dir) {
            Ok(value) => value,
            Err(_) if request.operation == "tick" => {
                return Ok(json!({"skipped":true,"reason":"webdav_not_configured"}));
            }
            Err(error) => return Err(error),
        };
        let full = if current.full_configuration {
            let database = self.database.lock().await;
            let digest = crate::portable_configuration::export::configuration_digest(
                database.connection(),
                &self.data_dir,
                current.include_secrets,
            )?;
            let snapshot = crate::portable_configuration::export::collect(
                database.connection(),
                &self.data_dir,
                current.include_secrets,
            )?;
            Some((snapshot, digest))
        } else {
            None
        };
        let id = request
            .command_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        begin(
            &self.data_dir,
            current,
            connection,
            revision,
            id,
            request.operation == "upload",
            full,
        )
    }
}
