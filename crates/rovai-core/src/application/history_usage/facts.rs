use chrono::{DateTime,Utc};
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UsageFact {
    pub turn_key: String,
    pub occurred_at: DateTime<Utc>,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub duration_ms: i64,
}

impl UsageFact {
    pub fn total_tokens(&self) -> i64 {
        self.input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.cache_creation_tokens)
            .saturating_add(self.cache_read_tokens)
    }
}

/// A fact joined to the conversation dimensions the dashboard filters on.
#[derive(Debug, Clone)]
pub struct UsageFactRow {
    pub conversation_id: i32,
    pub folder_id: i32,
    pub agent_type: String,
    pub model: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub total_tokens: i64,
    pub duration_ms: i64,
}

