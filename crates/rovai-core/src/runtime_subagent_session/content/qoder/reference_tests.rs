//! Fixed Codeg Qoder assertions. Existing source owner supplies every case's
//! owned root; fixture discovery is an adapter, not User/Core provenance.
use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
};

struct ReferenceDirectory(PathBuf);
impl ReferenceDirectory {
    fn path(&self) -> &Path {
        &self.0
    }
}
fn reference_directory(root: &Path, name: &str) -> ReferenceDirectory {
    assert!(root.is_absolute());
    let path = root.join("qoder-original").join(name);
    fs::create_dir_all(&path).unwrap();
    ReferenceDirectory(path)
}
struct ReferenceParser(PathBuf);
fn parser_in(tmp: &Path) -> ReferenceParser {
    ReferenceParser(tmp.join("projects"))
}
struct ReferenceSummary {
    id: String,
    title: Option<String>,
    model: Option<String>,
    git_branch: Option<String>,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    message_count: u32,
}
struct ReferenceDetail {
    summary: ReferenceSummary,
    turns: Vec<MessageTurn>,
    session_stats: Option<SessionStats>,
    transcript_watermark: Option<u64>,
}
fn read_values(path: &Path) -> Vec<Value> {
    fs::read(path)
        .unwrap()
        .split(|b| *b == b'\n')
        .filter_map(|b| serde_json::from_slice(b).ok())
        .collect()
}
fn child_inputs(path: &Path) -> BTreeMap<String, QoderChild> {
    let stop = AtomicBool::new(false);
    let dir = path.with_extension("").join("subagents");
    let Ok(entries) = fs::read_dir(&dir) else {
        return BTreeMap::new();
    };
    let mut paths = entries.map(|e| e.unwrap().path()).collect::<Vec<_>>();
    paths.sort();
    let mut tasks = BTreeMap::<String, (Option<String>, Option<String>)>::new();
    for p in &paths {
        let name = p.file_name().unwrap().to_str().unwrap();
        if !name.starts_with("task-") || !name.ends_with(".json") {
            continue;
        }
        let v: Value = serde_json::from_slice(&fs::read(p).unwrap()).unwrap();
        if let Some(parent) = v["parentToolUseId"].as_str() {
            tasks.entry(parent.into()).or_insert((
                v["status"].as_str().map(str::to_string),
                v["result"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_string),
            ));
        }
    }
    let mut children = BTreeMap::new();
    for p in paths {
        if p.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let values = read_values(&p);
        let transcript =
            super::super::claude::parse_subagent_tool_calls(values.iter().cloned().map(Ok), &stop)
                .unwrap();
        let usage_calls = subagent_call_usage(values.into_iter().map(Ok), &stop).unwrap();
        let meta = p.with_extension("meta.json");
        let value: Value = fs::read(meta)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null);
        let parent = value["toolUseId"].as_str().map(str::to_string);
        let (status, result) = parent
            .as_ref()
            .and_then(|id| tasks.get(id))
            .cloned()
            .unwrap_or_default();
        children.insert(
            p.file_name().unwrap().to_str().unwrap().into(),
            QoderChild {
                parent_tool_use_id: parent,
                agent_type: value["agentType"].as_str().map(str::to_string),
                status,
                result,
                transcript,
                usage_calls,
            },
        );
    }
    children
}
impl ReferenceParser {
    fn get_conversation(&self, id: &str) -> Result<ReferenceDetail> {
        let path = self
            .0
            .join("-private-tmp-probe")
            .join(format!("{id}.jsonl"));
        ensure!(path.starts_with(&self.0), "fixture scope");
        let detail = parse(
            read_values(&path),
            "2026-10-19T00:00:00Z".parse().unwrap(),
            &AtomicBool::new(false),
            &child_inputs(&path),
            &mut |_, _| None,
        )?;
        let s = detail.summary;
        Ok(ReferenceDetail {
            summary: ReferenceSummary {
                id: id.into(),
                title: s.title,
                model: s.model,
                git_branch: s.git_branch,
                started_at: s.started_at,
                ended_at: s.ended_at,
                message_count: s.message_count,
            },
            turns: detail.turns,
            session_stats: detail.session_stats,
            transcript_watermark: Some(fs::metadata(path)?.len()),
        })
    }
    fn list_conversations(&self) -> Result<Vec<NativeTranscriptSummary>> {
        let dir = self.0.join("-private-tmp-probe");
        let mut paths = fs::read_dir(dir)?
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .collect::<Vec<_>>();
        paths.sort();
        Ok(paths
            .into_iter()
            .filter_map(|p| parse_summary(&p))
            .collect())
    }
}
fn parse_summary(path: &Path) -> Option<NativeTranscriptSummary> {
    let t = parse_transcript(read_values(path), &AtomicBool::new(false)).unwrap();
    let message_count = t.message_count();
    Some(NativeTranscriptSummary {
        title: t.title,
        model: t.model,
        git_branch: t.git_branch,
        started_at: t.first_ts?,
        ended_at: t.last_ts,
        message_count,
    })
}
#[derive(Debug, Clone)]
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

fn clamp_i64(v: u64) -> i64 {
    v.min(i64::MAX as u64) as i64
}

pub(crate) fn facts_from_turns(
    turns: &[MessageTurn],
    session_model: Option<&str>,
) -> Vec<UsageFact> {
    let session_model = session_model.map(str::trim).filter(|m| !m.is_empty());
    turns
        .iter()
        .enumerate()
        .filter_map(|(idx, turn)| {
            let usage = turn.usage.as_ref()?;
            let input = clamp_i64(usage.input_tokens);
            let output = clamp_i64(usage.output_tokens);
            let cache_create = clamp_i64(usage.cache_creation_input_tokens);
            let cache_read = clamp_i64(usage.cache_read_input_tokens);
            if input == 0 && output == 0 && cache_create == 0 && cache_read == 0 {
                return None;
            }
            let turn_key = if turn.id.is_empty() {
                format!("turn-{idx}")
            } else {
                turn.id.clone()
            };
            let model = turn
                .model
                .as_ref()
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty())
                .or_else(|| session_model.map(String::from));
            Some(UsageFact {
                turn_key,
                occurred_at: turn.timestamp,
                model,
                input_tokens: input,
                output_tokens: output,
                cache_creation_tokens: cache_create,
                cache_read_tokens: cache_read,
                duration_ms: clamp_i64(turn.duration_ms.unwrap_or(0)),
            })
        })
        .collect()
}

const RUNTIME_CONFIG: &str = r#"{"type":"runtime-config","sessionId":"s1","model":"qmodel_38max","reasoningEffort":null,"contextWindow":262144,"generation":null,"timestamp":1786895127277}"#;
const USER_LINE: &str = r#"{"type":"user","uuid":"u1","timestamp":"2026-08-16T15:45:27.384Z","message":{"role":"user","content":"read NOTES.md and reply"},"permissionMode":"default","origin":{"kind":"human"},"promptId":"s1","humanInput":{"text":"read NOTES.md and reply","mode":"prompt"},"parentUuid":null,"isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"cli","version":"1.1.23","gitBranch":"main"}"#;
const THINKING_LINE: &str = r#"{"type":"assistant","uuid":"a1","timestamp":"2026-08-16T15:45:33.089Z","message":{"id":"chatcmpl-1","type":"message","role":"assistant","model":"qmodel_38max","stop_reason":null,"content":[{"type":"thinking","thinking":"Need to read the file.","signature":""}],"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":5,"output_tokens":2}},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"cli","version":"1.1.23","gitBranch":"main"}"#;
const TOOL_USE_LINE: &str = r#"{"type":"assistant","uuid":"a2","timestamp":"2026-08-16T15:45:33.100Z","message":{"id":"chatcmpl-1","type":"message","role":"assistant","model":"qmodel_38max","stop_reason":"tool_use","content":[{"type":"tool_use","id":"call_1","name":"Read","input":{"file_path":"/private/tmp/probe/NOTES.md"}}],"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":5,"output_tokens":2}},"parentUuid":"a1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"cli","version":"1.1.23","gitBranch":"main"}"#;
const TOOL_RESULT_LINE: &str = r#"{"type":"user","uuid":"u2","timestamp":"2026-08-16T15:45:33.200Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_1","content":"1\tthe secret number is 42\n2\t"}]},"sourceToolAssistantUUID":"a2","promptId":"s1","toolUseResult":{"type":"text","file":{"filePath":"NOTES.md","content":"the secret number is 42\n","numLines":2,"startLine":1,"totalLines":2}},"parentUuid":"a2","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"cli","version":"1.1.23","gitBranch":"main"}"#;
const ANSWER_LINE: &str = r#"{"type":"assistant","uuid":"a3","timestamp":"2026-08-16T15:45:34.000Z","message":{"id":"chatcmpl-1","type":"message","role":"assistant","model":"qmodel_38max","stop_reason":"end_turn","content":[{"type":"text","text":"42","citations":null}],"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":5,"output_tokens":2,"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0}}},"parentUuid":"u2","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"cli","version":"1.1.23","gitBranch":"main"}"#;
const ACTIVE_LEAF_LINE: &str = r#"{"type":"active-leaf","sessionId":"s1","leafUuid":"a3","explicit":false,"timestamp":1786895133089}"#;
const SIDECHAIN_LINE: &str = r#"{"type":"assistant","uuid":"sc1","timestamp":"2026-08-16T15:45:35.000Z","message":{"role":"assistant","model":"qmodel_38max","content":[{"type":"text","text":"sub-agent internal output"}]},"parentUuid":null,"isSidechain":true,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"cli","version":"1.1.23","gitBranch":"main"}"#;
const ACP_USER_LINE: &str = r#"{"type":"user","uuid":"au1","timestamp":"2026-07-25T12:08:58.095Z","message":{"role":"user","content":[{"type":"text","text":"hi"}]},"permissionMode":"bypassPermissions","origin":{"kind":"human"},"promptId":"351c","parentUuid":null,"isSidechain":false,"cwd":"/Users/x/work/codeg","sessionId":"s2","userType":"external","entrypoint":"acp","version":"unknown"}"#;
const ACP_ERROR_LINE: &str = r#"{"type":"assistant","uuid":"aa1","timestamp":"2026-07-25T12:09:00.807Z","message":{"id":"eb4c","type":"message","role":"assistant","model":"<synthetic>","stop_reason":"stop_sequence","content":[{"type":"text","text":"{\"pricingUrl\":\"https://qoder.com/pricing?client=qoder\"}"}]},"isApiErrorMessage":true,"error":"unknown","displayErrorCode":"112","parentUuid":"au1","isSidechain":false,"cwd":"/Users/x/work/codeg","sessionId":"s2","userType":"external","entrypoint":"acp","version":"unknown"}"#;
const SUB_AGENT_CALL: &str = r#"{"type":"assistant","uuid":"c1","timestamp":"2026-08-16T15:45:35.000Z","isSidechain":true,"agentId":"aExplore-1","parent_tool_use_id":"call_1","message":{"id":"msg-sub-1","type":"message","role":"assistant","model":"auto","stop_reason":"end_turn","content":[{"type":"text","text":"done"}],"usage":{"input_tokens":35000,"cache_creation_input_tokens":0,"cache_read_input_tokens":34000,"output_tokens":300}},"version":"1.1.65"}"#;
const REDACTED: &str = r#"{"input_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":0,"credits":0.269,"billable":false,"context_usage_ratio":0.16721666666666668}"#;
const EXPOSED: &str = r#"{"input_tokens":2803,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":19,"credits":0.025,"billable":false,"context_usage_ratio":0.015572222222222222}"#;
fn write_session(tmp: &std::path::Path, id: &str, lines: &[&str]) -> PathBuf {
    let dir = tmp.join("projects").join("-private-tmp-probe");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{id}.jsonl"));
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    path
}

