use serde_json::{Value, json};

pub(crate) const GROK_OBSERVATION_EVENT: &str = "runtime.subagent.observed";

// 通知没有工具编号。这里只声明原生 Session 的子代理观察，不猜测发起它的 Run 或工具。
pub(crate) fn grok_observation(method: &str, params: &Value) -> Option<Value> {
    if !matches!(
        method,
        "_x.ai/session_notification" | "_x.ai/session/update"
    ) || !crate::runtime::is_root_output(params)
        || !crate::runtime::is_root_output(&params["update"])
    {
        return None;
    }
    let update = &params["update"];
    let (phase, status) = match update["sessionUpdate"].as_str()? {
        "subagent_spawned" => ("spawned", "running"),
        "subagent_finished" => (
            "finished",
            match update["status"].as_str() {
                Some("completed") => "completed",
                Some("failed" | "errored") => "failed",
                Some("cancelled" | "canceled") => "cancelled",
                Some("interrupted") => "interrupted",
                _ => "unknown",
            },
        ),
        _ => return None,
    };
    projection(
        &params["sessionId"],
        &update["subagent_id"],
        update.get("child_session_id"),
        phase,
        status,
    )
}

// 再投影一次闭合字段，避免任何调用者把原始provider包写入用户Evidence。
pub(crate) fn public_grok_observation(payload: &Value) -> Option<Value> {
    let observation = &payload["nativeSubagents"];
    if observation["schemaVersion"] != 1
        || observation["adapterKind"] != "grok-build"
        || observation["source"] != "grok-acp.subagent_notification"
        || observation["relationship"] != "runtime_reported_session_child"
    {
        return None;
    }
    let phase = observation["phase"].as_str()?;
    let child = &observation["children"];
    if child.as_array()?.len() != 1 || !matches!(phase, "spawned" | "finished") {
        return None;
    }
    let status = child[0]["status"].as_str()?;
    if !matches!(
        status,
        "running" | "completed" | "failed" | "cancelled" | "interrupted" | "unknown"
    ) || (phase == "spawned" && status != "running")
        || (phase == "finished" && status == "running")
    {
        return None;
    }
    projection(
        &observation["parentNativeSessionId"],
        &child[0]["nativeSubagentId"],
        child[0].get("nativeSessionId"),
        phase,
        status,
    )
}

fn projection(
    parent: &Value,
    subagent: &Value,
    session: Option<&Value>,
    phase: &str,
    status: &str,
) -> Option<Value> {
    let parent = identity(parent)?;
    let subagent = identity(subagent)?;
    let session = match session {
        None | Some(Value::Null) => None,
        Some(value) => Some(identity(value)?),
    };
    if subagent == parent || session == Some(parent) {
        return None;
    }
    let mut child = json!({"nativeSubagentId": subagent, "status": status});
    if let Some(session) = session {
        child["nativeSessionId"] = json!(session);
    }
    let observation = json!({
        "schemaVersion": 1,
        "adapterKind": "grok-build",
        "source": "grok-acp.subagent_notification",
        "relationship": "runtime_reported_session_child",
        "originatingRunKnown": false,
        "parentNativeSessionId": parent,
        "phase": phase,
        "children": [child],
        "receiverCount": 1,
        "omittedReceiverCount": 0,
    });
    let digest = crate::command::canonical_json_digest(&observation).ok()?;
    Some(json!({"eventId": format!("grok-subagent-{digest}"), "nativeSubagents": observation}))
}

fn identity(value: &Value) -> Option<&str> {
    value.as_str().filter(|id| {
        !id.is_empty()
            && id.len() <= 256
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    })
}
