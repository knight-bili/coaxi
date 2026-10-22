//! Frozen User target preferences. Native model/permission settings go through
//! the existing Registry; remaining ACP selections require the real Session catalog.
use std::future::Future;

use anyhow::{Context, Result, ensure};
use rusqlite::Connection;
use serde_json::{Value, json};

use crate::{
    agent_profile::{
        AdapterKind, FrozenAgentRuntimeConfig, MemberRuntimeConfiguration, ModelSelection,
    },
    agent_runtime_adapter::AgentRuntimeAdapterRegistry,
    delegation_policy::{FrozenPolicy, Role},
    delegation_settings::RuntimeDefaults,
};

pub(crate) fn empty(defaults: &RuntimeDefaults) -> bool {
    defaults.mode_id.is_none() && defaults.config_values.is_empty()
}

fn native_mode_key(kind: AdapterKind, mode: &str) -> Result<Option<String>> {
    let choices = AgentRuntimeAdapterRegistry::default().permission_options(kind);
    let keys = choices
        .iter()
        .filter(|option| {
            option.supported
                && option.key.ends_with("mode")
                && option.choices.iter().any(|choice| choice.value == mode)
        })
        .map(|option| option.key.clone())
        .collect::<Vec<_>>();
    ensure!(keys.len() <= 1, "delegation.runtime_mode_ambiguous");
    Ok(keys.into_iter().next())
}

fn uses_acp(kind: AdapterKind) -> bool {
    !matches!(
        kind,
        AdapterKind::CodexCli
            | AdapterKind::ClaudeCodeCli
            | AdapterKind::Pi
            | AdapterKind::AntigravityApp
    )
}

pub(crate) fn configuration(
    base: &MemberRuntimeConfiguration,
    defaults: &RuntimeDefaults,
) -> Result<MemberRuntimeConfiguration> {
    let mut selected = base.clone();
    let permission_options =
        AgentRuntimeAdapterRegistry::default().permission_options(base.adapter_kind);
    let values = selected
        .permissions
        .values
        .as_object_mut()
        .context("Invalid Runtime permissions")?;
    if let Some(mode) = &defaults.mode_id {
        ensure!(!mode.trim().is_empty(), "delegation.runtime_mode_empty");
        if let Some(key) = native_mode_key(base.adapter_kind, mode)? {
            if let Some(explicit) = defaults.config_values.get(&key) {
                ensure!(explicit == mode, "delegation.runtime_mode_conflict");
            }
            values.insert(key, json!(mode));
        } else {
            ensure!(
                uses_acp(base.adapter_kind),
                "delegation.runtime_mode_unsupported"
            );
        }
    }
    if let Some(model) = defaults.config_values.get("model") {
        ensure!(!model.is_empty(), "delegation.runtime_model_empty");
        let (options, source) = match &base.model {
            ModelSelection::Explicit {
                model_id,
                options,
                dsh_source,
            } if model_id == model => (options.clone(), *dsh_source),
            _ => (json!({}), None),
        };
        selected.model = ModelSelection::Explicit {
            model_id: model.clone(),
            options,
            dsh_source: source,
        };
    }
    for (key, value) in &defaults.config_values {
        if key == "model" {
            continue;
        }
        if permission_options.iter().any(|option| option.key == *key) {
            values.insert(key.clone(), json!(value));
        } else if key == "mode" {
            ensure!(
                defaults.mode_id.as_ref().is_none_or(|mode| mode == value),
                "delegation.runtime_mode_conflict"
            );
            let key = native_mode_key(base.adapter_kind, value)?
                .context("delegation.runtime_mode_mapping_required")?;
            values.insert(key, json!(value));
        } else if !uses_acp(base.adapter_kind) {
            let ModelSelection::Explicit { options, .. } = &mut selected.model else {
                anyhow::bail!("delegation.runtime_options_require_explicit_model");
            };
            options
                .as_object_mut()
                .context("Invalid model options")?
                .insert(key.clone(), json!(value));
        }
    }
    Ok(selected)
}

