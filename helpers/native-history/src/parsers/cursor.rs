use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::models::{
    AgentType, ContentBlock, ConversationDetail, ConversationSummary, MessageTurn, TurnRole,
};
use crate::parsers::{
    backfill_turn_durations, compute_session_stats, folder_name_from_path,
    merge_context_window_stats, relocate_orphaned_tool_results, structurize_read_tool_output,
    title_from_user_text, truncate_str, AgentParser, ParseError,
};

/// Caps mirroring the Grok parser: bound a single tool output / input preview
/// so one noisy command can't bloat a conversation detail payload.
const CURSOR_TOOL_OUTPUT_CAP: usize = 100_000;
const CURSOR_TOOL_INPUT_CAP: usize = 8_000;

/// Resolve Cursor's config home, mirroring the CLI's own chain:
/// `CURSOR_CONFIG_DIR`, else `$XDG_CONFIG_HOME/cursor`, else `~/.cursor`.
/// The CLI's chat store (`chats/`) and `cli-config.json` live under this path;
/// the user-level `mcp.json` does NOT (the CLI hardcodes `~/.cursor/mcp.json`).
pub(crate) fn resolve_cursor_config_dir() -> PathBuf {
    resolve_cursor_config_from(
        std::env::var_os("CURSOR_CONFIG_DIR"),
        std::env::var_os("XDG_CONFIG_HOME"),
        dirs::home_dir(),
    )
}

fn resolve_cursor_config_from(
    config_env: Option<OsString>,
    xdg_env: Option<OsString>,
    home_dir: Option<PathBuf>,
) -> PathBuf {
    if let Some(dir) = config_env.filter(|value| !value.is_empty()) {
        return PathBuf::from(dir);
    }
    if let Some(xdg) = xdg_env.filter(|value| !value.is_empty()) {
        return PathBuf::from(xdg).join("cursor");
    }
    home_dir.unwrap_or_default().join(".cursor")
}

/// Cursor (cursor-agent CLI) stores each conversation as a **SQLite blob
/// store**. Interactive CLI/IDE chats — and the sub-agent children spawned by
/// `task` tool calls — are grouped by the MD5 of the resolved working
/// directory, while ACP sessions (what codeg drives) live under a flat
/// per-session root with a JSON sidecar:
///
/// ```text
/// $CURSOR_CONFIG_DIR/                (default ~/.cursor)
/// ├── chats/
/// │   └── <md5-hex-of-cwd>/          # createHash("md5").update(resolve(cwd))
/// │       └── <chat-uuid>/
/// │           └── store.db           # blobs(id TEXT, data BLOB) + meta(key, value)
/// └── acp-sessions/
///     └── <session-uuid>/
///         ├── meta.json              # {schemaVersion, cwd, title}
///         └── store.db               # same blob-store format
/// ```
///
/// `meta` key `"0"` holds the chat metadata as **hex-encoded UTF-8 JSON**:
/// `{agentId, latestRootBlobId (hex), name, createdAt, mode, isRunEverything,
/// approvalMode, lastUsedModel, subagentInfo, …}` (legacy `mode:"auto-run"`
/// maps to `default`). `blobs` is a content-addressed DAG of protobuf
/// (`agent.v1.*`) messages, ids stored as lowercase hex:
///
/// ```text
/// latestRootBlobId → ConversationStateStructure
///   .turns[] (blob ids) → ConversationTurnStructure
///     .agent_conversation_turn:
///        .user_message (blob id) → UserMessage{text, rich_text, text_blob_id}
///        .steps[] (blob ids) → ConversationStep (oneof assistant_message /
///                              thinking_message / tool_call{ToolCall oneof})
///     .shell_conversation_turn:  user-typed `!` commands
///        .shell_command / .shell_output (blob ids)
///   .token_details {used_tokens, max_tokens}
///   .turn_timings[] {duration_ms, timestamp_ms}   (aligned with .turns)
///   .tracked_git_repo_branches[] {repo_path, branch_name}
/// ```
///
/// The schema was extracted from the CLI's own protobuf-es runtime field
/// tables (2026.07.16 bundle); every decode below is defensive — unknown
/// fields are skipped and unknown tool variants degrade to a generic tool
/// card — so an upstream schema drift renders less detail instead of failing.
pub struct CursorParser {
    chats_dir: PathBuf,
    acp_sessions_dir: PathBuf,
    /// Root of codeg's own per-turn timing journal (`crate::turn_timings`) —
    /// the fallback clock for turns whose store yields none (thinking+text
    /// only; the ACP store has no message timestamps).
    turn_timings_root: PathBuf,
}

impl CursorParser {
    pub fn new() -> Self {
        let config_dir = resolve_cursor_config_dir();
        Self {
            chats_dir: config_dir.join("chats"),
            acp_sessions_dir: config_dir.join("acp-sessions"),
            turn_timings_root: crate::paths::codeg_turn_timings_root(),
        }
    }

    /// Construct a parser pointed at an explicit `chats` directory (tests).
    /// The ACP-session root is derived as its `acp-sessions` sibling, and the
    /// timing journal as its `turn-timings` sibling.
    pub fn with_base_dir(chats_dir: PathBuf) -> Self {
        let sibling = |name: &str| {
            chats_dir
                .parent()
                .map(|p| p.join(name))
                .unwrap_or_else(|| PathBuf::from(name))
        };
        let acp_sessions_dir = sibling("acp-sessions");
        let turn_timings_root = sibling("turn-timings");
        Self {
            chats_dir,
            acp_sessions_dir,
            turn_timings_root,
        }
    }

    fn build_summary(&self, chat_dir: &Path, chat_id: &str) -> Option<ConversationSummary> {
        let conn = open_store(&chat_dir.join("store.db"))?;
        let meta = read_chat_meta(&conn)?;
        // Sub-agent transcripts are children of a main chat; only top-level
        // chats are listed (mirrors the other parsers).
        if meta.is_subagent {
            return None;
        }
        let state = meta
            .latest_root_blob_id
            .as_deref()
            .and_then(|id| read_blob(&conn, id))
            .map(|bytes| decode_state(&bytes))?;
        // A chat that never got a turn (e.g. `create-chat`, or an ACP session
        // opened without a prompt) is metadata-only — not listed.
        if state.turn_blob_ids.is_empty() {
            return None;
        }
        let sidecar = read_sidecar_meta(chat_dir);
        let title = meta
            .name
            .clone()
            .or_else(|| sidecar.title.clone())
            .or_else(|| {
                first_user_text(&conn, &state)
                    .as_deref()
                    .map(title_from_user_text)
            });
        Some(summary_from(chat_id, &meta, &state, &sidecar, title))
    }

    fn build_detail(&self, chat_dir: &Path, chat_id: &str) -> Option<ConversationDetail> {
        let conn = open_store(&chat_dir.join("store.db"))?;
        let meta = read_chat_meta(&conn)?;
        let state = meta
            .latest_root_blob_id
            .as_deref()
            .and_then(|id| read_blob(&conn, id))
            .map(|bytes| decode_state(&bytes))
            .unwrap_or_default();

        let (mut turns, store_clocked) = build_turns(&conn, &state, &meta);
        // Overlay codeg's own turn-span journal — recorded live by the ACP
        // connection layer. It fills turns the store left clockless (tool-free
        // turns) and replaces tool-span fallback clocks, which only cover the
        // tools, not the whole turn. CLI-native chats simply have no journal
        // file (empty vec, no-op).
        apply_turn_timing_journal(
            &mut turns,
            &crate::turn_timings::read_turn_timings_in(
                &self.turn_timings_root,
                crate::turn_timings::CURSOR_JOURNAL_AGENT,
                chat_id,
            ),
            &store_clocked,
        );
        relocate_orphaned_tool_results(&mut turns);
        structurize_read_tool_output(&mut turns);
        // Cursor's own `turn_timings` win; this only reaches turns the store
        // never timed (and in a store with no clock at all it changes nothing —
        // those turns share one synthetic timestamp, so every span is zero).
        backfill_turn_durations(&mut turns, &[]);

        // Cursor records only a session-level context meter (token_details);
        // pair it with the turns' aggregate stats so the status bar shows the
        // context ring.
        let session_stats = merge_context_window_stats(
            compute_session_stats(&turns),
            state.used_tokens,
            state.max_tokens,
        );

        let title = turns
            .iter()
            .find(|t| matches!(t.role, TurnRole::User))
            .and_then(|t| {
                t.blocks.iter().find_map(|b| match b {
                    ContentBlock::Text { text } if !text.trim().is_empty() => {
                        Some(title_from_user_text(text))
                    }
                    _ => None,
                })
            });
        let sidecar = read_sidecar_meta(chat_dir);
        let title = meta.name.clone().or_else(|| sidecar.title.clone()).or(title);
        let summary = summary_from(chat_id, &meta, &state, &sidecar, title);

        Some(ConversationDetail {
            summary,
            turns,
            session_stats,
            transcript_watermark: None,
        })
    }

