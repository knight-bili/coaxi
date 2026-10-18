// Adapted from fixed Codeg aff1e6c (Apache-2.0).
// Source ranges and changes: integration/inventory/native-codex-content-port.json.
//! A service error envelope, read down to the message it carries.
//!
//! codex keeps a provider's error body verbatim as a turn error's message. For
//! some failures, an HTTP 400 for a model the account cannot use for one, that
//! body is a JSON envelope:
//!
//! ```text
//! {"type":"error","status":400,"error":{"type":"invalid_request_error","message":"…"}}
//! ```
//!
//! codex-acp 2.1.1 (#572) reads the message out of it for the `sessionFailure`
//! title, but still forwards the raw JSON as a failed compaction's `error`,
//! which the compaction card shows (live: a compaction whose model request
//! gets that 400 reaches codeg as a readable banner beside a card reading the
//! JSON). [`readable_service_error_message`] is the adapter's
//! `readableServiceErrorMessage`, rule for rule and with JavaScript's idea of
//! a blank message, so the card reads the sentence the banner does. One input
//! it cannot follow: a body escaping a lone surrogate (`"\ud800"`), which
//! `JSON.parse` takes and serde_json refuses, so that body stays as written.

use serde_json::Value;

use super::js_text::is_js_blank;

/// The error types the adapter recognizes. Any other type is not the envelope.
const SERVICE_ERROR_TYPES: &[&str] = &[
    "invalid_request_error",
    "server_error",
    "rate_limit_error",
    "insufficient_quota",
    "authentication_error",
    "permission_error",
    "not_found_error",
    "conflict_error",
    "overloaded_error",
];

/// The message of a known service error envelope, or `None` for any other
/// text, which then stays as it is.
///
/// Strict on purpose, like the adapter: a key beyond `type`/`status`/`error`
/// (or beyond `type`/`message`/`code`/`param` inside `error`), a status that is
/// not a whole number from 400 to 599, an unknown error type, a non-string
/// `code`/`param`, or a blank message all mean the text is something else, and
/// custom JSON a provider wrote is never cut down to one field of it.
pub(crate) fn readable_service_error_message(text: &str) -> Option<String> {
    let Ok(Value::Object(envelope)) = serde_json::from_str::<Value>(text) else {
        return None;
    };
    if envelope
        .keys()
        .any(|key| !matches!(key.as_str(), "type" | "status" | "error"))
        || envelope.get("type").and_then(Value::as_str) != Some("error")
    {
        return None;
    }
    // A JSON number, whole (`400.0` is), in the error range.
    let status = envelope.get("status")?.as_f64()?;
    if status.fract() != 0.0 || !(400.0..=599.0).contains(&status) {
        return None;
    }
    let Value::Object(error) = envelope.get("error")? else {
        return None;
    };
    if error
        .keys()
        .any(|key| !matches!(key.as_str(), "type" | "message" | "code" | "param"))
        || !error
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| SERVICE_ERROR_TYPES.contains(&kind))
        || ["code", "param"].iter().any(|key| {
            error
                .get(*key)
                .is_some_and(|v| !v.is_null() && !v.is_string())
        })
    {
        return None;
    }
    let message = error.get("message")?.as_str()?;
    (!is_js_blank(message)).then(|| message.to_string())
}