/// Never reads current global defaults. Later User Runs and resume use the same
/// immutable delegation preset even after settings change or a database reopen.
pub(crate) fn for_run(connection: &Connection, run: &str) -> Result<Option<RuntimeDefaults>> {
    let (config, conversation): (String, String) = connection.query_row(
        "SELECT effective_config_json,conversation_id FROM agent_run WHERE id=?1",
        [run],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let config: Value = serde_json::from_str(&config)?;
    if !FrozenPolicy::from_config(&config)?.is_some_and(|p| p.role == Role::Child) {
        return Ok(None);
    }
    let preset: String = connection.query_row(
        "SELECT runtime_preset_json FROM delegation WHERE child_conversation_id=?1",
        [conversation],
        |r| r.get(0),
    )?;
    let defaults: RuntimeDefaults =
        serde_json::from_str(&preset).context("Invalid frozen delegation Runtime defaults")?;
    Ok((!empty(&defaults)).then_some(defaults))
}

/// Settings consumed by the native launch/model path must not be sent a second
/// time as guessed ACP option IDs. Unknown options remain pending, never discarded.
pub(crate) fn pending_acp(
    defaults: &RuntimeDefaults,
    runtime: &FrozenAgentRuntimeConfig,
) -> Result<RuntimeDefaults> {
    let descriptors =
        AgentRuntimeAdapterRegistry::default().permission_options(runtime.adapter_kind);
    let mut pending = RuntimeDefaults::default();
    if let Some(mode) = &defaults.mode_id {
        if let Some(key) = native_mode_key(runtime.adapter_kind, mode)? {
            ensure!(
                runtime.permissions.values[&key].as_str() == Some(mode),
                "delegation.runtime_frozen_mode_mismatch"
            );
        } else {
            pending.mode_id = Some(mode.clone());
        }
    }
    for (key, value) in &defaults.config_values {
        if key == "model" {
            ensure!(
                runtime.model.source == "explicit" && runtime.model.model_id == *value,
                "delegation.runtime_frozen_model_mismatch"
            );
        } else if descriptors.iter().any(|option| option.key == *key) {
            ensure!(
                runtime.permissions.values[key].as_str() == Some(value),
                "delegation.runtime_frozen_permission_mismatch"
            );
        } else if key == "mode" {
            let key = native_mode_key(runtime.adapter_kind, value)?
                .context("delegation.runtime_mode_mapping_required")?;
            ensure!(
                runtime.permissions.values[&key].as_str() == Some(value),
                "delegation.runtime_frozen_mode_mismatch"
            );
        } else {
            pending.config_values.insert(key.clone(), value.clone());
        }
    }
    Ok(pending)
}

fn option<'a>(catalog: &'a Value, id: &str, value: &str) -> Result<&'a Value> {
    let options = catalog["configOptions"]
        .as_array()
        .context("delegation.runtime_config_catalog_unavailable")?;
    let mut matches = options.iter().filter(|option| option["id"] == id);
    let selected = matches
        .next()
        .context("delegation.runtime_config_unavailable")?;
    ensure!(
        matches.next().is_none() && selected["type"] == "select",
        "delegation.runtime_config_unsupported"
    );
    let mut values = Vec::new();
    for item in selected["options"]
        .as_array()
        .context("delegation.runtime_config_choices_unavailable")?
    {
        if let Some(group) = item["options"].as_array() {
            values.extend(group.iter());
        } else {
            values.push(item);
        }
    }
    ensure!(
        values
            .iter()
            .filter(|choice| choice["value"] == value)
            .count()
            == 1,
        "delegation.runtime_config_value_unavailable"
    );
    Ok(selected)
}

pub(crate) fn has_choices(catalog: &Value, defaults: &RuntimeDefaults) -> bool {
    defaults
        .config_values
        .iter()
        .all(|(key, value)| option(catalog, key, value).is_ok())
}

/// Only the owning Host's already-known Session cache may consume this metadata.
/// These packets never acknowledge a prompt or become model input.
pub(crate) fn merge_catalog_update(catalog: &mut Value, session: &str, message: &Value) -> bool {
    if message.get("id").is_some()
        || message["method"] != "session/update"
        || message["params"]["sessionId"] != session
    {
        return false;
    }
    let update = &message["params"]["update"];
    match update["sessionUpdate"].as_str() {
        Some("config_option_update") if update["configOptions"].is_array() => {
            catalog["configOptions"] = update["configOptions"].clone();
            true
        }
        Some("current_mode_update")
            if update["currentModeId"]
                .as_str()
                .is_some_and(|id| !id.is_empty()) =>
        {
            catalog["modes"]["currentModeId"] = update["currentModeId"].clone();
            true
        }
        _ => false,
    }
}

