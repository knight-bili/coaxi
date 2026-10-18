// Adapted original Codeg regression bodies (aff1e6c, Apache-2.0).
// Invoked by the existing source filesystem owner, with no new test/DB/process owner.
use super::*;

fn parse_rollout(label: &str, content: &str, session_id: &str) -> ConversationDetail {
    let mut rows: Vec<Value> = content
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect();
    let reference = super::super::reference_tests::prepare_codex(label, &mut rows);
    let detail = project(
        rows.into_iter().map(Ok),
        DateTime::from_timestamp(1_600_000_000, 0).unwrap(),
        &AtomicBool::new(false),
        &mut |_| None,
        &mut |_, _| None,
    )
    .unwrap();
    if let Some((home, path, case)) = reference {
        super::super::reference_tests::compare(
            &case,
            &home,
            "codex-cli",
            &path,
            session_id,
            &detail.turns,
            detail.session_stats.as_ref(),
            Some(&detail.summary),
            None,
        );
    }
    detail
}

fn parse_lines(lines: &[String], tag: &str) -> ConversationDetail {
    parse_rollout(tag, &lines.join("\n"), tag)
}

pub(crate) fn assert_original_rules() {
    assert_plan_and_compaction();
    extracts_turn_usage_from_codex_usage_payload();
    a_cache_write_is_split_out_of_codex_input();
    an_over_reported_codex_cache_write_cannot_inflate_the_input();
    every_model_round_trip_of_a_turn_is_counted();
    a_restated_token_count_is_not_counted_twice();
    a_round_that_ran_before_any_assistant_message_is_not_lost();
    a_transcript_without_cumulative_totals_still_counts_each_call_once();
    a_transcript_that_mixes_both_shapes_counts_each_round_exactly_once();
    a_rollout_with_no_assistant_message_still_reports_its_spend();
    a_counter_that_restarts_after_compaction_does_not_wrap_to_zero();
    code_mode_single_call_renders_as_the_inner_tool();
    code_mode_parallel_calls_split_output_per_card();
    completed_mcp_items_split_a_two_call_one_chunk_script();
    a_failed_semantic_mcp_item_settles_its_card_as_failed();
    a_thrown_script_keeps_its_own_card_over_a_matching_semantic_item();
    code_mode_mismatched_chunks_keep_the_script_card();
    code_mode_script_failure_marks_the_result_as_an_error();
    a_deferred_scripts_result_lands_on_its_own_card();
    a_script_still_running_after_a_wait_stays_collectable();
    a_script_cell_is_never_mistaken_for_a_shell_session();
    consecutive_promotions_land_in_place_and_keep_their_order();
    promoted_assistant_text_keeps_its_whitespace();
}

fn turn_texts(detail: &ConversationDetail) -> Vec<(&'static str, Option<String>)> {
    detail
        .turns
        .iter()
        .map(|turn| {
            let role = match turn.role {
                TurnRole::User => "user",
                TurnRole::Assistant => "assistant",
                TurnRole::System => "system",
            };
            (
                role,
                turn.blocks.iter().find_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                }),
            )
        })
        .collect()
}

fn code_mode_rollout(script: &str, output: serde_json::Value) -> Vec<String> {
    vec![
        rollout_line(
            "2026-07-20T08:40:00Z",
            "event_msg",
            serde_json::json!({"type": "user_message", "message": "go"}),
        ),
        rollout_line(
            "2026-07-20T08:40:01Z",
            "response_item",
            serde_json::json!({
                "type": "custom_tool_call",
                "name": "exec",
                "call_id": "call_1",
                "input": script,
            }),
        ),
        rollout_line(
            "2026-07-20T08:40:02Z",
            "response_item",
            serde_json::json!({
                "type": "custom_tool_call_output",
                "call_id": "call_1",
                "output": output,
            }),
        ),
    ]
}

fn rollout_line(ts: &str, msg_type: &str, payload: serde_json::Value) -> String {
    serde_json::json!({ "timestamp": ts, "type": msg_type, "payload": payload }).to_string()
}

fn tool_uses(detail: &ConversationDetail) -> Vec<(String, String, Option<String>)> {
    detail
        .turns
        .iter()
        .flat_map(|t| t.blocks.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolUse {
                tool_use_id,
                tool_name,
                input_preview,
                ..
            } => Some((
                tool_use_id.clone().unwrap_or_default(),
                tool_name.clone(),
                input_preview.clone(),
            )),
            _ => None,
        })
        .collect()
}