fn answer_with_usage(uuid: &str, parent: &str, usage: &str) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"{uuid}","timestamp":"2026-09-19T10:51:07.266Z","message":{{"id":"chatcmpl-{uuid}","type":"message","role":"assistant","model":"qfmodel","stop_reason":"end_turn","content":[{{"type":"text","text":"ok","citations":null}}],"usage":{usage}}},"parentUuid":"{parent}","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"acp","version":"unknown"}}"#
    )
}

fn text_of(turn: &MessageTurn) -> String {
    turn.blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn agent_session_with_subagents(root: &Path, agent_result: &str) -> PathBuf {
    let agent_use = r#"{"type":"assistant","uuid":"a9","timestamp":"2026-08-16T15:45:33.100Z","message":{"role":"assistant","model":"qmodel","stop_reason":"tool_use","content":[{"type":"tool_use","id":"call_agent1","name":"Agent","input":{"description":"调研","subagent_type":"Explore"}}]},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"acp","version":"1.1.68","gitBranch":"main"}"#;
    let active_leaf = r#"{"type":"active-leaf","sessionId":"s1","leafUuid":"u9","explicit":false,"timestamp":1786895139000}"#;
    let path = write_session(
        root,
        "s1",
        &[USER_LINE, agent_use, agent_result, active_leaf],
    );
    let sub = path.with_extension("").join("subagents");
    std::fs::create_dir_all(&sub).unwrap();
    sub
}

fn write_child_run(sub: &Path, agent_id: &str, tool_use_id: &str, tool: &str) {
    std::fs::write(
        sub.join(format!("agent-{agent_id}.meta.json")),
        format!(r#"{{"agentType":"Explore","toolUseId":"{tool_use_id}","description":"调研"}}"#),
    )
    .unwrap();
    let call = format!(
        r#"{{"type":"assistant","uuid":"c1","timestamp":"2026-08-16T15:45:35.000Z","message":{{"role":"assistant","model":"qmodel","content":[{{"type":"tool_use","id":"call_{agent_id}","name":"{tool}","input":{{}}}}]}},"isSidechain":true,"sessionId":"s1"}}"#
    );
    let result = format!(
        r#"{{"type":"user","uuid":"c2","timestamp":"2026-08-16T15:45:36.000Z","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"call_{agent_id}","content":"ok"}}]}},"isSidechain":true,"sessionId":"s1"}}"#
    );
    std::fs::write(
        sub.join(format!("agent-{agent_id}.jsonl")),
        format!("{call}\n{result}\n"),
    )
    .unwrap();
}

fn agent_stats_of(tmp: &Path, tool_use_id: &str) -> Option<AgentExecutionStats> {
    parser_in(tmp)
        .get_conversation("s1")
        .unwrap()
        .turns
        .iter()
        .flat_map(|t| t.blocks.iter())
        .find_map(|b| match b {
            ContentBlock::ToolResult {
                tool_use_id: Some(id),
                agent_stats,
                ..
            } if id == tool_use_id => Some(agent_stats.clone()),
            _ => None,
        })
        .expect("the Agent result is in the turns")
}

fn write_subagent(tmp: &std::path::Path, session_id: &str, agent_id: &str, lines: &[&str]) {
    let dir = tmp
        .join("projects")
        .join("-private-tmp-probe")
        .join(session_id)
        .join("subagents");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("agent-{agent_id}.jsonl")),
        lines.join("\n") + "\n",
    )
    .unwrap();
}

fn summary_model_follows_a_mid_session_switch(_root: &Path) {
    let tmp = reference_directory(_root, "summary_model_follows_a_mid_session_switch");
    const SWITCHED_ANSWER_LINE: &str = r#"{"type":"assistant","uuid":"b1","timestamp":"2026-08-16T15:46:00.000Z","message":{"id":"chatcmpl-2","type":"message","role":"assistant","model":"qfmodel","stop_reason":"end_turn","content":[{"type":"text","text":"second","citations":null}],"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":2}},"parentUuid":"a3","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"cli","version":"1.1.45","gitBranch":"main"}"#;
    write_session(
        tmp.path(),
        "s1",
        &[
            RUNTIME_CONFIG,
            USER_LINE,
            THINKING_LINE,
            TOOL_USE_LINE,
            TOOL_RESULT_LINE,
            ANSWER_LINE,
            SWITCHED_ANSWER_LINE,
        ],
    );

    let summaries = parser_in(tmp.path()).list_conversations().unwrap();
    assert_eq!(summaries[0].model.as_deref(), Some("qfmodel"));
    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    assert_eq!(detail.summary.model.as_deref(), Some("qfmodel"));
}

fn summary_model_ignores_a_blank_later_model(_root: &Path) {
    let tmp = reference_directory(_root, "summary_model_ignores_a_blank_later_model");
    const BLANK_MODEL_LINE: &str = r#"{"type":"assistant","uuid":"b2","timestamp":"2026-08-16T15:47:00.000Z","message":{"id":"chatcmpl-3","type":"message","role":"assistant","model":"","stop_reason":"end_turn","content":[{"type":"text","text":"third","citations":null}],"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":2}},"parentUuid":"a3","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"cli","version":"1.1.45","gitBranch":"main"}"#;
    write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, ANSWER_LINE, BLANK_MODEL_LINE],
    );

    let summaries = parser_in(tmp.path()).list_conversations().unwrap();
    assert_eq!(summaries[0].model.as_deref(), Some("qmodel_38max"));
    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    assert_eq!(detail.summary.model.as_deref(), Some("qmodel_38max"));
    // Nor does it name the turn it opens: that turn names no model (the
    // Token board then falls back to the session's) rather than "".
    let blank_turn = detail
        .turns
        .iter()
        .rev()
        .find(|t| matches!(t.role, TurnRole::Assistant))
        .expect("assistant turn");
    assert_eq!(blank_turn.model, None);
}

fn a_blank_model_on_a_later_fragment_keeps_the_bubbles_model(_root: &Path) {
    let tmp = reference_directory(
        _root,
        "a_blank_model_on_a_later_fragment_keeps_the_bubbles_model",
    );
    let blank_tool_use = TOOL_USE_LINE.replace(r#""model":"qmodel_38max""#, r#""model":"""#);
    write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, THINKING_LINE, &blank_tool_use],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let assistants: Vec<_> = detail
        .turns
        .iter()
        .filter(|t| matches!(t.role, TurnRole::Assistant))
        .collect();
    assert_eq!(assistants.len(), 1, "the two fragments are one bubble");
    assert!(
        assistants[0]
            .blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. })),
        "the blank-model fragment's tool call is kept, not dropped"
    );
    assert_eq!(assistants[0].model.as_deref(), Some("qmodel_38max"));
}

