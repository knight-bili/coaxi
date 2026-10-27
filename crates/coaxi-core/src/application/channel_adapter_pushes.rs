//! Original daily reports and public notification subscriptions, with durable sends.
use super::channel_adapter_messages::source_id;
use super::channel_adapters::{Adapter, Binding};
use super::*;
use anyhow::ensure;
use chrono::{Local, TimeZone};
use coaxi_channels::{
    ChannelMessageTarget,
    chat_channel::{
        daily_report::{DailyReportData, format_daily_report},
        i18n::{self, Lang},
    },
};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Read,
    path::Path,
};

fn read_saved(path: &Path) -> Result<Value> {
    let mut file = crate::platform::open_private_read_file(path)?;
    let mut bytes = Vec::new();
    std::io::Read::take(&mut file, 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1024 * 1024, "channel_adapter.push_too_large");
    Ok(serde_json::from_slice(&bytes)?)
}
fn target(config: &Adapter, binding: &Binding) -> ChannelMessageTarget {
    ChannelMessageTarget {
        channel_id: config.local_id,
        chat_id: Some(binding.chat_id.clone()),
        thread_key: binding.thread_key.clone(),
        thread_kind: if config.provider == "telegram" {
            Some(
                if binding.thread_key.is_some() {
                    "telegram_forum_topic"
                } else {
                    "telegram_general_topic"
                }
                .into(),
            )
        } else {
            None
        },
        provider_payload: None,
    }
}
fn prepare(
    directory: &Path,
    config: &Adapter,
    binding: &Binding,
    configuration_digest: &str,
    key: Value,
    body: String,
    kind: &str,
) -> Result<()> {
    ensure!(body.len() <= 256 * 1024, "channel_adapter.push_too_large");
    let id = source_id(&key)?;
    let file = directory.join(format!("{id}.json"));
    if file.exists() {
        return Ok(());
    }
    let data = json!({"id":id,"adapterId":config.id,"configurationDigest":configuration_digest,"threadId":binding.thread_id,"target":target(config,binding),"body":body,"sourceKind":kind});
    let digest = crate::command::canonical_json_digest(&data)?;
    crate::platform::private_storage::atomic_write_private_json(
        &file,
        &json!({"digest":digest,"state":"ready","updatedAt":chrono::Utc::now().timestamp(),"data":data}),
    )
}
fn report(
    database: &Database,
    config: &Adapter,
    binding: &Binding,
    date: chrono::NaiveDate,
    lang: Lang,
) -> Result<String> {
    let start = Local
        .from_local_datetime(
            &date
                .and_hms_opt(0, 0, 0)
                .context("channel_adapter.invalid_date")?,
        )
        .earliest()
        .context("channel_adapter.invalid_timezone")?
        .with_timezone(&chrono::Utc)
        .to_rfc3339();
    let mut by_agent = BTreeMap::<String, u32>::new();
    let mut projects = HashSet::new();
    let mut activities = Vec::new();
    let mut conversations = HashSet::new();
    for candidate in config.bindings.iter().filter(|candidate| {
        candidate.chat_id == binding.chat_id && candidate.thread_key == binding.thread_key
    }) {
        let mut statement=database.connection().prepare("SELECT c.agent_id,COUNT(*) FROM agent_run ar JOIN conversation c ON c.id=ar.conversation_id WHERE ar.camp_id=?1 AND ar.created_at>=?2 GROUP BY c.agent_id")?;
        let rows = statement
            .query_map(rusqlite::params![candidate.thread_id, start], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (agent, count) in rows {
            if !candidate.agent_ids.contains(&agent) {
                continue;
            }
            *by_agent.entry(agent).or_default() += count;
            conversations.insert(candidate.thread_id.clone());
        }
        if conversations.contains(&candidate.thread_id) {
            let(title,path):(String,String)=database.connection().query_row("SELECT COALESCE(title,''),project_path FROM camp WHERE id=?1 AND deletion_operation_id IS NULL",[&candidate.thread_id],|row|Ok((row.get(0)?,row.get(1)?)))?;
            projects.insert(path);
            if activities.len() < 10 {
                activities.push(title)
            }
        }
    }
    let mut projects = projects.into_iter().collect::<Vec<_>>();
    projects.sort();
    let data = DailyReportData {
        date: date.to_string(),
        total_conversations: conversations.len().try_into()?,
        conversations_by_agent: by_agent.into_iter().collect(),
        projects_involved: projects,
        key_activities: activities,
    };
    Ok(format_daily_report(&data, lang).to_plain_text())
}
impl Core {
    pub(super) async fn channel_adapter_pushes(&self, adapters: &[Adapter]) -> Result<Vec<Value>> {
        let _guard = super::workbench::MUTATIONS.lock().await;
        let directory = crate::platform::prepare_private_directory(
            &self.data_dir.join("channel-adapter-outbox"),
        )?;
        let cursors = crate::platform::prepare_private_directory(
            &self.data_dir.join("channel-adapter-push-cursors"),
        )?;
        let enabled = adapters
            .iter()
            .filter(|adapter| {
                adapter.enabled
                    && (adapter.daily_report_enabled || !adapter.notification_kinds.is_empty())
            })
            .collect::<Vec<_>>();
        let scheduler = cursors.join("scheduler.json");
        let offset = if scheduler.exists() {
            read_saved(&scheduler)?["next"].as_u64().unwrap_or(0) as usize
        } else {
            0
        };
        let now = Local::now();
        let file_count = fs::read_dir(&directory)?.take(20_001).count();
        ensure!(
            file_count <= 20_000,
            "channel_adapter.outbox_capacity_reached"
        );
        for index in 0..enabled.len().min(8) {
            let config = enabled[(offset + index) % enabled.len()];
            let digest = crate::command::canonical_json_digest(&serde_json::to_value(config)?)?;
            let lang = Lang::from_str_lossy(&config.message_language);
            let mut database = self.database.lock().await;
            if config.daily_report_enabled
                && now.format("%H:%M").to_string() >= config.daily_report_time
            {
                let mut targets = HashSet::new();
                for binding in &config.bindings {
                    if !targets.insert((binding.chat_id.clone(), binding.thread_key.clone())) {
                        continue;
                    }
                    let key = json!({"adapterId":config.id,"kind":"daily_report","date":now.date_naive().to_string(),"chatId":binding.chat_id,"threadKey":binding.thread_key});
                    let id = source_id(&key)?;
                    if !directory.join(format!("{id}.json")).exists() {
                        prepare(
                            &directory,
                            config,
                            binding,
                            &digest,
                            key,
                            report(&database, config, binding, now.date_naive(), lang)?,
                            "daily_report",
                        )?;
                    }
                }
            }
            if !config.notification_kinds.is_empty() {
                let cursor_file = cursors.join(format!("{}.json", config.id));
                let saved = if cursor_file.exists() {
                    read_saved(&cursor_file)?
                } else {
                    Value::Null
                };
                let mut cursor = if saved["configurationDigest"] == digest {
                    saved["cursor"].as_str().map(str::to_owned)
                } else {
                    None
                };
                for _ in 0..2 {
                    let page = match crate::notification::NotificationEpisodeService::default()
                        .inbox(
                            &mut database,
                            CURRENT_USER_ID,
                            crate::notification::NotificationEpisodeFilter::All,
                            cursor.as_deref(),
                            100,
                        ) {
                        Ok(page) => page,
                        Err(error) if cursor.is_some() => {
                            cursor = None;
                            crate::platform::private_storage::atomic_write_private_json(
                                &cursor_file,
                                &json!({"configurationDigest":digest,"cursor":null}),
                            )?;
                            tracing::debug!("channel notification cursor restarted: {error:#}");
                            break;
                        }
                        Err(error) => return Err(error),
                    };
                    for episode in page.items {
                        let kind = serde_json::to_value(episode.kind)?
                            .as_str()
                            .unwrap_or_default()
                            .to_owned();
                        if !episode.unread
                            || !config.notification_kinds.contains(&kind)
                            || episode.primary_action.single_chat.is_some()
                            || episode
                                .secondary_actions
                                .iter()
                                .any(|action| action.single_chat.is_some())
                        {
                            continue;
                        }
                        for binding in config
                            .bindings
                            .iter()
                            .filter(|binding| binding.thread_id == episode.camp.id)
                        {
                            let title = if kind == "approval" {
                                i18n::permission_request_title(lang)
                            } else if kind == "message" {
                                i18n::user_message_title(lang)
                            } else if kind == "round" {
                                i18n::turn_complete_title(lang)
                            } else {
                                i18n::channel_status_title(lang)
                            };
                            let body = format!(
                                "{title}\n{}\n{:?}\ncoaxi://thread/{}",
                                episode.camp.title, episode.primary_semantic, episode.camp.id
                            );
                            let key = json!({"adapterId":config.id,"kind":"notification","episodeId":episode.id,"attentionRevision":episode.attention_revision,"chatId":binding.chat_id,"threadKey":binding.thread_key});
                            prepare(
                                &directory,
                                config,
                                binding,
                                &digest,
                                key,
                                body,
                                "notification",
                            )?;
                        }
                    }
                    cursor = page.next_cursor;
                    crate::platform::private_storage::atomic_write_private_json(
                        &cursor_file,
                        &json!({"configurationDigest":digest,"cursor":cursor}),
                    )?;
                    if cursor.is_none() {
                        break;
                    }
                }
            }
        }
        if !enabled.is_empty() {
            crate::platform::private_storage::atomic_write_private_json(
                &scheduler,
                &json!({"next":(offset+8)%enabled.len()}),
            )?;
        }
        let mut pending = Vec::<Value>::new();
        let mut sent = Vec::new();
        for entry in fs::read_dir(&directory)?.take(20_001) {
            let path = entry?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let saved = read_saved(&path)?;
            let Some(data) = saved.get("data") else {
                continue;
            };
            let Some(config) = adapters
                .iter()
                .find(|config| config.enabled && data["adapterId"] == config.id)
            else {
                continue;
            };
            let digest = crate::command::canonical_json_digest(&serde_json::to_value(config)?)?;
            if data["configurationDigest"] != digest {
                continue;
            }
            if saved["state"] == "sent" {
                if chrono::Utc::now().timestamp() - saved["updatedAt"].as_i64().unwrap_or(i64::MAX)
                    > 30 * 86400
                {
                    sent.push(path)
                }
                continue;
            }
            let id = data["id"]
                .as_str()
                .context("channel_adapter.invalid_output_id")?;
            ensure!(
                path.file_stem().is_some_and(|name| name == id),
                "channel_adapter.output_path_mismatch"
            );
            ensure!(
                saved["digest"] == crate::command::canonical_json_digest(data)?,
                "channel_adapter.output_changed"
            );
            pending
                .push(json!({"id":id,"digest":saved["digest"],"state":saved["state"],"data":data}));
        }
        pending.sort_by_key(|value| {
            (
                value["state"] != "ready",
                value["id"].as_str().unwrap_or_default().to_owned(),
            )
        });
        pending.truncate(128);
        for path in sent {
            let _ = fs::remove_file(path);
        }
        Ok(pending)
    }
}