fn deferred_script_rollout(wait_output: serde_json::Value) -> Vec<String> {
    let mut lines = code_mode_rollout(
        "const r = await tools.mcp__codeg_mcp__get_delegation_status({task_ids:[\"t1\"],wait_ms:60000});\ntext(JSON.stringify(r));\n",
        serde_json::json!("Script running with cell ID 34\nWall time 11.0 seconds\nOutput:\n"),
    );
    lines.push(rollout_line(
        "2026-07-20T08:40:03Z",
        "response_item",
        serde_json::json!({
            "type": "function_call",
            "name": "wait",
            "call_id": "call_w",
            "arguments": "{\"cell_id\":\"34\",\"yield_time_ms\":60000}",
        }),
    ));
    lines.push(rollout_line(
        "2026-07-20T08:40:48Z",
        "response_item",
        serde_json::json!({
            "type": "function_call_output",
            "call_id": "call_w",
            "output": wait_output,
        }),
    ));
    lines
}

fn tool_results(detail: &ConversationDetail) -> Vec<(String, Option<String>, bool)> {
    detail
        .turns
        .iter()
        .flat_map(|t| t.blocks.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolResult {
                tool_use_id,
                output_preview,
                is_error,
                ..
            } => Some((
                tool_use_id.clone().unwrap_or_default(),
                output_preview.clone(),
                *is_error,
            )),
            _ => None,
        })
        .collect()
}

fn tool_use_statuses(detail: &ConversationDetail) -> Vec<(String, Option<String>)> {
    detail
        .turns
        .iter()
        .flat_map(|t| t.blocks.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolUse {
                tool_use_id,
                status,
                ..
            } => Some((tool_use_id.clone().unwrap_or_default(), status.clone())),
            _ => None,
        })
        .collect()
}

fn turn_usage_total(detail: &ConversationDetail) -> u64 {
    detail
        .turns
        .iter()
        .filter_map(|t| t.usage.as_ref())
        .map(|u| {
            u.input_tokens
                + u.output_tokens
                + u.cache_creation_input_tokens
                + u.cache_read_input_tokens
        })
        .sum()
}

fn extracts_turn_usage_from_codex_usage_payload() {
    let usage = serde_json::json!({
        "input_tokens": 120,
        "cached_input_tokens": 80,
        "output_tokens": 16
    });
    let parsed = extract_turn_usage_from_codex_usage(&usage).expect("usage");
    assert_eq!(parsed.input_tokens, 40);
    assert_eq!(parsed.output_tokens, 16);
    assert_eq!(parsed.cache_creation_input_tokens, 0);
    assert_eq!(parsed.cache_read_input_tokens, 80);
}

fn a_cache_write_is_split_out_of_codex_input() {
    // A `last_token_usage` codex wrote for gpt-5.6-terra on codeg's bound
    // provider: the 19 915 input tokens are 17 152 cache reads, 2 304
    // cache writes and only 459 fresh ones.
    let usage = serde_json::json!({
        "input_tokens": 19_915,
        "cached_input_tokens": 17_152,
        "cache_write_input_tokens": 2_304,
        "output_tokens": 2_095,
        "reasoning_output_tokens": 0,
        "total_tokens": 22_010
    });
    let parsed = extract_turn_usage_from_codex_usage(&usage).expect("usage");
    assert_eq!(parsed.input_tokens, 459);
    assert_eq!(parsed.cache_creation_input_tokens, 2_304);
    assert_eq!(parsed.cache_read_input_tokens, 17_152);
    assert_eq!(parsed.output_tokens, 2_095);
    // The split moves tokens between columns; the total is untouched.
    assert_eq!(
        parsed.input_tokens
            + parsed.cache_creation_input_tokens
            + parsed.cache_read_input_tokens
            + parsed.output_tokens,
        22_010
    );
    assert_eq!(codex_usage_counters(&usage), parsed);
}

fn an_over_reported_codex_cache_write_cannot_inflate_the_input() {
    let usage = serde_json::json!({
        "input_tokens": 100,
        "cached_input_tokens": 80,
        "cache_write_input_tokens": 50,
        "output_tokens": 1
    });
    let parsed = codex_usage_counters(&usage);
    assert_eq!(parsed.input_tokens, 0);
    assert_eq!(parsed.cache_creation_input_tokens, 20);
    assert_eq!(parsed.cache_read_input_tokens, 80);
}