fn sub_agent_usage_folds_from_its_transcript_with_cache_split(_root: &Path) {
    let tmp = reference_directory(
        _root,
        "sub_agent_usage_folds_from_its_transcript_with_cache_split",
    );
    write_session(tmp.path(), "s1", &[USER_LINE, ANSWER_LINE]);
    // The same response as Qoder 1.1.65 streams it — several fragments,
    // each repeating the identical usage block.
    write_subagent(
        tmp.path(),
        "s1",
        "aExplore-1",
        &[SUB_AGENT_CALL, SUB_AGENT_CALL],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    // The call (15:45:35) lands on the assistant turn current then —
    // ANSWER_LINE's, at 15:45:34.
    let turn = detail
        .turns
        .iter()
        .rev()
        .find(|t| matches!(t.role, TurnRole::Assistant))
        .expect("assistant turn");
    let usage = turn.usage.clone().expect("turn usage");
    // 35,000 input of which 34,000 cached → 1,000 fresh in; counted ONCE
    // despite two records sharing `msg-sub-1`.
    assert_eq!(usage.input_tokens, 5 + 1_000);
    assert_eq!(usage.cache_read_input_tokens, 5 + 34_000);
    assert_eq!(usage.output_tokens, 2 + 300);

    // The details dialog's total is the sum of the turns, sub-agent spend
    // included — one number for the dialog and the Token board.
    let stats = detail.session_stats.expect("session stats");
    assert_eq!(stats.total_tokens, Some(5 + 2 + 5 + 1_000 + 34_000 + 300));
    // The board's side of that number: its per-turn facts are built from
    // turn usage, which is why the fold lands there and not only in the
    // session total.
    let facts = facts_from_turns(&detail.turns, detail.summary.model.as_deref());
    let board_total: i64 = facts.iter().map(|f| f.total_tokens()).sum();
    assert_eq!(board_total, 5 + 2 + 5 + 1_000 + 34_000 + 300);
}

fn sub_agent_records_without_a_message_id_each_count(_root: &Path) {
    let tmp = reference_directory(_root, "sub_agent_records_without_a_message_id_each_count");
    write_session(tmp.path(), "s1", &[USER_LINE, ANSWER_LINE]);
    let no_id = SUB_AGENT_CALL.replace(r#""id":"msg-sub-1","#, "");
    let empty_id = SUB_AGENT_CALL.replace(r#""id":"msg-sub-1""#, r#""id":"""#);
    // Two of each shape: neither the missing ids nor the empty ones may
    // collapse into one call between themselves.
    write_subagent(
        tmp.path(),
        "s1",
        "aExplore-1",
        &[&no_id, &no_id, &empty_id, &empty_id],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let usage = detail
        .turns
        .iter()
        .rev()
        .find(|t| matches!(t.role, TurnRole::Assistant))
        .and_then(|t| t.usage.clone())
        .expect("turn usage");
    assert_eq!(usage.input_tokens, 5 + 4 * 1_000);
    assert_eq!(usage.cache_read_input_tokens, 5 + 4 * 34_000);
    assert_eq!(usage.output_tokens, 2 + 4 * 300);
}

fn usage_on_the_last_fragment_only_counts_once(_root: &Path) {
    let tmp = reference_directory(_root, "usage_on_the_last_fragment_only_counts_once");
    let thinking = THINKING_LINE.replace(
        r#","usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":5,"output_tokens":2}"#,
        "",
    );
    assert_ne!(thinking, THINKING_LINE, "fixture must lose its usage");
    // In 1.1.67 the answer after a tool result is a response of its own.
    let answer = ANSWER_LINE.replace(r#""id":"chatcmpl-1""#, r#""id":"chatcmpl-2""#);
    write_session(
        tmp.path(),
        "s1",
        &[
            USER_LINE,
            &thinking,
            TOOL_USE_LINE,
            TOOL_RESULT_LINE,
            &answer,
        ],
    );
    let opening = SUB_AGENT_CALL.replace(
        r#","usage":{"input_tokens":35000,"cache_creation_input_tokens":0,"cache_read_input_tokens":34000,"output_tokens":300}"#,
        "",
    );
    assert_ne!(opening, SUB_AGENT_CALL, "fixture must lose its usage");
    write_subagent(tmp.path(), "s1", "aExplore-1", &[&opening, SUB_AGENT_CALL]);

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let assistant: Vec<_> = detail
        .turns
        .iter()
        .filter(|t| matches!(t.role, TurnRole::Assistant))
        .collect();
    assert_eq!(assistant.len(), 2);
    // The thinking + tool_use bubble keeps the usage its second fragment
    // brought: input 10 of which 5 cached.
    let first = assistant[0].usage.clone().expect("first response's usage");
    assert_eq!(first.input_tokens, 5);
    assert_eq!(first.cache_read_input_tokens, 5);
    assert_eq!(first.output_tokens, 2);
    // The sub-agent call (15:45:35) lands on the answer (15:45:34), once.
    let second = assistant[1].usage.clone().expect("second response's usage");
    assert_eq!(second.input_tokens, 5 + 1_000);
    assert_eq!(second.cache_read_input_tokens, 5 + 34_000);
    assert_eq!(second.output_tokens, 2 + 300);
    assert_eq!(
        detail.session_stats.expect("session stats").total_tokens,
        Some(2 * (5 + 5 + 2) + 1_000 + 34_000 + 300)
    );
}

fn sub_agent_calls_land_on_the_turn_current_when_each_happened(_root: &Path) {
    let tmp = reference_directory(
        _root,
        "sub_agent_calls_land_on_the_turn_current_when_each_happened",
    );
    const SWITCHED_ANSWER_LINE: &str = r#"{"type":"assistant","uuid":"b1","timestamp":"2026-08-16T15:46:00.000Z","message":{"id":"chatcmpl-2","type":"message","role":"assistant","model":"qfmodel","stop_reason":"end_turn","content":[{"type":"text","text":"second","citations":null}],"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":2}},"parentUuid":"a3","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"cli","version":"1.1.45","gitBranch":"main"}"#;
    const SECOND_CALL: &str = r#"{"type":"assistant","uuid":"c2","timestamp":"2026-08-16T15:46:30.000Z","isSidechain":true,"agentId":"aExplore-1","parent_tool_use_id":"call_1","message":{"id":"msg-sub-2","type":"message","role":"assistant","model":"auto","stop_reason":"end_turn","content":[{"type":"text","text":"done"}],"usage":{"input_tokens":35000,"cache_creation_input_tokens":0,"cache_read_input_tokens":34000,"output_tokens":300}},"version":"1.1.65"}"#;
    write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, ANSWER_LINE, SWITCHED_ANSWER_LINE],
    );
    // First call right after the first answer; second after the switch.
    write_subagent(
        tmp.path(),
        "s1",
        "aExplore-1",
        &[SUB_AGENT_CALL, SECOND_CALL],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let assistants: Vec<_> = detail
        .turns
        .iter()
        .filter(|t| matches!(t.role, TurnRole::Assistant))
        .collect();
    assert_eq!(assistants.len(), 2, "two answers, two turns");
    let first = assistants[0].usage.clone().expect("first turn usage");
    let second = assistants[1].usage.clone().expect("second turn usage");
    // Each call lands on the turn current at ITS OWN timestamp — a
    // background sub-agent keeps working after the parent moves on.
    assert_eq!(first.input_tokens, 5 + 1_000);
    assert_eq!(first.cache_read_input_tokens, 5 + 34_000);
    assert_eq!(first.output_tokens, 2 + 300);
    assert_eq!(second.input_tokens, 10 + 1_000);
    assert_eq!(second.cache_read_input_tokens, 34_000);
    assert_eq!(second.output_tokens, 2 + 300);
}

fn sub_agent_usage_does_not_touch_the_context_window(_root: &Path) {
    let tmp = reference_directory(_root, "sub_agent_usage_does_not_touch_the_context_window");
    write_session(tmp.path(), "s1", &[RUNTIME_CONFIG, USER_LINE, ANSWER_LINE]);
    // Control parse: no `subagents/` directory at all.
    let baseline = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let baseline_stats = baseline.session_stats.clone().expect("baseline stats");
    assert_eq!(baseline_stats.total_tokens, Some(5 + 2 + 5));

    write_subagent(tmp.path(), "s1", "aExplore-1", &[SUB_AGENT_CALL]);
    let with_sub = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let stats = with_sub.session_stats.expect("stats");
    // A sub-agent's context is its own: occupancy and window are untouched
    // (the fold runs after they were read) while the totals grow.
    assert_eq!(
        stats.context_window_used_tokens,
        baseline_stats.context_window_used_tokens
    );
    assert_eq!(
        stats.context_window_max_tokens,
        baseline_stats.context_window_max_tokens
    );
    assert!(stats.total_tokens.unwrap() > baseline_stats.total_tokens.unwrap());
}

fn agent_card_stats_carry_the_childs_mixed_timeline_in_arrival_order(_root: &Path) {
    let tmp = reference_directory(
        _root,
        "agent_card_stats_carry_the_childs_mixed_timeline_in_arrival_order",
    );
    let agent_use = r#"{"type":"assistant","uuid":"a9","timestamp":"2026-08-16T15:45:33.100Z","message":{"role":"assistant","model":"qmodel","stop_reason":"tool_use","content":[{"type":"tool_use","id":"call_agent1","name":"Agent","input":{"description":"读文件","subagent_type":"Explore"}}]},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"acp","version":"1.1.67","gitBranch":"main"}"#;
    let agent_result = r#"{"type":"user","uuid":"u9","timestamp":"2026-08-16T15:45:39.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_agent1","content":"以下是文件前 60 行…"}]},"sourceToolAssistantUUID":"a9","promptId":"s1","toolUseResult":{"isAsync":true,"status":"async_launched","agentId":"aExplore-abc123","description":"读文件"},"parentUuid":"a9","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"acp","version":"1.1.67","gitBranch":"main"}"#;
    let active_leaf = r#"{"type":"active-leaf","sessionId":"s1","leafUuid":"u9","explicit":false,"timestamp":1786895139000}"#;
    let path = write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, agent_use, agent_result, active_leaf],
    );

    let sub = path.with_extension("").join("subagents");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(
        sub.join("agent-aExplore-abc123.meta.json"),
        r#"{"agentType":"Explore","toolUseId":"call_agent1","description":"读文件","invocationName":"Explore"}"#,
    )
    .unwrap();
    // The child thinks, narrates, calls, then reports — the exact shape
    // Anthropic-form transcripts interleave on disk.
    let child_thinking = r#"{"type":"assistant","uuid":"c0","timestamp":"2026-08-16T15:45:34.000Z","message":{"id":"cm1","role":"assistant","model":"qmodel","content":[{"type":"thinking","thinking":"Need the notes first.","signature":""}]}}"#;
    let child_text = r#"{"type":"assistant","uuid":"c0b","timestamp":"2026-08-16T15:45:34.200Z","message":{"id":"cm1","role":"assistant","model":"qmodel","content":[{"type":"text","text":"Reading."}]}}"#;
    let child_use = r#"{"type":"assistant","uuid":"c1","timestamp":"2026-08-16T15:45:35.000Z","message":{"role":"assistant","model":"qmodel","content":[{"type":"tool_use","id":"call_r1","name":"Read","input":{"file_path":"/private/tmp/probe/NOTES.md"}}]},"isSidechain":true,"sessionId":"s1"}"#;
    let child_result = r#"{"type":"user","uuid":"c2","timestamp":"2026-08-16T15:45:35.500Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_r1","content":"1\t# NOTES"}]},"isSidechain":true,"sessionId":"s1"}"#;
    std::fs::write(
        sub.join("agent-aExplore-abc123.jsonl"),
        [child_thinking, child_text, child_use, child_result].join("\n") + "\n",
    )
    .unwrap();

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let stats = detail
        .turns
        .iter()
        .flat_map(|t| t.blocks.iter())
        .find_map(|b| match b {
            ContentBlock::ToolResult {
                tool_use_id,
                agent_stats,
                ..
            } if tool_use_id.as_deref() == Some("call_agent1") => agent_stats.clone(),
            _ => None,
        })
        .expect("Agent result carries agent_stats");
    assert_eq!(
        stats
            .timeline
            .iter()
            .map(|e| (e.entry_type.as_str(), e.child_index, e.text.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("thinking", None, Some("Need the notes first.")),
            ("text", None, Some("Reading.")),
            ("tool_call", Some(0), None),
        ]
    );
    assert_eq!(stats.tool_calls[0].tool_name, "Read");
    // No task file here, and Qoder's never records a duration anyway: the
    // child transcript's own first-to-last record span (34.000 → 35.500).
    assert_eq!(stats.total_duration_ms, Some(1_500));
}

fn an_errored_agent_result_still_carries_stats_and_the_recovered_result(_root: &Path) {
    let tmp = reference_directory(
        _root,
        "an_errored_agent_result_still_carries_stats_and_the_recovered_result",
    );
    let agent_use = r#"{"type":"assistant","uuid":"a9","timestamp":"2026-08-16T15:45:33.100Z","message":{"role":"assistant","model":"qmodel","stop_reason":"tool_use","content":[{"type":"tool_use","id":"call_agent1","name":"Agent","input":{"description":"调研","subagent_type":"Explore"}}]},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"acp","version":"1.1.67","gitBranch":"main"}"#;
    let agent_result = r#"{"type":"user","uuid":"u9","timestamp":"2026-08-16T15:45:39.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_agent1","is_error":true,"content":"Error: tool execution produced no result"}]},"sourceToolAssistantUUID":"a9","promptId":"s1","toolUseResult":{"isAsync":true,"status":"async_launched","agentId":"aExplore-abc123","description":"调研"},"parentUuid":"a9","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"acp","version":"1.1.67","gitBranch":"main"}"#;
    let active_leaf = r#"{"type":"active-leaf","sessionId":"s1","leafUuid":"u9","explicit":false,"timestamp":1786895139000}"#;
    let path = write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, agent_use, agent_result, active_leaf],
    );

    let sub = path.with_extension("").join("subagents");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(
        sub.join("agent-aExplore-abc123.meta.json"),
        r#"{"agentType":"Explore","toolUseId":"call_agent1","description":"调研","invocationName":"Explore"}"#,
    )
    .unwrap();
    std::fs::write(
        sub.join("task-t1.json"),
        // The shape qodercli 1.1.68 writes once the run completes: no
        // usage or duration (those ride only on the task notification).
        r#"{"taskId":"t1","sessionId":"s1","executionId":"e1","agentId":"aExplore-abc123","agentType":"Explore","description":"调研","invocationName":"Explore","parentToolUseId":"call_agent1","status":"completed","summary":"Agent \"Explore\" finished","result":"The full research report, thousands of words.","createdAt":1786895133100,"updatedAt":1786895138000,"completedAt":1786895138000}"#,
    )
    .unwrap();
    let child_use = r#"{"type":"assistant","uuid":"c1","timestamp":"2026-08-16T15:45:35.000Z","message":{"role":"assistant","model":"qmodel","content":[{"type":"tool_use","id":"call_r1","name":"Read","input":{"file_path":"/private/tmp/probe/NOTES.md"}}]},"isSidechain":true,"sessionId":"s1"}"#;
    let child_result = r#"{"type":"user","uuid":"c2","timestamp":"2026-08-16T15:45:35.500Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_r1","content":"1\t# NOTES"}]},"isSidechain":true,"sessionId":"s1"}"#;
    std::fs::write(
        sub.join("agent-aExplore-abc123.jsonl"),
        [child_use, child_result].join("\n") + "\n",
    )
    .unwrap();

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let (is_error, stats) = detail
        .turns
        .iter()
        .flat_map(|t| t.blocks.iter())
        .find_map(|b| match b {
            ContentBlock::ToolResult {
                tool_use_id,
                is_error,
                agent_stats,
                ..
            } if tool_use_id.as_deref() == Some("call_agent1") => {
                Some((*is_error, agent_stats.clone()))
            }
            _ => None,
        })
        .expect("Agent result present");
    assert!(is_error, "fixture precondition: the result is an error");
    let stats = stats.expect("errored Agent result still carries stats");
    assert_eq!(stats.status.as_deref(), Some("completed"));
    assert_eq!(
        stats.recovered_result.as_deref(),
        Some("The full research report, thousands of words.")
    );
    assert_eq!(
        stats
            .timeline
            .iter()
            .map(|e| e.entry_type.as_str())
            .collect::<Vec<_>>(),
        vec!["tool_call"]
    );
}

fn a_task_file_without_a_result_field_leaves_recovered_result_absent(_root: &Path) {
    let tmp = reference_directory(
        _root,
        "a_task_file_without_a_result_field_leaves_recovered_result_absent",
    );
    let agent_use = r#"{"type":"assistant","uuid":"a9","timestamp":"2026-08-16T15:45:33.100Z","message":{"role":"assistant","model":"qmodel","stop_reason":"tool_use","content":[{"type":"tool_use","id":"call_agent1","name":"Agent","input":{"description":"调研","subagent_type":"Explore"}}]},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"acp","version":"1.1.67","gitBranch":"main"}"#;
    let agent_result = r#"{"type":"user","uuid":"u9","timestamp":"2026-08-16T15:45:39.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_agent1","content":"以下是文件前 60 行…"}]},"sourceToolAssistantUUID":"a9","promptId":"s1","toolUseResult":{"isAsync":true,"status":"async_launched","agentId":"aExplore-abc123","description":"调研"},"parentUuid":"a9","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"acp","version":"1.1.67","gitBranch":"main"}"#;
    let active_leaf = r#"{"type":"active-leaf","sessionId":"s1","leafUuid":"u9","explicit":false,"timestamp":1786895139000}"#;
    let path = write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, agent_use, agent_result, active_leaf],
    );

    let sub = path.with_extension("").join("subagents");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(
        sub.join("agent-aExplore-abc123.meta.json"),
        r#"{"agentType":"Explore","toolUseId":"call_agent1","description":"调研","invocationName":"Explore"}"#,
    )
    .unwrap();
    // A task still `running` — the only state Qoder writes without a
    // `result` (every terminal transition files one).
    std::fs::write(
        sub.join("task-t1.json"),
        r#"{"taskId":"t1","sessionId":"s1","executionId":"e1","agentId":"aExplore-abc123","agentType":"Explore","description":"调研","invocationName":"Explore","parentToolUseId":"call_agent1","status":"running","createdAt":1786895133100,"updatedAt":1786895133100}"#,
    )
    .unwrap();
    let child_use = r#"{"type":"assistant","uuid":"c1","timestamp":"2026-08-16T15:45:35.000Z","message":{"role":"assistant","model":"qmodel","content":[{"type":"tool_use","id":"call_r1","name":"Read","input":{"file_path":"/private/tmp/probe/NOTES.md"}}]},"isSidechain":true,"sessionId":"s1"}"#;
    let child_result = r#"{"type":"user","uuid":"c2","timestamp":"2026-08-16T15:45:35.500Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_r1","content":"1\t# NOTES"}]},"isSidechain":true,"sessionId":"s1"}"#;
    std::fs::write(
        sub.join("agent-aExplore-abc123.jsonl"),
        [child_use, child_result].join("\n") + "\n",
    )
    .unwrap();

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let stats = detail
        .turns
        .iter()
        .flat_map(|t| t.blocks.iter())
        .find_map(|b| match b {
            ContentBlock::ToolResult {
                tool_use_id,
                agent_stats,
                ..
            } if tool_use_id.as_deref() == Some("call_agent1") => agent_stats.clone(),
            _ => None,
        })
        .expect("Agent result carries agent_stats");
    assert_eq!(stats.status.as_deref(), Some("running"));
    assert_eq!(stats.recovered_result, None);
}

