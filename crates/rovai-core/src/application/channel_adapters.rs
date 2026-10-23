//! Telegram/Weixin transport configuration references canonical Core Threads.
//! Secret snapshots are available only to the local Host, never public clients.
use super::workbench::MUTATIONS;
use super::*;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Binding {
    pub chat_id: String,
    pub thread_key: Option<String>,
    pub thread_id: String,
    pub agent_ids: Vec<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Adapter {
    pub id: String,
    pub local_id: i32,
    pub provider: String,
    pub name: String,
    pub enabled: bool,
    pub token: String,
    pub base_url: String,
    pub chat_id: String,
    pub topic_mode: bool,
    pub allowed_senders: Vec<String>,
    #[serde(default)]
    pub control_senders: Vec<String>,
    #[serde(default)]
    pub approval_senders: Vec<String>,
    #[serde(default = "default_prefix")]
    pub command_prefix: String,
    #[serde(default = "default_language")]
    pub message_language: String,
    #[serde(default)]
    pub notification_kinds: Vec<String>,
    #[serde(default)]
    pub daily_report_enabled: bool,
    #[serde(default = "default_report_time")]
    pub daily_report_time: String,
    pub bindings: Vec<Binding>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    operation: String,
    id: Option<String>,
    command_id: Option<String>,
    expected_revision: Option<String>,
    adapter: Option<AdapterInput>,
    attempt_id: Option<String>,
    #[serde(default)]
    confirmed: bool,
    outbox_id: Option<String>,
    expected_output_digest: Option<String>,
    after: Option<i64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AdapterInput {
    id: String,
    provider: String,
    name: String,
    enabled: bool,
    token: Option<String>,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    chat_id: String,
    #[serde(default)]
    topic_mode: bool,
    #[serde(default)]
    allowed_senders: Vec<String>,
    #[serde(default)]
    control_senders: Vec<String>,
    #[serde(default)]
    approval_senders: Vec<String>,
    #[serde(default = "default_prefix")]
    command_prefix: String,
    #[serde(default = "default_language")]
    message_language: String,
    #[serde(default)]
    notification_kinds: Vec<String>,
    #[serde(default)]
    daily_report_enabled: bool,
    #[serde(default = "default_report_time")]
    daily_report_time: String,
    #[serde(default)]
    bindings: Vec<Binding>,
}
fn default_prefix() -> String {
    "/".into()
}
fn default_language() -> String {
    "zh-cn".into()
}
fn default_report_time() -> String {
    "18:00".into()
}
fn identity(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}
fn projected(adapters: &[Adapter]) -> Value {
    json!(adapters.iter().map(|value|json!({"id":value.id,"localId":value.local_id,"provider":value.provider,"name":value.name,"enabled":value.enabled,"hasToken":!value.token.is_empty(),"baseUrl":value.base_url,"chatId":value.chat_id,"topicMode":value.topic_mode,"allowedSenders":value.allowed_senders,"controlSenders":value.control_senders,"approvalSenders":value.approval_senders,"commandPrefix":value.command_prefix,"messageLanguage":value.message_language,"notificationKinds":value.notification_kinds,"dailyReportEnabled":value.daily_report_enabled,"dailyReportTime":value.daily_report_time,"bindings":value.bindings})).collect::<Vec<_>>())
}
impl Core {
    fn channel_adapter_file(&self) -> std::path::PathBuf {
        self.data_dir.join("channel-adapters.json")
    }
    pub(super) fn read_channel_adapters(&self) -> Result<Vec<Adapter>> {
        let path = self.channel_adapter_file();
        if !path.exists() {
            return Ok(Vec::new());
        }
        ensure!(
            fs::metadata(&path)?.len() <= 4 * 1024 * 1024,
            "channel_adapter.configuration_too_large"
        );
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }
    pub(super) async fn channel_adapter_operation(
        &self,
        value: Value,
        private: bool,
    ) -> Result<Value> {
        let request: Request = serde_json::from_value(value.clone())?;
        let _guard = MUTATIONS.lock().await;
        let mut adapters = self.read_channel_adapters()?;
        let revision = crate::command::canonical_json_digest(&serde_json::to_value(&adapters)?)?;
        if request.operation == "outbox_inspect" {
            drop(_guard);
            return self
                .channel_adapter_outbox(json!({"operation":"list","after":request.after}))
                .await;
        }
        if request.operation == "list" {
            return Ok(
                json!({"revision":revision,"adapters":if private{serde_json::to_value(adapters)?}else{projected(&adapters)},"hostState":self.channel_adapter_public_status()}),
            );
        }
        ensure!(
            !private
                && [
                    "save",
                    "remove",
                    "weixin_qr_begin",
                    "weixin_qr_check",
                    "outbox_retry",
                    "outbox_confirm"
                ]
                .contains(&request.operation.as_str()),
            "channel_adapter.operation_not_admitted"
        );
        let command_id = request
            .command_id
            .as_deref()
            .context("channel_adapter.command_required")?;
        uuid::Uuid::parse_str(command_id)?;
        let digest = crate::command::canonical_json_digest(&value)?;
        let directory = self.data_dir.join("channel-adapter-operations");
        crate::platform::private_storage::prepare_private_directory(&directory)?;
        let receipt = directory.join(format!("{command_id}.json"));
        if receipt.exists() {
            let previous: Value = serde_json::from_slice(&fs::read(&receipt)?)?;
            ensure!(
                previous["digest"] == digest,
                "channel_adapter.command_conflict"
            );
            ensure!(
                previous["state"] == "finished",
                "channel_adapter.result_unknown"
            );
            return Ok(previous["result"].clone());
        }
        ensure!(
            request.expected_revision.as_deref() == Some(revision.as_str()),
            "channel_adapter.settings_changed"
        );
        if matches!(
            request.operation.as_str(),
            "outbox_retry" | "outbox_confirm"
        ) {
            ensure!(request.confirmed, "channel_adapter.confirmation_required");
            let id = request
                .outbox_id
                .as_deref()
                .context("channel_adapter.output_id_required")?;
            ensure!(
                id.starts_with("adapter_")
                    && id[8..].len() == 64
                    && id[8..].bytes().all(|byte| byte.is_ascii_hexdigit()),
                "channel_adapter.invalid_output_id"
            );
            let file = self
                .data_dir
                .join("channel-adapter-outbox")
                .join(format!("{id}.json"));
            let mut saved: Value = serde_json::from_slice(&fs::read(&file)?)?;
            ensure!(
                saved["digest"]
                    == request
                        .expected_output_digest
                        .as_deref()
                        .context("channel_adapter.output_digest_required")?,
                "channel_adapter.output_changed"
            );
            ensure!(
                ["sending", "unknown"]
                    .iter()
                    .any(|state| saved["state"] == *state),
                "channel_adapter.output_not_unknown"
            );
            ensure!(
                saved["state"] != "sending"
                    || chrono::Utc::now().timestamp()
                        - saved["updatedAt"].as_i64().unwrap_or(i64::MAX)
                        > 120,
                "channel_adapter.output_still_sending"
            );
            saved["state"] = json!(if request.operation == "outbox_retry" {
                "ready"
            } else {
                "sent"
            });
            saved["updatedAt"] = json!(chrono::Utc::now().timestamp());
            saved["resolution"] =
                json!({"commandId":command_id,"operation":request.operation,"actor":"user"});
            crate::platform::private_storage::atomic_write_private_json(
                &receipt,
                &json!({"digest":digest,"state":"started"}),
            )?;
            crate::platform::private_storage::atomic_write_private_json(&file, &saved)?;
            let result = json!({"applied":true,"state":saved["state"]});
            crate::platform::private_storage::atomic_write_private_json(
                &receipt,
                &json!({"digest":digest,"state":"finished","result":result}),
            )?;
            return Ok(result);
        }
        if request.operation == "weixin_qr_begin" {
            crate::platform::private_storage::atomic_write_private_json(
                &receipt,
                &json!({"digest":digest,"state":"started"}),
            )?;
            let qr = rovai_channels::weixin_get_qrcode().await?;
            let id = uuid::Uuid::new_v4().to_string();
            let pending = self.data_dir.join("channel-adapter-qr");
            crate::platform::private_storage::prepare_private_directory(&pending)?;
            crate::platform::private_storage::atomic_write_private_json(
                &pending.join(format!("{id}.json")),
                &json!({"qrcode":qr.qrcode_id,"createdAt":chrono::Utc::now().timestamp(),"revision":revision}),
            )?;
            let result = json!({"attemptId":id,"image":qr.qrcode_img_content,"revision":revision});
            crate::platform::private_storage::atomic_write_private_json(
                &receipt,
                &json!({"digest":digest,"state":"finished","result":result}),
            )?;
            return Ok(result);
        }
        let mut input = request.adapter;
        if request.operation == "weixin_qr_check" {
            let id = request
                .attempt_id
                .as_deref()
                .context("channel_adapter.attempt_required")?;
            uuid::Uuid::parse_str(id)?;
            let path = self
                .data_dir
                .join("channel-adapter-qr")
                .join(format!("{id}.json"));
            let pending: Value = serde_json::from_slice(&fs::read(&path)?)?;
            ensure!(
                chrono::Utc::now().timestamp() - pending["createdAt"].as_i64().unwrap_or(0) <= 600,
                "channel_adapter.qr_expired"
            );
            ensure!(
                pending["revision"] == revision,
                "channel_adapter.settings_changed"
            );
            let status = rovai_channels::weixin_check_qrcode(
                pending["qrcode"]
                    .as_str()
                    .context("channel_adapter.invalid_qr")?,
            )
            .await?;
            if status.bot_token.is_none() {
                return Ok(json!({"status":status.status,"revision":revision}));
            }
            let draft = input
                .as_mut()
                .context("channel_adapter.configuration_required")?;
            ensure!(
                draft.provider == "weixin",
                "channel_adapter.provider_mismatch"
            );
            draft.token = status.bot_token;
            draft.base_url = status
                .base_url
                .context("channel_adapter.invalid_base_url")?;
        }
        if request.operation == "remove" {
            let id = request
                .id
                .as_deref()
                .context("channel_adapter.id_required")?;
            uuid::Uuid::parse_str(id)?;
            adapters.retain(|item| item.id != id);
        } else {
            let input = input.context("channel_adapter.configuration_required")?;
            uuid::Uuid::parse_str(&input.id)?;
            ensure!(
                ["telegram", "weixin"].contains(&input.provider.as_str()) && identity(&input.name),
                "channel_adapter.invalid_configuration"
            );
            ensure!(
                input.allowed_senders.len() <= 512
                    && input.bindings.len() <= 512
                    && input.allowed_senders.iter().all(|value| identity(value)),
                "channel_adapter.invalid_allowlist"
            );
            ensure!(
                input.control_senders.len() <= 512
                    && input.approval_senders.len() <= 512
                    && input
                        .control_senders
                        .iter()
                        .chain(input.approval_senders.iter())
                        .all(|sender| input.allowed_senders.contains(sender)),
                "channel_adapter.control_sender_not_allowed"
            );
            ensure!(
                !input.command_prefix.is_empty()
                    && input.command_prefix.len() <= 8
                    && !input
                        .command_prefix
                        .chars()
                        .any(|value| value.is_whitespace() || value.is_control()),
                "channel_adapter.invalid_command_prefix"
            );
            ensure!(
                [
                    "en", "zh-cn", "zh-tw", "ja", "ko", "es", "de", "fr", "pt", "ar"
                ]
                .contains(&input.message_language.as_str()),
                "channel_adapter.invalid_language"
            );
            ensure!(
                input.notification_kinds.len() <= 6
                    && input.notification_kinds.iter().all(|kind| [
                        "approval",
                        "message",
                        "round",
                        "collaboration",
                        "mission",
                        "task"
                    ]
                    .contains(&kind.as_str())),
                "channel_adapter.invalid_notification_kind"
            );
            let report_time = chrono::NaiveTime::parse_from_str(&input.daily_report_time, "%H:%M")
                .context("channel_adapter.invalid_report_time")?;
            ensure!(
                report_time.format("%H:%M").to_string() == input.daily_report_time,
                "channel_adapter.invalid_report_time"
            );
            if input.provider == "telegram" {
                ensure!(identity(&input.chat_id), "channel_adapter.chat_required")
            } else {
                let url = reqwest::Url::parse(&input.base_url)?;
                let host = url.host_str().unwrap_or_default();
                ensure!(
                    url.scheme() == "https"
                        && url.port().is_none()
                        && url.username().is_empty()
                        && url.password().is_none()
                        && url.query().is_none()
                        && (host == "weixin.qq.com" || host.ends_with(".weixin.qq.com")),
                    "channel_adapter.invalid_base_url"
                )
            }
            if input.enabled && input.provider == "weixin" {
                ensure!(
                    input.allowed_senders.len() == 1 && input.bindings.len() == 1,
                    "channel_adapter.weixin_requires_one_explicit_peer_binding"
                );
            }
            let previous = adapters.iter().find(|item| item.id == input.id);
            let token = input
                .token
                .or_else(|| {
                    previous
                        .filter(|item| item.provider == input.provider)
                        .map(|item| item.token.clone())
                })
                .unwrap_or_default();
            ensure!(
                token.len() <= 4096 && !token.contains(['\r', '\n', '\0']),
                "channel_adapter.invalid_credential"
            );
            ensure!(
                !input.enabled
                    || (!token.is_empty()
                        && !input.allowed_senders.is_empty()
                        && !input.bindings.is_empty()),
                "channel_adapter.activation_requires_binding_and_allowlist"
            );
            {
                let database = self.database.lock().await;
                for binding in &input.bindings {
                    ensure!(
                        identity(&binding.thread_id)
                            && !binding.agent_ids.is_empty()
                            && binding.agent_ids.len() <= 128,
                        "channel_adapter.invalid_binding"
                    );
                    let exists:bool=database.connection().query_row("SELECT EXISTS(SELECT 1 FROM camp WHERE id=?1 AND deletion_operation_id IS NULL)",[&binding.thread_id],|row|row.get(0))?;
                    ensure!(exists, "channel_adapter.thread_unavailable");
                    for agent in &binding.agent_ids {
                        let active:bool=database.connection().query_row("SELECT EXISTS(SELECT 1 FROM camp_member WHERE camp_id=?1 AND agent_id=?2 AND status='active')",rusqlite::params![binding.thread_id,agent],|row|row.get(0))?;
                        ensure!(active, "channel_adapter.member_not_in_thread");
                    }
                }
            }
            use sha2::Digest;
            let hash = sha2::Sha256::digest(input.id.as_bytes());
            let local_id = (u32::from_be_bytes(hash[..4].try_into()?) & 0x7fffffff).max(1) as i32;
            ensure!(
                !adapters
                    .iter()
                    .any(|item| item.id != input.id && item.local_id == local_id),
                "channel_adapter.id_collision"
            );
            adapters.retain(|item| item.id != input.id);
            adapters.push(Adapter {
                id: input.id,
                local_id,
                provider: input.provider,
                name: input.name,
                enabled: input.enabled,
                token,
                base_url: input.base_url,
                chat_id: input.chat_id,
                topic_mode: input.topic_mode,
                allowed_senders: input.allowed_senders,
                control_senders: input.control_senders,
                approval_senders: input.approval_senders,
                command_prefix: input.command_prefix,
                message_language: input.message_language,
                notification_kinds: input.notification_kinds,
                daily_report_enabled: input.daily_report_enabled,
                daily_report_time: input.daily_report_time,
                bindings: input.bindings,
            });
        }
        crate::platform::private_storage::atomic_write_private_json(
            &receipt,
            &json!({"digest":digest,"state":"started"}),
        )?;
        crate::platform::private_storage::atomic_write_private_json(
            &self.channel_adapter_file(),
            &adapters,
        )?;
        let result = json!({"revision":crate::command::canonical_json_digest(&serde_json::to_value(&adapters)?)?,"adapters":projected(&adapters)});
        crate::platform::private_storage::atomic_write_private_json(
            &receipt,
            &json!({"digest":digest,"state":"finished","result":result}),
        )?;
        emit(
            &self.output,
            "channels.adapters.changed",
            json!({"revision":result["revision"]}),
        );
        Ok(result)
    }
}
