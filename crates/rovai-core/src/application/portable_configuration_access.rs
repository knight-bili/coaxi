use super::*;
use crate::portable_configuration::{self as configuration, files, members, migration, providers};

impl Core {
    pub(super) async fn portable_configuration_access(
        self: &Arc<Self>,
        method: &str,
        mut value: Value,
        client: &crate::draft_client::DraftClient,
    ) -> Result<Value> {
        if matches!(
            method,
            "configuration.migration.archives" | "configuration.migration.targets"
        ) {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                after: Option<String>,
                limit: u32,
                domain: Option<String>,
            }
            anyhow::ensure!(
                serde_json::to_vec(&value)?.len() <= 4096,
                "configuration.catalog_request_limit"
            );
            let input: Input = serde_json::from_value(value)?;
            let database = self.database.lock().await;
            return if method == "configuration.migration.archives" {
                anyhow::ensure!(
                    input.domain.is_none(),
                    "configuration.catalog_domain_invalid"
                );
                configuration::catalog::archives(
                    database.connection(),
                    client.id(),
                    input.after.as_deref(),
                    input.limit,
                )
            } else {
                configuration::catalog::targets(
                    database.connection(),
                    &self.data_dir,
                    input
                        .domain
                        .as_deref()
                        .context("configuration.domain_required")?,
                    input.after.as_deref(),
                    input.limit,
                )
            };
        }
        configuration::validate_operation(method, &value)?;
        if matches!(
            method,
            "configuration.migration.preview"
                | "configuration.migration.undoPreview"
                | "configuration.migration.receipt"
        ) {
            let _lock = workbench::MUTATIONS.lock().await;
            let database = self.database.lock().await;
            return match method {
                "configuration.migration.preview" => {
                    migration::preview(database.connection(), &self.data_dir, client.id(), &value)
                }
                "configuration.migration.undoPreview" => migration::undo_preview(
                    database.connection(),
                    &self.data_dir,
                    client.id(),
                    &value,
                ),
                _ => migration::receipt(
                    database.connection(),
                    client.id(),
                    value["commandId"]
                        .as_str()
                        .context("configuration.command_required")?,
                ),
            };
        }
        if method == "configuration.providers.list" {
            let database = self.database.lock().await;
            return providers::list(
                database.connection(),
                value["after"].as_str(),
                value["limit"]
                    .as_u64()
                    .context("configuration.limit_invalid")? as u32,
            );
        }
        if method == "configuration.memberConnection.get" {
            let database = self.database.lock().await;
            return members::get(
                database.connection(),
                value["agentId"]
                    .as_str()
                    .context("configuration.member_required")?,
            );
        }
        let command = value["commandId"]
            .as_str()
            .context("configuration.command_required")?
            .to_owned();
        value
            .as_object_mut()
            .context("configuration.request_invalid")?
            .remove("commandId");
        let mut database = self.database.lock().await;
        let execution = match method {
            "configuration.migration.confirm" => {
                let mut input: migration::Confirm = serde_json::from_value(value)?;
                input.client = client.clone();
                input.core_instance = self.interaction_tools.core_instance().into();
                migration::confirm(
                    &mut database,
                    &self.data_dir,
                    &user_command_envelope(command.clone(), input),
                )?
            }
            "configuration.migration.undoConfirm" => {
                let mut input: migration::Confirm = serde_json::from_value(value)?;
                input.client = client.clone();
                input.core_instance = self.interaction_tools.core_instance().into();
                migration::undo_confirm(
                    &mut database,
                    &self.data_dir,
                    &user_command_envelope(command.clone(), migration::UndoConfirm(input)),
                )?
            }
            "configuration.providers.upsert" => {
                let mut input: providers::Upsert = serde_json::from_value(value)?;
                input.client = client.clone();
                providers::upsert(
                    &mut database,
                    &user_command_envelope(command.clone(), input),
                )?
            }
            "configuration.providers.remove" => {
                let mut input: providers::Remove = serde_json::from_value(value)?;
                input.client = client.clone();
                providers::remove(
                    &mut database,
                    &user_command_envelope(command.clone(), input),
                )?
            }
            "configuration.memberConnection.set" => {
                let mut input: members::SetConnection = serde_json::from_value(value)?;
                input.client = client.clone();
                members::set(
                    &mut database,
                    &user_command_envelope(command.clone(), input),
                )?
            }
            _ => anyhow::bail!("configuration.operation_unknown"),
        };
        let is_migration = matches!(
            method,
            "configuration.migration.confirm" | "configuration.migration.undoConfirm"
        );
        let result = if is_migration {
            migration::receipt(database.connection(), client.id(), &command)?
        } else {
            serde_json::to_value(execution.result)?
        };
        drop(database);
        if is_migration && result["state"] == "applied" {
            let core = self.clone();
            tokio::spawn(async move {
                core.portable_configuration_drain(&command).await;
            });
        }
        emit(
            &self.output,
            "portableConfiguration.changed",
            json!({"operation":method}),
        );
        Ok(result)
    }
    async fn portable_configuration_drain(&self, command: &str) {
        let _lock = workbench::MUTATIONS.lock().await;
        let database = self.database.lock().await;
        let result = files::drain(database.connection(), &self.data_dir, command);
        drop(database);
        emit(
            &self.output,
            "portableConfiguration.filesChanged",
            json!({"commandId":command,"readReceipt":true,"requiresAttention":result.as_ref().map_or(true,|items|items.iter().any(|item|item["state"]!="applied"))}),
        );
    }
    pub(super) async fn portable_configuration_recover_outbox(&self) {
        let mut after = String::new();
        loop {
            let commands = {
                let database = self.database.lock().await;
                (|| -> Result<Vec<String>> {
                    let mut query=database.connection().prepare("SELECT DISTINCT command_id FROM configuration_file_outbox WHERE state='pending' AND command_id>?1 ORDER BY command_id LIMIT 100")?;
                    Ok(query
                        .query_map([&after], |row| row.get::<_, String>(0))?
                        .collect::<rusqlite::Result<Vec<_>>>()?)
                })()
            };
            let Ok(commands) = commands else { return };
            let Some(last) = commands.last() else { return };
            after = last.clone();
            for command in commands {
                self.portable_configuration_drain(&command).await;
            }
            tokio::task::yield_now().await;
        }
    }
}