fn an_interrupted_tasks_placeholder_result_rides_along_with_its_status(_root: &Path) {
    let tmp = reference_directory(
        _root,
        "an_interrupted_tasks_placeholder_result_rides_along_with_its_status",
    );
    let errored = r#"{"type":"user","uuid":"u9","timestamp":"2026-08-16T15:45:39.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_agent1","is_error":true,"content":"Error: tool execution produced no result"}]},"sourceToolAssistantUUID":"a9","promptId":"s1","parentUuid":"a9","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"acp","version":"1.1.68","gitBranch":"main"}"#;
    let sub = agent_session_with_subagents(tmp.path(), errored);
    write_child_run(&sub, "aExplore-abc123", "call_agent1", "Read");
    std::fs::write(
        sub.join("task-t1.json"),
        r#"{"taskId":"t1","agentId":"aExplore-abc123","parentToolUseId":"call_agent1","status":"interrupted","summary":"Agent task \"调研\" stopped before it could finish.","result":"The process that owned this subagent task is no longer running.","createdAt":1786895133100,"updatedAt":1786895200000,"completedAt":1786895200000}"#,
    )
    .unwrap();

    let stats = agent_stats_of(tmp.path(), "call_agent1").expect("stats attached");
    assert_eq!(stats.status.as_deref(), Some("interrupted"));
    assert_eq!(
        stats.recovered_result.as_deref(),
        Some("The process that owned this subagent task is no longer running.")
    );
}

