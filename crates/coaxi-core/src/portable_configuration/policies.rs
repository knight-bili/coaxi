//! 行为设置只有逐项明确选择才进入原策略；使用原修订与更新原语，不触碰既有 Run。
use super::*;
use rusqlite::{Connection, Transaction};

pub(crate) const CONVERSATION_FIELDS: &[(&str, &str, &str)] = &[
    ("question.enabled", "question", "允许向你提问"),
    (
        "session_info.enabled",
        "sessionInfo",
        "允许读取明确授权的旧讨论",
    ),
    (
        "chat_authoring.automations_enabled",
        "automations",
        "允许从聊天创建自动化",
    ),
    (
        "chat_authoring.work_tasks_enabled",
        "workTasks",
        "允许从聊天创建使命",
    ),
    ("feedback.enabled", "feedbackGate", "文字与卡片反馈总开关"),
];
pub(crate) fn owns(target: &str) -> bool {
    matches!(target, "delegationPolicy" | "conversationToolsPolicy")
}
pub(crate) fn patch(entry: &Value) -> Result<Option<(&'static str, Value)>> {
    let key = entry["key"]
        .as_str()
        .context("configuration.preference_key_required")?;
    let raw = entry["value"]
        .as_str()
        .context("configuration.preference_value_required")?;
    if let Some((_, field, _)) = CONVERSATION_FIELDS
        .iter()
        .find(|(source, _, _)| *source == key)
    {
        let value: Value = serde_json::from_str(raw)
            .map_err(|_| anyhow::anyhow!("configuration.conversation_preference_invalid"))?;
        ensure!(
            value.is_boolean(),
            "configuration.conversation_preference_boolean_required"
        );
        return Ok(Some(("conversationToolsPolicy", json!({*field:value}))));
    }
    let field = match key {
        "delegation.enabled" => "enabled",
        "delegation.depth_limit" => "depthLimit",
        "delegation.agent_defaults" => "perRuntimeDefaults",
        "delegation.completed_cache_max_mb" => "completedCacheMaxMb",
        _ => return Ok(None),
    };
    let value: Value = serde_json::from_str(raw)
        .map_err(|_| anyhow::anyhow!("configuration.delegation_preference_invalid"))?;
    let candidate = json!({field:value});
    let typed: crate::delegation_settings::SettingsPatch =
        serde_json::from_value(candidate.clone())
            .map_err(|_| anyhow::anyhow!("configuration.delegation_preference_invalid"))?;
    ensure!(
        typed
            .depth_limit
            .is_none_or(|depth| (1..=8).contains(&depth))
            && typed
                .completed_cache_max_mb
                .is_none_or(|budget| (0..=9007199254740991).contains(&budget)),
        "configuration.delegation_preference_out_of_range"
    );
    bounded(&candidate, 0)?;
    Ok(Some(("delegationPolicy", candidate)))
}
pub(crate) fn basis(connection: &Connection, target: &str) -> Result<Value> {
    ensure!(owns(target), "configuration.policy_target_invalid");
    if target == "conversationToolsPolicy" {
        let value = crate::conversation_tools::policy::get(connection)?;
        return Ok(
            json!({"version":value.revision,"value":{"question":value.global_flags.question,"sessionInfo":value.global_flags.session_info,"automations":value.global_flags.automations,"workTasks":value.global_flags.work_tasks,"feedbackGate":value.feedback_gate}}),
        );
    }
    let value = crate::delegation_settings::read(connection)?;
    Ok(json!({"version":value.version,"value":value}))
}
pub(crate) fn merge(target: &str, original: &Value, patch: &Value) -> Result<Value> {
    let mut value = original.clone();
    let object = value
        .as_object_mut()
        .context("configuration.policy_invalid")?;
    for (key, field) in patch
        .as_object()
        .context("configuration.policy_patch_invalid")?
    {
        ensure!(key != "version", "configuration.policy_version_immutable");
        if target == "conversationToolsPolicy" {
            ensure!(
                CONVERSATION_FIELDS.iter().any(|(_, name, _)| *name == key)
                    && (field.is_boolean() || key == "feedbackGate" && field.is_null()),
                "configuration.conversation_policy_field_forbidden"
            );
        }
        object.insert(key.clone(), field.clone());
    }
    if target == "conversationToolsPolicy" {
        return Ok(value);
    }
    let _: crate::delegation_settings::DelegationSettings = serde_json::from_value(value.clone())?;
    Ok(value)
}
pub(crate) fn apply(
    tx: &Transaction<'_>,
    target: &str,
    before: &Value,
    after: &Value,
) -> Result<Value> {
    ensure!(
        owns(target) && basis(tx, target)? == *before,
        "configuration.policy_changed"
    );
    if target == "conversationToolsPolicy" {
        let after = merge(target, &before["value"], after)?;
        let current = crate::conversation_tools::policy::get(tx)?;
        let boolean = |name: &str| -> Result<bool> {
            after[name]
                .as_bool()
                .context("configuration.conversation_policy_invalid")
        };
        let flags = crate::conversation_tools::FlagsV1 {
            question: boolean("question")?,
            session_info: boolean("sessionInfo")?,
            automations: boolean("automations")?,
            work_tasks: boolean("workTasks")?,
        };
        let feedback = if after["feedbackGate"].is_null() {
            None
        } else {
            Some(boolean("feedbackGate")?)
        };
        crate::conversation_tools::policy::replace_on(
            tx,
            current.revision,
            flags,
            feedback,
            current.members,
        )?;
        return basis(tx, target);
    }
    let mut patch = after.clone();
    patch
        .as_object_mut()
        .context("configuration.policy_invalid")?
        .remove("version");
    let input = crate::delegation_settings::UpdateSettingsParams {
        expected_version: before["version"]
            .as_i64()
            .context("configuration.policy_version_missing")?,
        patch: serde_json::from_value(patch)?,
    };
    crate::delegation_settings::update(tx, &input)?;
    basis(tx, target)
}