fn every_model_round_trip_of_a_turn_is_counted() {
    // Codex emits a `token_count` after each model call, so one turn that
    // calls tools four times reports four times. Only the first was kept
    // (`if last_msg.usage.is_none()`), which lost most of a working turn's
    // spend — measured at 61 % of all Codex tokens in a real session tree.
    let content = concat!(
        "{\"timestamp\":\"2026-03-01T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"rounds-1\",\"cwd\":\"/tmp/demo\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:01Z\",\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-5-codex\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:02Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"working\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:03Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":20224,\"cached_input_tokens\":0,\"output_tokens\":458},\"last_token_usage\":{\"input_tokens\":20224,\"cached_input_tokens\":0,\"output_tokens\":458}}}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:13Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":51186,\"cached_input_tokens\":18000,\"output_tokens\":886},\"last_token_usage\":{\"input_tokens\":30962,\"cached_input_tokens\":18000,\"output_tokens\":428}}}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:29Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":92652,\"cached_input_tokens\":45000,\"output_tokens\":1420},\"last_token_usage\":{\"input_tokens\":41466,\"cached_input_tokens\":27000,\"output_tokens\":534}}}}\n"
    );
    let detail = parse_rollout("rounds", content, "rounds-1");

    // The session's own cumulative counter is the ground truth, and the
    // per-turn rows now reconstruct it exactly.
    assert_eq!(turn_usage_total(&detail), 92_652 + 1_420);
    let stats = detail.session_stats.expect("session stats");
    let total = stats.total_usage.expect("total usage");
    assert_eq!(
        total.input_tokens + total.output_tokens + total.cache_read_input_tokens,
        92_652 + 1_420
    );
}

fn a_restated_token_count_is_not_counted_twice() {
    // Codex sometimes reports the same call twice (7 380 of 31 846 events
    // in a real tree). Differencing the cumulative counter makes the repeat
    // contribute nothing, where summing `last_token_usage` would inflate it.
    let content = concat!(
        "{\"timestamp\":\"2026-03-01T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"repeat-1\",\"cwd\":\"/tmp/demo\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"hi\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:02Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":1000,\"cached_input_tokens\":0,\"output_tokens\":50},\"last_token_usage\":{\"input_tokens\":1000,\"cached_input_tokens\":0,\"output_tokens\":50}}}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:03Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":1000,\"cached_input_tokens\":0,\"output_tokens\":50},\"last_token_usage\":{\"input_tokens\":1000,\"cached_input_tokens\":0,\"output_tokens\":50}}}}\n"
    );
    let detail = parse_rollout("repeat", content, "repeat-1");
    assert_eq!(turn_usage_total(&detail), 1_050);
}

fn a_round_that_ran_before_any_assistant_message_is_not_lost() {
    // When the model opens a turn by calling a tool, its first round-trips
    // finish before it ever speaks. Those had no assistant message to
    // attach to and were dropped outright — 3 % of all recorded spend.
    let content = concat!(
        "{\"timestamp\":\"2026-03-01T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"early-1\",\"cwd\":\"/tmp/demo\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"go\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:02Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":800,\"cached_input_tokens\":0,\"output_tokens\":40},\"last_token_usage\":{\"input_tokens\":800,\"cached_input_tokens\":0,\"output_tokens\":40}}}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:03Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"done\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:04Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":1900,\"cached_input_tokens\":0,\"output_tokens\":110},\"last_token_usage\":{\"input_tokens\":1100,\"cached_input_tokens\":0,\"output_tokens\":70}}}}\n"
    );
    let detail = parse_rollout("early", content, "early-1");
    assert_eq!(turn_usage_total(&detail), 1_900 + 110);
}

fn a_transcript_without_cumulative_totals_still_counts_each_call_once() {
    // Fallback path: no `total_token_usage` to difference, so a `token_count`
    // that merely restates the previous one is recognized by its payload.
    let content = concat!(
        "{\"timestamp\":\"2026-03-01T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"nototal-1\",\"cwd\":\"/tmp/demo\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"hi\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:02Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":500,\"cached_input_tokens\":100,\"output_tokens\":25}}}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:03Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":500,\"cached_input_tokens\":100,\"output_tokens\":25}}}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:04Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":700,\"cached_input_tokens\":200,\"output_tokens\":30}}}}\n"
    );
    let detail = parse_rollout("nototal", content, "nototal-1");
    assert_eq!(turn_usage_total(&detail), 525 + 730);
}

fn a_transcript_that_mixes_both_shapes_counts_each_round_exactly_once() {
    // A no-total event contributes its own `last_token_usage`, so the
    // cumulative baseline has to learn about it. Otherwise the next
    // total-bearing event differences against a stale figure and bills that
    // round a second time.
    let content = concat!(
        "{\"timestamp\":\"2026-03-01T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"mixed-1\",\"cwd\":\"/tmp/demo\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"hi\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:02Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":100,\"cached_input_tokens\":0,\"output_tokens\":0},\"last_token_usage\":{\"input_tokens\":100,\"cached_input_tokens\":0,\"output_tokens\":0}}}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:03Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":50,\"cached_input_tokens\":0,\"output_tokens\":0}}}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:04Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":180,\"cached_input_tokens\":0,\"output_tokens\":0},\"last_token_usage\":{\"input_tokens\":30,\"cached_input_tokens\":0,\"output_tokens\":0}}}}\n"
    );
    let detail = parse_rollout("mixed", content, "mixed-1");
    // 100 + 50 + (180 - 150) — the session's own counter says 180 total.
    assert_eq!(turn_usage_total(&detail), 180);
}