fn the_first_run_in_name_order_wins_a_shared_tool_call(_root: &Path) {
    let tmp = reference_directory(_root, "the_first_run_in_name_order_wins_a_shared_tool_call");
    let result = r#"{"type":"user","uuid":"u9","timestamp":"2026-08-16T15:45:39.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_agent1","content":"report"}]},"sourceToolAssistantUUID":"a9","promptId":"s1","parentUuid":"a9","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1","userType":"external","entrypoint":"acp","version":"1.1.68","gitBranch":"main"}"#;
    let sub = agent_session_with_subagents(tmp.path(), result);
    // Twelve rivals, each calling a tool named after its run, created out
    // of name order with the winner neither first nor last: a listing in
    // creation order (or its reverse) never agrees with name order, and a
    // hash-ordered one only by a one-in-twelve chance.
    for n in [7, 3, 0, 10, 5, 1, 11, 8, 2, 9, 4, 6] {
        let run = format!("run-{n:02}");
        write_child_run(&sub, &run, "call_agent1", &run);
    }
    // Sorts ahead of every rival, but its tool call is not in the turns.
    write_child_run(&sub, "a-elsewhere", "call_rewound_away", "Grep");

    let stats = agent_stats_of(tmp.path(), "call_agent1").expect("stats attached");
    assert_eq!(
        stats
            .tool_calls
            .iter()
            .map(|c| c.tool_name.as_str())
            .collect::<Vec<_>>(),
        vec!["run-00"]
    );
}

fn detail_builds_turns_with_tool_pairing(_root: &Path) {
    let tmp = reference_directory(_root, "detail_builds_turns_with_tool_pairing");
    let path = write_session(
        tmp.path(),
        "s1",
        &[
            RUNTIME_CONFIG,
            USER_LINE,
            THINKING_LINE,
            TOOL_USE_LINE,
            TOOL_RESULT_LINE,
            ANSWER_LINE,
        ],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    assert_eq!(detail.summary.id, "s1");
    assert_eq!(
        detail.transcript_watermark,
        Some(std::fs::metadata(&path).unwrap().len())
    );

    // One user turn, then TWO assistant turns: the thinking + tool_use
    // fragments share a `message.id` and merge into one message whose
    // paired tool result is absorbed, and the end-turn answer (which lands
    // after that tool-result boundary) forms the next turn.
    assert_eq!(detail.turns.len(), 3);
    assert!(matches!(detail.turns[0].role, TurnRole::User));
    assert!(matches!(detail.turns[1].role, TurnRole::Assistant));
    assert!(matches!(detail.turns[2].role, TurnRole::Assistant));

    let assistant = &detail.turns[1];
    let tool_use = assistant
        .blocks
        .iter()
        .find_map(|b| match b {
            ContentBlock::ToolUse {
                tool_use_id,
                tool_name,
                ..
            } => Some((tool_use_id.clone(), tool_name.clone())),
            _ => None,
        })
        .expect("tool_use block");
    assert_eq!(tool_use, (Some("call_1".to_string()), "Read".to_string()));
    let tool_result = assistant
        .blocks
        .iter()
        .find_map(|b| match b {
            ContentBlock::ToolResult {
                tool_use_id,
                output_preview,
                ..
            } => Some((tool_use_id.clone(), output_preview.clone())),
            _ => None,
        })
        .expect("tool_result block paired into the turn");
    assert_eq!(tool_result.0.as_deref(), Some("call_1"));
    assert!(tool_result.1.as_deref().unwrap().contains("42"));
    assert!(
        assistant.blocks.iter().any(
            |b| matches!(b, ContentBlock::Thinking { text } if text.contains("read the file"))
        )
    );
    assert!(
        assistant
            .blocks
            .iter()
            .all(|b| !matches!(b, ContentBlock::Text { text } if text == "42"))
    );
    let answer = &detail.turns[2];
    assert!(
        answer
            .blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::Text { text } if text == "42"))
    );

    // All three fragments repeat ONE response's usage. Exactly one bubble
    // may keep it, or the session totals bill the response twice.
    let charged: Vec<u64> = detail
        .turns
        .iter()
        .filter_map(|t| t.usage.as_ref().map(|u| u.output_tokens))
        .collect();
    assert_eq!(charged, vec![2], "one claim per message.id");
    // Prompt occupancy excludes `output_tokens`, and the window comes from
    // `runtime-config` (no heuristic can size `qmodel_38max`).
    //
    // 10, not 15: the fixture's `input_tokens: 10` already CONTAINS its
    // `cache_read_input_tokens: 5` (Qoder's counters are OpenAI-shaped —
    // see `qoder_turn_usage`), so the prompt is ten tokens of which five
    // came from cache, not fifteen.
    let stats = detail.session_stats.expect("session stats");
    assert_eq!(stats.context_window_used_tokens, Some(10));
    assert_eq!(stats.context_window_max_tokens, Some(262_144));
}

fn parallel_tool_results_all_survive_the_branch_walk(_root: &Path) {
    let tmp = reference_directory(_root, "parallel_tool_results_all_survive_the_branch_walk");
    let call_1 = r#"{"type":"assistant","uuid":"p1","timestamp":"2026-08-16T15:45:33.000Z","message":{"id":"cc-par","role":"assistant","model":"qmodel_38max","content":[{"type":"tool_use","id":"call_1","name":"Read","input":{"file_path":"/private/tmp/probe/A.md"}}]},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    let call_2 = r#"{"type":"assistant","uuid":"p2","timestamp":"2026-08-16T15:45:33.010Z","message":{"id":"cc-par","role":"assistant","model":"qmodel_38max","content":[{"type":"tool_use","id":"call_2","name":"Read","input":{"file_path":"/private/tmp/probe/B.md"}}]},"parentUuid":"p1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    let result_1 = r#"{"type":"user","uuid":"r1","timestamp":"2026-08-16T15:45:33.100Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_1","content":"ALPHA"}]},"sourceToolAssistantUUID":"p1","parentUuid":"p1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    let result_2 = r#"{"type":"user","uuid":"r2","timestamp":"2026-08-16T15:45:33.110Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_2","content":"BETA"}]},"sourceToolAssistantUUID":"p2","parentUuid":"p2","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    let answer = r#"{"type":"assistant","uuid":"p3","timestamp":"2026-08-16T15:45:34.000Z","message":{"id":"cc-ans","role":"assistant","model":"qmodel_38max","content":[{"type":"text","text":"read both"}]},"parentUuid":"r2","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, call_1, call_2, result_1, result_2, answer],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let results: Vec<String> = detail
        .turns
        .iter()
        .flat_map(|turn| turn.blocks.iter())
        .filter_map(|block| match block {
            ContentBlock::ToolResult {
                output_preview: Some(output),
                ..
            } => Some(output.clone()),
            _ => None,
        })
        .collect();
    assert!(
        results.iter().any(|c| c.contains("ALPHA")),
        "the FIRST call's result is the one a pure ancestor walk loses: {results:?}"
    );
    assert!(
        results.iter().any(|c| c.contains("BETA")),
        "the last call's result must still be there: {results:?}"
    );
}

