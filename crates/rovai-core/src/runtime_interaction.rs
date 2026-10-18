//! Native User interactions normalized for Core persistence, never for auto-approval.
//! Adapted from Codeg aff1e6c278453ed8d0b96a5f664edcdca621aa3f,
//! src-tauri/src/acp/{question,cursor_ext,plan_approval}.rs (Apache-2.0).
//! Retained license: integration/licenses/codeg/LICENSE.
//! Wire conversion is pure; storage owns frozen requests and User-only decisions.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;

use crate::agent_profile::AdapterKind;

pub mod storage;

pub const MAX_QUESTIONS: usize = 4;
pub const MAX_OPTIONS: usize = 4;
pub const MAX_TEXT_CHARS: usize = 4096;
pub const MAX_HEADER_CHARS: usize = 12;
pub const MAX_PLAN_CHARS: usize = 262_144;
pub const MAX_FEEDBACK_CHARS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    CursorAcp,
    GrokAcp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
    pub native_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Question {
    /// Deterministic within this immutable request; never an authorization key.
    pub id: String,
    pub native_id: Option<String>,
    pub question: String,
    pub header: String,
    pub multi_select: bool,
    pub options: Vec<QuestionOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RequestBody {
    Question {
        questions: Vec<Question>,
    },
    PlanApproval {
        #[serde(rename = "planMarkdown")]
        plan_markdown: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub schema_version: u32,
    pub protocol: Protocol,
    pub tool_call_id: Option<String>,
    pub body: RequestBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuestionAnswer {
    pub question_id: String,
    /// Known option labels and at most one free-text Other answer.
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanDecision {
    Approve,
    RequestChanges,
    Abandon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Answer {
    Question {
        answers: Vec<QuestionAnswer>,
        declined: bool,
    },
    PlanApproval {
        decision: PlanDecision,
        feedback: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub native_response: Value,
    /// Grok discards feedback on keep_planning. The coordinator must persist
    /// and deliver these exact User words separately, never invent a prompt.
    pub followup_feedback: Option<String>,
}

fn bounded(value: &str, max: usize) -> Result<()> {
    ensure!(value.chars().count() <= max, "interaction.field_too_large");
    Ok(())
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    let value = value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .context("interaction.invalid_text")?;
    bounded(value, MAX_TEXT_CHARS)?;
    Ok(value)
}
fn optional_text<'a>(value: &'a Value, field: &str, max: usize) -> Result<Option<&'a str>> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            bounded(s, max)?;
            Ok(Some(s))
        }
        _ => anyhow::bail!("interaction.invalid_text"),
    }
}
fn native_id(value: &Value, field: &str, fallback: String) -> Result<String> {
    // Preserve wire identity byte-for-byte, including meaningful whitespace.
    Ok(optional_text(value, field, MAX_TEXT_CHARS)?
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or(fallback))
}
fn cursor_outcome(outcome: &str) -> Value {
    json!({"outcome":{"outcome":outcome}})
}

impl Request {
    pub fn kind(&self) -> &'static str {
        match self.body {
            RequestBody::Question { .. } => "question",
            RequestBody::PlanApproval { .. } => "plan_approval",
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "interaction.schema_unsupported");
        if let Some(id) = &self.tool_call_id {
            bounded(id, MAX_TEXT_CHARS)?;
        }
        match &self.body {
            RequestBody::PlanApproval { plan_markdown } => bounded(plan_markdown, MAX_PLAN_CHARS),
            RequestBody::Question { questions } => {
                ensure!(
                    (1..=MAX_QUESTIONS).contains(&questions.len()),
                    "interaction.invalid_question_count"
                );
                let mut native_ids = HashSet::new();
                let mut prompts = HashSet::new();
                for (index, q) in questions.iter().enumerate() {
                    ensure!(
                        q.id == format!("q{index}"),
                        "interaction.question_identity_changed"
                    );
                    ensure!(
                        !q.question.is_empty()
                            && q.question.trim() == q.question
                            && !q.header.is_empty()
                            && q.header.trim() == q.header,
                        "interaction.invalid_text"
                    );
                    bounded(&q.question, MAX_TEXT_CHARS)?;
                    bounded(&q.header, MAX_HEADER_CHARS)?;
                    ensure!(
                        (2..=MAX_OPTIONS).contains(&q.options.len()),
                        "interaction.invalid_option_count"
                    );
                    match self.protocol {
                        Protocol::CursorAcp => {
                            let id = q
                                .native_id
                                .as_deref()
                                .context("interaction.native_id_missing")?;
                            bounded(id, MAX_TEXT_CHARS)?;
                            ensure!(
                                !id.trim().is_empty() && native_ids.insert(id),
                                "interaction.duplicate_native_id"
                            );
                        }
                        Protocol::GrokAcp => ensure!(
                            q.native_id.is_none() && prompts.insert(&q.question),
                            "interaction.ambiguous_question"
                        ),
                    }
                    let mut labels = HashSet::new();
                    let mut option_ids = HashSet::new();
                    for option in &q.options {
                        ensure!(
                            !option.label.is_empty()
                                && option.label.trim() == option.label
                                && labels.insert(option.label.as_str()),
                            "interaction.duplicate_option"
                        );
                        bounded(&option.label, MAX_TEXT_CHARS)?;
                        bounded(&option.description, MAX_TEXT_CHARS)?;
                        match self.protocol {
                            Protocol::CursorAcp => {
                                let id = option
                                    .native_id
                                    .as_deref()
                                    .context("interaction.native_id_missing")?;
                                bounded(id, MAX_TEXT_CHARS)?;
                                ensure!(
                                    !id.trim().is_empty() && option_ids.insert(id),
                                    "interaction.duplicate_native_id"
                                );
                            }
                            Protocol::GrokAcp => ensure!(
                                option.native_id.is_none(),
                                "interaction.native_id_unexpected"
                            ),
                        }
                    }
                }
                Ok(())
            }
        }
    }

    pub fn from_acp(adapter: AdapterKind, method: &str, params: &Value) -> Result<Option<Self>> {
        let (protocol, question) = match (adapter, method) {
            (AdapterKind::CursorAgent, "cursor/ask_question") => (Protocol::CursorAcp, true),
            (AdapterKind::CursorAgent, "cursor/create_plan") => (Protocol::CursorAcp, false),
            (AdapterKind::GrokBuild, "_x.ai/ask_user_question") => (Protocol::GrokAcp, true),
            (AdapterKind::GrokBuild, "_x.ai/exit_plan_mode") => (Protocol::GrokAcp, false),
            _ => return Ok(None),
        };
        ensure!(params.is_object(), "interaction.invalid_request");
        let tool_call_id = optional_text(params, "toolCallId", MAX_TEXT_CHARS)?.map(str::to_owned);
        let body = if question {
            let array = params
                .get("questions")
                .and_then(Value::as_array)
                .context("interaction.questions_missing")?;
            ensure!(
                (1..=MAX_QUESTIONS).contains(&array.len()),
                "interaction.invalid_question_count"
            );
            let mut questions = Vec::with_capacity(array.len());
            for (index, q) in array.iter().enumerate() {
                let cursor = protocol == Protocol::CursorAcp;
                let prompt = text(q, if cursor { "prompt" } else { "question" })?;
                let header = if cursor {
                    optional_text(params, "title", MAX_TEXT_CHARS)?
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or(prompt)
                } else {
                    prompt
                };
                let field = if cursor {
                    "allowMultiple"
                } else {
                    "multiSelect"
                };
                let multi_select = match q.get(field) {
                    None => false,
                    Some(Value::Bool(b)) => *b,
                    _ => anyhow::bail!("interaction.invalid_multiple"),
                };
                let options = q
                    .get("options")
                    .and_then(Value::as_array)
                    .context("interaction.options_missing")?;
                ensure!(
                    (2..=MAX_OPTIONS).contains(&options.len()),
                    "interaction.invalid_option_count"
                );
                let options = options
                    .iter()
                    .map(|option| {
                        let label = text(option, "label")?.to_owned();
                        Ok(QuestionOption {
                            native_id: if cursor {
                                Some(native_id(option, "id", label.clone())?)
                            } else {
                                None
                            },
                            label,
                            description: if cursor {
                                String::new()
                            } else {
                                optional_text(option, "description", MAX_TEXT_CHARS)?
                                    .unwrap_or("")
                                    .trim()
                                    .to_owned()
                            },
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                questions.push(Question {
                    id: format!("q{index}"),
                    native_id: if cursor {
                        Some(native_id(q, "id", format!("q{index}"))?)
                    } else {
                        None
                    },
                    question: prompt.into(),
                    header: header
                        .trim()
                        .chars()
                        .take(MAX_HEADER_CHARS)
                        .collect::<String>()
                        .trim()
                        .to_owned(),
                    multi_select,
                    options,
                });
            }
            RequestBody::Question { questions }
        } else {
            let field = if protocol == Protocol::CursorAcp {
                "plan"
            } else {
                "planContent"
            };
            let mut plan = optional_text(params, field, MAX_PLAN_CHARS)?.unwrap_or("");
            if plan.is_empty() && protocol == Protocol::CursorAcp {
                plan = optional_text(params, "overview", MAX_PLAN_CHARS)?.unwrap_or("");
            }
            RequestBody::PlanApproval {
                plan_markdown: plan.into(),
            }
        };
        let request = Self {
            schema_version: 1,
            protocol,
            tool_call_id,
            body,
        };
        request.validate()?;
        Ok(Some(request))
    }

    pub fn reply(&self, answer: &Answer) -> Result<Reply> {
        self.validate()?;
        let mut followup_feedback = None;
        let native_response = match (&self.body, answer) {
            (RequestBody::Question { questions }, Answer::Question { answers, declined }) => {
                if *declined {
                    ensure!(answers.is_empty(), "interaction.declined_with_answers");
                    return Ok(Reply {
                        native_response: self.interrupted_response(),
                        followup_feedback: None,
                    });
                }
                ensure!(
                    answers.len() == questions.len(),
                    "interaction.incomplete_answer"
                );
                let mut answer_ids = HashSet::new();
                for answer in answers {
                    ensure!(
                        answer_ids.insert(&answer.question_id)
                            && questions.iter().any(|q| q.id == answer.question_id),
                        "interaction.answer_scope_mismatch"
                    );
                }
                let mut cursor_answers = Vec::new();
                let mut grok_answers = serde_json::Map::new();
                let mut all_mapped = true;
                let mut prose = Vec::new();
                for q in questions {
                    let answer = answers
                        .iter()
                        .find(|a| a.question_id == q.id)
                        .context("interaction.incomplete_answer")?;
                    ensure!(
                        !answer.labels.is_empty()
                            && answer.labels.len() <= q.options.len() + 1
                            && (q.multi_select || answer.labels.len() == 1),
                        "interaction.invalid_selection_count"
                    );
                    let mut seen = HashSet::new();
                    let mut other = 0;
                    let mut selected = Vec::new();
                    let mut native_ids = Vec::new();
                    for label in &answer.labels {
                        let label = label.trim();
                        bounded(label, MAX_TEXT_CHARS)?;
                        ensure!(
                            !label.is_empty() && seen.insert(label),
                            "interaction.invalid_answer"
                        );
                        selected.push(label.to_owned());
                        if let Some(option) = q.options.iter().find(|o| o.label == label) {
                            if let Some(id) = &option.native_id {
                                native_ids.push(id.clone());
                            }
                        } else {
                            other += 1;
                            all_mapped = false;
                        }
                    }
                    ensure!(other <= 1, "interaction.multiple_other_answers");
                    cursor_answers
                        .push(json!({"questionId":q.native_id,"selectedOptionIds":native_ids}));
                    prose.push(format!("{}: {}", q.question, selected.join(", ")));
                    grok_answers.insert(
                        q.question.clone(),
                        if q.multi_select {
                            json!(selected)
                        } else {
                            json!(selected[0])
                        },
                    );
                }
                match self.protocol {
                    Protocol::GrokAcp => {
                        json!({"outcome":"accepted","answers":grok_answers,"partial_answers":{}})
                    }
                    Protocol::CursorAcp if all_mapped => {
                        json!({"outcome":{"outcome":"answered","answers":cursor_answers}})
                    }
                    Protocol::CursorAcp => {
                        let reason =
                            format!("the user answered in free text — {}", prose.join(" | "));
                        // Do not silently drop the typed half of a mixed answer.
                        bounded(&reason, MAX_TEXT_CHARS)?;
                        json!({"outcome":{"outcome":"skipped","reason":reason}})
                    }
                }
            }
            (RequestBody::PlanApproval { .. }, Answer::PlanApproval { decision, feedback }) => {
                let feedback = feedback.as_deref().unwrap_or("").trim();
                bounded(feedback, MAX_FEEDBACK_CHARS)?;
                ensure!(
                    *decision != PlanDecision::RequestChanges || !feedback.is_empty(),
                    "interaction.feedback_required"
                );
                ensure!(
                    *decision != PlanDecision::Abandon || feedback.is_empty(),
                    "interaction.feedback_not_applicable"
                );
                match self.protocol {
                    Protocol::CursorAcp => match decision {
                        PlanDecision::Approve => {
                            ensure!(feedback.is_empty(), "interaction.feedback_not_applicable");
                            cursor_outcome("accepted")
                        }
                        PlanDecision::RequestChanges => {
                            json!({"outcome":{"outcome":"rejected","reason":feedback}})
                        }
                        PlanDecision::Abandon => cursor_outcome("cancelled"),
                    },
                    Protocol::GrokAcp => {
                        let outcome = match decision {
                            PlanDecision::Approve => "approved",
                            PlanDecision::RequestChanges => {
                                followup_feedback = Some(feedback.to_owned());
                                "keep_planning"
                            }
                            PlanDecision::Abandon => "abandoned",
                        };
                        json!({"outcome":outcome,"feedback":feedback})
                    }
                }
            }
            _ => anyhow::bail!("interaction.answer_kind_mismatch"),
        };
        Ok(Reply {
            native_response,
            followup_feedback,
        })
    }

    /// A lost request never means approval or a user-authored answer.
    pub fn interrupted_response(&self) -> Value {
        match (&self.body, self.protocol) {
            (RequestBody::Question { .. }, Protocol::GrokAcp) => {
                json!({"outcome":"skip_interview"})
            }
            (RequestBody::Question { .. }, Protocol::CursorAcp) => cursor_outcome("skipped"),
            (RequestBody::PlanApproval { .. }, Protocol::GrokAcp) => {
                json!({"outcome":"keep_planning","feedback":""})
            }
            (RequestBody::PlanApproval { .. }, Protocol::CursorAcp) => cursor_outcome("cancelled"),
        }
    }
}

#[cfg(test)]
mod tests;