fn a_rollout_with_no_assistant_message_still_reports_its_spend() {
    // A turn that only ran tools and was interrupted leaves `token_count`
    // events with no assistant turn to carry them, so `reconcile_turn_usage`
    // has no target. The tokens are not lost: the session-level total is
    // populated, and that is the fallback `facts_from_detail` records as a
    // single fact row (covered by `a_session_level_total_is_recorded_when_
    // no_turn_reports_usage` in commands::token_usage).
    let content = concat!(
        "{\"timestamp\":\"2026-03-01T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"toolonly-1\",\"cwd\":\"/tmp/demo\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"go\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:02Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":900,\"cached_input_tokens\":100,\"output_tokens\":40},\"last_token_usage\":{\"input_tokens\":900,\"cached_input_tokens\":100,\"output_tokens\":40}}}}\n"
    );
    let detail = parse_rollout("toolonly", content, "toolonly-1");
    assert!(
        !detail
            .turns
            .iter()
            .any(|t| matches!(t.role, TurnRole::Assistant)),
        "precondition: this rollout has no assistant turn"
    );
    let total = detail
        .session_stats
        .as_ref()
        .and_then(|s| s.total_usage.as_ref())
        .expect("session-level total must survive as the fallback fact source");
    assert_eq!(
        total.input_tokens + total.output_tokens + total.cache_read_input_tokens,
        940
    );
}

fn a_counter_that_restarts_after_compaction_does_not_wrap_to_zero() {
    // A cumulative counter that moves backwards means a fresh context, so
    // the new value is that round's own spend rather than a negative delta
    // silently clamped away.
    let content = concat!(
        "{\"timestamp\":\"2026-03-01T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"compact-1\",\"cwd\":\"/tmp/demo\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"hi\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:02Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":90000,\"cached_input_tokens\":0,\"output_tokens\":2000}}}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:03Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":5000,\"cached_input_tokens\":0,\"output_tokens\":100}}}}\n"
    );
    let detail = parse_rollout("compact", content, "compact-1");
    assert_eq!(turn_usage_total(&detail), 92_000 + 5_100);
}

fn code_mode_single_call_renders_as_the_inner_tool() {
    let lines = code_mode_rollout(
        "const r = await tools.exec_command({\n  cmd: \"git status --short\",\n  workdir: \"/repo\"\n});\ntext(r.output);\n",
        serde_json::json!([
            {"type": "input_text", "text": "Script completed\nWall time 0.5 seconds\nOutput:\n"},
            {"type": "input_text", "text": " M src/main.rs"},
        ]),
    );

    let detail = parse_lines(&lines, "code-mode-single");

    assert_eq!(
        tool_uses(&detail),
        vec![(
            "call_1".to_string(),
            "exec_command".to_string(),
            Some("git status --short".to_string())
        )]
    );
    assert_eq!(
        tool_results(&detail),
        vec![(
            "call_1".to_string(),
            Some(" M src/main.rs".to_string()),
            false
        )]
    );
}

fn code_mode_parallel_calls_split_output_per_card() {
    let lines = code_mode_rollout(
        "const rs = await Promise.all([\n  tools.exec_command({cmd:\"one\"}),\n  tools.exec_command({cmd:\"two\"}),\n  tools.exec_command({cmd:\"three\"})\n]);\nrs.forEach(r => text(r.output));\n",
        serde_json::json!([
            {"type": "input_text", "text": "Script completed\nWall time 0.1 seconds\nOutput:\n"},
            {"type": "input_text", "text": "out-one"},
            {"type": "input_text", "text": "out-two"},
            {"type": "input_text", "text": "out-three"},
        ]),
    );

    let detail = parse_lines(&lines, "code-mode-split");

    assert_eq!(
        tool_uses(&detail),
        vec![
            (
                "call_1#0".to_string(),
                "exec_command".to_string(),
                Some("one".to_string())
            ),
            (
                "call_1#1".to_string(),
                "exec_command".to_string(),
                Some("two".to_string())
            ),
            (
                "call_1#2".to_string(),
                "exec_command".to_string(),
                Some("three".to_string())
            ),
        ]
    );
    assert_eq!(
        tool_results(&detail),
        vec![
            ("call_1#0".to_string(), Some("out-one".to_string()), false),
            ("call_1#1".to_string(), Some("out-two".to_string()), false),
            ("call_1#2".to_string(), Some("out-three".to_string()), false),
        ]
    );
}