fn reattaching_tool_results_does_not_resurrect_a_rewound_branch(_root: &Path) {
    let tmp = reference_directory(
        _root,
        "reattaching_tool_results_does_not_resurrect_a_rewound_branch",
    );
    let abandoned_call = r#"{"type":"assistant","uuid":"x1","timestamp":"2026-08-16T15:45:30.000Z","message":{"id":"cc-x","role":"assistant","model":"qmodel_38max","content":[{"type":"tool_use","id":"call_x","name":"Read","input":{"file_path":"/private/tmp/probe/X.md"}}]},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    let abandoned_result = r#"{"type":"user","uuid":"x2","timestamp":"2026-08-16T15:45:30.100Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_x","content":"ABANDONED-RESULT"}]},"sourceToolAssistantUUID":"x1","parentUuid":"x1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    let retried = r#"{"type":"assistant","uuid":"y1","timestamp":"2026-08-16T15:45:40.000Z","message":{"id":"cc-y","role":"assistant","model":"qmodel_38max","content":[{"type":"text","text":"KEPT"}]},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, abandoned_call, abandoned_result, retried],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let rendered = format!("{:?}", detail.turns);
    assert!(rendered.contains("KEPT"));
    assert!(
        !rendered.contains("ABANDONED-RESULT"),
        "the abandoned branch's issuer is off-chain, so its result stays off too"
    );
}

fn sidechain_entries_are_excluded(_root: &Path) {
    let tmp = reference_directory(_root, "sidechain_entries_are_excluded");
    write_session(tmp.path(), "s1", &[USER_LINE, ANSWER_LINE, SIDECHAIN_LINE]);

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    assert_eq!(detail.summary.message_count, 2);
    assert!(detail.turns.iter().all(|t| {
        t.blocks
            .iter()
            .all(|b| !matches!(b, ContentBlock::Text { text } if text.contains("sub-agent")))
    }));
}

fn tail_metadata_records_do_not_extend_timestamps(_root: &Path) {
    let tmp = reference_directory(_root, "tail_metadata_records_do_not_extend_timestamps");
    write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, ANSWER_LINE, ACTIVE_LEAF_LINE, RUNTIME_CONFIG],
    );

    let summaries = parser_in(tmp.path()).list_conversations().unwrap();
    // `active-leaf` carries a LATER epoch than any content record; it must
    // not leak into `ended_at`.
    assert_eq!(
        summaries[0].ended_at.as_ref().map(|t| t.to_rfc3339()),
        Some("2026-08-16T15:45:34+00:00".to_string())
    );
}

fn a_null_active_leaf_clears_the_conversation(_root: &Path) {
    let tmp = reference_directory(_root, "a_null_active_leaf_clears_the_conversation");
    let cleared = r#"{"type":"active-leaf","sessionId":"s1","leafUuid":null,"explicit":true,"rewound":true,"timestamp":1786895140000}"#;
    let path = write_session(tmp.path(), "s1", &[USER_LINE, ANSWER_LINE, cleared]);

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    assert!(
        detail.turns.is_empty(),
        "a null leaf is an EMPTY branch, not a broken marker: {:?}",
        detail.turns
    );
    assert_eq!(detail.summary.message_count, 0);
    // The consequence, named on purpose: with no content record left there
    // is no `started_at`, so a cleared session drops out of the SCAN list —
    // the same path every content-free transcript already takes. It is not a
    // deletion: an already-imported session keeps its row in codeg's DB, and
    // the file itself is untouched.
    assert!(parse_summary(&path).is_none());
}

fn a_null_active_leaf_never_hides_newer_or_malformed_state(_root: &Path) {
    let tmp = reference_directory(
        _root,
        "a_null_active_leaf_never_hides_newer_or_malformed_state",
    );
    let cleared = r#"{"type":"active-leaf","sessionId":"s1","leafUuid":null,"explicit":true,"rewound":true,"timestamp":1786895140000}"#;
    // Cleared, then the user typed again: the fresh turn must survive.
    write_session(tmp.path(), "s1", &[USER_LINE, cleared, ANSWER_LINE]);
    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    assert!(
        !detail.turns.is_empty(),
        "a stale null marker must not erase what came after it"
    );

    // A marker with no `leafUuid` at all is a writer this parser does not
    // understand — fall through to showing history, never to hiding it.
    let malformed =
        r#"{"type":"active-leaf","sessionId":"s1","explicit":true,"timestamp":1786895140000}"#;
    write_session(tmp.path(), "s2", &[USER_LINE, ANSWER_LINE, malformed]);
    let detail = parser_in(tmp.path()).get_conversation("s2").unwrap();
    assert!(
        !detail.turns.is_empty(),
        "missing key is not an empty branch"
    );
}

fn acp_block_array_prompt_renders_and_titles(_root: &Path) {
    let tmp = reference_directory(_root, "acp_block_array_prompt_renders_and_titles");
    write_session(tmp.path(), "s2", &[ACP_USER_LINE, ACP_ERROR_LINE]);

    let summary = &parser_in(tmp.path()).list_conversations().unwrap()[0];
    assert_eq!(summary.title.as_deref(), Some("hi"));
    assert_eq!(summary.message_count, 1, "the error record is not a reply");
    // `<synthetic>` is a placeholder, never a model the session ran on.
    assert_eq!(summary.model, None);

    let detail = parser_in(tmp.path()).get_conversation("s2").unwrap();
    assert_eq!(detail.turns.len(), 2);
    assert!(matches!(detail.turns[0].role, TurnRole::User));
    assert_eq!(text_of(&detail.turns[0]), "hi");
    assert!(
        !detail
            .turns
            .iter()
            .any(|t| text_of(t).contains("pricingUrl")),
        "a failed API turn must not render as the assistant's answer"
    );
    // It closes the round as the failure line instead.
    assert!(matches!(detail.turns[1].role, TurnRole::System));
    assert!(
        matches!(
            detail.turns[1].blocks.as_slice(),
            [ContentBlock::TurnError { message }]
                if message == r#"{"pricingUrl":"https://qoder.com/pricing?client=qoder"}"#
        ),
        "{:?}",
        detail.turns[1].blocks
    );
}

fn image_prompts_survive(_root: &Path) {
    let tmp = reference_directory(_root, "image_prompts_survive");
    let line = r#"{"type":"user","uuid":"iu1","timestamp":"2026-08-16T15:45:27.384Z","message":{"role":"user","content":[{"type":"text","text":"what is this"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"iVBORw0KGgo="}}]},"origin":{"kind":"human"},"parentUuid":null,"isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s3"}"#;
    write_session(tmp.path(), "s3", &[line]);

    let detail = parser_in(tmp.path()).get_conversation("s3").unwrap();
    assert_eq!(detail.turns.len(), 1);
    assert!(
        detail.turns[0].blocks.iter().any(
            |b| matches!(b, ContentBlock::Image { mime_type, .. } if mime_type == "image/png")
        )
    );
    assert_eq!(detail.summary.title.as_deref(), Some("what is this"));
}

fn meta_injections_never_render_as_user_messages(_root: &Path) {
    let tmp = reference_directory(_root, "meta_injections_never_render_as_user_messages");
    let meta = r#"{"type":"user","uuid":"m1","timestamp":"2026-08-16T15:45:20.000Z","message":{"role":"user","content":[{"type":"text","text":"Continue from where you left off"}]},"isMeta":true,"origin":{"kind":"human"},"parentUuid":null,"isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    write_session(tmp.path(), "s1", &[meta, USER_LINE, ANSWER_LINE]);

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    assert!(
        !detail
            .turns
            .iter()
            .any(|t| text_of(t).contains("Continue from where")),
        "meta injections are addressed to the model"
    );
    assert_eq!(
        detail.summary.title.as_deref(),
        Some("read NOTES.md and reply")
    );
}

fn rewound_branch_is_not_replayed(_root: &Path) {
    let tmp = reference_directory(_root, "rewound_branch_is_not_replayed");
    let abandoned = r#"{"type":"assistant","uuid":"old","timestamp":"2026-08-16T15:45:30.000Z","message":{"id":"chatcmpl-old","role":"assistant","model":"qmodel_38max","content":[{"type":"text","text":"ABANDONED"}],"usage":{"input_tokens":900,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":900}},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    let retried = r#"{"type":"assistant","uuid":"new","timestamp":"2026-08-16T15:45:40.000Z","message":{"id":"chatcmpl-new","role":"assistant","model":"qmodel_38max","content":[{"type":"text","text":"KEPT"}],"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":1}},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    write_session(tmp.path(), "s1", &[USER_LINE, abandoned, retried]);

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let rendered: Vec<String> = detail.turns.iter().map(text_of).collect();
    assert!(rendered.iter().any(|t| t == "KEPT"));
    assert!(
        !rendered.iter().any(|t| t.contains("ABANDONED")),
        "rewound branch must not be replayed: {rendered:?}"
    );
    assert_eq!(detail.summary.message_count, 2);
}

fn active_leaf_marker_selects_the_live_branch(_root: &Path) {
    let tmp = reference_directory(_root, "active_leaf_marker_selects_the_live_branch");
    let branch_a = r#"{"type":"assistant","uuid":"ba","timestamp":"2026-08-16T15:45:30.000Z","message":{"id":"cc-a","role":"assistant","model":"qmodel_38max","content":[{"type":"text","text":"BRANCH-A"}]},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    let branch_b = r#"{"type":"assistant","uuid":"bb","timestamp":"2026-08-16T15:45:31.000Z","message":{"id":"cc-b","role":"assistant","model":"qmodel_38max","content":[{"type":"text","text":"BRANCH-B"}]},"parentUuid":"u1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    let leaf_a = r#"{"type":"active-leaf","sessionId":"s1","leafUuid":"ba","explicit":true,"rewound":true,"timestamp":1786895140000}"#;
    write_session(tmp.path(), "s1", &[USER_LINE, branch_a, branch_b, leaf_a]);

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let rendered: Vec<String> = detail.turns.iter().map(text_of).collect();
    assert!(rendered.iter().any(|t| t == "BRANCH-A"));
    assert!(!rendered.iter().any(|t| t == "BRANCH-B"));
}

