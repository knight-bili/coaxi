//! 客户确认的浏览器与电脑工具接线 r1。目录关闭时不改变既有模型输入。
//! 工具定义来自固定 Codeg 源，版权与 Apache 2.0 NOTICE 由移植清单保留。
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::LazyLock,
};
pub mod broker;
pub mod output;
pub mod renderers;
pub mod runtime_support;
pub mod stdio;

pub const SERVER_NAME: &str = "rovai-interaction";
pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;
pub const MAX_RESULT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
static CATALOG: LazyLock<Vec<Value>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("catalog.json")).expect("frozen interaction catalog")
});
static LEGACY_CATALOG: LazyLock<Vec<Value>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("catalog-v1.json")).expect("legacy interaction catalog")
});
pub const DIAGNOSTIC_NAME: &str = "coaxi_diagnose_capabilities";
static DIAGNOSTIC: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("diagnostic-tool.json")).expect("confirmed diagnostic tool")
});
static SCHEMAS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("schemas.json")).expect("frozen interaction shapes")
});

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Features {
    pub browser: bool,
    pub browser_eval: bool,
    pub computer: bool,
    pub computer_launch: bool,
    pub computer_clipboard: bool,
}
impl Features {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.browser_eval || self.browser,
            "interaction.browser_parent_required"
        );
        ensure!(
            !(self.computer_launch || self.computer_clipboard) || self.computer,
            "interaction.computer_parent_required"
        );
        Ok(())
    }
    pub fn any(self) -> bool {
        self.browser || self.computer
    }
    pub fn permits(self, name: &str) -> bool {
        match name {
            "browser_eval" => self.browser && self.browser_eval,
            "computer_launch_app" | "computer_set_window_frame" => {
                self.computer && self.computer_launch
            }
            "computer_clipboard_read" | "computer_clipboard_write" => {
                self.computer && self.computer_clipboard
            }
            _ if name.starts_with("browser_") => {
                self.browser && CATALOG.iter().any(|tool| tool["name"] == name)
            }
            _ if name.starts_with("computer_") => {
                self.computer && CATALOG.iter().any(|tool| tool["name"] == name)
            }
            _ => false,
        }
    }
    pub fn catalog(self) -> Vec<Value> {
        LEGACY_CATALOG
            .iter()
            .filter(|tool| tool["name"].as_str().is_some_and(|name| self.permits(name)))
            .cloned()
            .collect()
    }
    pub fn catalog_with_diagnostics(self) -> Vec<Value> {
        let mut tools: Vec<Value> = CATALOG
            .iter()
            .filter(|tool| tool["name"].as_str().is_some_and(|name| self.permits(name)))
            .cloned()
            .collect();
        tools.push(DIAGNOSTIC.clone());
        tools
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemberPolicy {
    pub agent_id: String,
    pub features: Features,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    pub schema_version: u32,
    pub revision: u64,
    pub members: Vec<MemberPolicy>,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            members: Vec::new(),
        }
    }
}
impl Policy {
    pub fn features(&self, agent: &str) -> Features {
        self.members
            .iter()
            .find(|member| member.agent_id == agent)
            .map(|member| member.features)
            .unwrap_or_default()
    }
    pub fn validate(&self) -> Result<()> {
        validate_shape("policy", &serde_json::to_value(self)?)?;
        let mut ids = BTreeSet::new();
        for member in &self.members {
            ensure!(ids.insert(&member.agent_id), "interaction.duplicate_member");
            member.features.validate()?;
        }
        Ok(())
    }
}

pub struct PolicyStore {
    path: PathBuf,
    gate: std::sync::Mutex<()>,
}
impl PolicyStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("interaction-tools/policy-v1.json"),
            gate: Default::default(),
        }
    }
    fn read_unlocked(&self) -> Result<Policy> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Policy::default());
            }
            Err(error) => return Err(error.into()),
        };
        ensure!(
            bytes.len() <= MAX_REQUEST_BYTES,
            "interaction.policy_too_large"
        );
        let value: Policy = serde_json::from_slice(&bytes)?;
        value.validate()?;
        Ok(value)
    }
    pub fn read(&self) -> Result<Policy> {
        let _gate = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("interaction.policy_lock"))?;
        self.read_unlocked()
    }
    pub fn replace(&self, expected: u64, value: Policy) -> Result<Policy> {
        value.validate()?;
        ensure!(
            expected < MAX_SAFE_INTEGER && value.revision == expected + 1,
            "interaction.invalid_revision"
        );
        let _gate = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("interaction.policy_lock"))?;
        let current = self.read_unlocked()?;
        if current == value {
            return Ok(current);
        }
        ensure!(current.revision == expected, "interaction.policy_changed");
        crate::platform::private_storage::prepare_private_directory(
            self.path.parent().context("interaction.policy_path")?,
        )?;
        crate::platform::private_storage::atomic_write_private_json(
            &self.path,
            &serde_json::to_value(&value)?,
        )?;
        Ok(value)
    }
}