fn completed_mcp_items_split_a_two_call_one_chunk_script() {
    let script = concat!(
        "const wd=\"/tmp\";const taskA=\"A\";const taskB=\"B\";",
        "const [a,b]=await Promise.all([",
        "tools.mcp__codeg_mcp__delegate_to_agent({agent_type:\"codex\",working_dir:wd,task:taskA}),",
        "tools.mcp__codeg_mcp__delegate_to_agent({agent_type:\"codex\",working_dir:wd,task:taskB})",
        "]);text(JSON.stringify({a,b}));"
    );
    assert!(
        super::code_mode::parse_code_mode_script(script)
            .calls
            .is_none(),
        "the real variable-argument shape cannot be statically evaluated"
    );
    let mut lines = code_mode_rollout(
        script,
        serde_json::json!([
            {"type":"input_text","text":"Script completed\nWall time 0.2 seconds\nOutput:\n"},
            {"type":"input_text","text":"{\"a\":{},\"b\":{}}"},
        ]),
    );
    for (offset, (id, task_id, task)) in [("exec-b", "task-b", "B"), ("exec-a", "task-a", "A")]
        .into_iter()
        .enumerate()
    {
        lines.insert(
            2 + offset,
            rollout_line(
                "2026-07-20T08:40:01Z",
                "event_msg",
                serde_json::json!({
                    "type": "item_completed",
                    "item": {
                        "type": "McpToolCall",
                        "id": id,
                        "server": "codeg-mcp",
                        "tool": "delegate_to_agent",
                        "arguments": {"agent_type":"codex", "task":task},
                        "status": "completed",
                        "result": {
                            "content": [{"type":"text", "text":format!(
                                "Delegation successful. task_id={task_id}."
                            )}],
                            "structuredContent": {"task_id":task_id, "status":"running"},
                            "isError": false
                        }
                    }
                }),
            ),
        );
    }

    let detail = parse_lines(&lines, "code-mode-semantic-mcp");
    let uses = tool_uses(&detail);
    assert_eq!(
        uses.iter()
            .map(|(id, name, _)| (id.as_str(), name.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("exec-b", "mcp__codeg_mcp__delegate_to_agent"),
            ("exec-a", "mcp__codeg_mcp__delegate_to_agent"),
        ],
        "semantic items replace the outer script with real MCP cards"
    );
    assert_eq!(
        uses[0].2.as_deref(),
        Some(r#"{"agent_type":"codex","task":"B"}"#)
    );
    assert_eq!(
        uses[1].2.as_deref(),
        Some(r#"{"agent_type":"codex","task":"A"}"#)
    );
    assert_eq!(
        tool_results(&detail)
            .into_iter()
            .map(|(id, output, _)| (id, output))
            .collect::<Vec<_>>(),
        vec![
            (
                "exec-b".into(),
                Some("Delegation successful. task_id=task-b.".into())
            ),
            (
                "exec-a".into(),
                Some("Delegation successful. task_id=task-a.".into())
            ),
        ]
    );
}

fn a_failed_semantic_mcp_item_settles_its_card_as_failed() {
    let script = concat!(
        "const dir=\"/tmp/w\";",
        "const res=await tools.mcp__codeg_mcp__delegate_to_agent({agent_type:\"codex\",working_dir:dir,task:t});",
        "text(res.content[0].text);"
    );
    let mut lines = code_mode_rollout(
        script,
        serde_json::json!([
            {"type":"input_text","text":"Script completed\nWall time 0.0 seconds\nOutput:\n"},
            {"type":"input_text","text":"depth limit exceeded (2 >= 2)"},
        ]),
    );
    lines.insert(
        2,
        rollout_line(
            "2026-07-20T08:40:01Z",
            "event_msg",
            serde_json::json!({
                "type": "item_completed",
                "item": {
                    "type": "McpToolCall",
                    "id": "exec-depth-limit",
                    "server": "codeg-mcp",
                    "tool": "delegate_to_agent",
                    "arguments": {"agent_type":"codex", "working_dir":"/tmp/w"},
                    "status": "failed",
                    "result": {
                        "content": [{"type":"text", "text":"depth limit exceeded (2 >= 2)"}],
                        "structuredContent": {"error_code":"depth_limit", "status":"failed"},
                        "isError": true
                    }
                }
            }),
        ),
    );

    let detail = parse_lines(&lines, "semantic-mcp-failed");
    assert_eq!(
        tool_use_statuses(&detail),
        vec![("exec-depth-limit".to_string(), Some("failed".to_string()))],
        "the card must report the call's own outcome, not the wrapper's"
    );
    assert_eq!(
        tool_results(&detail),
        vec![(
            "exec-depth-limit".to_string(),
            Some("depth limit exceeded (2 >= 2)".to_string()),
            true,
        )]
    );
}

fn a_thrown_script_keeps_its_own_card_over_a_matching_semantic_item() {
    let script = concat!(
        "const r=await tools.mcp__codeg_mcp__task_progress({message:m});",
        "text(r.content[0].text.toUpperCase());"
    );
    let mut lines = code_mode_rollout(
        script,
        serde_json::json!([
            {"type":"input_text","text":"Script failed\nWall time 0.1 seconds\nOutput:\n"},
            {"type":"input_text","text":"Script error:\nTypeError: Cannot read properties of undefined"},
        ]),
    );
    lines.insert(
        2,
        rollout_line(
            "2026-07-20T08:40:01Z",
            "event_msg",
            serde_json::json!({
                "type": "item_completed",
                "item": {
                    "type": "McpToolCall",
                    "id": "exec-progress",
                    "server": "codeg-mcp",
                    "tool": "task_progress",
                    "arguments": {"message":"halfway"},
                    "status": "completed",
                    "result": {"content":[{"type":"text","text":"recorded"}], "isError":false},
                },
            }),
        ),
    );

    let detail = parse_lines(&lines, "semantic-mcp-thrown-script");
    assert!(
        !tool_uses(&detail)
            .iter()
            .any(|(id, _, _)| id == "exec-progress"),
        "a thrown script must not be replaced by the call that did answer"
    );
    let results = tool_results(&detail);
    assert_eq!(results.len(), 1, "one card: {results:?}");
    assert!(results[0].2, "a thrown script still renders as an error");
    assert!(
        results[0]
            .1
            .as_deref()
            .is_some_and(|text| text.contains("TypeError")),
        "the thrown script's own error must survive: {:?}",
        results[0].1
    );
}

fn code_mode_mismatched_chunks_keep_the_script_card() {
    let lines = code_mode_rollout(
        "const rs = await Promise.all([\n  tools.exec_command({cmd:\"one\"}),\n  tools.exec_command({cmd:\"two\"})\n]);\ntext(JSON.stringify(rs));\n",
        serde_json::json!([
            {"type": "input_text", "text": "Script completed\nWall time 0.1 seconds\nOutput:\n"},
            {"type": "input_text", "text": "[{\"output\":\"a\"},{\"output\":\"b\"}]"},
        ]),
    );

    let detail = parse_lines(&lines, "code-mode-mismatch");

    let uses = tool_uses(&detail);
    assert_eq!(uses.len(), 1);
    assert_eq!(uses[0].1, CODEX_SCRIPT_TOOL_NAME);
    let card: serde_json::Value =
        serde_json::from_str(uses[0].2.as_deref().expect("script card input"))
            .expect("script card input is JSON");
    assert_eq!(card["title"], "one");
    assert_eq!(card["call_count"], 2);
    assert!(
        card["source"]
            .as_str()
            .expect("source")
            .contains("Promise.all")
    );

    let results = tool_results(&detail);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].1.as_deref(),
        Some("[{\"output\":\"a\"},{\"output\":\"b\"}]")
    );
}

fn code_mode_script_failure_marks_the_result_as_an_error() {
    let lines = code_mode_rollout(
        "const r = await tools.exec_command({cmd: \"rm -rf /tmp/x\"});\ntext(r.output);\n",
        serde_json::json!([
            {"type": "input_text", "text": "Script failed\nWall time 0.0 seconds\nOutput:\n"},
            {"type": "input_text", "text": "Script error:\nexec_command failed for `rm -rf /tmp/x`"},
        ]),
    );

    let detail = parse_lines(&lines, "code-mode-failed");

    let results = tool_results(&detail);
    assert_eq!(results.len(), 1);
    assert!(results[0].2, "a failed script must render as an error");
}

fn a_deferred_scripts_result_lands_on_its_own_card() {
    let lines = deferred_script_rollout(serde_json::json!([
        {"type": "input_text", "text": "Script completed\nWall time 44.5 seconds\nOutput:\n"},
        {"type": "input_text", "text": "{\"tasks\":[{\"task_id\":\"t1\",\"child_conversation_id\":2890}]}"},
    ]));

    let detail = parse_lines(&lines, "deferred-script");

    // One card for the call, carrying the result the `wait` collected.
    assert_eq!(
        tool_uses(&detail)
            .into_iter()
            .map(|(id, name, _)| (id, name))
            .collect::<Vec<_>>(),
        vec![(
            "call_1".to_string(),
            "mcp__codeg_mcp__get_delegation_status".to_string()
        )]
    );
    assert_eq!(
        tool_results(&detail),
        vec![(
            "call_1".to_string(),
            Some("{\"tasks\":[{\"task_id\":\"t1\",\"child_conversation_id\":2890}]}".to_string()),
            false
        )]
    );
}

fn a_script_still_running_after_a_wait_stays_collectable() {
    // The wait came back with the script STILL parked: the card keeps the
    // running note, and the next wait must still find it.
    let lines = deferred_script_rollout(serde_json::json!(
        "Script running with cell ID 34\nWall time 60.0 seconds\nOutput:\n"
    ));

    let detail = parse_lines(&lines, "deferred-still-running");

    assert_eq!(tool_uses(&detail).len(), 1);
    assert_eq!(
        tool_results(&detail),
        vec![(
            "call_1".to_string(),
            Some("Script running with cell ID 34".to_string()),
            false
        )]
    );
}

fn a_script_cell_is_never_mistaken_for_a_shell_session() {
    // `Script running with cell ID N` names the SCRIPT, not a shell inside
    // it — so the wait that collects it folds into the script's card and
    // contributes no card of its own. Nothing may leak into the shell
    // session map either: that cell is not a shell, and the long-running
    // call here is not even a command.
    let mut lines = code_mode_rollout(
        "const r = await tools.mcp__codeg_mcp__ask_user_question({questions: []});\ntext(JSON.stringify(r));\n",
        serde_json::json!([
            {"type": "input_text", "text": "Script running with cell ID 15\nWall time 11.0 seconds\nOutput:\n"},
        ]),
    );
    lines.push(rollout_line(
        "2026-07-20T08:40:03Z",
        "response_item",
        serde_json::json!({
            "type": "function_call",
            "name": "wait",
            "call_id": "call_s",
            "arguments": "{\"cell_id\":\"15\"}",
        }),
    ));

    let detail = parse_lines(&lines, "session-script-cell");
    let uses = tool_uses(&detail);
    assert_eq!(
        uses.iter()
            .map(|(id, name, _)| (id.as_str(), name.as_str()))
            .collect::<Vec<_>>(),
        vec![("call_1", "mcp__codeg_mcp__ask_user_question")]
    );
}

fn consecutive_promotions_land_in_place_and_keep_their_order() {
    // Several candidates share one insertion point when nothing was pushed
    // between them. The splice walks insertion points in DESCENDING order,
    // so a same-index group has to go back as a GROUP — one at a time would
    // reverse it.
    //
    // The tool calls on either side are what give this teeth: they make the
    // insertion point interior, so appending the promotions at the end and
    // splicing them one by one BOTH produce a different transcript.
    let content = concat!(
        "{\"timestamp\":\"2026-03-01T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"ord-1\",\"cwd\":\"/tmp/demo\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:01Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"name\":\"shell\",\"call_id\":\"before\",\"arguments\":\"{\\\"command\\\":[\\\"echo\\\",\\\"before\\\"]}\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call_output\",\"call_id\":\"before\",\"output\":\"before\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"first\"}]}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:04Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"second\"}]}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:05Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"third\"}]}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:06Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"name\":\"shell\",\"call_id\":\"after\",\"arguments\":\"{\\\"command\\\":[\\\"echo\\\",\\\"after\\\"]}\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:07Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call_output\",\"call_id\":\"after\",\"output\":\"after\"}}\n"
    );
    let detail = parse_rollout("order", content, "ord-1");

    assert_eq!(
        turn_texts(&detail),
        vec![
            ("assistant", None),
            ("assistant", Some("first".into())),
            ("assistant", Some("second".into())),
            ("assistant", Some("third".into())),
            ("assistant", None),
        ],
        "the group lands between the two tool turns, in source order"
    );
}

fn promoted_assistant_text_keeps_its_whitespace() {
    // The `event_msg.agent_message` arm stores its text verbatim, while the
    // user arms run `strip_blocked_resource_mentions` (which collapses runs
    // of spaces — right for a typed prompt, ruinous for indented markdown).
    // Promotion has to match each of them, not pick one.
    let content = concat!(
        "{\"timestamp\":\"2026-03-01T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"ws-1\",\"cwd\":\"/tmp/demo\"}}\n",
        "{\"timestamp\":\"2026-03-01T10:00:01Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"```sh\\n    indented\\n```\"}]}}\n"
    );
    let detail = parse_rollout("whitespace", content, "ws-1");

    assert_eq!(
        turn_texts(&detail)[0].1.as_deref(),
        Some("```sh\n    indented\n```")
    );
}

fn plan_turn(announce: bool, assistant_record: Option<&str>) -> String {
    let mut lines = String::from(
        "{\"timestamp\":\"2026-03-01T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"plan-1\",\"cwd\":\"/tmp/demo\"}}\n",
    );
    lines.push_str("{\"timestamp\":\"2026-03-01T10:00:01Z\",\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-5-codex\",\"collaboration_mode\":{\"mode\":\"plan\"}}}\n");
    lines.push_str("{\"timestamp\":\"2026-03-01T10:00:02Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"列个计划\"}}\n");
    if announce {
        lines.push_str("{\"timestamp\":\"2026-03-01T10:00:03Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"item_completed\",\"item\":{\"type\":\"Plan\",\"id\":\"turn-plan\",\"text\":\"# Plan\\n\\n- step one\"}}}\n");
    }
    if let Some(text) = assistant_record {
        lines.push_str(&format!(
                "{{\"timestamp\":\"2026-03-01T10:00:04Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":{}}}]}}}}\n",
                serde_json::to_string(text).expect("encode")
            ));
    }
    lines
}