fn stale_active_leaf_does_not_hide_newer_messages(_root: &Path) {
    let tmp = reference_directory(_root, "stale_active_leaf_does_not_hide_newer_messages");
    let leaf_u1 = r#"{"type":"active-leaf","sessionId":"s1","leafUuid":"u1","explicit":false,"timestamp":1786895127400}"#;
    write_session(tmp.path(), "s1", &[USER_LINE, leaf_u1, ANSWER_LINE]);

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    assert_eq!(detail.turns.len(), 2);
    assert!(detail.turns.iter().any(|t| text_of(t) == "42"));
}

fn unlinked_records_fall_back_to_append_order(_root: &Path) {
    let tmp = reference_directory(_root, "unlinked_records_fall_back_to_append_order");
    let a = r#"{"type":"user","uuid":"x1","timestamp":"2026-08-16T15:45:27.000Z","message":{"role":"user","content":"first"},"origin":{"kind":"human"},"isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s4"}"#;
    let b = r#"{"type":"assistant","uuid":"x2","timestamp":"2026-08-16T15:45:28.000Z","message":{"id":"cc-1","role":"assistant","model":"m","content":[{"type":"text","text":"second"}]},"isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s4"}"#;
    let c = r#"{"type":"user","uuid":"x3","timestamp":"2026-08-16T15:45:29.000Z","message":{"role":"user","content":"third"},"origin":{"kind":"human"},"isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s4"}"#;
    write_session(tmp.path(), "s4", &[a, b, c]);

    let detail = parser_in(tmp.path()).get_conversation("s4").unwrap();
    assert_eq!(detail.turns.len(), 3);
    assert_eq!(detail.summary.title.as_deref(), Some("first"));
}

fn title_records_outrank_the_first_prompt(_root: &Path) {
    let tmp = reference_directory(_root, "title_records_outrank_the_first_prompt");
    let ai = r#"{"type":"ai-title","sessionId":"s1","aiTitle":"Reading project notes"}"#;
    let custom = r#"{"type":"custom-title","sessionId":"s1","customTitle":"notes probe"}"#;

    write_session(tmp.path(), "s1", &[USER_LINE, ANSWER_LINE, ai]);
    assert_eq!(
        parser_in(tmp.path()).list_conversations().unwrap()[0]
            .title
            .as_deref(),
        Some("Reading project notes")
    );

    let tmp2 = reference_directory(_root, "title_records_outrank_the_first_prompt");
    write_session(tmp2.path(), "s1", &[USER_LINE, ANSWER_LINE, ai, custom]);
    let summary_title = parser_in(tmp2.path()).list_conversations().unwrap()[0]
        .title
        .clone();
    assert_eq!(summary_title.as_deref(), Some("notes probe"));
    // Summary and detail MUST agree or the auto-title backfill oscillates.
    assert_eq!(
        parser_in(tmp2.path())
            .get_conversation("s1")
            .unwrap()
            .summary
            .title,
        summary_title
    );
}

fn slash_commands_render_as_the_invocation(_root: &Path) {
    let tmp = reference_directory(_root, "slash_commands_render_as_the_invocation");
    let cmd = r#"{"type":"user","uuid":"c1","timestamp":"2026-08-16T15:45:27.000Z","message":{"role":"user","content":"<command-message>init</command-message>\n<command-name>/init</command-name>\n<command-args>--force</command-args>"},"origin":{"kind":"human"},"parentUuid":null,"isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s5"}"#;
    write_session(tmp.path(), "s5", &[cmd]);

    let detail = parser_in(tmp.path()).get_conversation("s5").unwrap();
    assert_eq!(detail.turns.len(), 1, "the record must not be dropped");
    assert_eq!(text_of(&detail.turns[0]), "/init --force");
    assert_eq!(detail.summary.title.as_deref(), Some("/init --force"));
}

fn slash_commands_render_from_block_arrays_too(_root: &Path) {
    let tmp = reference_directory(_root, "slash_commands_render_from_block_arrays_too");
    let cmd = r#"{"type":"user","uuid":"c2","timestamp":"2026-08-16T15:45:27.000Z","message":{"role":"user","content":[{"type":"text","text":"<command-name>/compact</command-name>"}]},"origin":{"kind":"human"},"parentUuid":null,"isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s6"}"#;
    write_session(tmp.path(), "s6", &[cmd]);

    let detail = parser_in(tmp.path()).get_conversation("s6").unwrap();
    assert_eq!(text_of(&detail.turns[0]), "/compact");
}

fn a_prompt_without_a_command_name_is_left_alone(_root: &Path) {
    let tmp = reference_directory(_root, "a_prompt_without_a_command_name_is_left_alone");
    let line = r#"{"type":"user","uuid":"c3","timestamp":"2026-08-16T15:45:27.000Z","message":{"role":"user","content":"explain <command-args>foo</command-args> please"},"origin":{"kind":"human"},"parentUuid":null,"isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s7"}"#;
    write_session(tmp.path(), "s7", &[line]);

    let detail = parser_in(tmp.path()).get_conversation("s7").unwrap();
    assert_eq!(text_of(&detail.turns[0]), "explain  please");
}

fn compaction_boundary_does_not_hide_earlier_history(_root: &Path) {
    let tmp = reference_directory(_root, "compaction_boundary_does_not_hide_earlier_history");
    let boundary = r#"{"type":"system","uuid":"cb1","subtype":"compact_boundary","timestamp":"2026-08-16T15:45:35.000Z","parentUuid":null,"logicalParentUuid":"a3","requiresActiveLeafCommit":true,"isSidechain":false,"sessionId":"s1"}"#;
    let summary = r#"{"type":"user","uuid":"cs1","timestamp":"2026-08-16T15:45:36.000Z","message":{"role":"user","content":[{"type":"text","text":"SUMMARY OF EARLIER WORK"}]},"isCompactSummary":true,"parentUuid":"cb1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    let after = r#"{"type":"assistant","uuid":"post","timestamp":"2026-08-16T15:45:37.000Z","message":{"id":"cc-post","role":"assistant","model":"qmodel_38max","content":[{"type":"text","text":"AFTER COMPACTION"}]},"parentUuid":"cs1","isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    let leaf = r#"{"type":"active-leaf","sessionId":"s1","leafUuid":"post","explicit":false,"timestamp":1786895137000}"#;
    write_session(
        tmp.path(),
        "s1",
        &[
            RUNTIME_CONFIG,
            USER_LINE,
            THINKING_LINE,
            TOOL_USE_LINE,
            TOOL_RESULT_LINE,
            ANSWER_LINE,
            boundary,
            summary,
            after,
            leaf,
        ],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let rendered: Vec<String> = detail.turns.iter().map(text_of).collect();
    assert!(
        rendered.iter().any(|t| t.contains("read NOTES.md")),
        "pre-compaction history must survive: {rendered:?}"
    );
    assert!(rendered.iter().any(|t| t == "42"));
    assert!(rendered.iter().any(|t| t == "AFTER COMPACTION"));

    // The summary is content worth seeing, but it is not something the user
    // said — so it is a system turn, and it never becomes the title.
    let summary_turn = detail
        .turns
        .iter()
        .find(|t| text_of(t).contains("SUMMARY OF EARLIER WORK"))
        .expect("summary rendered");
    assert!(matches!(summary_turn.role, TurnRole::System));
    assert_eq!(
        detail.summary.title.as_deref(),
        Some("read NOTES.md and reply")
    );
}

fn a_leading_compaction_summary_is_never_the_title(_root: &Path) {
    let tmp = reference_directory(_root, "a_leading_compaction_summary_is_never_the_title");
    let summary = r#"{"type":"user","uuid":"cs2","timestamp":"2026-08-16T15:45:20.000Z","message":{"role":"user","content":[{"type":"text","text":"SUMMARY OF EARLIER WORK"}]},"isCompactSummary":true,"parentUuid":null,"isSidechain":false,"cwd":"/private/tmp/probe","sessionId":"s1"}"#;
    write_session(tmp.path(), "s1", &[summary, USER_LINE, ANSWER_LINE]);

    let summaries = parser_in(tmp.path()).list_conversations().unwrap();
    assert_eq!(
        summaries[0].title.as_deref(),
        Some("read NOTES.md and reply")
    );
}

fn token_stats_beat_reconstructed_usage(_root: &Path) {
    let tmp = reference_directory(_root, "token_stats_beat_reconstructed_usage");
    let stats = r#"{"type":"token-stats","sessionId":"s1","promptTokenCount":4242,"timestamp":1786895134000}"#;
    write_session(
        tmp.path(),
        "s1",
        &[RUNTIME_CONFIG, USER_LINE, ANSWER_LINE, stats],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let s = detail.session_stats.expect("session stats");
    assert_eq!(s.context_window_used_tokens, Some(4242));
    assert_eq!(s.context_window_max_tokens, Some(262_144));
}

fn redacted_usage_still_yields_a_context_percentage(_root: &Path) {
    let tmp = reference_directory(_root, "redacted_usage_still_yields_a_context_percentage");
    write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, &answer_with_usage("a9", "u1", REDACTED)],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let s = detail.session_stats.expect("session stats");
    let percent = s.context_window_usage_percent.expect("percent");
    assert!((percent - 16.721_666_666_666_668).abs() < 1e-9, "{percent}");
    // Nothing to divide and nothing to invert, so neither figure is
    // invented — the gauge shows a percentage with no "x / y".
    assert_eq!(s.context_window_used_tokens, None);
    assert_eq!(s.context_window_max_tokens, None);
}

fn exposed_usage_inverts_the_ratio_into_the_window(_root: &Path) {
    let tmp = reference_directory(_root, "exposed_usage_inverts_the_ratio_into_the_window");
    write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, &answer_with_usage("a9", "u1", EXPOSED)],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let s = detail.session_stats.expect("session stats");
    assert_eq!(s.context_window_used_tokens, Some(2803));
    assert_eq!(s.context_window_max_tokens, Some(180_000));
    let percent = s.context_window_usage_percent.expect("percent");
    assert!(
        (percent - 1.557_222_222_222_222_2).abs() < 1e-9,
        "{percent}"
    );
}

