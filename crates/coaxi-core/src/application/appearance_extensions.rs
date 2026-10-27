//! Supplemental source appearance settings; existing host day/night, language,
//! reading sizes and reduced-motion contracts remain the authority for those.
use super::*;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomTheme {
    light: BTreeMap<String, String>,
    dark: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Appearance {
    preset: String,
    custom_theme: CustomTheme,
    custom_css: String,
    custom_enabled: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            preset: "neutral".into(),
            custom_theme: Default::default(),
            custom_css: String::new(),
            custom_enabled: false,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stored {
    revision: String,
    value: Appearance,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    operation: String,
    #[serde(default)]
    expected_revision: Option<String>,
    #[serde(default)]
    value: Option<Appearance>,
}
const TOKENS: &[&str] = &[
    "primary",
    "primary-foreground",
    "background",
    "foreground",
    "accent",
    "border",
    "sidebar",
    "radius",
    "card",
    "card-foreground",
    "popover",
    "popover-foreground",
    "secondary",
    "secondary-foreground",
    "muted",
    "muted-foreground",
    "accent-foreground",
    "destructive",
    "input",
    "ring",
    "chart-1",
    "chart-2",
    "chart-3",
    "chart-4",
    "chart-5",
    "sidebar-foreground",
    "sidebar-primary",
    "sidebar-primary-foreground",
    "sidebar-accent",
    "sidebar-accent-foreground",
    "sidebar-border",
    "sidebar-ring",
];
impl Core {
    pub(super) async fn appearance_extensions(&self, value: Value) -> Result<Value> {
        let request: Request = serde_json::from_value(value)?;
        let path = self.data_dir.join("appearance-extensions-v1.json");
        let _lock = workbench::MUTATIONS.lock().await;
        let previous: Option<Stored> = if path.exists() {
            Some(serde_json::from_slice(&std::fs::read(&path)?)?)
        } else {
            None
        };
        if request.operation == "get" {
            return Ok(match previous {
                Some(value) => serde_json::to_value(value)?,
                None => json!({"revision":null,"value":Appearance::default()}),
            });
        }
        ensure!(
            request.operation == "set"
                && previous.as_ref().map(|previous| previous.revision.as_str())
                    == request.expected_revision.as_deref(),
            "appearance.settings_changed"
        );
        let next = request.value.context("appearance.value_required")?;
        ensure!(
            matches!(
                next.preset.as_str(),
                "coaxi"
                    | "neutral"
                    | "zinc"
                    | "slate"
                    | "stone"
                    | "gray"
                    | "red"
                    | "rose"
                    | "orange"
                    | "green"
                    | "blue"
                    | "yellow"
                    | "violet"
            ) && next.custom_css.len() <= 64 * 1024,
            "appearance.invalid_preset_or_css"
        );
        for side in [&next.custom_theme.light, &next.custom_theme.dark] {
            for (key, value) in side {
                ensure!(
                    TOKENS.contains(&key.as_str())
                        && !value.is_empty()
                        && value.len() <= 64
                        && value
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric()
                                || "#%.,()/ +-\t\r\n".contains(character)),
                    "appearance.invalid_theme_token"
                );
            }
        }
        let stored = Stored {
            revision: uuid::Uuid::new_v4().to_string(),
            value: next,
        };
        crate::platform::private_storage::atomic_write_private_json(&path, &stored)?;
        Ok(serde_json::to_value(stored)?)
    }
}
