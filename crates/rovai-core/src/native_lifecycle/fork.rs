// Adapted from Codeg aff1e6c src-tauri/src/acp/fork.rs (Apache-2.0).
// 只移植定位与协议 shape；执行、权限与回执仍由 Core 拥有。
use super::*;
use crate::runtime_subagent_session::content::model::{ContentBlock, MessageTurn, TurnRole};
use sha2::{Digest, Sha256};

fn text(turn: &MessageTurn) -> String {
    turn.blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}
pub(crate) fn fingerprint(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}
fn original_id(id: &str) -> &str {
    match id.rsplit_once(":segment:") {
        Some((base, segment))
            if !base.is_empty()
                && !segment.is_empty()
                && segment.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            base
        }
        _ => id,
    }
}
pub(crate) fn resolve(
    provider: &str,
    turns: &[MessageTurn],
    selected: &str,
) -> Result<NativeForkPointV1> {
    ensure!(
        matches!(provider, "codex" | "claude" | "deepseek"),
        "native_lifecycle.exact_fork_not_supported"
    );
    let index = turns
        .iter()
        .position(|turn| turn.id == selected && matches!(turn.role, TurnRole::Assistant))
        .context("native_lifecycle.assistant_fork_point_required")?;
    let turn = &turns[index];
    let content = text(turn);
    let fingerprint = (!content.trim().is_empty()).then(|| fingerprint(&content));
    let native = turn
        .agent_message_id
        .as_deref()
        .filter(|id| !id.trim().is_empty());
    ensure!(
        fingerprint.is_some() || (provider != "codex" && native.is_some()),
        "native_lifecycle.fork_point_unavailable"
    );
    let occurrence = fingerprint
        .as_ref()
        .map(|expected| -> Result<u32> {
            let count = turns[..index]
                .iter()
                .filter(|turn| {
                    matches!(turn.role, TurnRole::Assistant)
                        && self::fingerprint(&text(turn)) == *expected
                })
                .count();
            u32::try_from(count + 1).context("native_lifecycle.fork_occurrence_exhausted")
        })
        .transpose()?;
    let id = if provider == "codex" {
        selected
    } else {
        native.unwrap_or(selected)
    };
    let point = NativeForkPointV1 {
        selected_turn_id: selected.into(),
        message_id: original_id(id).into(),
        message_fingerprint: fingerprint,
        message_occurrence: occurrence,
    };
    validate_shape("NativeForkPointV1", &serde_json::to_value(&point)?)?;
    Ok(point)
}
pub(crate) fn request(
    session: &str,
    cwd: &str,
    point: Option<&NativeForkPointV1>,
) -> Result<Value> {
    ensure!(
        !session.is_empty() && session.len() <= 4096 && !session.contains('\0'),
        "native_lifecycle.native_session_invalid"
    );
    let mut params = json!({"sessionId":session,"cwd":cwd});
    if let Some(point) = point {
        validate_shape("NativeForkPointV1", &serde_json::to_value(point)?)?;
        let mut fork = json!({"version":1,"messageId":original_id(&point.message_id)});
        if let Some(fingerprint) = &point.message_fingerprint {
            fork["messageFingerprint"] = json!(fingerprint);
        }
        if let Some(occurrence) = point.message_occurrence {
            fork["messageOccurrence"] = json!(occurrence);
        }
        params["_meta"] = json!({"jetbrains":{"air":{"fork":fork}}});
    }
    Ok(params)
}
