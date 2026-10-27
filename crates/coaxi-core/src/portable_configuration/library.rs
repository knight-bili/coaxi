//! 原快捷语、模板和智能体设置进入库；只有后续显式映射才影响新任务。
use super::*;
use rusqlite::Connection;

pub(crate) fn file_target(domain: &str) -> Option<&'static str> {
    match domain {
        "quickMessages" => Some("messageLibrary"),
        "taskTemplates" => Some("taskTemplates"),
        "agentSettings" => Some("agentSettings"),
        _ => None,
    }
}
pub(crate) fn initial(domain: &str, original: &Value) -> Result<Value> {
    if !original.is_null() {
        ensure!(original.is_object(), "configuration.library_invalid");
        return Ok(original.clone());
    }
    Ok(if domain == "quickMessages" {
        json!({"tags":[],"directoryAliases":[],"quickMessages":[]})
    } else {
        json!({"schemaVersion":1,"revision":0,"items":[]})
    })
}
fn rows_key(domain: &str) -> &'static str {
    if domain == "quickMessages" {
        "quickMessages"
    } else {
        "items"
    }
}
pub(crate) fn item<'a>(domain: &str, library: &'a Value, id: &str) -> Result<Option<&'a Value>> {
    if library.is_null() {
        return Ok(None);
    }
    let rows = library[rows_key(domain)]
        .as_array()
        .context("configuration.library_invalid")?;
    ensure!(rows.len() <= 8192, "configuration.library_capacity");
    Ok(rows.iter().find(|row| row["id"] == id))
}
pub(crate) fn revision(domain: &str, library: &Value, id: &str) -> Result<Option<i64>> {
    Ok(item(domain, library, id)?.map(|row| row["revision"].as_i64().unwrap_or(1)))
}
pub(crate) fn candidate(
    connection: &Connection,
    client: &str,
    domain: &str,
    id: &str,
    entry: &Value,
    commit: bool,
) -> Result<Value> {
    uuid::Uuid::parse_str(id)?;
    match domain {
        "quickMessages" => {
            let source: archive::CoaxiQuickMessage = serde_json::from_value(entry.clone())
                .map_err(|_| anyhow::anyhow!("configuration.quick_message_invalid"))?;
            ensure!(
                !source.title.trim().is_empty()
                    && source.title.len() <= 512
                    && source.content.len() <= 1024 * 1024,
                "configuration.quick_message_invalid"
            );
            Ok(json!({"id":id,"title":source.title,"text":source.content}))
        }
        "taskTemplates" => {
            let source: archive::CoaxiTaskTemplate = serde_json::from_value(entry.clone())
                .map_err(|_| anyhow::anyhow!("configuration.task_template_invalid"))?;
            ensure!(
                !source.name.trim().is_empty()
                    && source.name.len() <= 512
                    && source.title.len() <= 4096
                    && source.config.len() <= 1024 * 1024,
                "configuration.task_template_invalid"
            );
            let parsed = serde_json::from_str::<Value>(&source.config)
                .ok()
                .filter(Value::is_object);
            // 原 WorkTaskConfig 的模型选项和材料保持原文，跨主机不当作可直接执行配置。
            let needs = parsed.as_ref().is_none_or(|value| {
                !value["agent_type"].is_null()
                    || !value["mode_id"].is_null()
                    || value["config_values"]
                        .as_object()
                        .is_some_and(|map| !map.is_empty())
                    || value["prompt_blocks"]
                        .as_array()
                        .is_some_and(|blocks| blocks.iter().any(|block| block["type"] != "text"))
            });
            Ok(
                json!({"id":id,"name":source.name,"title":source.title,"config":source.config,"needsEnvironment":needs,"automaticExecutionAllowed":false}),
            )
        }
        "agentSettings" => {
            let source: archive::CoaxiAgentSetting = serde_json::from_value(entry.clone())
                .map_err(|_| anyhow::anyhow!("configuration.agent_setting_invalid"))?;
            let environment = source
                .env_json
                .as_deref()
                .map(|raw| crate::member_environment::parse(raw, cfg!(windows)))
                .transpose()?
                .unwrap_or_default();
            let mut refs = Vec::new();
            for (name, plain) in environment {
                let secret = if commit {
                    serde_json::to_value(secrets::import_on(
                        connection,
                        client,
                        "member_environment",
                        &plain,
                    )?)?
                } else {
                    Value::Null
                };
                refs.push(json!({"name":name,"secret":secret}));
            }
            Ok(
                json!({"id":id,"agentType":source.agent_type,"registryId":source.registry_id,"enabled":source.enabled,"sortOrder":source.sort_order,"environment":refs,"provider":source.provider.map(|provider|json!({"agentType":provider.agent_type,"name":provider.name})),"automaticExecutionAllowed":false}),
            )
        }
        _ => anyhow::bail!("configuration.library_domain_unknown"),
    }
}
pub(crate) fn merge(
    domain: &str,
    library: &mut Value,
    mut value: Value,
    action: &str,
    expected: Option<i64>,
) -> Result<()> {
    let id = value["id"]
        .as_str()
        .context("configuration.library_id_missing")?
        .to_owned();
    let rows = library[rows_key(domain)]
        .as_array_mut()
        .context("configuration.library_invalid")?;
    let position = rows.iter().position(|row| row["id"] == id);
    if action == "create" {
        ensure!(
            position.is_none() && expected.is_none(),
            "configuration.library_target_changed"
        );
    } else {
        let position = position.context("configuration.library_target_missing")?;
        ensure!(
            expected == Some(rows[position]["revision"].as_i64().unwrap_or(1)),
            "configuration.library_target_changed"
        );
    }
    if domain != "quickMessages" {
        value["revision"] = json!(expected.unwrap_or(0) + 1);
    }
    if let Some(position) = position {
        rows[position] = value
    } else {
        ensure!(
            rows.len() < if domain == "quickMessages" { 512 } else { 8192 },
            "configuration.library_capacity"
        );
        rows.push(value);
    }
    if domain != "quickMessages" {
        library["revision"] = json!(library["revision"].as_i64().unwrap_or(0) + 1);
    }
    ensure!(
        serde_json::to_vec(library)?.len() <= 1024 * 1024,
        "configuration.library_too_large"
    );
    Ok(())
}
