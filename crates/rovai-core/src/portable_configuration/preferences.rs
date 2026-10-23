//! 只映射源应用可迁移的明确枚举；账号、代理、终端和本机目录没有普通偏好入口。
use super::*;
pub(crate) fn selected_patch(entry: &Value, decision: &Value) -> Result<(String, Value)> {
    if let Some(value) = super::channel_preferences::selected(entry, decision)? {
        return Ok(value);
    }
    let (target, value) = if let Some(value) = policies::patch(entry)? {
        value
    } else {
        file_patch(entry)?
    };
    Ok((target.into(), value))
}
pub(crate) fn merge_at(
    root: &std::path::Path,
    target: &str,
    original: &Value,
    patch: &Value,
) -> Result<Value> {
    if super::channel_preferences::owns(target) {
        super::channel_preferences::merge(root, target, original, patch)
    } else {
        merge(target, original, patch)
    }
}

pub(crate) fn file_patch(entry: &Value) -> Result<(&'static str, Value)> {
    let key = entry["key"]
        .as_str()
        .context("configuration.preference_key_required")?;
    let raw = entry["value"]
        .as_str()
        .context("configuration.preference_value_required")?;
    if key.starts_with("rovai.") {
        return native_patch(key, raw);
    }
    match key {
        "logging.level" => {
            let value: Value = serde_json::from_str(raw)
                .map_err(|_| anyhow::anyhow!("configuration.logging_invalid"))?;
            ensure!(
                value.as_object().is_some_and(|value| value
                    .keys()
                    .all(|key| ["level", "targets"].contains(&key.as_str())))
                    && ["off", "error", "warn", "info", "debug", "trace"]
                        .contains(&value["level"].as_str().unwrap_or_default()),
                "configuration.logging_invalid"
            );
            let targets = value.get("targets").cloned().unwrap_or(json!([]));
            let rows = targets
                .as_array()
                .context("configuration.logging_targets_invalid")?;
            ensure!(rows.len() <= 1024, "configuration.logging_target_limit");
            let mut seen = std::collections::BTreeSet::new();
            let pattern = regex::Regex::new(r"^[a-zA-Z0-9_.:-]{1,256}$")?;
            for row in rows {
                let name = row["target"]
                    .as_str()
                    .context("configuration.logging_target_invalid")?;
                ensure!(
                    row.as_object().is_some_and(|row| row.len() == 2)
                        && pattern.is_match(name)
                        && seen.insert(name)
                        && ["off", "error", "warn", "info", "debug", "trace"]
                            .contains(&row["level"].as_str().unwrap_or_default()),
                    "configuration.logging_target_invalid"
                );
            }
            Ok(("logging", json!({"level":value["level"],"targets":targets})))
        }
        "appearance_mode" => {
            let mode = match raw {
                "system" => "system",
                "light" => "day",
                "dark" => "night",
                _ => anyhow::bail!("configuration.theme_invalid"),
            };
            Ok(("appearance", json!({"themePreference":mode})))
        }
        "appearance_zoom_level" => {
            let zoom = raw
                .parse::<u32>()
                .map_err(|_| anyhow::anyhow!("configuration.zoom_invalid"))?;
            ensure!((50..=300).contains(&zoom), "configuration.zoom_invalid");
            Ok(("appearance", json!({"zoomPercentage":zoom})))
        }
        "system_language_settings" => {
            let value: Value = serde_json::from_str(raw)
                .map_err(|_| anyhow::anyhow!("configuration.language_invalid"))?;
            let mode = value["mode"].as_str().unwrap_or("system");
            ensure!(
                matches!(mode, "system" | "manual"),
                "configuration.language_mode_invalid"
            );
            let language = match value["language"].as_str().unwrap_or("en") {
                "zh_cn" => "zh-CN",
                "zh_tw" => "zh-TW",
                language @ ("en" | "ja" | "ko" | "es" | "de" | "fr" | "pt" | "ar") => language,
                _ => anyhow::bail!("configuration.language_invalid"),
            };
            Ok((
                "general",
                json!({"interfaceLanguage":language,"interfaceLanguageMode":mode}),
            ))
        }
        "system_close_behavior_settings" => {
            ensure!(
                cfg!(windows),
                "configuration.close_behavior_platform_unavailable"
            );
            let value: Value = serde_json::from_str(raw)
                .map_err(|_| anyhow::anyhow!("configuration.close_behavior_invalid"))?;
            let behavior = match value["behavior"].as_str().unwrap_or("ask") {
                "ask" => "ask",
                "minimize" => "tray",
                "exit" => "exit",
                _ => anyhow::bail!("configuration.close_behavior_invalid"),
            };
            Ok(("windowClose", json!({"behavior":behavior})))
        }
        _ => anyhow::bail!("configuration.preference_needs_explicit_policy_mapping"),
    }
}
pub(crate) fn before_fields(original: &Value, patch: &Value) -> Value {
    Value::Object(
        patch
            .as_object()
            .into_iter()
            .flat_map(|map| map.keys())
            .map(|key| {
                (
                    key.clone(),
                    original.get(key).cloned().unwrap_or(Value::Null),
                )
            })
            .collect(),
    )
}

