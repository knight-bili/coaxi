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

fn assert_project_files(home: &Path) {
    use super::project::ProjectFiles;
    use crate::runtime_subagent_session::source::NativeDocument;
    let root = home.join("trusted-patch");
    let nested = root.join("nested");
    fs::create_dir_all(&nested).unwrap();
    let file = root.join("file.ts");
    let outside = home.join("outside-patch.txt");
    fs::write(&file, "甲\r\n乙\n").unwrap();
    fs::write(&outside, "OUTSIDE_MUST_NOT_READ\n").unwrap();
    let stop = AtomicBool::new(false);
    let expected = vec!["甲".to_owned(), "乙".to_owned()];
    let mut files = ProjectFiles::new(Some(&root));
    assert_eq!(files.lines("file.ts", None, &stop), Some(expected.clone()));
    assert_eq!(
        files.lines("../file.ts", nested.to_str(), &stop),
        Some(expected.clone())
    );
    assert_eq!(
        files.lines(file.to_str().unwrap(), home.to_str(), &stop),
        Some(expected.clone())
    );
    assert!(files.lines("file.ts", home.to_str(), &stop).is_none());
    assert!(files.lines("../outside-patch.txt", None, &stop).is_none());
    assert!(
        files
            .lines(outside.to_str().unwrap(), None, &stop)
            .is_none()
    );
    assert!(files.lines("nested", None, &stop).is_none());
    assert!(files.lines("missing", None, &stop).is_none());
    assert!(files.lines("file.ts\0", None, &stop).is_none());
    assert!(
        ProjectFiles::new(None)
            .lines(file.to_str().unwrap(), None, &stop)
            .is_none()
    );
    fs::write(&file, "later\n").unwrap();
    assert_eq!(files.lines("file.ts", None, &stop), Some(expected));
    assert_eq!(
        ProjectFiles::new(Some(&root)).lines("file.ts", None, &stop),
        Some(vec!["later".into()])
    );
    fs::write(root.join("invalid"), [0xff, 0xfe]).unwrap();
    assert!(files.lines("invalid", None, &stop).is_none());
    assert!(
        files
            .lines("file.ts", None, &AtomicBool::new(true))
            .is_none()
    );
    let canonical = fs::canonicalize(&root).unwrap();
    let mut captured =
        NativeDocument::open_project(&canonical, &canonical.join("file.ts"), &stop).unwrap();
    fs::write(&file, "other\n").unwrap();
    assert!(captured.read_chunk(0).is_err());
    assert!(
        NativeDocument::open_project(
            &canonical,
            &canonical.join("file.ts"),
            &AtomicBool::new(true)
        )
        .is_err()
    );
    #[cfg(windows)]
    {
        fs::write(root.join("file.ts:private"), "STREAM_MUST_NOT_READ").unwrap();
        assert!(files.lines("file.ts:private", None, &stop).is_none());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, root.join("outside-link")).unwrap();
        assert!(files.lines("outside-link", None, &stop).is_none());
        std::os::unix::fs::symlink(&file, root.join("inside-link")).unwrap();
        assert_eq!(
            ProjectFiles::new(Some(&root)).lines("inside-link", None, &stop),
            Some(vec!["other".into()])
        );
    }
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
    super::codex::tests::assert_original_rules();
    assert_codex_file_content(home, base);
    assert_project_files(home);
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
            .contains("[[coaxi-background-task]]")
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
    if super::reference_tests::enabled() {
        let mut exact_rows = rows.clone();
        for (index, value) in exact_rows.iter_mut().enumerate() {
            value["timestamp"] = (1700000000_i64 + index as i64).into();
        }
        write(
            home,
            "sessions/group-b/child/updates.jsonl",
            lines(&exact_rows),
        );
        let complete = grok_content(&target, fallback, &stop).unwrap();
        super::reference_tests::compare_native(
            "grok-long-complete",
            home,
            "grok-build",
            &home.join("sessions/group-b/child"),
            "child",
            &complete,
            Some(("mcp", &long)),
        );
        let small_rows: Vec<Value> = exact_rows
            .iter()
            .map(|value| {
                serde_json::from_str(&value.to_string().replace(&long, "完整工具JSON")).unwrap()
            })
            .collect();
        write(
            home,
            "sessions/group-b/child/updates.jsonl",
            lines(&small_rows),
        );
        let compact = grok_content(&target, fallback, &stop).unwrap();
        super::reference_tests::compare_native(
            "grok-compact-tools",
            home,
            "grok-build",
            &home.join("sessions/group-b/child"),
            "child",
            &compact,
            None,
        );
    }

    // Two independently proven descendants exercise the actual lifecycle map,
    // whose iteration order must not change the content/cursor identity.
    let second_spawn: Vec<_> = rows
        .iter()
        .filter(|value| {
            let update = &value["params"]["update"];
            update["toolCallId"] == "spawn" || update["subagent_id"] == "nested-short"
        })
        .map(|value| {
            serde_json::from_str::<Value>(
                &value
                    .to_string()
                    .replace("nested-short", "nested-other")
                    .replace("\"spawn\"", "\"spawn-other\""),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(second_spawn.len(), 4);
    rows.extend(second_spawn);
    write(home, "sessions/group-b/child/updates.jsonl", lines(&rows));
    write(home,"sessions/group-b/child/subagents/nested-other/meta.json",serde_json::to_vec(&json!({"child_session_id":"grandchild-other","parent_session_id":"child","status":"completed","tool_calls":1})).unwrap());
    write(
        home,
        "sessions/group-c/grandchild-other/updates.jsonl",
        lines(&[
            row(
                json!({"sessionUpdate":"tool_call","toolCallId":"other-read","title":"read_file","rawInput":{"path":"other.ts"}}),
            ),
            row(
                json!({"sessionUpdate":"tool_call_update","toolCallId":"other-read","status":"completed","content":[{"type":"content","content":{"type":"text","text":"other child contents"}}]}),
            ),
        ]),
    );
    let multiple = grok_content(&target, fallback, &stop).unwrap();
    assert_eq!(
        multiple.verified_children,
        ["grandchild", "grandchild-other"]
    );
    let multiple_wire = serde_json::to_value(&multiple.turns).unwrap();
    for _ in 0..4 {
        let reread = grok_content(&target, fallback, &stop).unwrap();
        assert_eq!(reread.snapshot_digest, multiple.snapshot_digest);
        assert_eq!(reread.verified_children, multiple.verified_children);
        assert_eq!(serde_json::to_value(reread.turns).unwrap(), multiple_wire);
    }

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
    assert_grok_stats(home, base);
}

fn assert_grok_stats(home: &Path, base: &ObservedNativeSessionTarget) {
    let mut target = base.clone();
    target.adapter_kind = "grok-build".into();
    target.child_native_session_id = "grok-stats".into();
    target.recorded_home = Some(home.into());
    write(
        home,
        "sessions/group-a/parent/updates.jsonl",
        lines(&[row(
            json!({"sessionUpdate":"subagent_spawned","subagent_id":"stats-short","child_session_id":"grok-stats","parent_session_id":target.parent_native_session_id}),
        )]),
    );
    let mut rows = vec![
        row(
            json!({"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"请检查"}}),
        ),
        row(
            json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"第一段"}}),
        ),
        json!({"params":{"_meta":{"totalTokens":80000},"update":{"sessionUpdate":"turn_completed","usage":{"inputTokens":100,"cachedReadTokens":20,"cacheCreationTokens":5,"outputTokens":10}}}}),
        row(
            json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"第二段"}}),
        ),
        row(
            json!({"sessionUpdate":"turn_completed","usage":{"inputTokens":200,"cachedReadTokens":50,"cacheCreationTokens":10,"outputTokens":20}}),
        ),
    ];
    for (index, value) in rows.iter_mut().enumerate() {
        value["timestamp"] = (1700000000_i64 + index as i64).into();
    }
    write(
        home,
        "sessions/group-b/grok-stats/updates.jsonl",
        lines(&rows),
    );
    let summary = |model: &str| {
        serde_json::to_vec(&json!({"current_model_id":model,"generated_title":" 摘要标题 ","session_summary":"次选标题","head_branch":" feat/stats ","created_at":"2026-10-18T16:00:00+08:00","updated_at":"2026-10-18T17:00:00+08:00","info":{"cwd":"PRIVATE_GROK_CWD"},"api_key":"PRIVATE_GROK_SUMMARY"})).unwrap()
    };
    write(
        home,
        "sessions/group-b/grok-stats/summary.json",
        summary("custom-native"),
    );
    write(home,"models_cache.json",serde_json::to_vec(&json!({"models":{"custom-native":{"info":{"context_window":320000}}},"api_key":"PRIVATE_GROK_CACHE"})).unwrap());
    write(
        home,
        "config.toml",
        "[model.custom-native]\ncontext_window=640000\napi_key='PRIVATE_GROK_CONFIG'\n",
    );
    let stop = AtomicBool::new(false);
    let fallback = Utc.timestamp_opt(1600000000, 0).single().unwrap();
    let content = grok_content(&target, fallback, &stop).unwrap();
    super::reference_tests::compare_native(
        "grok-cache-priority",
        home,
        "grok-build",
        &home.join("sessions/group-b/grok-stats"),
        "grok-stats",
        &content,
        None,
    );
    let stats = content.session_stats.as_ref().unwrap();
    assert_eq!(stats.context_window_used_tokens, Some(80000));
    assert_eq!(stats.context_window_max_tokens, Some(320000));
    assert_eq!(stats.context_window_usage_percent, Some(25.0));
    assert_eq!(stats.total_tokens, Some((100 + 10) + (200 + 20)));
    let usage = stats.total_usage.as_ref().unwrap();
    assert_eq!(usage.input_tokens, (100 - 20 - 5) + (200 - 50 - 10));
    assert_eq!(usage.output_tokens, 10 + 20);
    assert_eq!(usage.cache_creation_input_tokens, 5 + 10);
    assert_eq!(usage.cache_read_input_tokens, 20 + 50);
    let meta = content.summary.as_ref().unwrap();
    assert_eq!(meta.title.as_deref(), Some("摘要标题"));
    assert_eq!(meta.model.as_deref(), Some("custom-native"));
    assert_eq!(meta.git_branch.as_deref(), Some("feat/stats"));
    assert_eq!(meta.started_at.to_rfc3339(), "2026-10-18T08:00:00+00:00");
    assert_eq!(
        meta.ended_at.unwrap().to_rfc3339(),
        "2026-10-18T09:00:00+00:00"
    );
    assert_eq!(meta.message_count, 3);
    let wire =
        serde_json::to_string(&(&content.turns, &content.session_stats, &content.summary)).unwrap();
    for marker in [
        "PRIVATE_GROK_CWD",
        "PRIVATE_GROK_SUMMARY",
        "PRIVATE_GROK_CACHE",
        "PRIVATE_GROK_CONFIG",
    ] {
        assert!(!wire.contains(marker));
    }
    let again = grok_content(&target, fallback, &stop).unwrap();
    assert_eq!(again.snapshot_digest, content.snapshot_digest);
    write(home, "models_cache.json", "{\"models\":{}}");
    let configured = grok_content(&target, fallback, &stop).unwrap();
    super::reference_tests::compare_native(
        "grok-config-fallback",
        home,
        "grok-build",
        &home.join("sessions/group-b/grok-stats"),
        "grok-stats",
        &configured,
        None,
    );
    assert_eq!(
        configured
            .session_stats
            .as_ref()
            .unwrap()
            .context_window_max_tokens,
        Some(640000)
    );
    assert_ne!(configured.snapshot_digest, content.snapshot_digest);
    write(home, "models_cache.json", "broken json");
    write(
        home,
        "sessions/group-b/grok-stats/summary.json",
        summary("grok-4.5"),
    );
    let inferred = grok_content(&target, fallback, &stop).unwrap();
    super::reference_tests::compare_native(
        "grok-id-inference",
        home,
        "grok-build",
        &home.join("sessions/group-b/grok-stats"),
        "grok-stats",
        &inferred,
        None,
    );
    assert_eq!(
        inferred
            .session_stats
            .as_ref()
            .unwrap()
            .context_window_max_tokens,
        Some(500000)
    );
    assert_eq!(inferred.notes.malformed_records, 1);
    write(
        home,
        "sessions/group-b/grok-stats/summary.json",
        summary("unknown-native"),
    );
    write(
        home,
        "models_cache.json",
        "{\"models\":{\"unknown-native\":{\"info\":{\"context_window\":0}}}}",
    );
    write(
        home,
        "config.toml",
        "[model.unknown-native]\ncontext_window=-1\n",
    );
    let unknown = grok_content(&target, fallback, &stop).unwrap();
    super::reference_tests::compare_native(
        "grok-unknown-window",
        home,
        "grok-build",
        &home.join("sessions/group-b/grok-stats"),
        "grok-stats",
        &unknown,
        None,
    );
    assert_eq!(
        unknown
            .session_stats
            .as_ref()
            .unwrap()
            .context_window_max_tokens,
        None
    );
    assert_eq!(
        unknown
            .session_stats
            .as_ref()
            .unwrap()
            .context_window_usage_percent,
        None
    );
    fs::remove_file(home.join("models_cache.json")).unwrap();
    fs::create_dir(home.join("models_cache.json")).unwrap();
    write(
        home,
        "config.toml",
        "[model.unknown-native]\ncontext_window=123456\napi_key='PRIVATE_GROK_CONFIG'\n",
    );
    let advisory = grok_content(&target, fallback, &stop).unwrap();
    super::reference_tests::compare_native(
        "grok-unreadable-advisory",
        home,
        "grok-build",
        &home.join("sessions/group-b/grok-stats"),
        "grok-stats",
        &advisory,
        None,
    );
    assert_eq!(
        advisory
            .session_stats
            .as_ref()
            .unwrap()
            .context_window_max_tokens,
        Some(123456)
    );
    assert_eq!(advisory.notes.unavailable_advisories, 1);
    assert_eq!(
        serde_json::to_value(&advisory.turns).unwrap(),
        serde_json::to_value(&unknown.turns).unwrap()
    );
}