fn the_newest_reported_ratio_wins(_root: &Path) {
    let tmp = reference_directory(_root, "the_newest_reported_ratio_wins");
    write_session(
        tmp.path(),
        "s1",
        &[
            USER_LINE,
            &answer_with_usage("a8", "u1", REDACTED),
            &answer_with_usage("a9", "a8", EXPOSED),
        ],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let s = detail.session_stats.expect("session stats");
    let percent = s.context_window_usage_percent.expect("percent");
    assert!(
        (percent - 1.557_222_222_222_222_2).abs() < 1e-9,
        "{percent}"
    );
}

fn a_rewound_branch_does_not_supply_the_ratio(_root: &Path) {
    let tmp = reference_directory(_root, "a_rewound_branch_does_not_supply_the_ratio");
    let leaf = r#"{"type":"active-leaf","sessionId":"s1","leafUuid":"a9","explicit":true,"rewound":true,"timestamp":1789815619554}"#;
    write_session(
        tmp.path(),
        "s1",
        &[
            USER_LINE,
            &answer_with_usage("a9", "u1", EXPOSED),
            &answer_with_usage("a8", "u1", REDACTED),
            leaf,
        ],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let s = detail.session_stats.expect("session stats");
    let percent = s.context_window_usage_percent.expect("percent");
    assert!(
        (percent - 1.557_222_222_222_222_2).abs() < 1e-9,
        "{percent}"
    );
    assert_eq!(s.context_window_max_tokens, Some(180_000));
}

fn runtime_config_window_beats_the_inverted_one(_root: &Path) {
    let tmp = reference_directory(_root, "runtime_config_window_beats_the_inverted_one");
    write_session(
        tmp.path(),
        "s1",
        &[
            RUNTIME_CONFIG,
            USER_LINE,
            &answer_with_usage("a9", "u1", EXPOSED),
        ],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let s = detail.session_stats.expect("session stats");
    assert_eq!(s.context_window_max_tokens, Some(262_144));
    // ...but the percentage is still Qoder's own, not 2803/262144.
    let percent = s.context_window_usage_percent.expect("percent");
    assert!(
        (percent - 1.557_222_222_222_222_2).abs() < 1e-9,
        "{percent}"
    );
}

fn a_saturated_ratio_names_no_window(_root: &Path) {
    let tmp = reference_directory(_root, "a_saturated_ratio_names_no_window");
    let full = r#"{"input_tokens":190000,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":5,"context_usage_ratio":1}"#;
    write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, &answer_with_usage("a9", "u1", full)],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let s = detail.session_stats.expect("session stats");
    assert_eq!(s.context_window_max_tokens, None);
    assert_eq!(s.context_window_usage_percent, Some(100.0));
}

fn the_gauge_does_not_count_the_cached_prefix_twice(_root: &Path) {
    let tmp = reference_directory(_root, "the_gauge_does_not_count_the_cached_prefix_twice");
    let cached = r#"{"input_tokens":2803,"cache_creation_input_tokens":100,"cache_read_input_tokens":900,"output_tokens":19,"context_usage_ratio":0.015572222222222222}"#;
    write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, &answer_with_usage("a9", "u1", cached)],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let s = detail.session_stats.expect("session stats");
    assert_eq!(s.context_window_max_tokens, Some(180_000));
    // The whole prompt, cached part included exactly once — and therefore
    // the very numerator Qoder divided to get its ratio.
    assert_eq!(s.context_window_used_tokens, Some(2803));
    let percent = s.context_window_usage_percent.expect("percent");
    assert!(
        (percent - 1.557_222_222_222_222_2).abs() < 1e-9,
        "{percent}"
    );
    // The stated percentage and the used/max pair now tell the same story.
    let recomputed = 2803.0 / 180_000.0 * 100.0;
    assert!(
        (percent - recomputed).abs() < 1e-6,
        "{percent} vs {recomputed}"
    );

    // The split is a re-labelling, not a deduction: the turn's own counters
    // still add back up to the prompt Qoder charged for.
    let usage = detail.turns[1].usage.as_ref().expect("usage");
    assert_eq!(usage.input_tokens, 1803);
    assert_eq!(usage.cache_read_input_tokens, 900);
    assert_eq!(usage.cache_creation_input_tokens, 100);
    assert_eq!(
        usage.input_tokens + usage.cache_read_input_tokens + usage.cache_creation_input_tokens,
        2803
    );
}

fn an_absurd_ratio_names_no_window(_root: &Path) {
    let tmp = reference_directory(_root, "an_absurd_ratio_names_no_window");
    let absurd = r#"{"input_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":1,"context_usage_ratio":5e-20}"#;
    write_session(
        tmp.path(),
        "s1",
        &[USER_LINE, &answer_with_usage("a9", "u1", absurd)],
    );

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let s = detail.session_stats.expect("session stats");
    assert_eq!(s.context_window_max_tokens, None);
    // The occupancy it stated is still its own statement, and still shown.
    let percent = s.context_window_usage_percent.expect("percent");
    assert!((percent - 5e-18).abs() < 1e-24, "{percent}");
}

fn a_transcript_without_a_ratio_falls_back_to_the_counters(_root: &Path) {
    let tmp = reference_directory(
        _root,
        "a_transcript_without_a_ratio_falls_back_to_the_counters",
    );
    write_session(tmp.path(), "s1", &[RUNTIME_CONFIG, USER_LINE, ANSWER_LINE]);

    let detail = parser_in(tmp.path()).get_conversation("s1").unwrap();
    let s = detail.session_stats.expect("session stats");
    assert_eq!(s.context_window_used_tokens, Some(10));
    assert_eq!(s.context_window_max_tokens, Some(262_144));
    let percent = s.context_window_usage_percent.expect("percent");
    assert!(
        (percent - (10.0 / 262_144.0) * 100.0).abs() < 1e-9,
        "{percent}"
    );
}

pub(in crate::runtime_subagent_session) fn assert_original_cases(root: &Path) {
    summary_model_follows_a_mid_session_switch(root);
    summary_model_ignores_a_blank_later_model(root);
    a_blank_model_on_a_later_fragment_keeps_the_bubbles_model(root);
    sub_agent_usage_folds_from_its_transcript_with_cache_split(root);
    sub_agent_records_without_a_message_id_each_count(root);
    usage_on_the_last_fragment_only_counts_once(root);
    sub_agent_calls_land_on_the_turn_current_when_each_happened(root);
    sub_agent_usage_does_not_touch_the_context_window(root);
    agent_card_stats_carry_the_childs_mixed_timeline_in_arrival_order(root);
    an_errored_agent_result_still_carries_stats_and_the_recovered_result(root);
    a_task_file_without_a_result_field_leaves_recovered_result_absent(root);
    an_interrupted_tasks_placeholder_result_rides_along_with_its_status(root);
    the_first_run_in_name_order_wins_a_shared_tool_call(root);
    detail_builds_turns_with_tool_pairing(root);
    parallel_tool_results_all_survive_the_branch_walk(root);
    reattaching_tool_results_does_not_resurrect_a_rewound_branch(root);
    sidechain_entries_are_excluded(root);
    tail_metadata_records_do_not_extend_timestamps(root);
    a_null_active_leaf_clears_the_conversation(root);
    a_null_active_leaf_never_hides_newer_or_malformed_state(root);
    acp_block_array_prompt_renders_and_titles(root);
    image_prompts_survive(root);
    meta_injections_never_render_as_user_messages(root);
    rewound_branch_is_not_replayed(root);
    active_leaf_marker_selects_the_live_branch(root);
    stale_active_leaf_does_not_hide_newer_messages(root);
    unlinked_records_fall_back_to_append_order(root);
    title_records_outrank_the_first_prompt(root);
    slash_commands_render_as_the_invocation(root);
    slash_commands_render_from_block_arrays_too(root);
    a_prompt_without_a_command_name_is_left_alone(root);
    compaction_boundary_does_not_hide_earlier_history(root);
    a_leading_compaction_summary_is_never_the_title(root);
    token_stats_beat_reconstructed_usage(root);
    redacted_usage_still_yields_a_context_percentage(root);
    exposed_usage_inverts_the_ratio_into_the_window(root);
    the_newest_reported_ratio_wins(root);
    a_rewound_branch_does_not_supply_the_ratio(root);
    runtime_config_window_beats_the_inverted_one(root);
    a_saturated_ratio_names_no_window(root);
    the_gauge_does_not_count_the_cached_prefix_twice(root);
    an_absurd_ratio_names_no_window(root);
    a_transcript_without_a_ratio_falls_back_to_the_counters(root);
}