pub(crate) const NATIVE_FIELDS: &[(&str, &str)] = &[
    ("rovai.appearance.preference", "themePreference"),
    ("rovai.appearance.themePreference", "themePreference"),
    ("rovai.appearance.chatFontSize", "chatFontSize"),
    ("rovai.appearance.documentFontSize", "documentFontSize"),
    ("rovai.appearance.codeFontSize", "codeFontSize"),
    ("rovai.appearance.readingDensity", "readingDensity"),
    ("rovai.appearance.motionPreference", "motionPreference"),
    ("rovai.appearance.zoomPercentage", "zoomPercentage"),
    ("rovai.general.interfaceLanguage", "interfaceLanguage"),
    (
        "rovai.general.interfaceLanguageMode",
        "interfaceLanguageMode",
    ),
    ("rovai.general.startupLocationMode", "startupLocationMode"),
    ("rovai.general.lastSettingsSection", "lastSettingsSection"),
    (
        "rovai.general.executionConsolePlacement",
        "executionConsolePlacement",
    ),
    ("rovai.general.worldMapEnabled", "worldMapEnabled"),
    ("rovai.windowClose.behavior", "behavior"),
    ("rovai.messageLibrary.tags", "tags"),
];
pub(crate) fn merge(target: &str, original: &Value, patch: &Value) -> Result<Value> {
    let mut next = if original.is_null() {
        match target {
            "appearance" => {
                json!({"schemaVersion":2,"themePreference":"system","chatFontSize":13,"documentFontSize":15,"codeFontSize":14,"readingDensity":"standard","motionPreference":"system","zoomPercentage":100})
            }
            "general" => json!({"schemaVersion":6}),
            "windowClose" => json!({"schemaVersion":1,"behavior":"ask"}),
            "logging" => json!({"schemaVersion":1,"level":"trace","targets":[]}),
            "messageLibrary" => json!({"tags":[],"directoryAliases":[],"quickMessages":[]}),
            _ => anyhow::bail!("configuration.preference_target_invalid"),
        }
    } else {
        original.clone()
    };
    let object = next
        .as_object_mut()
        .context("configuration.preference_file_invalid")?;
    for (key, value) in patch
        .as_object()
        .context("configuration.preference_patch_invalid")?
    {
        object.insert(key.clone(), value.clone());
    }
    if target == "general" {
        crate::backup::light_config::complete_general_preferences(object, patch)?;
    }
    Ok(next)
}