fn approved_plan_rollout_with(
    prompt: &str,
    flip_out_of_plan: bool,
    repeat_context: bool,
) -> String {
    let mut lines = plan_turn(
        true,
        Some("<proposed_plan>\n# Plan\n\n- step one\n</proposed_plan>"),
    );
    let mode = if flip_out_of_plan { "default" } else { "plan" };
    lines.push_str(&format!(
            "{{\"timestamp\":\"2026-03-01T10:00:05Z\",\"type\":\"turn_context\",\"payload\":{{\"model\":\"gpt-5-codex\",\"collaboration_mode\":{{\"mode\":\"{mode}\"}}}}}}\n"
        ));
    if repeat_context {
        lines.push_str(&format!(
                "{{\"timestamp\":\"2026-03-01T10:00:05Z\",\"type\":\"turn_context\",\"payload\":{{\"model\":\"gpt-5-codex\",\"collaboration_mode\":{{\"mode\":\"{mode}\"}}}}}}\n"
            ));
    }
    lines.push_str(&format!(
            "{{\"timestamp\":\"2026-03-01T10:00:06Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":{}}}}}\n",
            serde_json::to_string(prompt).expect("encode")
        ));
    lines.push_str("{\"timestamp\":\"2026-03-01T10:00:07Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"开始实施。\"}}\n");
    lines
}