pub fn validate_arguments(features: Features, name: &str, arguments: &Value) -> Result<()> {
    ensure!(features.permits(name), "interaction_disabled");
    ensure!(
        serde_json::to_vec(arguments)?.len() <= MAX_REQUEST_BYTES,
        "invalid_arguments"
    );
    let tool = CATALOG
        .iter()
        .find(|tool| tool["name"] == name)
        .context("invalid_arguments")?;
    validate_value(&tool["inputSchema"], arguments, 0).context("invalid_arguments")
}
pub fn validate_shape(name: &str, value: &Value) -> Result<()> {
    let schema = SCHEMAS.get(name).context("interaction.unknown_shape")?;
    validate_value(schema, value, 0)
}

// 仅执行本 r1 冻结合同实际使用的约束；不接受外部 Schema 或动态引用。
pub(super) fn validate_value(schema: &Value, value: &Value, depth: usize) -> Result<()> {
    ensure!(depth <= 48, "interaction.shape_depth");
    if let Some(values) = schema["anyOf"].as_array() {
        ensure!(
            values
                .iter()
                .any(|item| validate_value(item, value, depth + 1).is_ok()),
            "interaction.shape_any"
        );
    }
    if let Some(values) = schema["oneOf"].as_array() {
        ensure!(
            values
                .iter()
                .filter(|item| validate_value(item, value, depth + 1).is_ok())
                .count()
                == 1,
            "interaction.shape_one"
        );
    }
    if let Some(expected) = schema.get("const") {
        ensure!(expected == value, "interaction.shape_constant");
    }
    if let Some(values) = schema["enum"].as_array() {
        ensure!(values.contains(value), "interaction.shape_enum");
    }
    if let Some(kind) = schema["type"].as_str() {
        ensure!(
            match kind {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                "integer" => value.is_i64() || value.is_u64(),
                "number" => value.is_number(),
                "boolean" => value.is_boolean(),
                "null" => value.is_null(),
                _ => false,
            },
            "interaction.shape_type"
        );
    }
    if let Some(object) = value.as_object() {
        if let Some(required) = schema["required"].as_array() {
            for name in required {
                ensure!(
                    object.contains_key(name.as_str().context("interaction.shape_key")?),
                    "interaction.shape_required"
                );
            }
        }
        let properties = schema["properties"].as_object();
        for (key, item) in object {
            if let Some(property) = properties.and_then(|properties| properties.get(key)) {
                validate_value(property, item, depth + 1)?;
            } else {
                ensure!(
                    schema["additionalProperties"] != false,
                    "interaction.shape_unknown_field"
                );
            }
        }
    }
    if let Some(items) = value.as_array() {
        if let Some(limit) = schema["minItems"].as_u64() {
            ensure!(items.len() as u64 >= limit, "interaction.shape_min_items");
        }
        if let Some(limit) = schema["maxItems"].as_u64() {
            ensure!(items.len() as u64 <= limit, "interaction.shape_max_items");
        }
        if schema["uniqueItems"] == true {
            let mut seen = BTreeSet::new();
            for item in items {
                ensure!(
                    seen.insert(serde_json::to_string(item)?),
                    "interaction.shape_duplicate"
                );
            }
        }
        if let Some(item_schema) = schema.get("items") {
            for item in items {
                validate_value(item_schema, item, depth + 1)?;
            }
        }
    }
    if let Some(text) = value.as_str() {
        if let Some(limit) = schema["minLength"].as_u64() {
            ensure!(
                text.chars().count() as u64 >= limit,
                "interaction.shape_min_length"
            );
        }
        if let Some(limit) = schema["maxLength"].as_u64() {
            ensure!(
                text.chars().count() as u64 <= limit,
                "interaction.shape_max_length"
            );
        }
        if let Some(pattern) = schema["pattern"].as_str() {
            ensure!(
                regex::Regex::new(pattern)?.is_match(text),
                "interaction.shape_pattern"
            );
        }
        match schema["format"].as_str() {
            Some("uuid") => {
                uuid::Uuid::parse_str(text)?;
            }
            Some("date-time") => {
                chrono::DateTime::parse_from_rfc3339(text)?;
            }
            _ => {}
        }
    }
    if let Some(number) = value.as_f64() {
        if let Some(limit) = schema["minimum"].as_f64() {
            ensure!(number >= limit, "interaction.shape_minimum");
        }
        if let Some(limit) = schema["maximum"].as_f64() {
            ensure!(number <= limit, "interaction.shape_maximum");
        }
    }
    Ok(())
}

pub fn failure(status: &str, code: &str) -> Value {
    let result = json!({"content":[{"type":"text","text":"Interaction tool is unavailable for this run."}],"isError":true,"structuredContent":{"schemaVersion":1,"status":status,"code":code}});
    debug_assert!(validate_shape("failureResult", &result).is_ok());
    result
}

pub fn mutates(name: &str) -> bool {
    !matches!(
        name,
        "browser_list_tabs"
            | "browser_snapshot"
            | "browser_console_messages"
            | "browser_screenshot"
            | "computer_list_apps"
            | "computer_list_windows"
            | "computer_screenshot"
            | "computer_snapshot"
            | "computer_verify"
            | "computer_clipboard_read"
    )
}
pub fn needs_local_approval(name: &str) -> bool {
    matches!(
        name,
        "browser_eval"
            | "computer_launch_app"
            | "computer_clipboard_read"
            | "computer_clipboard_write"
    )
}
