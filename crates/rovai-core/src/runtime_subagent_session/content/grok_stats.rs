// Adapted from fixed Codeg aff1e6c (Apache-2.0).
// Source ranges/changes: integration/inventory/native-grok-stats-port.json.
use chrono::{DateTime, Utc};
use serde_json::Value;
#[derive(Default)]
pub(super) struct SummaryMeta {
    pub(super) title: Option<String>,
    pub(super) model: Option<String>,
    pub(super) git_branch: Option<String>,
    pub(super) created_at: Option<DateTime<Utc>>,
    pub(super) updated_at: Option<DateTime<Utc>>,
}
pub(super) fn summary_meta(v: &Value) -> SummaryMeta {
    let non_empty = |s: &str| {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_string())
    };
    SummaryMeta {
        // `generated_title` is the model-generated title; `session_summary` is
        // the fallback one-liner. Prefer the title.
        title: v
            .get("generated_title")
            .and_then(Value::as_str)
            .and_then(non_empty)
            .or_else(|| {
                v.get("session_summary")
                    .and_then(Value::as_str)
                    .and_then(non_empty)
            }),
        model: v
            .get("current_model_id")
            .and_then(Value::as_str)
            .and_then(non_empty),
        git_branch: v
            .get("head_branch")
            .and_then(Value::as_str)
            .and_then(non_empty),
        created_at: v
            .get("created_at")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339),
        updated_at: v
            .get("updated_at")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339),
    }
}

fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

pub(super) fn cache_window(cache: &Value, model: &str) -> Option<u64> {
    cache
        .get("models")?
        .get(model)?
        .get("info")?
        .get("context_window")?
        .as_u64()
        .filter(|window| *window > 0)
}

pub(super) fn config_window(raw: &str, model: &str) -> Option<u64> {
    raw.parse::<toml::Table>()
        .ok()?
        .get("model")?
        .as_table()?
        .get(model)?
        .as_table()?
        .get("context_window")?
        .as_integer()
        .filter(|window| *window > 0)
        .map(|window| window as u64)
}