fn assert_codex_file_content(home: &Path, base: &ObservedNativeSessionTarget) {
    use super::codex_content;
    let mut target = base.clone();
    target.adapter_kind = "codex-cli".into();
    target.child_native_session_id = "codex-content".into();
    target.recorded_home = Some(home.into());
    let record = |second: u32, kind: &str, payload: Value| json!({"timestamp": format!("2026-10-18T13:00:{second:02}Z"), "type":kind, "payload":payload});
    let patch = "*** Begin Patch\n*** Update File: src/file.ts\n@@\n-old\n+new\n*** End Patch";
    let project_path = home.join("project").to_string_lossy().into_owned();
    write(home, "project/src/file.ts", "old\n");
    let long = "完整结果中文".repeat(40_000);
    let rows = vec![
        record(
            0,
            "session_meta",
            json!({"id":"codex-content","parent_thread_id":target.parent_native_session_id,"cwd":project_path,"api_key":"PRIVATE_CODEX_STARTUP"}),
        ),
        record(
            1,
            "event_msg",
            json!({"type":"thread_goal_updated","goal":{"objective":"完成修改","status":"active"}}),
        ),
        record(2, "turn_context", json!({"model":"gpt-5-codex"})),
        record(3, "event_msg", json!({"type":"task_started"})),
        record(
            4,
            "event_msg",
            json!({"type":"user_message","message":"收到"}),
        ),
        record(
            4,
            "response_item",
            json!({"type":"message","role":"user","content":[{"type":"input_text","text":"收到"}]}),
        ),
        record(
            5,
            "event_msg",
            json!({"type":"agent_reasoning","text":"先检查"}),
        ),
        record(
            6,
            "event_msg",
            json!({"type":"agent_reasoning","text":"再修改"}),
        ),
        record(
            7,
            "response_item",
            json!({"type":"custom_tool_call","call_id":"patch-1","name":"apply_patch","input":patch}),
        ),
        record(
            8,
            "response_item",
            json!({"type":"custom_tool_call_output","call_id":"patch-1","output":"Applied"}),
        ),
        record(
            9,
            "response_item",
            json!({"type":"function_call","call_id":"long-1","name":"exec_command","arguments":"{\"cmd\":\"echo complete\"}"}),
        ),
        record(
            10,
            "response_item",
            json!({"type":"function_call_output","call_id":"long-1","output":long}),
        ),
        record(
            11,
            "event_msg",
            json!({"type":"agent_message","message":"完成"}),
        ),
        record(
            12,
            "event_msg",
            json!({"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":20,"output_tokens":10},"last_token_usage":{"input_tokens":100,"cached_input_tokens":20,"output_tokens":10},"model_context_window":200000}}),
        ),
        record(
            13,
            "event_msg",
            json!({"type":"thread_goal_updated","goal":null}),
        ),
    ];
    write(
        home,
        "sessions/2026/10/18/rollout-2026-10-18T13-00-00-codex-content.jsonl",
        lines(&rows),
    );
    let stop = AtomicBool::new(false);
    let fallback = Utc.timestamp_opt(1600000000, 0).single().unwrap();
    let mut reads = 0;
    let content = codex_content(&target, fallback, &stop, &mut |path, cwd| {
        assert_eq!(path, "src/file.ts");
        assert_eq!(cwd, Some(project_path.as_str()));
        reads += 1;
        Some(vec!["old".into()])
    })
    .unwrap();
    assert_eq!(reads, 1);
    super::reference_tests::compare_native(
        "codex-file-long-complete",
        home,
        "codex-cli",
        &home.join("sessions/2026/10/18/rollout-2026-10-18T13-00-00-codex-content.jsonl"),
        "codex-content",
        &content,
        Some(("long-1", &long)),
    );
    let summary = content.summary.as_ref().unwrap();
    assert_eq!(summary.title.as_deref(), Some("完成修改"));
    assert_eq!(summary.model.as_deref(), Some("gpt-5-codex"));
    let summary_wire = serde_json::to_string(summary).unwrap();
    assert!(!summary_wire.contains(&project_path));
    assert!(!summary_wire.contains("PRIVATE_CODEX_STARTUP"));
    let users: Vec<_> = content
        .turns
        .iter()
        .filter(|t| matches!(t.role, super::model::TurnRole::User))
        .flat_map(|t| &t.blocks)
        .filter_map(|b| {
            if let super::model::ContentBlock::Text { text } = b {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(users, ["/goal 完成修改", "收到"]);
    let wire = serde_json::to_value(&content.turns).unwrap();
    let text = wire.to_string();
    assert!(!text.contains("PRIVATE_CODEX_STARTUP"));
    assert!(text.contains("@@ -1,1 +1,1 @@"));
    let blocks: Vec<_> = content.turns.iter().flat_map(|t| &t.blocks).collect();
    assert!(blocks.iter().any(|b|matches!(b,super::model::ContentBlock::ToolResult{tool_use_id:Some(id),output_preview:Some(output),..} if id=="long-1" && output==&long)));
    for name in ["create_goal", "update_goal"] {
        assert_eq!(blocks.iter().filter(|b|matches!(b,super::model::ContentBlock::ToolUse{tool_name,..} if tool_name==name)).count(), 1);
    }
    let stats = content.session_stats.as_ref().unwrap();
    assert_eq!(stats.total_tokens, Some(110));
    assert_eq!(
        stats.total_usage.as_ref().unwrap().cache_read_input_tokens,
        20
    );
    assert_eq!(stats.context_window_max_tokens, Some(200000));
    let repeated = codex_content(&target, fallback, &stop, &mut |_, _| {
        Some(vec!["old".into()])
    })
    .unwrap();
    assert_eq!(serde_json::to_value(repeated.turns).unwrap(), wire);
    assert_eq!(content.snapshot_digest, repeated.snapshot_digest);
    assert!(codex_content(&target, fallback, &AtomicBool::new(true), &mut |_, _| None).is_err());
    let without_files = codex_content(&target, fallback, &stop, &mut |_, _| None).unwrap();
    assert_ne!(content.snapshot_digest, without_files.snapshot_digest);
    assert!(
        !serde_json::to_string(&without_files.turns)
            .unwrap()
            .contains("@@ -1,1 +1,1 @@")
    );
}
