//! 固定 Coaxi 的 ACP 广告、精确程序身份、版本门槛；不从 UI 名称推断能力。
use super::*;

pub const CLAUDE_ACP: &str = "@agentclientprotocol/claude-agent-acp";
fn stable_version(value: &str) -> Option<(u64, u64, u64)> {
    let version = value.trim();
    let core = version.split_once('+').map_or(version, |(core, _)| core);
    if core.contains('-') {
        return None;
    }
    let parts = core.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }
    let number = |part: &str| {
        if part.is_empty()
            || !part.bytes().all(|byte| byte.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            None
        } else {
            part.parse::<u64>().ok()
        }
    };
    Some((number(parts[0])?, number(parts[1])?, number(parts[2])?))
}
pub(crate) fn steering_available(initialize: &Value) -> bool {
    initialize
        .pointer("/_meta/steering/supported")
        .and_then(Value::as_bool)
        == Some(true)
        && initialize
            .pointer("/agentInfo/name")
            .and_then(Value::as_str)
            == Some(CLAUDE_ACP)
        && initialize
            .pointer("/agentInfo/version")
            .and_then(Value::as_str)
            .and_then(stable_version)
            .is_some_and(|version| version >= (0, 65, 0))
}
pub(crate) fn exact_fork_available(provider: &str, initialize: &Value) -> bool {
    let name = initialize
        .pointer("/agentInfo/name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let Some(version) = initialize
        .pointer("/agentInfo/version")
        .and_then(Value::as_str)
        .and_then(stable_version)
    else {
        return false;
    };
    match provider {
        "claude" => name == CLAUDE_ACP && version >= (0, 75, 1),
        "codex" => {
            matches!(name, "codex-acp" | "@zed-industries/codex-acp") && version >= (1, 8, 0)
        }
        "deepseek" => {
            matches!(name, "deepseek-acp" | "@deepseek-ai/deepseek-acp") && version >= (0, 8, 0)
        }
        _ => false,
    }
}
pub(crate) fn provider_matches(provider: &str, initialize: &Value) -> bool {
    let name = initialize
        .pointer("/agentInfo/name")
        .and_then(Value::as_str)
        .unwrap_or("");
    match provider {
        "claude" => name == CLAUDE_ACP,
        "codex" => matches!(name, "codex-acp" | "@zed-industries/codex-acp"),
        "deepseek" => matches!(name, "deepseek-acp" | "@deepseek-ai/deepseek-acp"),
        _ => false,
    }
}
pub(crate) fn native_outcome(value: &Value) -> Result<&str> {
    match value["outcome"].as_str() {
        Some(value @ ("injected" | "promptRequired" | "startedNewTurn")) => Ok(value),
        _ => anyhow::bail!("native_lifecycle.native_receipt_unknown"),
    }
}
pub(crate) fn steer_params(session: &str, blocks: &[Value]) -> Value {
    json!({"sessionId":session,"prompt":blocks,"_meta":{"steering":{"idleBehavior":"promptRequired"}}})
}
