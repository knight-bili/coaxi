use super::super::source::NativeDocuments;
use super::*;
use chrono::TimeZone;
use serde_json::{Value, json};
use std::{fs, path::Path, sync::atomic::AtomicBool};

fn write(home: &Path, path: &str, bytes: impl AsRef<[u8]>) {
    let target = home.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, bytes).unwrap();
}
fn row(update: Value) -> Value {
    json!({"timestamp":1700000000,"params":{"update":update}})
}
fn lines(rows: &[Value]) -> Vec<u8> {
    rows.iter()
        .flat_map(|value| {
            let mut bytes = serde_json::to_vec(value).unwrap();
            bytes.push(b'\n');
            bytes
        })
        .collect()
}

// Extends the existing filesystem owner; no second database/process fixture.
pub(crate) fn assert_content_projection(home: &Path, base: &ObservedNativeSessionTarget) {
    let mut target = base.clone();
    target.adapter_kind = "grok-build".into();
    target.recorded_home = Some(home.into());
    write(
        home,
        "sessions/group-a/parent/updates.jsonl",
        lines(&[row(
            json!({"sessionUpdate":"subagent_spawned","subagent_id":"different-short","child_session_id":"child","parent_session_id":"parent"}),
        )]),
    );
    let long = format!("开始{}结束", "甲乙丙".repeat(50000));
    let mut rows = vec![
        json!({"type":"startup","api_key":"PRIVATE_STARTUP_MUST_NOT_RENDER"}),
        row(
            json!({"sessionUpdate":"user_message_chunk","_meta":{"promptIndex":1},"content":{"type":"text","text":"用户原问题"}}),
        ),
        row(
            json!({"sessionUpdate":"user_message_chunk","_meta":{"promptIndex":1},"content":{"type":"image","data":"aQ==","mimeType":"image/png","uri":"image.png"}}),
        ),
        row(json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"甲"}})),
        row(
            json!({"sessionUpdate":"user_message_chunk","_meta":{"hideFromScrollback":true},"content":{"type":"text","text":"HIDDEN_REMINDER_MUST_NOT_RENDER"}}),
        ),
        row(json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"乙"}})),
        row(
            json!({"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"思考A"}}),
        ),
        row(
            json!({"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"思考B"}}),
        ),
        json!({"timestamp":1700000000,"params":{"_meta":{"updateParams":{"status":"Pending"},"totalTokens":9999,"turnStartMs":1000,"agentTimestampMs":2000},"update":{"sessionUpdate":"tool_call","toolCallId":"read","title":"read_file","rawInput":{"path":"a.ts"}}}}),
        row(
            json!({"sessionUpdate":"tool_call_update","toolCallId":"read","status":"completed","content":[{"type":"content","content":{"type":"text","text":"1→one\n2→two"}}]}),
        ),
        row(
            json!({"sessionUpdate":"tool_call","toolCallId":"mcp","title":"use_tool","rawInput":{"tool_name":"delegate_to_agent","tool_input":{"task":long}}}),
        ),
        row(
            json!({"sessionUpdate":"tool_call_update","toolCallId":"mcp","content":[{"type":"content","content":{"type":"text","text":"first cumulative"}}]}),
        ),
        row(
            json!({"sessionUpdate":"tool_call_update","toolCallId":"mcp","status":"completed","rawOutput":{"type":"TaskOutput","Result":{"output":long,"exit_code":0}}}),
        ),
        row(
            json!({"sessionUpdate":"tool_call","toolCallId":"empty","title":"empty_tool","rawInput":{}}),
        ),
        row(
            json!({"sessionUpdate":"tool_call_update","toolCallId":"empty","status":"completed","rawOutput":""}),
        ),
        row(
            json!({"sessionUpdate":"tool_call","toolCallId":"ask","title":"ask_user_question","rawInput":{"questions":[{"question":"选择？","options":["A","B"]}]}}),
        ),
        row(
            json!({"sessionUpdate":"tool_call","toolCallId":"spawn","_meta":{"x.ai/tool":{"name":"spawn_subagent"}},"rawInput":{"description":"帮忙","background":true}}),
        ),
        row(
            json!({"sessionUpdate":"tool_call_update","toolCallId":"spawn","status":"completed","content":[{"type":"content","content":{"type":"text","text":"Subagent started in background.\nsubagent_id: nested-short"}}]}),
        ),
        row(
            json!({"sessionUpdate":"subagent_spawned","subagent_id":"nested-short","subagent_type":"explore"}),
        ),
        row(
            json!({"sessionUpdate":"subagent_finished","subagent_id":"nested-short","status":"completed","output":"独立最终结果","duration_ms":1234,"tool_calls":1,"tokens_used":456}),
        ),
        row(
            json!({"sessionUpdate":"auto_compact_completed","tokens_before":9999,"tokens_after":5000}),
        ),
        row(json!({"sessionUpdate":"auto_compact_failed","reason":"compact failed"})),
        row(
            json!({"sessionUpdate":"turn_completed","usage":{"inputTokens":100,"cachedReadTokens":20,"cacheCreationTokens":5,"outputTokens":10}}),
        ),
    ];
    // Unknown timestamps use the supplied stable provenance time, not read time.
    rows.push(json!({"params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"未结尾回复"}}}}));
    let mut bytes = lines(&rows);
    bytes.extend(b"malformed completed row\n{\"params\":");
    write(home, "sessions/group-b/child/updates.jsonl", bytes);
    write(
        home,
        "sessions/group-b/child/chat_history.jsonl",
        lines(&[
            json!({"type":"tool_result","tool_call_id":"unrelated","content":"PRIVATE_UNRELATED_HISTORY"}),
            json!({"type":"tool_result","tool_call_id":"ask","content":"User has answered your questions: \"选择？\"=\"A, B\". You can now continue."}),
        ]),
    );
    write(home,"sessions/group-b/child/summary.json",serde_json::to_vec(&json!({"current_model_id":"model-from-native-summary","api_key":"PRIVATE_SUMMARY_MUST_NOT_RENDER"})).unwrap());
    write(home,"sessions/group-b/child/subagents/nested-short/meta.json",serde_json::to_vec(&json!({"child_session_id":"grandchild","parent_session_id":"child","status":"completed","tool_calls":1})).unwrap());
    write(
        home,
        "sessions/group-c/grandchild/updates.jsonl",
        lines(&[
            row(
                json!({"sessionUpdate":"tool_call","toolCallId":"nested-read","title":"read_file","rawInput":{"path":"nested.ts"}}),
            ),
            row(
                json!({"sessionUpdate":"tool_call_update","toolCallId":"nested-read","status":"completed","content":[{"type":"content","content":{"type":"text","text":"nested contents"}}]}),
            ),
        ]),
    );
    let stop = AtomicBool::new(false);
    let fallback = Utc.timestamp_opt(1600000000, 0).single().unwrap();
    let content = grok_content(&target, fallback, &stop).unwrap();
    let wire = serde_json::to_value(&content.turns).unwrap();
    let text = wire.to_string();
    for secret in [
        "PRIVATE_STARTUP_MUST_NOT_RENDER",
        "PRIVATE_SUMMARY_MUST_NOT_RENDER",
        "PRIVATE_UNRELATED_HISTORY",
        "HIDDEN_REMINDER_MUST_NOT_RENDER",
    ] {
        assert!(!text.contains(secret), "{secret}");
    }
    assert_eq!(content.turns.len(), 3);
    assert!(matches!(content.turns[0].role, model::TurnRole::User));
    assert_eq!(content.turns[0].blocks.len(), 2);
    assert_eq!(wire[0]["blocks"][1]["data"], "aQ==");
    let assistant = &content.turns[1];
    assert!(
        assistant
            .blocks
            .iter()
            .any(|b| matches!(b,model::ContentBlock::Text{text} if text=="甲乙"))
    );
    assert!(
        assistant
            .blocks
            .iter()
            .any(|b| matches!(b,model::ContentBlock::Thinking{text} if text=="思考A\n思考B"))
    );
    let blocks = serde_json::to_value(&assistant.blocks).unwrap();
    let tool = |id: &str, kind: &str| {
        blocks
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["type"] == kind && v["tool_use_id"] == id)
            .unwrap()
    };
    assert_eq!(tool("mcp", "tool_use")["tool_name"], "delegate_to_agent");
    let input: Value =
        serde_json::from_str(tool("mcp", "tool_use")["input_preview"].as_str().unwrap()).unwrap();
    assert_eq!(input["task"], long);
    let output: Value = serde_json::from_str(
        tool("mcp", "tool_result")["output_preview"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(output["Result"]["output"], long);
    assert_eq!(tool("empty", "tool_use")["status"], "completed");
    assert!(tool("empty", "tool_result")["output_preview"].is_null());
    assert_eq!(
        serde_json::from_str::<Value>(
            tool("read", "tool_result")["output_preview"]
                .as_str()
                .unwrap()
        )
        .unwrap(),
        json!({"start_line":1,"content":"one\ntwo"})
    );
    let answer: Value = serde_json::from_str(
        tool("ask", "tool_result")["output_preview"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(answer["answers"][0]["selected"], json!(["A", "B"]));
    let spawn = tool("spawn", "tool_result");
    assert_eq!(spawn["agent_stats"]["child_session_id"], "grandchild");
    assert_eq!(spawn["agent_stats"]["status"], "completed");
    assert_eq!(
        spawn["agent_stats"]["tool_calls"].as_array().unwrap().len(),
        1
    );
    assert!(
        spawn["output_preview"]
            .as_str()
            .unwrap()
            .contains("[[codeg-background-task]]")
    );
    assert_eq!(content.verified_children, vec!["grandchild"]);
    let usage = assistant.usage.as_ref().unwrap();
    assert_eq!(
        (
            usage.input_tokens,
            usage.cache_read_input_tokens,
            usage.cache_creation_input_tokens,
            usage.output_tokens
        ),
        (75, 20, 5, 10)
    );
    assert_eq!(content.context_tokens, Some(9999));
    assert_eq!(assistant.duration_ms, Some(1000));
    assert!(assistant.blocks.iter().any(|b|matches!(b,model::ContentBlock::ToolUse{meta:Some(meta),..} if meta.get("contextCompaction").is_some())));
    assert!(content.notes.incomplete_tail);
    assert_eq!(content.notes.malformed_records, 1);
    assert_eq!(content.turns[2].timestamp, fallback);
    assert!(content.turns[2].usage.is_none());
    let repeated = grok_content(&target, fallback, &stop).unwrap();
    assert_eq!(serde_json::to_value(repeated.turns).unwrap(), wire);
    assert_eq!(content.snapshot_digest, repeated.snapshot_digest);
    assert!(grok_content(&target, fallback, &AtomicBool::new(true)).is_err());

    let mut codex = base.clone();
    codex.adapter_kind = "codex-cli".into();
    codex.recorded_home = Some(home.into());
    let oldest = vec![
        json!({"type":"session_meta","ordinal":0,"payload":{"id":"child","parent_thread_id":"parent","subagent_history_start_ordinal":2}}),
        json!({"ordinal":1,"type":"response_item","payload":{"text":"parent seed must disappear"}}),
        json!({"ordinal":2,"type":"response_item","payload":{"text":"child original turn"}}),
        json!({"ordinal":3,"type":"response_item","payload":{"text":"reverted turn must disappear"}}),
    ];
    let newest = vec![
        json!({"type":"session_meta","ordinal":4,"payload":{"id":"child","parent_thread_id":"parent","history_base":{"thread_id":"child","end_ordinal_exclusive":3}}}),
        json!({"ordinal":3,"type":"response_item","payload":{"text":"replacement turn"}}),
    ];
    write(
        home,
        "sessions/2026/10/18/rollout-2026-10-18T12-00-00-child.jsonl",
        lines(&oldest),
    );
    write(
        home,
        "sessions/2026/10/18/rollout-2026-10-18T12-01-00-child_revert-id.jsonl",
        lines(&newest),
    );
    let mut docs = NativeDocuments::open_recorded(&codex).unwrap();
    let mut notes = ParseNotes::default();
    let joined = codex_history_records(&mut docs, &stop, &mut notes).unwrap();
    assert_eq!(joined[0], newest[0]);
    assert!(joined.contains(&oldest[2]));
    assert!(joined.contains(&newest[1]));
    assert!(!joined.contains(&oldest[1]));
    assert!(!joined.contains(&oldest[3]));
}