// Additional content seam checks; original summary/list parsing is not simulated.
fn assert_plan_and_compaction() {
    for repeated in [false, true] {
        let content = approved_plan_rollout_with(CODEX_PLAN_APPROVAL_PROMPT, true, repeated);
        let detail = parse_rollout("plan-approved", &content, "plan-1");
        assert_eq!(
            turn_texts(&detail)
                .iter()
                .filter(|(role, _)| *role == "user")
                .count(),
            1
        );
        assert_eq!(
            tool_uses(&detail)
                .iter()
                .filter(|(_, name, _)| name == "plan_review")
                .count(),
            1
        );
        assert!(
            tool_results(&detail)
                .iter()
                .any(
                    |(_, output, error)| output.as_deref() == Some(CODEX_PLAN_APPROVED_OUTPUT)
                        && !error
                )
        );
    }
    let typed = approved_plan_rollout_with(CODEX_PLAN_APPROVAL_PROMPT, false, false);
    let detail = parse_rollout("typed-plan-words", &typed, "plan-1");
    assert_eq!(
        turn_texts(&detail)
            .iter()
            .filter(|(role, _)| *role == "user")
            .count(),
        2
    );

    let row = |second: u32, kind: &str, payload: Value| serde_json::json!({"timestamp":format!("2026-10-18T14:00:{second:02}Z"),"type":kind,"payload":payload});
    let message = |role: &str, text: &str| serde_json::json!({"type":"message","role":role,"content":[{"type":if role=="user"{"input_text"}else{"output_text"},"text":text}]});
    let handoff = "压缩后的交接材料";
    let rows = vec![
        row(0, "session_meta", serde_json::json!({"id":"compaction-1"})),
        row(1, "event_msg", serde_json::json!({"type":"task_started"})),
        row(2, "response_item", message("user", "继续")),
        row(3, "response_item", message("assistant", "之前的工作")),
        row(4, "response_item", message("assistant", handoff)),
        row(
            5,
            "compacted",
            serde_json::json!({"message":format!("{CODEX_COMPACTION_SUMMARY_PREFIX}\n{handoff}"),"replacement_history":[]}),
        ),
        row(
            6,
            "event_msg",
            serde_json::json!({"type":"item_completed","item":{"type":"ContextCompaction","id":"cmp-one"}}),
        ),
        row(7, "response_item", message("assistant", "继续完成")),
    ];
    let content = rows
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let detail = parse_rollout("compaction", &content, "compaction-1");
    let texts = turn_texts(&detail);
    assert!(
        texts
            .iter()
            .any(|(_, text)| text.as_deref() == Some("之前的工作"))
    );
    assert!(
        texts
            .iter()
            .any(|(_, text)| text.as_deref() == Some("继续完成"))
    );
    assert!(
        !texts
            .iter()
            .any(|(_, text)| text.as_deref() == Some(handoff))
    );
    assert!(
        tool_uses(&detail)
            .iter()
            .any(|(id, name, _)| id == "cmp-one" && name == "context_compaction")
    );
    assert!(
        tool_results(&detail)
            .iter()
            .any(|(id, output, error)| id == "cmp-one"
                && output.as_deref() == Some(handoff)
                && !error)
    );
}