    /// Locate the directory matching `conversation_id`: the flat
    /// `acp-sessions/<uuid>/` root first (direct hit), then the
    /// `chats/<md5>/<uuid>/` buckets (two shallow levels).
    fn find_chat_dir(&self, conversation_id: &str) -> Option<PathBuf> {
        let acp_candidate = self.acp_sessions_dir.join(conversation_id);
        if acp_candidate.join("store.db").is_file() {
            return Some(acp_candidate);
        }
        for group in read_subdirs(&self.chats_dir) {
            let candidate = group.join(conversation_id);
            if candidate.join("store.db").is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Append `chat_dir`'s summary to `out` when it holds a listable store.
    fn collect_summary(&self, chat_dir: &Path, out: &mut Vec<ConversationSummary>) {
        let Some(chat_id) = chat_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
        else {
            return;
        };
        if !chat_dir.join("store.db").is_file() {
            return;
        }
        if let Some(summary) = self.build_summary(chat_dir, &chat_id) {
            out.push(summary);
        }
    }
}

impl Default for CursorParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentParser for CursorParser {
    fn list_conversations(&self) -> Result<Vec<ConversationSummary>, ParseError> {
        let mut conversations = Vec::new();
        for group in read_subdirs(&self.chats_dir) {
            for chat_dir in read_subdirs(&group) {
                self.collect_summary(&chat_dir, &mut conversations);
            }
        }
        // ACP sessions (codeg-driven) live under the flat per-uuid root.
        for chat_dir in read_subdirs(&self.acp_sessions_dir) {
            self.collect_summary(&chat_dir, &mut conversations);
        }
        conversations.sort_by_key(|c| std::cmp::Reverse(c.started_at));
        Ok(conversations)
    }

    fn get_conversation(&self, conversation_id: &str) -> Result<ConversationDetail, ParseError> {
        let chat_dir = self
            .find_chat_dir(conversation_id)
            .ok_or_else(|| ParseError::ConversationNotFound(conversation_id.to_string()))?;
        self.build_detail(&chat_dir, conversation_id)
            .ok_or_else(|| ParseError::ConversationNotFound(conversation_id.to_string()))
    }
}

fn summary_from(
    chat_id: &str,
    meta: &ChatMeta,
    state: &DecodedState,
    sidecar: &SidecarMeta,
    title: Option<String>,
) -> ConversationSummary {
    // The sidecar cwd is authoritative for ACP sessions: their stores are
    // keyed by session uuid (no md5(cwd) bucket), and the workspace refs
    // inside the DAG only appear once a turn ran.
    let folder_path = sidecar.cwd.clone().or_else(|| state.workspace_path());
    let folder_name = folder_path.as_deref().map(folder_name_from_path);
    let started_at = meta
        .created_at_ms
        .or(state.started_ms)
        .or_else(|| state.timings.first().and_then(|t| t.timestamp_ms))
        .and_then(ms_to_utc)
        .unwrap_or_else(Utc::now);
    let ended_at = state
        .timings
        .last()
        .and_then(|t| Some(t.timestamp_ms? + t.duration_ms.unwrap_or(0)))
        .and_then(ms_to_utc);
    ConversationSummary {
        id: chat_id.to_string(),
        agent_type: AgentType::Cursor,
        folder_path,
        folder_name,
        title,
        started_at,
        ended_at,
        // user + assistant per agent turn (approximation before full decode).
        message_count: (state.turn_blob_ids.len() as u32).saturating_mul(2),
        model: meta.last_used_model.clone(),
        git_branch: state.git_branch.clone(),
        parent_id: None,
        parent_tool_use_id: None,
        delegation_call_id: None,
    }
}

fn ms_to_utc(ms: u64) -> Option<DateTime<Utc>> {
    Utc.timestamp_millis_opt(i64::try_from(ms).ok()?).single()
}

/// First user prompt text, for the title fallback (decodes only turn 0).
fn first_user_text(conn: &Connection, state: &DecodedState) -> Option<String> {
    let turn_bytes = read_blob(conn, state.turn_blob_ids.first()?)?;
    let agent_turn = wire::first_message(&turn_bytes, 1)?;
    let user_ref = wire::first_bytes(&agent_turn, 1)?;
    let msg = read_user_message(conn, &user_ref);
    let text = decode_user_text(conn, &msg)?;
    (!text.trim().is_empty()).then_some(text)
}

// ---------------------------------------------------------------------------
// store.db access
// ---------------------------------------------------------------------------

/// Chat metadata from `meta["0"]` (hex-encoded UTF-8 JSON).
#[derive(Default)]
struct ChatMeta {
    name: Option<String>,
    created_at_ms: Option<u64>,
    last_used_model: Option<String>,
    latest_root_blob_id: Option<Vec<u8>>,
    is_subagent: bool,
}

fn open_store(path: &Path) -> Option<Connection> {
    // Prefer a read-only handle — codeg never mutates cursor's stores.
    if let Some(conn) =
        try_open_store(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
    {
        return Some(conn);
    }
    // The CLI keeps live stores in WAL mode with most data in `-wal`; after a
    // CLI crash the wal-index can need recovery, which a read-only connection
    // is not allowed to run (SQLITE_READONLY_RECOVERY). Fall back to a
    // read-write handle (no CREATE) — recovery rebuilds the index without
    // altering database content, exactly as the CLI's own next open would.
    try_open_store(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

fn try_open_store(path: &Path, flags: OpenFlags) -> Option<Connection> {
    let conn = Connection::open_with_flags(path, flags).ok()?;
    // The CLI may hold the store open while codeg lists sessions; give reads a
    // short grace period instead of failing on a transient lock.
    let _ = conn.busy_timeout(std::time::Duration::from_millis(200));
    // Opening is lazy — probe so a handle that cannot actually read (stale WAL
    // index, foreign schema) reports failure here instead of yielding no rows.
    conn.query_row("SELECT count(*) FROM meta", [], |row| row.get::<_, i64>(0))
        .ok()?;
    Some(conn)
}

/// Sidecar `meta.json` beside an ACP session's `store.db`
/// (`acp-sessions/<uuid>/meta.json`): `{schemaVersion, cwd, title}`. Absent
/// for `chats/` stores — every field degrades to `None`.
#[derive(Default)]
struct SidecarMeta {
    cwd: Option<String>,
    title: Option<String>,
}

fn read_sidecar_meta(chat_dir: &Path) -> SidecarMeta {
    let Ok(raw) = crate::scoped_fs::read_to_string(chat_dir.join("meta.json")) else {
        return SidecarMeta::default();
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        return SidecarMeta::default();
    };
    let non_empty = |field: &str| {
        v.get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    SidecarMeta {
        cwd: non_empty("cwd"),
        title: non_empty("title"),
    }
}

fn read_chat_meta(conn: &Connection) -> Option<ChatMeta> {
    let raw: String = conn
        .query_row("SELECT value FROM meta WHERE key = '0'", [], |row| {
            row.get(0)
        })
        .ok()?;
    // The value is hex-encoded UTF-8 JSON; tolerate a plain-JSON variant
    // defensively (older stores).
    let json_text = if raw.trim_start().starts_with('{') {
        raw
    } else {
        String::from_utf8(hex_decode(raw.trim())?).ok()?
    };
    let v: Value = serde_json::from_str(&json_text).ok()?;

    let non_empty = |s: &str| {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_string())
    };
    Some(ChatMeta {
        name: v.get("name").and_then(Value::as_str).and_then(non_empty),
        created_at_ms: json_ms(v.get("createdAt")),
        last_used_model: v
            .get("lastUsedModel")
            .and_then(Value::as_str)
            .and_then(non_empty),
        latest_root_blob_id: v
            .get("latestRootBlobId")
            .and_then(Value::as_str)
            .and_then(hex_decode),
        is_subagent: v
            .get("subagentInfo")
            .is_some_and(|s| !s.is_null()),
    })
}

/// Accept `createdAt` as epoch-ms number or RFC3339 string.
fn json_ms(v: Option<&Value>) -> Option<u64> {
    let v = v?;
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    let s = v.as_str()?;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .and_then(|dt| u64::try_from(dt.timestamp_millis()).ok())
}

fn read_blob(conn: &Connection, id: &[u8]) -> Option<Vec<u8>> {
    conn.query_row(
        "SELECT data FROM blobs WHERE id = ?1",
        [hex_encode(id)],
        |row| row.get::<_, Vec<u8>>(0),
    )
    .ok()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || s.is_empty() {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

// ---------------------------------------------------------------------------
// Minimal protobuf wire-format reader (varint / length-delimited / fixed).
// ---------------------------------------------------------------------------

mod wire {
    pub enum Val<'a> {
        Varint(u64),
        Fixed64(u64),
        Bytes(&'a [u8]),
        /// 32-bit scalars (float/fixed32) — parsed to keep the field stream
        /// aligned; no cursor message we decode carries one we read.
        Fixed32,
    }

    impl<'a> Val<'a> {
        pub fn bytes(&self) -> Option<&'a [u8]> {
            match self {
                Val::Bytes(b) => Some(b),
                _ => None,
            }
        }
        pub fn str(&self) -> Option<&'a str> {
            std::str::from_utf8(self.bytes()?).ok()
        }
        pub fn u64(&self) -> Option<u64> {
            match self {
                Val::Varint(v) => Some(*v),
                _ => None,
            }
        }
        pub fn i32(&self) -> Option<i32> {
            // int32/int64/enum are varint-encoded (negatives use 10 bytes).
            self.u64().map(|v| v as i64 as i32)
        }
        pub fn f64(&self) -> Option<f64> {
            match self {
                Val::Fixed64(v) => Some(f64::from_bits(*v)),
                _ => None,
            }
        }
    }

    /// Iterate `(field_number, value)` pairs; stops silently at malformed
    /// input (truncated varint, out-of-bounds length, group wire types).
    pub struct Fields<'a> {
        buf: &'a [u8],
        pos: usize,
        malformed: bool,
    }

    impl<'a> Fields<'a> {
        pub fn new(buf: &'a [u8]) -> Self {
            Self {
                buf,
                pos: 0,
                malformed: false,
            }
        }

        /// Bytes consumed so far.
        pub fn consumed(&self) -> usize {
            self.pos
        }

        /// Whether iteration stopped on malformed input rather than a clean
        /// end-of-buffer. A failed parse may still have advanced `pos` (even
        /// to the buffer end, e.g. a truncated trailing varint), so
        /// `consumed()` alone cannot distinguish the two.
        pub fn is_malformed(&self) -> bool {
            self.malformed
        }

        fn varint(&mut self) -> Option<u64> {
            let mut out: u64 = 0;
            for i in 0..10 {
                let b = *self.buf.get(self.pos)?;
                self.pos += 1;
                out |= u64::from(b & 0x7f) << (7 * i);
                if b & 0x80 == 0 {
                    return Some(out);
                }
            }
            None
        }
    }

    impl<'a> Fields<'a> {
        fn parse_next(&mut self) -> Option<(u32, Val<'a>)> {
            let key = self.varint()?;
            let field_no = u32::try_from(key >> 3).ok()?;
            match key & 0x7 {
                0 => Some((field_no, Val::Varint(self.varint()?))),
                1 => {
                    let end = self.pos.checked_add(8)?;
                    let raw = self.buf.get(self.pos..end)?;
                    self.pos = end;
                    Some((field_no, Val::Fixed64(u64::from_le_bytes(raw.try_into().ok()?))))
                }
                2 => {
                    let len = usize::try_from(self.varint()?).ok()?;
                    let end = self.pos.checked_add(len)?;
                    let raw = self.buf.get(self.pos..end)?;
                    self.pos = end;
                    Some((field_no, Val::Bytes(raw)))
                }
                5 => {
                    let end = self.pos.checked_add(4)?;
                    self.buf.get(self.pos..end)?;
                    self.pos = end;
                    Some((field_no, Val::Fixed32))
                }
                _ => None, // groups (3/4) / invalid: stop
            }
        }
    }

    impl<'a> Iterator for Fields<'a> {
        type Item = (u32, Val<'a>);

        fn next(&mut self) -> Option<Self::Item> {
            if self.pos >= self.buf.len() {
                return None; // clean end
            }
            let item = self.parse_next();
            if item.is_none() {
                self.malformed = true;
            }
            item
        }
    }

    /// First occurrence of length-delimited `field` as raw bytes.
    pub fn first_bytes(buf: &[u8], field: u32) -> Option<Vec<u8>> {
        Fields::new(buf)
            .find(|(no, v)| *no == field && v.bytes().is_some())
            .and_then(|(_, v)| v.bytes().map(<[u8]>::to_vec))
    }

    /// First occurrence of `field` as an embedded message (same wire type as
    /// bytes — the caller decides how to interpret it).
    pub fn first_message(buf: &[u8], field: u32) -> Option<Vec<u8>> {
        first_bytes(buf, field)
    }

    /// First occurrence of `field` as a UTF-8 string.
    pub fn first_str(buf: &[u8], field: u32) -> Option<String> {
        Fields::new(buf)
            .find(|(no, v)| *no == field && v.str().is_some())
            .and_then(|(_, v)| v.str().map(str::to_string))
    }

    /// Whether `buf` decodes as a well-formed field stream to its exact end.
    /// Distinguishes a genuine message wrapper from a text payload whose
    /// leading bytes merely *look* like a field frame (those either fail to
    /// parse — flagged malformed even when the failed parse advanced to the
    /// buffer end — or stop short of the end).
    pub fn is_complete_message(buf: &[u8]) -> bool {
        let mut fields = Fields::new(buf);
        while fields.next().is_some() {}
        !fields.is_malformed() && fields.consumed() == buf.len()
    }
}

/// Decode a protobuf map<string, google.protobuf.Value> whose entries are the
/// repeated occurrences of `entry_field` in `buf` (each entry = {1: key,
/// 2: Value}). This is both `google.protobuf.Struct` (entries at field 1) and
/// `McpArgs.args` (map entries directly at field 2). Unknown/malformed parts
/// degrade to `null`s rather than failing.
fn decode_value_map(buf: &[u8], entry_field: u32) -> Value {
    let mut map = serde_json::Map::new();
    for (no, val) in wire::Fields::new(buf) {
        if no != entry_field {
            continue;
        }
        let Some(entry) = val.bytes() else { continue };
        let mut key: Option<String> = None;
        let mut value = Value::Null;
        for (eno, ev) in wire::Fields::new(entry) {
            match eno {
                1 => key = ev.str().map(str::to_string),
                2 => {
                    if let Some(b) = ev.bytes() {
                        value = decode_proto_value(b);
                    }
                }
                _ => {}
            }
        }
        if let Some(key) = key {
            map.insert(key, value);
        }
    }
    Value::Object(map)
}

/// `google.protobuf.Struct`: its `fields` map entries live at field 1.
fn decode_proto_struct(buf: &[u8]) -> Value {
    decode_value_map(buf, 1)
}

fn decode_proto_value(buf: &[u8]) -> Value {
    for (no, val) in wire::Fields::new(buf) {
        match no {
            1 => return Value::Null,
            2 => {
                if let Some(f) = val.f64() {
                    return serde_json::Number::from_f64(f)
                        .map(Value::Number)
                        .unwrap_or(Value::Null);
                }
            }
            3 => {
                if let Some(s) = val.str() {
                    return Value::String(s.to_string());
                }
            }
            4 => {
                if let Some(b) = val.u64() {
                    return Value::Bool(b != 0);
                }
            }
            5 => {
                if let Some(b) = val.bytes() {
                    return decode_proto_struct(b);
                }
            }
            6 => {
                if let Some(b) = val.bytes() {
                    let items = wire::Fields::new(b)
                        .filter(|(no, _)| *no == 1)
                        .filter_map(|(_, v)| v.bytes().map(decode_proto_value))
                        .collect();
                    return Value::Array(items);
                }
            }
            _ => {}
        }
    }
    Value::Null
}

// ---------------------------------------------------------------------------
// agent.v1 message decoding
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TurnTiming {
    duration_ms: Option<u64>,
    timestamp_ms: Option<u64>,
}

#[derive(Default)]
struct DecodedState {
    turn_blob_ids: Vec<Vec<u8>>,
    used_tokens: Option<u64>,
    max_tokens: Option<u64>,
    repo_path: Option<String>,
    git_branch: Option<String>,
    timings: Vec<TurnTiming>,
    started_ms: Option<u64>,
    workspace_uris: Vec<String>,
    /// `working_directory` seen on a shell tool call — the workspace fallback
    /// for non-git projects. Filled lazily during turn decoding.
    shell_cwd: std::cell::RefCell<Option<String>>,
}

impl DecodedState {
    /// Workspace path, best source first: the tracked git repo, then any
    /// recorded workspace URI, then a shell call's working directory.
    fn workspace_path(&self) -> Option<String> {
        if let Some(repo) = &self.repo_path {
            return Some(repo.clone());
        }
        if let Some(uri) = self.workspace_uris.last() {
            let path = uri.strip_prefix("file://").unwrap_or(uri);
            if !path.trim().is_empty() {
                return Some(path.to_string());
            }
        }
        self.shell_cwd.borrow().clone()
    }
}

/// Decode `agent.v1.ConversationStateStructure` (the DAG root).
fn decode_state(buf: &[u8]) -> DecodedState {
    let mut state = DecodedState::default();
    for (no, val) in wire::Fields::new(buf) {
        match no {
            // 8: turns (repeated bytes — blob ids)
            8 => {
                if let Some(b) = val.bytes() {
                    state.turn_blob_ids.push(b.to_vec());
                }
            }
            // 5: token_details {1 used_tokens, 2 max_tokens}
            5 => {
                if let Some(b) = val.bytes() {
                    for (tno, tval) in wire::Fields::new(b) {
                        match tno {
                            1 => state.used_tokens = tval.u64(),
                            2 => state.max_tokens = tval.u64(),
                            _ => {}
                        }
                    }
                }
            }
            // 9: previous_workspace_uris (repeated string)
            9 => {
                if let Some(s) = val.str() {
                    state.workspace_uris.push(s.to_string());
                }
            }
            // 14: turn_timings {1 duration_ms, 2 timestamp_ms}
            14 => {
                if let Some(b) = val.bytes() {
                    let mut timing = TurnTiming::default();
                    for (tno, tval) in wire::Fields::new(b) {
                        match tno {
                            1 => timing.duration_ms = tval.u64(),
                            2 => timing.timestamp_ms = tval.u64(),
                            _ => {}
                        }
                    }
                    state.timings.push(timing);
                }
            }
            // 21: tracked_git_repo_branches {1 repo_path, 2 branch_name}
            21 => {
                if let Some(b) = val.bytes() {
                    if state.repo_path.is_none() {
                        state.repo_path =
                            wire::first_str(b, 1).filter(|s| !s.trim().is_empty());
                        state.git_branch =
                            wire::first_str(b, 2).filter(|s| !s.trim().is_empty());
                    }
                }
            }
            // 26: conversation_started_timestamp_ms
            26 => state.started_ms = val.u64(),
            _ => {}
        }
    }
    state
}

/// Resolve a turn's user-message reference into the raw `UserMessage` bytes.
/// `user_ref` is normally a blob id; fall back to treating it as an inline
/// `UserMessage` if the blob is absent (defensive dual-path).
fn read_user_message(conn: &Connection, user_ref: &[u8]) -> Vec<u8> {
    read_blob(conn, user_ref).unwrap_or_else(|| user_ref.to_vec())
}

/// User prompt text from a decoded `UserMessage`: `text` (1), else the
/// large-text blob (`text_blob_id`, 18), else `rich_text` (8).
fn decode_user_text(conn: &Connection, msg: &[u8]) -> Option<String> {
    let text = wire::first_str(msg, 1).filter(|s| !s.is_empty());
    if text.is_some() {
        return text;
    }
    if let Some(id) = wire::first_bytes(msg, 18) {
        if let Some(blob) = read_blob(conn, &id) {
            if let Ok(s) = String::from_utf8(blob) {
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    wire::first_str(msg, 8).filter(|s| !s.is_empty())
}

/// User-attached images from a decoded `UserMessage`.
///
/// Each attachment is a repeated field-3 entry wrapping an image variant at
/// field 1: `{1: image blob id (32 bytes), 2: attachment uuid, 7: mime}`; the
/// referenced blob holds the raw encoded bytes (PNG/JPEG/…), verified against
/// a real ACP session (`89504e47…` magic under `blobs`). Re-encoded as base64
/// for the `ContentBlock::Image` wire shape the frontend renders. Non-image
/// mimes, unresolvable blob ids, and malformed entries are skipped — an
/// attachment we can't decode must not sink the turn's text.
fn decode_user_images(conn: &Connection, msg: &[u8]) -> Vec<ContentBlock> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let mut out = Vec::new();
    for (no, val) in wire::Fields::new(msg) {
        if no != 3 {
            continue;
        }
        let Some(attachment) = val.bytes() else {
            continue;
        };
        let Some(image) = wire::first_message(attachment, 1) else {
            continue;
        };
        let Some(mime_type) = wire::first_str(&image, 7).filter(|m| m.starts_with("image/"))
        else {
            continue;
        };
        let Some(bytes) = wire::first_bytes(&image, 1).and_then(|id| read_blob(conn, &id))
        else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        out.push(ContentBlock::Image {
            data: STANDARD.encode(&bytes),
            mime_type,
            uri: None,
        });
    }
    out
}

/// Build codeg turns from the DAG: one User + one Assistant `MessageTurn` per
/// `AgentConversationTurnStructure`; `!`-shell turns render as a user command
/// turn plus an assistant turn holding the terminal card.
///
/// The second return value indexes the assistant turns whose clock came from
/// the STORE's own root `turn_timings` (IDE-written chats). Those clocks are
/// authoritative; everything else — tool-span fallback or no clock at all —
/// may be upgraded by codeg's turn-span journal downstream.
fn build_turns(
    conn: &Connection,
    state: &DecodedState,
    meta: &ChatMeta,
) -> (Vec<MessageTurn>, HashSet<usize>) {
    let mut turns: Vec<MessageTurn> = Vec::new();
    let mut store_clocked: HashSet<usize> = HashSet::new();

    for (i, turn_id) in state.turn_blob_ids.iter().enumerate() {
        let Some(turn_bytes) = read_blob(conn, turn_id) else {
            continue;
        };
        let timing = state.timings.get(i);
        let root_ts_ms = timing.and_then(|t| t.timestamp_ms);
        let root_duration_ms = timing.and_then(|t| t.duration_ms).filter(|d| *d > 0);
        let store_clock = root_ts_ms.is_some() || root_duration_ms.is_some();
        let ts = root_ts_ms
            .and_then(ms_to_utc)
            .or_else(|| meta.created_at_ms.and_then(ms_to_utc))
            .unwrap_or_else(Utc::now);
        let completed_at = timing
            .and_then(|t| Some(t.timestamp_ms? + t.duration_ms.unwrap_or(0)))
            .and_then(ms_to_utc);
        let duration_ms = root_duration_ms;

        if let Some(agent_turn) = wire::first_message(&turn_bytes, 1) {
            // One pass over the steps: decode blocks and (only when the root
            // carries no timing for this turn) fold the tool-call clock span,
            // without retaining every raw step payload.
            let want_span = !store_clock;
            let mut span: Option<(u64, u64)> = None;
            let mut blocks: Vec<ContentBlock> = Vec::new();
            for (no, val) in wire::Fields::new(&agent_turn) {
                if no != 2 {
                    continue;
                }
                let Some(step_ref) = val.bytes() else { continue };
                let step = read_blob(conn, step_ref).unwrap_or_else(|| step_ref.to_vec());
                if want_span {
                    fold_tool_span(&step, &mut span);
                }
                decode_step(conn, &step, state, &mut blocks);
            }
            // ACP-written stores carry no root `turn_timings`; the span of
            // the turn's own tool calls is the fallback clock. Thinking and
            // text steps carry no clock at all, so a tool-free turn stays
            // clockless rather than borrowing a wrong timestamp.
            let ts = span.and_then(|(lo, _)| ms_to_utc(lo)).unwrap_or(ts);
            let completed_at =
                completed_at.or_else(|| span.and_then(|(_, hi)| ms_to_utc(hi)));
            let duration_ms = duration_ms
                .or_else(|| span.and_then(|(lo, hi)| (hi > lo).then_some(hi - lo)));

            // --- user prompt (text + attached images) ---
            if let Some(user_ref) = wire::first_bytes(&agent_turn, 1) {
                let msg = read_user_message(conn, &user_ref);
                let mut user_blocks: Vec<ContentBlock> = Vec::new();
                if let Some(text) = decode_user_text(conn, &msg) {
                    if !text.trim().is_empty() {
                        user_blocks.push(ContentBlock::Text { text });
                    }
                }
                // Text first, then attachments — matching the composer's
                // send order. An image-only prompt still produces the turn.
                user_blocks.extend(decode_user_images(conn, &msg));
                if !user_blocks.is_empty() {
                    turns.push(MessageTurn {
                        id: String::new(),
                        role: TurnRole::User,
                        blocks: user_blocks,
                        timestamp: ts,
                        usage: None,
                        duration_ms: None,
                        model: None,
                        completed_at: None,
                    agent_message_id: None,
                    });
                }
            }
            if !blocks.is_empty() {
                if store_clock {
                    store_clocked.insert(turns.len());
                }
                turns.push(MessageTurn {
                    id: String::new(),
                    role: TurnRole::Assistant,
                    blocks,
                    timestamp: ts,
                    usage: None,
                    duration_ms,
                    model: meta.last_used_model.clone(),
                    completed_at,
                agent_message_id: None,
                });
            }
        } else if let Some(shell_turn) = wire::first_message(&turn_bytes, 2) {
            // --- user-typed `!` shell command ---
            let command = wire::first_bytes(&shell_turn, 1)
                .and_then(|id| {
                    let msg = read_blob(conn, &id).unwrap_or(id);
                    wire::first_str(&msg, 1)
                })
                .unwrap_or_default();
            let output = wire::first_bytes(&shell_turn, 2).map(|id| {
                let msg = read_blob(conn, &id).unwrap_or(id);
                let stdout = wire::first_str(&msg, 1).unwrap_or_default();
                let stderr = wire::first_str(&msg, 2).unwrap_or_default();
                let exit_code = wire::Fields::new(&msg)
                    .find(|(no, _)| *no == 3)
                    .and_then(|(_, v)| v.i32())
                    .unwrap_or(0);
                (join_streams(&stdout, &stderr), exit_code)
            });
            if command.trim().is_empty() {
                continue;
            }
            turns.push(MessageTurn {
                id: String::new(),
                role: TurnRole::User,
                blocks: vec![ContentBlock::Text {
                    text: format!("! {command}"),
                }],
                timestamp: ts,
                usage: None,
                duration_ms: None,
                model: None,
                completed_at: None,
            agent_message_id: None,
            });
            let tool_id = format!("cursor-shell-{i}");
            let (preview, exit_code) = output.unwrap_or((None, 0));
            if store_clock {
                store_clocked.insert(turns.len());
            }
            turns.push(MessageTurn {
                id: String::new(),
                role: TurnRole::Assistant,
                blocks: vec![
                    ContentBlock::ToolUse {
                        tool_use_id: Some(tool_id.clone()),
                        tool_name: "shell".to_string(),
                        input_preview: bounded_json_preview(
                            &serde_json::json!({ "command": command }),
                        ),
                        status: None,
                        meta: None,
                    },
                    ContentBlock::ToolResult {
                        tool_use_id: Some(tool_id),
                        output_preview: preview,
                        is_error: exit_code != 0,
                        agent_stats: None,
                        images: Vec::new(),
                    },
                ],
                timestamp: ts,
                usage: None,
                duration_ms,
                model: None,
                completed_at,
            agent_message_id: None,
            });
        }
    }

    for (i, turn) in turns.iter_mut().enumerate() {
        turn.id = format!("cursor-turn-{i}");
    }
    (turns, store_clocked)
}

/// Merge codeg's own turn-span journal (`crate::turn_timings`) onto the
/// store's turns: it fills turns the store left clockless (thinking+text-only
/// turns have neither root `turn_timings` nor tool `f59`/`f60` stamps) AND
/// replaces tool-span fallback clocks — the tool span only covers
/// first-tool-start → last-tool-end (often a sub-second slice of a long
/// turn), while a matched journal line measured the full prompt → `end_turn`
/// wall span at the connection. Replacement additionally requires the line
/// to BRACKET the existing tool span (see the containment guard below), so
/// a stale line misattributed across identical prompts can only ever reach
/// turns that had no clock to lose.
///
/// Alignment is tail-anchored and hash-verified: walk the user turns and the
/// journal both **backwards**, pairing while `prompt_hash(user text)` matches
/// the journal line, and STOP at the first mismatch. The tail anchor is what
/// makes the dominant divergence safe — turns recorded before the journal
/// feature existed all sit at the FRONT of a session, so the recorded tail
/// still aligns; a mid-walk confusion (a crash gap, a turn codeg never saw)
/// stops the walk instead of scanning, which could misassign spans between
/// identical prompts. STORE-native clocks (root `turn_timings`, indexed by
/// `store_clocked` — IDE-written chats) always win: the journal only
/// corroborates those pairs.
///
/// Identical-prompt runs get an extra equality guard (Codex review): a line
/// can exist for a turn the store never kept (see the count guard below for
/// the sources), so a run of same-hash journal entries can be LONGER than
/// the store's run of same-text turns — hash pairing alone would then hand
/// that phantom's span to an older identical prompt (image-only prompts all
/// hash as "" and are the easiest collision). Whenever either side is in a
/// same-hash run, both runs must have EQUAL length or the walk stops before
/// assigning anything from that run.
///
/// These guards are defense-in-depth, not a proof: two narrow residuals
/// remain where a WRONG span (not just a missing one) can be assigned — a
/// clean-`end_turn` turn Cursor failed to persist, and a missing journal
/// SUFFIX whose stale tail hash-collides with the store's newest turn. Both
/// are documented in `turn_timings`' module docs; the latter is pinned by
/// `accepted_residual_missing_tail_lines_can_misattribute_span`.
fn apply_turn_timing_journal(
    turns: &mut [MessageTurn],
    journal: &[crate::turn_timings::TurnTiming],
    store_clocked: &HashSet<usize>,
) {
    if journal.is_empty() {
        return;
    }
    // Contiguity anchor (Codex review R5/R6): trust only the journal's
    // trailing run of SAME-CONNECTION, strictly consecutive ordinals. An
    // ordinal step ≠ 1 walking backwards means a gap (dropped line, skipped
    // non-`end_turn` turn) — and ordinals are only comparable within one
    // connection, so a `conn` change ends trust too (numerically consecutive
    // ordinals across a resume boundary prove nothing: old conn's ord 1
    // followed by a new conn's ord 2 whose ord-1 turn was canceled). Either
    // way positions across the boundary can't anchor, and the reverse walk
    // could otherwise slide over the gap onto an older same-hash entry
    // (store [x, y, x, z] with journal [x, z] would hand the FIRST x's span
    // to the THIRD turn). Within the trusted run, adjacent entries are
    // adjacent turns by construction.
    let mut trusted_start = journal.len() - 1;
    while trusted_start > 0 {
        let prev = &journal[trusted_start - 1];
        let cur = &journal[trusted_start];
        if prev.ord + 1 != cur.ord || prev.conn != cur.conn {
            break;
        }
        trusted_start -= 1;
    }
    let journal = &journal[trusted_start..];
    // (user turn index, following assistant turn index if adjacent)
    let user_indices: Vec<usize> = turns
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t.role, TurnRole::User))
        .map(|(i, _)| i)
        .collect();
    // Count guard (Codex review R4-2): the writer journals only cleanly
    // completed turns, which Cursor persists in all but the failure case
    // documented as the first `turn_timings` residual — so a legitimate
    // journal can have FEWER entries than the store has user turns
    // (pre-feature turns, dropped lines) but, that residual aside, never
    // MORE. More entries means phantom lines (turns the store never kept, a
    // foreign file), and a phantom at the tail would hash-pair with an older
    // same-text turn even across non-contiguous positions. Nothing is
    // salvageable then.
    if journal.len() > user_indices.len() {
        return;
    }
    let user_hash = |turns: &[MessageTurn], user_idx: usize| -> String {
        let text: String = turns[user_idx]
            .blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        crate::turn_timings::prompt_hash(&text)
    };
    let mut j = journal.len();
    for (pos, &user_idx) in user_indices.iter().enumerate().rev() {
        if j == 0 {
            break;
        }
        let turn_sha = user_hash(turns, user_idx);
        let entry = &journal[j - 1];
        if turn_sha != entry.prompt_sha {
            break;
        }
        // Same-hash-run equality guard (see the doc comment). Runs are counted
        // from this pairing backwards on both sides; inequality means the
        // journal recorded turns the store never kept (or vice versa) and the
        // 1:1 tail alignment inside the run is unprovable.
        let journal_run = journal[..j]
            .iter()
            .rev()
            .take_while(|e| e.prompt_sha == entry.prompt_sha)
            .count();
        let store_run = user_indices[..=pos]
            .iter()
            .rev()
            .take_while(|&&u| user_hash(turns, u) == entry.prompt_sha)
            .count();
        if (journal_run > 1 || store_run > 1) && journal_run != store_run {
            break;
        }
        j -= 1;
        let started = ms_to_utc(entry.started_at_ms);
        let assistant_idx = (user_idx + 1 < turns.len()
            && matches!(turns[user_idx + 1].role, TurnRole::Assistant))
        .then_some(user_idx + 1);
        if assistant_idx.map(|a| store_clocked.contains(&a)).unwrap_or(false) {
            continue; // store-native root timing present — journal only corroborates
        }
        let Some(a) = assistant_idx else { continue };
        // A tool-span fallback clock is a subset of the turn; the matched
        // journal line replaces it with the full prompt→end_turn span — but
        // only when the line BRACKETS the span. For a line that truly
        // measured this turn, containment is structural: the connection
        // stamps the start before the prompt reaches the agent (so before
        // any tool runs) and the end when `TurnComplete` arrives (after the
        // last tool finished), all on one machine's clock. A line
        // misattributed across identical prompts (the documented
        // missing-suffix residual) belongs to an OLDER turn, which ended
        // before this turn's tools started — it fails the check and the tool
        // span, though partial, stays as the safer clock. The check also
        // subsumes degenerate zero-span lines (they can't bracket a real
        // span and have no duration to offer).
        if turns[a].duration_ms.is_some() || turns[a].completed_at.is_some() {
            let fallback_start = turns[a].timestamp.timestamp_millis();
            let fallback_end = turns[a]
                .completed_at
                .map(|c| c.timestamp_millis())
                .unwrap_or(fallback_start);
            let brackets = (entry.started_at_ms as i64) <= fallback_start
                && (entry.ended_at_ms as i64) >= fallback_end;
            if !brackets {
                continue;
            }
        }
        let journal_span_ms = (entry.ended_at_ms > entry.started_at_ms)
            .then(|| entry.ended_at_ms - entry.started_at_ms);
        turns[a].completed_at = ms_to_utc(entry.ended_at_ms);
        turns[a].duration_ms = journal_span_ms;
        // The turn's fallback timestamp (session createdAt or first tool
        // start) upgrades to the observed send time on both siblings.
        if let Some(start) = started {
            turns[user_idx].timestamp = start;
            turns[a].timestamp = start;
        }
    }
}

/// Fold one step's tool-call wall-clock stamps into a min/max span:
/// `ToolCall.started_at_ms` (field 59) / `ended_at_ms` (field 60). A
/// failed-fast call may carry only the end stamp, so both fields fold into
/// the same span. Non-tool steps contribute nothing.
fn fold_tool_span(step: &[u8], span: &mut Option<(u64, u64)>) {
    // ConversationStep oneof field 2 = tool_call.
    let Some(tool_call) = wire::first_message(step, 2) else {
        return;
    };
    for (no, val) in wire::Fields::new(&tool_call) {
        if no != 59 && no != 60 {
            continue;
        }
        let Some(ms) = val.u64().filter(|ms| *ms > 0) else {
            continue;
        };
        *span = Some(match *span {
            Some((lo, hi)) => (lo.min(ms), hi.max(ms)),
            None => (ms, ms),
        });
    }
}

/// Decode one `ConversationStep` into content blocks.
fn decode_step(
    conn: &Connection,
    step: &[u8],
    state: &DecodedState,
    blocks: &mut Vec<ContentBlock>,
) {
    for (no, val) in wire::Fields::new(step) {
        let Some(body) = val.bytes() else { continue };
        match no {
            // assistant_message { 1: text }
            1 => {
                if let Some(text) = wire::first_str(body, 1).filter(|t| !t.is_empty()) {
                    if let Some(ContentBlock::Text { text: last }) = blocks.last_mut() {
                        last.push_str(&text);
                    } else {
                        blocks.push(ContentBlock::Text { text });
                    }
                }
            }
            // tool_call { oneof … }
            2 => decode_tool_call(conn, body, state, blocks),
            // thinking_message { 1: text }
            3 => {
                if let Some(text) = wire::first_str(body, 1).filter(|t| !t.is_empty()) {
                    if let Some(ContentBlock::Thinking { text: last }) = blocks.last_mut() {
                        last.push('\n');
                        last.push_str(&text);
                    } else {
                        blocks.push(ContentBlock::Thinking { text });
                    }
                }
            }
            _ => {}
        }
    }
}

/// `agent.v1.ToolCall` oneof field number → tool name, for every variant the
/// 2026.07.16 CLI models. Undecoded variants still render a named card.
fn tool_name_for_field(no: u32) -> Option<&'static str> {
    Some(match no {
        1 => "shell",
        3 => "delete",
        4 => "glob",
        5 => "grep",
        8 => "read",
        9 => "update_todos",
        10 => "read_todos",
        12 => "edit",
        13 => "ls",
        14 => "read_lints",
        15 => "mcp",
        16 => "sem_search",
        17 => "create_plan",
        18 => "web_search",
        19 => "task",
        20 => "list_mcp_resources",
        21 => "read_mcp_resource",
        22 => "apply_agent_diff",
        23 => "ask_question",
        24 => "fetch",
        25 => "switch_mode",
        28 => "generate_image",
        29 => "record_screen",
        30 => "computer_use",
        31 => "write_shell_stdin",
        32 => "reflect",
        33 => "setup_vm_environment",
        34 => "truncated_tool_call",
        35 => "start_grind_execution",
        36 => "start_grind_planning",
        37 => "web_fetch",
        38 => "report_bugfix_results",
        39 => "ai_attribution",
        40 => "pr_management",
        41 => "mcp_auth",
        42 => "await",
        43 => "blame_by_file_path",
        44 => "get_mcp_tools",
        45 => "report_bug",
        46 => "set_active_branch",
        48 => "communicate_update",
        49 => "send_final_summary",
        50 => "update_pr_code_tour",
        51 => "replace_env",
        52 => "edit_pr_labels",
        53 => "record_ci_investigation_findings",
        55 => "send_message",
        56 => "fetch_cloud_agent_data",
        58 => "send_to_user",
        61 => "pi_read",
        62 => "pi_bash",
        63 => "pi_edit",
        64 => "pi_write",
        65 => "pi_grep",
        66 => "pi_find",
        67 => "pi_ls",
        68 => "connect_scm",
        69 => "search_conversations",
        _ => return None,
    })
}

struct DecodedTool {
    name: String,
    input: Option<Value>,
    output: Option<String>,
    is_error: bool,
}

/// Decode one `agent.v1.ToolCall` into a ToolUse + ToolResult block pair.
fn decode_tool_call(
    conn: &Connection,
    body: &[u8],
    state: &DecodedState,
    blocks: &mut Vec<ContentBlock>,
) {
    let mut tool: Option<DecodedTool> = None;
    let mut tool_call_id: Option<String> = None;

    for (no, val) in wire::Fields::new(body) {
        match no {
            57 => tool_call_id = val.str().map(str::to_string),
            _ => {
                if tool.is_none() {
                    if let (Some(name), Some(payload)) = (tool_name_for_field(no), val.bytes()) {
                        tool = Some(decode_tool_variant(conn, no, name, payload, state));
                    }
                }
            }
        }
    }

    let Some(tool) = tool else { return };
    let id = tool_call_id
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("cursor-tool-{}", blocks.len()));
    blocks.push(ContentBlock::ToolUse {
        tool_use_id: Some(id.clone()),
        tool_name: tool.name,
        input_preview: tool.input.as_ref().and_then(bounded_json_preview_ref),
        status: None,
        meta: None,
    });
    blocks.push(ContentBlock::ToolResult {
        tool_use_id: Some(id),
        output_preview: tool
            .output
            .map(|o| truncate_str(&o, CURSOR_TOOL_OUTPUT_CAP)),
        is_error: tool.is_error,
        agent_stats: None,
        images: Vec::new(),
    });
}

/// Per-variant decode. `args` is field 1, `result` field 2 on every
/// `agent.v1.<X>ToolCall` message.
fn decode_tool_variant(
    conn: &Connection,
    field_no: u32,
    name: &str,
    payload: &[u8],
    state: &DecodedState,
) -> DecodedTool {
    let args = wire::first_message(payload, 1);
    let result = wire::first_message(payload, 2);
    let mut tool = DecodedTool {
        name: name.to_string(),
        input: None,
        output: None,
        is_error: false,
    };

    match field_no {
        // shell
        1 => {
            if let Some(a) = &args {
                let command = wire::first_str(a, 1);
                if let Some(cwd) = wire::first_str(a, 2).filter(|s| !s.trim().is_empty()) {
                    state.shell_cwd.borrow_mut().get_or_insert(cwd);
                }
                let description = wire::first_str(a, 15);
                tool.input = Some(json_obj(&[
                    ("command", command),
                    ("description", description),
                ]));
            }
            if let Some(r) = &result {
                // oneof: 1 success / 2 failure / 3 timeout / 4 rejected /
                //        5 spawn_error / 7 permission_denied
                for (no, val) in wire::Fields::new(r) {
                    let Some(b) = val.bytes() else { continue };
                    match no {
                        1 | 2 => {
                            let interleaved_field = if no == 1 { 10 } else { 9 };
                            let out = wire::first_str(b, interleaved_field)
                                .filter(|s| !s.is_empty())
                                .or_else(|| {
                                    let stdout = wire::first_str(b, 5).unwrap_or_default();
                                    let stderr = wire::first_str(b, 6).unwrap_or_default();
                                    join_streams(&stdout, &stderr)
                                });
                            let exit_code = wire::Fields::new(b)
                                .find(|(fno, _)| *fno == 3)
                                .and_then(|(_, v)| v.i32())
                                .unwrap_or(0);
                            tool.output = out;
                            tool.is_error = no == 2 || exit_code != 0;
                            break;
                        }
                        3 => {
                            tool.output = Some("Command timed out".to_string());
                            tool.is_error = true;
                            break;
                        }
                        4 => {
                            tool.output = Some("Command rejected".to_string());
                            tool.is_error = true;
                            break;
                        }
                        5 | 7 => {
                            tool.output = wire::first_str(b, 1);
                            tool.is_error = true;
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        // delete
        3 => {
            tool.input = args
                .as_deref()
                .map(|a| json_obj(&[("file_path", wire::first_str(a, 1))]));
        }
        // glob
        4 => {
            if let Some(a) = &args {
                tool.input = Some(json_obj(&[
                    ("pattern", wire::first_str(a, 2)),
                    ("path", wire::first_str(a, 1)),
                ]));
            }
            if let Some(success) = result.as_deref().and_then(|r| wire::first_message(r, 1)) {
                let files: Vec<String> = wire::Fields::new(&success)
                    .filter(|(no, _)| *no == 3)
                    .filter_map(|(_, v)| v.str().map(str::to_string))
                    .collect();
                if !files.is_empty() {
                    tool.output = Some(files.join("\n"));
                }
            }
        }
        // grep
        5 => {
            if let Some(a) = &args {
                tool.input = Some(json_obj(&[
                    ("pattern", wire::first_str(a, 1)),
                    ("path", wire::first_str(a, 2)),
                    ("glob", wire::first_str(a, 3)),
                ]));
            }
        }
        // read
        8 => {
            if let Some(a) = &args {
                let mut obj = serde_json::Map::new();
                if let Some(path) = wire::first_str(a, 1) {
                    obj.insert("file_path".into(), Value::String(path));
                }
                for (key, field) in [("offset", 2), ("limit", 3)] {
                    if let Some(n) = wire::Fields::new(a)
                        .find(|(no, _)| *no == field)
                        .and_then(|(_, v)| v.i32())
                    {
                        obj.insert(key.into(), Value::Number(n.into()));
                    }
                }
                tool.input = Some(Value::Object(obj));
            }
            if let Some(r) = &result {
                if let Some(success) = wire::first_message(r, 1) {
                    // content oneof: 1 inline string / 10 content blob /
                    //                6 binary data / 9 binary blob id
                    tool.output = wire::first_str(&success, 1)
                        .filter(|s| !s.is_empty())
                        .or_else(|| {
                            wire::first_bytes(&success, 10)
                                .and_then(|id| read_blob(conn, &id))
                                .and_then(|b| String::from_utf8(b).ok())
                        })
                        .or_else(|| {
                            let has_binary = wire::first_bytes(&success, 6).is_some()
                                || wire::first_bytes(&success, 9).is_some();
                            has_binary.then(|| "<binary file content>".to_string())
                        });
                } else if let Some(err) = wire::first_message(r, 2) {
                    tool.output = wire::first_str(&err, 1);
                    tool.is_error = true;
                }
            }
        }
        // update_todos
        9 => {
            if let Some(a) = &args {
                let todos: Vec<Value> = wire::Fields::new(a)
                    .filter(|(no, _)| *no == 1)
                    .filter_map(|(_, v)| v.bytes().map(<[u8]>::to_vec))
                    .filter_map(|item| {
                        wire::first_str(&item, 2).map(|content| {
                            serde_json::json!({ "content": content })
                        })
                    })
                    .collect();
                tool.input = Some(serde_json::json!({ "todos": todos }));
            }
        }
        // edit
        12 => {
            tool.input = args
                .as_deref()
                .map(|a| json_obj(&[("file_path", wire::first_str(a, 1))]));
            if let Some(r) = &result {
                for (no, val) in wire::Fields::new(r) {
                    let Some(b) = val.bytes() else { continue };
                    match no {
                        // success: prefer the ready-made diff, else message
                        1 => {
                            tool.output = wire::first_str(b, 5)
                                .filter(|s| !s.is_empty())
                                .or_else(|| wire::first_str(b, 8));
                            break;
                        }
                        // 2 file_not_found / 3+4 permission / 6 rejected / 7 error
                        2..=7 => {
                            tool.output = wire::first_str(b, 2).or_else(|| wire::first_str(b, 1));
                            tool.is_error = true;
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        // ls
        13 => {
            tool.input = args
                .as_deref()
                .map(|a| json_obj(&[("path", wire::first_str(a, 1))]));
        }
        // mcp
        15 => {
            if let Some(a) = &args {
                let server = wire::first_str(a, 9)
                    .or_else(|| wire::first_str(a, 4))
                    .filter(|s| !s.is_empty());
                let tool_name = wire::first_str(a, 5)
                    .filter(|s| !s.is_empty())
                    .or_else(|| wire::first_str(a, 1));
                tool.name = match (server, tool_name) {
                    (Some(server), Some(t)) => format!("{server}__{t}"),
                    (None, Some(t)) => t,
                    _ => "mcp".to_string(),
                };
                // `McpArgs.args` is map<string, google.protobuf.Value>: the
                // map entries are the repeated field-2 occurrences on McpArgs
                // itself (there is no wrapping Struct message).
                let decoded = decode_value_map(a, 2);
                if decoded.as_object().is_some_and(|m| !m.is_empty()) {
                    tool.input = Some(decoded);
                }
            }
            if let Some(r) = &result {
                if let Some(success) = wire::first_message(r, 1) {
                    let mut texts: Vec<String> = Vec::new();
                    for (no, val) in wire::Fields::new(&success) {
                        if no == 1 {
                            if let Some(item) = val.bytes() {
                                if let Some(text_msg) = wire::first_message(item, 1) {
                                    if let Some(t) = wire::first_str(&text_msg, 1) {
                                        texts.push(t);
                                    }
                                }
                            }
                        } else if no == 2 {
                            tool.is_error = val.u64() == Some(1);
                        }
                    }
                    if !texts.is_empty() {
                        tool.output = Some(texts.join("\n"));
                    }
                } else if let Some(err) = wire::first_message(r, 2) {
                    tool.output = wire::first_str(&err, 1);
                    tool.is_error = true;
                }
            }
        }
        // sem_search
        16 => {
            if let Some(success) = result.as_deref().and_then(|r| wire::first_message(r, 1)) {
                tool.output = wire::first_str(&success, 1).filter(|s| !s.is_empty());
            }
        }
        // create_plan
        17 => {
            if let Some(a) = &args {
                tool.input = Some(json_obj(&[("name", wire::first_str(a, 4))]));
                tool.output = wire::first_str(a, 1).filter(|s| !s.is_empty());
            }
        }
        // web_search
        18 => {
            tool.input = args
                .as_deref()
                .map(|a| json_obj(&[("query", wire::first_str(a, 1))]));
        }
        // task (sub-agent)
        19 => {
            if let Some(a) = &args {
                // `subagent_type` is a oneof of empty marker messages; the
                // variant order matches the CLI's own enum error text
                // ("generalPurpose | cursor-guide | best-of-n-runner").
                let subagent_type = wire::first_message(a, 3).and_then(|m| {
                    wire::Fields::new(&m).next().map(|(no, _)| {
                        match no {
                            1 => "generalPurpose",
                            2 => "cursor-guide",
                            3 => "best-of-n-runner",
                            _ => "subagent",
                        }
                        .to_string()
                    })
                });
                tool.input = Some(json_obj(&[
                    ("description", wire::first_str(a, 1)),
                    ("prompt", wire::first_str(a, 2)),
                    ("subagent_type", subagent_type),
                    ("model", wire::first_str(a, 4)),
                ]));
            }
            if let Some(r) = &result {
                // oneof: 1 success {1 report (two rich-text wrapper layers),
                //                   2 sub-chat id, 4 duration_ms}
                //        2 error {1 message}
                if let Some(success) = wire::first_message(r, 1) {
                    tool.output = task_report_text(&success);
                } else if let Some(err) = wire::first_message(r, 2) {
                    tool.output = wire::first_str(&err, 1);
                    tool.is_error = true;
                }
            }
        }
        // ask_question
        23 => {
            if let Some(a) = &args {
                let questions: Vec<Value> = wire::Fields::new(a)
                    .filter(|(no, _)| *no == 2)
                    .filter_map(|(_, v)| v.bytes().map(<[u8]>::to_vec))
                    .filter_map(|q| wire::first_str(&q, 2).map(Value::String))
                    .collect();
                tool.input = Some(serde_json::json!({
                    "title": wire::first_str(a, 1),
                    "questions": questions,
                }));
            }
        }
        // fetch / web_fetch
        24 | 37 => {
            tool.input = args
                .as_deref()
                .map(|a| json_obj(&[("url", wire::first_str(a, 1))]));
        }
        _ => {}
    }

    tool
}

/// A task success's report text sits at a fixed depth:
/// `success.f1 (rich-text container) → .f1 (paragraph wrapper) → .f1 (text)`.
///
/// On the wire, `bytes` and an embedded message are the same type — a text
/// payload can be indistinguishable from a wrapper (e.g. `"\n#"` + exactly 35
/// bytes IS a complete field-1 frame), so no decoder can pick the layer count
/// from bytes alone. This walk fails toward the SAFE side: descending one
/// level only when the current field-1 payload cannot be the text itself
/// (not valid printable UTF-8) and decodes end-to-end as a message. A
/// layer-count drift then returns the full text or nothing — never a
/// truncated substring. Residual trade-off: a report short enough (< 128
/// bytes) for its wrapper frame to read as printable ASCII surfaces with a
/// two-byte prefix instead of descending — mangled-prefix beats truncation.
fn task_report_text(success: &[u8]) -> Option<String> {
    let mut layer: Vec<u8> = wire::first_message(success, 1)?;
    // Real schema is two wrapper hops; bound the walk defensively.
    for _ in 0..3 {
        let payload = wire::first_bytes(&layer, 1)?;
        if let Ok(text) = std::str::from_utf8(&payload) {
            if is_report_text(text) {
                return Some(text.to_string());
            }
        }
        if !wire::is_complete_message(&payload) {
            return None;
        }
        layer = payload;
    }
    None
}

/// Printable text (common whitespace allowed) with non-whitespace content —
/// the shape of a real report body, as opposed to a wrapper frame whose
/// length prefix usually injects control/invalid bytes.
fn is_report_text(s: &str) -> bool {
    !s.trim().is_empty()
        && s.chars()
            .all(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
}

fn json_obj(entries: &[(&str, Option<String>)]) -> Value {
    let mut map = serde_json::Map::new();
    for (key, value) in entries {
        if let Some(v) = value {
            if !v.is_empty() {
                map.insert((*key).to_string(), Value::String(v.clone()));
            }
        }
    }
    Value::Object(map)
}

fn join_streams(stdout: &str, stderr: &str) -> Option<String> {
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => None,
        (false, true) => Some(stdout.to_string()),
        (true, false) => Some(stderr.to_string()),
        (false, false) => Some(format!("{stdout}\n{stderr}")),
    }
}

fn bounded_json_preview_ref(value: &Value) -> Option<String> {
    bounded_json_preview(value)
}

/// Serialize a tool input as VALID JSON bounded by `CURSOR_TOOL_INPUT_CAP`:
/// string values are truncated (structure preserved) and the per-string cap
/// halves until the whole serialized preview fits, so downstream `JSON.parse`
/// (e.g. the delegation card) always succeeds. Mirrors the Grok parser.
fn bounded_json_preview(value: &Value) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let mut per_string = CURSOR_TOOL_INPUT_CAP;
    loop {
        let serialized = serde_json::to_string(&cap_json_string_values(value, per_string)).ok()?;
        if serialized.len() <= CURSOR_TOOL_INPUT_CAP || per_string == 0 {
            return Some(serialized);
        }
        per_string /= 2;
    }
}

fn cap_json_string_values(value: &Value, cap: usize) -> Value {
    match value {
        Value::String(s) => Value::String(truncate_str(s, cap)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|v| cap_json_string_values(v, cap))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), cap_json_string_values(v, cap)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Immediate subdirectories of `dir` (non-recursive). Missing dir → empty.
fn read_subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = crate::scoped_fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

