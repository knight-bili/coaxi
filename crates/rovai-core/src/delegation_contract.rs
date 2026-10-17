//! Approved I04 delegation r1 wire inputs. Parent authority is supplied by Core, never by JSON.
use serde::{Deserialize, Deserializer, Serialize};
use std::path::Path;

pub const RUNTIME_TARGETS: &[&str] = &[
    "antigravity",
    "claude_code",
    "cline",
    "code_buddy",
    "codex",
    "command_code",
    "copilot",
    "cursor",
    "deepseek",
    "gemini",
    "grok",
    "hermes",
    "kimi_code",
    "kiro",
    "open_claw",
    "open_code",
    "pi",
    "qoder",
    "qwen_code",
    "trae_cn",
    "zcode",
];

// A present optional property must contain its value; null is not omission in the r1 schemas.
pub(crate) fn present<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NonEmpty(String);

impl NonEmpty {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for NonEmpty {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err("delegation value must not be empty")
        } else {
            Ok(Self(value))
        }
    }
}
impl From<NonEmpty> for String {
    fn from(value: NonEmpty) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RuntimeTarget(String);

impl RuntimeTarget {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for RuntimeTarget {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let custom = value.strip_prefix("custom:").is_some_and(|id| {
            !id.is_empty()
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        });
        if RUNTIME_TARGETS.contains(&value.as_str()) || custom {
            Ok(Self(value))
        } else {
            Err("unknown delegation runtime target")
        }
    }
}
impl From<RuntimeTarget> for String {
    fn from(value: RuntimeTarget) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorkingDirectory(String);
impl WorkingDirectory {
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}
impl TryFrom<String> for WorkingDirectory {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        // Existence, actual execution root and permission are checked by the Core admission path.
        if Path::new(&value).is_absolute() && !value.contains('\0') {
            Ok(Self(value))
        } else {
            Err("delegation working_dir must be an absolute path")
        }
    }
}
impl From<WorkingDirectory> for String {
    fn from(value: WorkingDirectory) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InterruptionReason(String);
impl InterruptionReason {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for InterruptionReason {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !value.is_empty() && value.len() <= 2048 {
            Ok(Self(value))
        } else {
            Err("delegation reason must contain 1..2048 UTF-8 bytes")
        }
    }
}
impl From<InterruptionReason> for String {
    fn from(value: InterruptionReason) -> Self {
        value.0
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartParams {
    pub agent_type: RuntimeTarget,
    pub task: NonEmpty,
    #[serde(default, deserialize_with = "present")]
    pub working_dir: Option<WorkingDirectory>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusParams {
    #[serde(deserialize_with = "nonempty_ids")]
    pub task_ids: Vec<NonEmpty>,
    #[serde(default, deserialize_with = "present")]
    pub wait_ms: Option<u64>,
    #[serde(default, deserialize_with = "present")]
    pub after_cursor: Option<NonEmpty>,
}
impl StatusParams {
    /// None is a snapshot, zero is unbounded; positive waits are capped, never converted to zero.
    pub fn bounded_wait_ms(&self) -> Option<u64> {
        self.wait_ms.map(|wait| wait.min(60_000))
    }
}
fn nonempty_ids<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<NonEmpty>, D::Error> {
    let ids = Vec::<NonEmpty>::deserialize(d)?;
    if ids.is_empty() {
        Err(serde::de::Error::custom(
            "delegation task_ids must not be empty",
        ))
    } else {
        Ok(ids)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelParams {
    pub task_id: NonEmpty,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeParams {
    pub task_id: NonEmpty,
    #[serde(default, deserialize_with = "present")]
    pub reason: Option<InterruptionReason>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadParams {
    pub task_id: NonEmpty,
    #[serde(default, deserialize_with = "present")]
    pub cursor: Option<NonEmpty>,
    #[serde(default, deserialize_with = "read_limit")]
    pub limit_bytes: Option<u32>,
}
impl ReadParams {
    pub fn page_bytes(&self) -> u32 {
        self.limit_bytes.unwrap_or(16 * 1024)
    }
}
fn read_limit<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    bounded_limit(d, 65_536)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryParams {
    pub task_id: NonEmpty,
    #[serde(default, deserialize_with = "present")]
    pub before_cursor: Option<NonEmpty>,
    #[serde(default, deserialize_with = "history_limit")]
    pub limit: Option<u32>,
}
impl HistoryParams {
    pub fn page_size(&self) -> u32 {
        self.limit.unwrap_or(20)
    }
}
fn history_limit<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    bounded_limit(d, 100)
}
fn bounded_limit<'de, D: Deserializer<'de>>(d: D, max: u32) -> Result<Option<u32>, D::Error> {
    let limit = u32::deserialize(d)?;
    if (1..=max).contains(&limit) {
        Ok(Some(limit))
    } else {
        Err(serde::de::Error::custom(
            "delegation page limit out of range",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn approved_wire_inputs_reject_authority_injection_and_preserve_status_order() {
        fn accepts<T: serde::de::DeserializeOwned>(value: Value) -> bool {
            serde_json::from_value::<T>(value).is_ok()
        }
        let mut valid = json!({"agent_type":"codex", "task":"检查汉字材料"});
        assert!(accepts::<StartParams>(valid.clone()));
        let whole_task = "保留完整任务".repeat(65536);
        let full: StartParams = serde_json::from_value(json!({"agent_type":"codex","task":whole_task,"working_dir":std::env::temp_dir().to_string_lossy()})).unwrap();
        assert_eq!(full.task.as_str(), whole_task);
        for (key, value) in [
            ("parentId", json!("other")),
            ("ownerAgentId", json!("other")),
            ("epoch", json!(2)),
            ("working_dir", Value::Null),
            ("working_dir", json!("relative")),
            ("agent_type", json!("custom:")),
            ("agent_type", json!("custom:../../escape")),
            ("agent_type", json!("unknown")),
            ("task", json!("")),
        ] {
            let mut invalid = valid.clone();
            invalid[key] = value;
            assert!(!accepts::<StartParams>(invalid), "{key}");
        }
        for runtime in RUNTIME_TARGETS
            .iter()
            .copied()
            .chain(["custom:fixture.runtime-1"])
        {
            valid["agent_type"] = json!(runtime);
            assert!(accepts::<StartParams>(valid.clone()), "{runtime}");
        }
        let status: StatusParams =
            serde_json::from_value(json!({"task_ids":["b","a","b"],"wait_ms":90000})).unwrap();
        assert_eq!(
            status
                .task_ids
                .iter()
                .map(NonEmpty::as_str)
                .collect::<Vec<_>>(),
            ["b", "a", "b"]
        );
        assert_eq!(status.bounded_wait_ms(), Some(60000));
        for (wait, expected) in [(None, None), (Some(0), Some(0)), (Some(1), Some(1))] {
            let mut input = json!({"task_ids":["a"]});
            if let Some(wait) = wait {
                input["wait_ms"] = json!(wait);
            }
            assert_eq!(
                serde_json::from_value::<StatusParams>(input)
                    .unwrap()
                    .bounded_wait_ms(),
                expected
            );
        }
        for input in [
            json!({"task_ids":[]}),
            json!({"task_ids":[""]}),
            json!({"task_ids":["a"],"wait_ms":-1}),
            json!({"task_ids":["a"],"after_cursor":null}),
        ] {
            assert!(!accepts::<StatusParams>(input));
        }
        assert!(accepts::<ResumeParams>(
            json!({"task_id":"a","reason":"中".repeat(682)+"ab"})
        ));
        for input in [
            json!({"task_id":"a","task":"repeat original"}),
            json!({"task_id":"a","reason":"中".repeat(683)}),
            json!({"task_id":"a","reason":null}),
        ] {
            assert!(!accepts::<ResumeParams>(input));
        }
        assert!(accepts::<CancelParams>(json!({"task_id":"a"})));
        assert!(!accepts::<CancelParams>(
            json!({"task_id":"a","parentId":"other"})
        ));
        assert_eq!(
            serde_json::from_value::<ReadParams>(json!({"task_id":"a"}))
                .unwrap()
                .page_bytes(),
            16384
        );
        assert_eq!(
            serde_json::from_value::<HistoryParams>(json!({"task_id":"a"}))
                .unwrap()
                .page_size(),
            20
        );
        for limit in [0, 65537] {
            assert!(!accepts::<ReadParams>(
                json!({"task_id":"a","limit_bytes":limit})
            ));
        }
        for limit in [0, 101] {
            assert!(!accepts::<HistoryParams>(
                json!({"task_id":"a","limit":limit})
            ));
        }
        for limit in [1, 65536] {
            assert_eq!(
                serde_json::from_value::<ReadParams>(json!({"task_id":"a","limit_bytes":limit}))
                    .unwrap()
                    .page_bytes(),
                limit
            );
        }
        for limit in [1, 100] {
            assert_eq!(
                serde_json::from_value::<HistoryParams>(json!({"task_id":"a","limit":limit}))
                    .unwrap()
                    .page_size(),
                limit
            );
        }
        assert!(!accepts::<ReadParams>(json!({"task_id":"a","cursor":null})));
        assert!(!accepts::<HistoryParams>(
            json!({"task_id":"a","limit":null})
        ));
    }
}