/// Real ACP RPCs are supplied by AcpHost. Tests own only the deterministic
/// protocol boundary; no model input is part of this operation.
pub(crate) async fn apply_acp<F, Fut>(
    defaults: &RuntimeDefaults,
    session: &str,
    catalog: &Value,
    read_only: bool,
    mut rpc: F,
) -> Result<Value>
where
    F: FnMut(&'static str, Value) -> Fut,
    Fut: Future<Output = Result<Value>>,
{
    let mut catalog = catalog.clone();
    // Mode can change the available config catalog. Without a mode change the
    // complete plan can be checked immediately; otherwise check its resulting catalog.
    if defaults.mode_id.is_none() {
        for (key, value) in &defaults.config_values {
            option(&catalog, key, value)?;
        }
    }
    if let Some(mode) = &defaults.mode_id {
        let modes = &catalog["modes"];
        ensure!(
            modes["availableModes"].as_array().is_some_and(|items| items
                .iter()
                .filter(|item| item["id"] == *mode)
                .count()
                == 1),
            "delegation.runtime_mode_unavailable"
        );
        ensure!(
            !read_only || modes["currentModeId"] == *mode,
            "delegation.runtime_read_only_mode_conflict"
        );
        let result = rpc(
            "session/set_mode",
            json!({"sessionId":session,"modeId":mode}),
        )
        .await?;
        ensure!(result.is_object(), "delegation.runtime_mode_ack_invalid");
        catalog["modes"]["currentModeId"] = json!(mode);
        if let Some(options) = result.get("configOptions") {
            catalog["configOptions"] = options.clone();
        }
    }
    for (key, value) in &defaults.config_values {
        let selected = option(&catalog, key, value)?;
        ensure!(
            selected["category"] != "mode" || selected["currentValue"] == *value,
            "delegation.runtime_mode_mapping_required"
        );
        let sensitive = matches!(
            key.as_str(),
            "mode"
                | "permission_mode"
                | "approval_mode"
                | "approval_policy"
                | "sandbox_mode"
                | "sandbox"
                | "auto_approve"
                | "trust_all_tools"
                | "allow_all"
                | "execution_mode"
                | "dangerously_skip_permissions"
        ) || matches!(selected["category"].as_str(), Some("mode" | "permission"));
        ensure!(
            !read_only || !sensitive || selected["currentValue"] == *value,
            "delegation.runtime_read_only_config_conflict"
        );
        let result = rpc(
            "session/set_config_option",
            json!({"sessionId":session,"configId":key,"type":"select","value":value}),
        )
        .await?;
        ensure!(result.is_object(), "delegation.runtime_config_ack_invalid");
        if let Some(options) = result.get("configOptions") {
            catalog["configOptions"] = options.clone();
            ensure!(
                option(&catalog, key, value)?["currentValue"] == *value,
                "delegation.runtime_config_not_applied"
            );
        } else {
            anyhow::bail!("delegation.runtime_config_ack_missing");
        }
    }
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_profile::{AdapterPermissionConfig, ModelSelection};
    use std::{cell::RefCell, collections::BTreeMap};

    fn base(kind: AdapterKind) -> MemberRuntimeConfiguration {
        MemberRuntimeConfiguration {
            custom_runtime_ref: None,
            adapter_kind: kind,
            model: ModelSelection::RuntimeDefault,
            permissions: AdapterPermissionConfig {
                adapter_kind: kind,
                schema_version: 1,
                values: AgentRuntimeAdapterRegistry::default().member_permission_defaults(kind),
            },
        }
    }
    fn catalog() -> Value {
        json!({"modes":{"currentModeId":"plan","availableModes":[{"id":"plan"},{"id":"chat"}]},
            "configOptions":[{"id":"tone","type":"select","currentValue":"normal","options":[{"value":"normal"},{"value":"concise"}]},
                {"id":"thinking","type":"select","currentValue":"low","options":[{"group":"effort","options":[{"value":"low"},{"value":"high"}]}]}]})
    }
    fn preferences() -> RuntimeDefaults {
        RuntimeDefaults {
            mode_id: Some("chat".into()),
            config_values: BTreeMap::from([
                ("tone".into(), "concise".into()),
                ("thinking".into(), "high".into()),
            ]),
        }
    }

    #[tokio::test]
    async fn frozen_target_defaults_use_native_configuration_and_acknowledged_acp_choices() {
        let codex = base(AdapterKind::CodexCli);
        assert_eq!(
            configuration(&codex, &RuntimeDefaults::default()).unwrap(),
            codex
        );
        let native = RuntimeDefaults {
            mode_id: Some("read-only".into()),
            config_values: BTreeMap::from([
                ("approval_policy".into(), "on-request".into()),
                ("model".into(), "test-model".into()),
                ("reasoning_effort".into(), "high".into()),
            ]),
        };
        let selected = configuration(&codex, &native).unwrap();
        assert_eq!(selected.permissions.values["sandbox_mode"], "read-only");
        assert_eq!(selected.permissions.values["approval_policy"], "on-request");
        assert_eq!(
            codex.permissions.values["sandbox_mode"],
            "danger-full-access"
        );
        assert!(
            matches!(selected.model,ModelSelection::Explicit{ref model_id,ref options,..} if model_id=="test-model" && options["reasoning_effort"]=="high")
        );
        let conflict = RuntimeDefaults {
            mode_id: Some("read-only".into()),
            config_values: BTreeMap::from([("sandbox_mode".into(), "workspace-write".into())]),
        };
        assert!(
            configuration(&codex, &conflict)
                .unwrap_err()
                .to_string()
                .contains("mode_conflict")
        );
        for mode in ["", "not-a-native-mode"] {
            assert!(
                configuration(
                    &codex,
                    &RuntimeDefaults {
                        mode_id: Some(mode.into()),
                        config_values: BTreeMap::new()
                    }
                )
                .is_err()
            );
        }
        let unknown = RuntimeDefaults {
            mode_id: None,
            config_values: BTreeMap::from([("unmapped".into(), "v".into())]),
        };
        assert!(configuration(&codex, &unknown).is_err());
        assert!(configuration(&base(AdapterKind::CursorAgent), &unknown).is_ok());
        let cline = configuration(
            &base(AdapterKind::ClineCli),
            &RuntimeDefaults {
                mode_id: Some("plan".into()),
                config_values: BTreeMap::new(),
            },
        )
        .unwrap();
        assert_eq!(cline.permissions.values["mode"], "plan");
        let alias = configuration(
            &codex,
            &RuntimeDefaults {
                mode_id: None,
                config_values: BTreeMap::from([("mode".into(), "read-only".into())]),
            },
        )
        .unwrap();
        assert_eq!(alias.permissions.values["sandbox_mode"], "read-only");

        let calls = RefCell::new(Vec::new());
        let current = RefCell::new(catalog());
        let result = apply_acp(
            &preferences(),
            "exact-child",
            &catalog(),
            false,
            |method, params| {
                calls.borrow_mut().push((method, params.clone()));
                let reply = if method == "session/set_mode" {
                    json!({})
                } else {
                    let mut value = current.borrow_mut();
                    let option = value["configOptions"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|option| option["id"] == params["configId"])
                        .unwrap();
                    option["currentValue"] = params["value"].clone();
                    json!({"configOptions":value["configOptions"]})
                };
                std::future::ready(Ok(reply))
            },
        )
        .await
        .unwrap();
        let calls = calls.into_inner();
        assert_eq!(
            calls.iter().map(|call| call.0).collect::<Vec<_>>(),
            vec![
                "session/set_mode",
                "session/set_config_option",
                "session/set_config_option"
            ]
        );
        assert!(
            calls
                .iter()
                .all(|(_, params)| params["sessionId"] == "exact-child")
        );
        assert_eq!(calls[1].1["configId"], "thinking");
        assert_eq!(calls[2].1["configId"], "tone");
        assert_eq!(result["modes"]["currentModeId"], "chat");
        assert_eq!(
            option(&result, "tone", "concise").unwrap()["currentValue"],
            "concise"
        );
        let mut initial = catalog();
        initial["configOptions"] = json!([]);
        let mut calls = Vec::new();
        apply_acp(
            &RuntimeDefaults {
                mode_id: Some("chat".into()),
                config_values: BTreeMap::from([("tone".into(), "concise".into())]),
            },
            "child",
            &initial,
            false,
            |method, _| {
                calls.push(method);
                let mut updated = catalog();
                if method == "session/set_config_option" {
                    updated["configOptions"][0]["currentValue"] = json!("concise");
                }
                std::future::ready(Ok(json!({"configOptions":updated["configOptions"]})))
            },
        )
        .await
        .unwrap();
        assert_eq!(calls, vec!["session/set_mode", "session/set_config_option"]);
        let mut metadata = catalog();
        let packet = json!({"method":"session/update","params":{"sessionId":"child","update":{"sessionUpdate":"config_option_update","configOptions":[]}}});
        assert!(!merge_catalog_update(&mut metadata, "other-child", &packet));
        assert_eq!(metadata, catalog());
        assert!(merge_catalog_update(&mut metadata, "child", &packet));
        assert!(!has_choices(&metadata, &preferences()));
        let packet = json!({"method":"session/update","params":{"sessionId":"child","update":{"sessionUpdate":"config_option_update","configOptions":catalog()["configOptions"]}}});
        assert!(merge_catalog_update(&mut metadata, "child", &packet));
        assert!(has_choices(&metadata, &preferences()));
        assert!(merge_catalog_update(
            &mut metadata,
            "child",
            &json!({"method":"session/update","params":{"sessionId":"child","update":{"sessionUpdate":"current_mode_update","currentModeId":"chat"}}})
        ));
        assert_eq!(metadata["modes"]["currentModeId"], "chat");

        for (mut options, defaults, read_only) in [
            (
                catalog(),
                RuntimeDefaults {
                    mode_id: Some("missing".into()),
                    config_values: BTreeMap::new(),
                },
                false,
            ),
            (catalog(), unknown, false),
            (catalog(), preferences(), true),
            (
                catalog(),
                RuntimeDefaults {
                    mode_id: None,
                    config_values: BTreeMap::from([("tone".into(), "unknown".into())]),
                },
                false,
            ),
        ] {
            let mut calls = 0;
            let result = apply_acp(&defaults, "child", &options, read_only, |_, _| {
                calls += 1;
                std::future::ready(Ok(json!({})))
            })
            .await;
            assert!(result.is_err());
            assert_eq!(calls, 0);
            options["configOptions"][0]["type"] = json!("text");
            assert!(option(&options, "tone", "concise").is_err());
        }
        for failure in ["rpc-error", "missing-ack", "wrong-value", "lost-option"] {
            let mut calls = 0;
            let result = apply_acp(&preferences(), "child", &catalog(), false, |method, _| {
                calls += 1;
                let reply = if method == "session/set_mode" {
                    if failure == "lost-option" {
                        Ok(json!({"configOptions":[]}))
                    } else {
                        Ok(json!({}))
                    }
                } else {
                    match failure {
                        "rpc-error" => Err(anyhow::anyhow!("fixture RPC rejected")),
                        "missing-ack" => Ok(json!({})),
                        _ => Ok(json!({"configOptions":catalog()["configOptions"]})),
                    }
                };
                std::future::ready(reply)
            })
            .await;
            assert!(result.is_err(), "{failure}");
            assert_eq!(calls, if failure == "lost-option" { 1 } else { 2 });
        }
        let mut readonly = catalog();
        readonly["configOptions"][0]["category"] = json!("permission");
        let mut calls = 0;
        let result = apply_acp(
            &RuntimeDefaults {
                mode_id: None,
                config_values: BTreeMap::from([("tone".into(), "concise".into())]),
            },
            "child",
            &readonly,
            true,
            |_, _| {
                calls += 1;
                std::future::ready(Ok(json!({})))
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(calls, 0);
    }
}