fn native_patch(key: &str, raw: &str) -> Result<(&'static str, Value)> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|_| anyhow::anyhow!("configuration.fusion_preference_invalid"))?;
    let parts = key.split('.').collect::<Vec<_>>();
    ensure!(
        parts.len() == 3,
        "configuration.fusion_preference_key_invalid"
    );
    let target = match parts[1] {
        "appearance" => "appearance",
        "general" => "general",
        "windowClose" => "windowClose",
        "messageLibrary" => "messageLibrary",
        _ => anyhow::bail!("configuration.preference_target_invalid"),
    };
    let field = parts[2];
    let mut mapped = value.clone();
    let output = match (target, field) {
        ("appearance", "preference" | "themePreference") => {
            mapped = json!(match value.as_str() {
                Some("light" | "day") => "day",
                Some("dark" | "night") => "night",
                Some("system") => "system",
                _ => anyhow::bail!("configuration.theme_invalid"),
            });
            "themePreference"
        }
        ("appearance", "chatFontSize" | "documentFontSize" | "codeFontSize") => {
            ensure!(
                value
                    .as_u64()
                    .is_some_and(|number| (12..=24).contains(&number)),
                "configuration.font_size_invalid"
            );
            field
        }
        ("appearance", "zoomPercentage") => {
            ensure!(
                value
                    .as_u64()
                    .is_some_and(|number| (10..=500).contains(&number)),
                "configuration.zoom_invalid"
            );
            field
        }
        ("appearance", "readingDensity") => {
            ensure!(
                matches!(value.as_str(), Some("standard" | "relaxed")),
                "configuration.density_invalid"
            );
            field
        }
        ("appearance", "motionPreference") => {
            ensure!(
                matches!(value.as_str(), Some("system" | "reduce")),
                "configuration.motion_invalid"
            );
            field
        }
        ("general", "interfaceLanguage") => {
            ensure!(
                matches!(
                    value.as_str(),
                    Some("zh-CN" | "zh-TW" | "en" | "ja" | "ko" | "es" | "de" | "fr" | "pt" | "ar")
                ),
                "configuration.language_invalid"
            );
            field
        }
        ("general", "interfaceLanguageMode") => {
            ensure!(
                matches!(value.as_str(), Some("system" | "manual")),
                "configuration.language_mode_invalid"
            );
            field
        }
        ("general", "startupLocationMode") => {
            ensure!(
                matches!(value.as_str(), Some("last_location" | "quick_chat")),
                "configuration.startup_location_invalid"
            );
            field
        }
        ("general", "executionConsolePlacement") => {
            ensure!(
                matches!(value.as_str(), Some("right" | "inspector" | "bottom")),
                "configuration.console_placement_invalid"
            );
            field
        }
        ("general", "lastSettingsSection") => {
            ensure!(
                matches!(
                    value.as_str(),
                    Some(
                        "remote"
                            | "general"
                            | "skills"
                            | "toolbox"
                            | "mcp"
                            | "runtime"
                            | "channels"
                            | "appearance"
                            | "notifications"
                            | "monitoring"
                            | "diagnostics"
                            | "about"
                    )
                ),
                "configuration.settings_section_invalid"
            );
            field
        }
        ("general", "worldMapEnabled") => {
            ensure!(
                value.is_boolean(),
                "configuration.preference_boolean_required"
            );
            field
        }
        ("windowClose", "behavior") => {
            ensure!(
                cfg!(windows) && matches!(value.as_str(), Some("ask" | "tray" | "exit")),
                "configuration.close_behavior_platform_unavailable"
            );
            field
        }
        ("messageLibrary", "tags") => {
            let rows = value.as_array().context("configuration.tags_invalid")?;
            ensure!(rows.len() <= 512, "configuration.tag_limit");
            let mut seen = std::collections::BTreeSet::new();
            for row in rows {
                let tag = row.as_str().context("configuration.tag_invalid")?;
                ensure!(
                    !tag.trim().is_empty() && tag.len() <= 256 && seen.insert(tag),
                    "configuration.tag_invalid"
                );
            }
            field
        }
        _ => anyhow::bail!("configuration.preference_not_portable"),
    };
    let mut patch = serde_json::Map::new();
    patch.insert(output.into(), mapped);
    Ok((target, Value::Object(patch)))
}
