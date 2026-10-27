use std::collections::HashMap;
use std::ffi::OsString;
use crate::scoped_fs as fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::models::{
    AgentType, ContentBlock, ConversationDetail, ConversationSummary, MessageTurn, TurnRole,
    TurnUsage,
};
use crate::parsers::expand_home_prefix;
use crate::parsers::{
    backfill_turn_durations, compute_session_stats, folder_name_from_path,
    infer_context_window_max_tokens, merge_context_window_stats, relocate_orphaned_tool_results,
    resolve_patch_line_numbers, structurize_read_tool_output, title_from_user_text, truncate_str,
    AgentParser, ParseError,
};

/// Resolve the DeepSeek Harness home the way `@deepseek-ai/dsh-home-paths`'
/// `resolveDshHome` does: `DSH_HOME` wins, else `~/.dsh`.
///
/// Two details are upstream's, not conveniences: a WHITESPACE-only `DSH_HOME`
/// counts as unset (`fromEnv.trim().length > 0`), so a blank override never
/// resolves the home to a whitespace path; and a leading `~` is expanded
/// (`expandHomePath`) before use. The value itself is otherwise passed through
/// untrimmed, again like upstream. Upstream additionally absolutizes a
/// relative value against the harness process's cwd — deliberately NOT
/// mirrored, because that cwd is the session workspace, not coaxi's, so
/// resolving it here would name a directory the agent never uses.
fn resolve_dsh_home_from(dsh_home_env: Option<OsString>, home_dir: Option<PathBuf>) -> PathBuf {
    dsh_home_env
        .filter(|value| !value.to_string_lossy().trim().is_empty())
        .map(|value| expand_home_prefix(&value.to_string_lossy(), home_dir.as_ref()))
        .unwrap_or_else(|| home_dir.unwrap_or_default().join(".dsh"))
}

/// The DeepSeek Harness home (`DSH_HOME`, default `~/.dsh`) — the root of the
/// skills store, the credentials document, and (unless relocated) the session
/// logs.
pub(crate) fn resolve_dsh_home_dir() -> PathBuf {
    resolve_dsh_home_from(std::env::var_os("DSH_HOME"), dirs::home_dir())
}

/// Resolve the shared cross-agent home the way `dsh-skill-filesystem` does:
/// `DSH_AGENTS_HOME` wins, else `~/.agents`. Its `skills/` subdirectory is the
/// same store Codex, pi, Cursor and OpenCode read, so a skill linked there is
/// visible to DeepSeek too.
pub(crate) fn resolve_dsh_agents_home_dir() -> PathBuf {
    resolve_dsh_agents_home_from(std::env::var_os("DSH_AGENTS_HOME"), dirs::home_dir())
}

fn resolve_dsh_agents_home_from(
    agents_home_env: Option<OsString>,
    home_dir: Option<PathBuf>,
) -> PathBuf {
    agents_home_env
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir.unwrap_or_default().join(".agents"))
}

/// Resolve the session-log root the way deepseek-acp's `sessionsRoot` does:
/// `DEEPSEEK_ACP_SESSIONS_ROOT` wins, else `$DSH_HOME/sessions`, else
/// `~/.dsh/sessions`.
pub(crate) fn resolve_deepseek_sessions_root() -> PathBuf {
    resolve_deepseek_sessions_root_from(
        std::env::var_os("DEEPSEEK_ACP_SESSIONS_ROOT"),
        std::env::var_os("DSH_HOME"),
        dirs::home_dir(),
    )
}

fn resolve_deepseek_sessions_root_from(
    sessions_env: Option<OsString>,
    dsh_home_env: Option<OsString>,
    home_dir: Option<PathBuf>,
) -> PathBuf {
    sessions_env
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_dsh_home_from(dsh_home_env, home_dir).join("sessions"))
}

/// Resolve the image attachment store the way `dsh-attachment-local` does:
/// always `$DSH_HOME/attachments/v1`, with no override of its own.
///
/// Deliberately NOT derived from the sessions root:
/// `DEEPSEEK_ACP_SESSIONS_ROOT` relocates the logs alone, so a deployment that
/// uses it keeps its attachments under `$DSH_HOME` all the same.
pub(crate) fn resolve_deepseek_attachments_root() -> PathBuf {
    resolve_dsh_home_dir().join("attachments").join("v1")
}

/// DeepSeek Harness (driven through the `deepseek-acp` editor bridge) persists
/// each session as an **append-only event log**:
///
/// ```text
/// $DSH_HOME/sessions/                  (default ~/.dsh/sessions; whole root
/// └── <munged cwd>/                     relocatable via DEEPSEEK_ACP_SESSIONS_ROOT)
///     └── <session uuid>/
///         ├── session.jsonl.zstd       # generation 0 (deepseek-acp <= 0.8.0)
///         └── session.v3.jsonl.zstd    # generation 3 (deepseek-acp >= 0.9.0)
/// ```
///
/// Since 0.9.0 the log is **generation-addressed**: generation 0 keeps the
/// original suffix-only name, every later one carries a lowercase `.vN`
/// component, and the numerically highest canonical generation is the live
/// one. A migration publishes its successor beside the predecessor rather than
/// replacing it, so BOTH names can sit in one directory — see
/// [`resolve_generation_log_path`], which is the only thing here that knows the
/// naming. A `compression: "none"` deployment writes the same names without the
/// `.zstd` suffix.
///
/// The `.zstd` file is a sequence of complete Zstandard frames (one appended
/// per write batch); a standard streaming decode walks them all. Each line is
/// `{type, seq, time, data}` (`time` in epoch ms). The first line is the
/// session header (`type: "session"`, carrying `id` / `createdAt` / `cwd` /
/// `delegationDepth`). Content comes from:
///
/// - `user/message` — `data.source.kind` separates a real human prompt
///   (`"user"`) from plugin-injected runtime context (`"plugin"`, skipped).
///   Since 0.6.0 its `content[]` may hold `image` blocks alongside the text;
///   a compaction's replacement message rides the same `"plugin"` skip
///   (`{kind: "plugin", plugin: "compact"}`), so the summary text never shows
///   up as something the user said.
/// - `assistant/message` — the ASSEMBLED assistant message for one step
///   (`content[]` of `text` / `reasoning` / `tool-call {id, name, arguments}`
///   blocks) plus that step's `usage` (`inputTokens` / `outputTokens` /
///   `cacheReadTokens` / `reasoningTokens`). Through 0.8.0 the raw stream
///   duplicated it as `assistant/chunk` / `*-chunks` ROWS; 0.9.0 stopped
///   persisting those and moved the compacted stream into this event's own
///   `stream` field, which is read as part of the event rather than as a row.
///   Either way only `message.content` is read — see the skip below.
/// - `tool/result` — the paired result (`message.content[0]` is a
///   `tool-result` with `toolCallId` / `content[]` / `isError`).
/// - `turn/start` / `turn/end` — authoritative turn boundaries; `turn/end`
///   carries the end reason (`completed` / `aborted` / `error` / …) and its
///   `time` is the turn's completion clock. A turn missing its `turn/end`
///   (killed mid-flight) keeps honest `None` clocks.
/// - `request/header` — per-request `config.model`; `request/context` — the
///   advertised `contextWindow`; `session/title` — the running title
///   (last one wins).
/// - `compaction/start` → `compaction/summary` → `compaction/end` (0.6.0) —
///   one context compaction, rendered as the shared context-compaction marker.
///
/// Images (0.6.0): an `image` content block carries only an
/// `ImageAttachmentRef`; the bytes live in the content-addressed store at
/// `$DSH_HOME/attachments/v1/objects/<xx>/<sha256>`. The detail path inlines
/// them, the list path does not — see [`attachment_image_block`].
///
/// Usage semantics: per-step records are SUMMED into the turn (mirroring the
/// Kimi parser); the context-window occupancy is the LATEST step's input side
/// (`inputTokens + cacheReadTokens`) — never the per-turn sum, which re-counts
/// the cached prefix once per step.
pub struct DeepSeekParser {
    base_dir: PathBuf,
    /// Root of the content-addressed attachment store (`$DSH_HOME/attachments/
    /// v1`), where the pixels behind an `image` content block live. Resolved
    /// separately from `base_dir`: `DEEPSEEK_ACP_SESSIONS_ROOT` moves the logs
    /// without moving the store.
    attachments_root: PathBuf,
}

impl DeepSeekParser {
    pub fn new() -> Self {
        Self {
            base_dir: resolve_deepseek_sessions_root(),
            attachments_root: resolve_deepseek_attachments_root(),
        }
    }

    /// Construct a parser pointed at an explicit sessions directory (test
    /// fixtures).
    ///
    /// The attachment store is placed at `<base_dir>/attachments/v1` rather
    /// than resolved from the environment: a fixture that stages no image bytes
    /// must not fall through to the developer's real `~/.dsh/attachments`. The
    /// real layout puts the two beside each other under `$DSH_HOME` instead,
    /// which is why production resolves them separately — the sessions root is
    /// independently relocatable via `DEEPSEEK_ACP_SESSIONS_ROOT`.
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        let attachments_root = base_dir.join("attachments").join("v1");
        Self {
            base_dir,
            attachments_root,
        }
    }

    fn build_summary(&self, session_dir: &Path, session_id: &str) -> Option<ConversationSummary> {
        // No attachment root: a summary renders no blocks, so inlining image
        // bytes here would read (and base64) every attachment in every listed
        // session for a payload nobody looks at.
        let parsed = parse_session_log(session_dir, None)?;
        // Sub-agent sessions (spawned by an upstream harness delegation) are
        // not editor conversations; only the top-level depth is listed.
        if parsed.delegation_depth > 0 {
            return None;
        }
        // A session that never produced a user/assistant/tool event (only the
        // header + config records) is treated as empty, matching the
        // "metadata-only is not listed" rule of the other parsers.
        if parsed.content_events == 0 {
            return None;
        }
        let started_at = parsed.created_at.or(parsed.first_ts)?;

        Some(ConversationSummary {
            id: session_id.to_string(),
            agent_type: AgentType::DeepSeek,
            folder_name: parsed.cwd.as_deref().map(folder_name_from_path),
            folder_path: parsed.cwd,
            title: parsed.title.or(parsed.first_user_text),
            started_at,
            ended_at: parsed.last_ts,
            message_count: parsed.message_count,
            model: parsed.model,
            git_branch: None,
            parent_id: None,
            parent_tool_use_id: None,
            delegation_call_id: None,
        })
    }

    fn build_detail(&self, session_dir: &Path, conversation_id: &str) -> ConversationDetail {
        let parsed =
            parse_session_log(session_dir, Some(&self.attachments_root)).unwrap_or_default();

        let mut turns = parsed.turns;
        relocate_orphaned_tool_results(&mut turns);
        structurize_read_tool_output(&mut turns);
        resolve_patch_line_numbers(&mut turns, parsed.cwd.as_deref());
        backfill_turn_durations(&mut turns, &[]);

        // Context-window occupancy is the LATEST step's input side; capacity
        // comes from the log's own `request/context.contextWindow` (falling
        // back to model-name inference for logs that never recorded one).
        let max_tokens = parsed
            .context_window
            .or_else(|| infer_context_window_max_tokens(parsed.model.as_deref()));
        let session_stats = merge_context_window_stats(
            compute_session_stats(&turns),
            parsed.last_step_input_side,
            max_tokens,
        );

        let summary = ConversationSummary {
            id: conversation_id.to_string(),
            agent_type: AgentType::DeepSeek,
            folder_name: parsed.cwd.as_deref().map(folder_name_from_path),
            folder_path: parsed.cwd,
            title: parsed.title.or(parsed.first_user_text),
            started_at: parsed
                .created_at
                .or(parsed.first_ts)
                .unwrap_or_else(Utc::now),
            ended_at: parsed.last_ts,
            message_count: parsed.message_count,
            model: parsed.model,
            git_branch: None,
            parent_id: None,
            parent_tool_use_id: None,
            delegation_call_id: None,
        };

        ConversationDetail {
            summary,
            turns,
            session_stats,
            transcript_watermark: None,
        }
    }

    /// Locate the `<session uuid>` directory matching `conversation_id` across
    /// the `base_dir/<munged cwd>/` buckets (two shallow levels).
    fn find_session_dir(&self, conversation_id: &str) -> Option<PathBuf> {
        for bucket in read_subdirs(&self.base_dir) {
            let candidate = bucket.join(conversation_id);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
        None
    }
}

impl Default for DeepSeekParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentParser for DeepSeekParser {
    fn list_conversations(&self) -> Result<Vec<ConversationSummary>, ParseError> {
        let mut conversations = Vec::new();
        if !self.base_dir.is_dir() {
            return Ok(conversations);
        }

        for bucket in read_subdirs(&self.base_dir) {
            for session_dir in read_subdirs(&bucket) {
                let Some(session_id) = session_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                else {
                    continue;
                };
                if let Some(summary) = self.build_summary(&session_dir, &session_id) {
                    conversations.push(summary);
                }
            }
        }

        conversations.sort_by_key(|c| std::cmp::Reverse(c.started_at));
        Ok(conversations)
    }

    fn get_conversation(&self, conversation_id: &str) -> Result<ConversationDetail, ParseError> {
        let Some(session_dir) = self.find_session_dir(conversation_id) else {
            return Err(ParseError::ConversationNotFound(
                conversation_id.to_string(),
            ));
        };
        Ok(self.build_detail(&session_dir, conversation_id))
    }
}

/// The accumulated result of scanning one session log.
#[derive(Default)]
struct SessionParse {
    turns: Vec<MessageTurn>,
    cwd: Option<String>,
    created_at: Option<DateTime<Utc>>,
    delegation_depth: u64,
    /// Latest `session/title` (the title service re-emits as it improves).
    title: Option<String>,
    /// First user prompt, already truncated for use as a fallback title.
    first_user_text: Option<String>,
    /// Latest `request/header` model (fallback: the assistant message source).
    model: Option<String>,
    /// Latest `request/context.contextWindow`.
    context_window: Option<u64>,
    /// The latest single step's input side (`inputTokens + cacheReadTokens`) —
    /// the context-window occupancy. NOT the per-turn sum (see the type doc).
    last_step_input_side: Option<u64>,
    first_ts: Option<DateTime<Utc>>,
    last_ts: Option<DateTime<Utc>>,
    /// User prompts + assistant text messages, the list view's activity count.
    message_count: u32,
    /// Content-bearing records — whether the session is worth listing at all.
    content_events: u32,
}

/// The physical encoding of one session log: Zstandard frames (upstream's
/// default, and the only one `deepseek-acp` itself configures) or the plain
/// newline-delimited text a `compression: "none"` deployment writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogEncoding {
    Zstd,
    Raw,
}

/// Read one session-directory entry name as a canonical generation log,
/// mirroring upstream's `parseSessionFormatLogFilename` plus the compression
/// suffix `dsh-session-persistence-jsonl` appends around it:
///
/// ```text
/// /^session(?:\.v([1-9][0-9]*))?\.jsonl$/u   (+ optional ".zstd")
/// ```
///
/// Generation 0 keeps the original suffix-only `session.jsonl`; every later
/// generation carries a lowercase `.vN` component. Everything else is NOT a
/// committed generation and must be ignored — which is what keeps the sibling
/// `session.lock`, the `session.migration.<token>.jsonl.zstd.tmp` staging file
/// a migration publishes through, and the deliberately non-canonical `.v0` /
/// leading-zero / uppercase spellings out of the selection.
fn parse_generation_log_filename(name: &str) -> Option<(u64, LogEncoding)> {
    /// Upstream refuses a version outside JavaScript's safe-integer range
    /// rather than saturating it, so the accepted band is exactly
    /// `1..=Number.MAX_SAFE_INTEGER`. A narrower Rust integer would not merely
    /// reject an absurd name: a generation it cannot hold stops being a
    /// generation, and a RETAINED PREDECESSOR beside it would then win — the
    /// silent-truncation bug this whole function exists to prevent.
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

    let (stem, encoding) = match name.strip_suffix(".zstd") {
        Some(stem) => (stem, LogEncoding::Zstd),
        None => (name, LogEncoding::Raw),
    };
    let stem = stem.strip_suffix(".jsonl")?;
    if stem == "session" {
        return Some((0, encoding));
    }
    let digits = stem.strip_prefix("session.v")?;
    // `[1-9][0-9]*`: no `.v0`, no leading zeros, no sign, decimal only.
    let mut chars = digits.chars();
    if !matches!(chars.next(), Some('1'..='9')) {
        return None;
    }
    if !chars.all(|c| c.is_ascii_digit()) {
        return None;
    }
    let version = digits.parse::<u64>().ok()?;
    if version > MAX_SAFE_INTEGER {
        return None;
    }
    Some((version, encoding))
}

/// Pick the log a session directory is currently addressed by.
///
/// **Encoding first, generation second.** Upstream states that a root belongs
/// to exactly one encoding and that lookup REJECTS generations carrying the
/// other suffix, so comparing generations across encodings would let a stale
/// file from a foreign encoding outrank the live one. A root holding both is
/// malformed by that rule; coaxi prefers the Zstandard set because
/// `deepseek-acp` never configures `compression`, so every generation it has
/// ever written is compressed — a raw file sharing the root is hand-placed or
/// from a non-stock deployment. The alternative (refuse to read an ambiguous
/// root) trades a documented guess for an empty conversation, which is the
/// very failure this function exists to fix.
///
/// Within the chosen encoding the NUMERICALLY HIGHEST canonical generation
/// wins, exactly as upstream's runtime operations select. There is deliberately
/// no fallback to a lower generation: a write open migrates the log and
/// publishes the successor while leaving the predecessor byte-identical on
/// disk, so falling back would render history frozen at the migration point as
/// if it were current — silently, with every turn appended afterwards missing.
/// An unreadable newest generation yields nothing, and nothing is the honest
/// answer. A listing that BREAKS PART WAY THROUGH is the same case: it cannot
/// prove which generation is newest, so it yields nothing too rather than
/// crowning whichever one happened to arrive first.
fn resolve_generation_log_path(session_dir: &Path) -> Option<(PathBuf, LogEncoding)> {
    let Ok(entries) = fs::read_dir(session_dir) else {
        return None;
    };
    select_generation_log(
        session_dir,
        entries.map(|entry| entry.map(|entry| (entry.file_name(), entry.path()))),
    )
}

/// The selection itself, over an already-opened listing.
///
/// Split out from [`resolve_generation_log_path`] only so a test can hand it a
/// listing that FAILS PART WAY THROUGH — the one branch here no real temporary
/// directory can be made to take.
fn select_generation_log<I>(session_dir: &Path, entries: I) -> Option<(PathBuf, LogEncoding)>
where
    I: IntoIterator<Item = std::io::Result<(OsString, PathBuf)>>,
{
    let mut best_zstd: Option<(u64, PathBuf)> = None;
    let mut best_raw: Option<(u64, PathBuf)> = None;
    for entry in entries {
        // `readdir` can fail after already yielding entries — an NFS handle
        // going stale, a FUSE mount erroring mid-stream. Skipping the failure
        // and keeping what arrived would silently select a retained
        // predecessor whenever the successor is the entry that never came.
        let Ok((name, path)) = entry else {
            tracing::warn!(
                "DeepSeek session {}: listing failed part way through; \
                 refusing to pick a generation from a partial listing",
                session_dir.display()
            );
            return None;
        };
        let Some(name) = name.to_str() else { continue };
        let Some((version, encoding)) = parse_generation_log_filename(name) else {
            continue;
        };
        // Follows symlinks, like `read_subdirs`: a directory that happens to be
        // named like a log is not one.
        if !path.is_file() {
            continue;
        }
        let slot = match encoding {
            LogEncoding::Zstd => &mut best_zstd,
            LogEncoding::Raw => &mut best_raw,
        };
        if slot.as_ref().is_none_or(|(best, _)| version > *best) {
            *slot = Some((version, path));
        }
    }
    match (best_zstd, best_raw) {
        (Some((_, zstd)), Some((_, raw))) => {
            tracing::warn!(
                "DeepSeek session {} holds both encodings; reading {} and ignoring {}",
                session_dir.display(),
                zstd.display(),
                raw.display()
            );
            Some((zstd, LogEncoding::Zstd))
        }
        (Some((_, zstd)), None) => Some((zstd, LogEncoding::Zstd)),
        (None, Some((_, raw))) => Some((raw, LogEncoding::Raw)),
        (None, None) => None,
    }
}

/// Read a session's log text from whichever generation currently addresses it.
fn read_session_log_text(session_dir: &Path) -> Option<String> {
    match resolve_generation_log_path(session_dir)? {
        (path, LogEncoding::Zstd) => decode_zstd_frames_prefix(&fs::read(path).ok()?),
        (path, LogEncoding::Raw) => fs::read_to_string(path).ok(),
    }
}

/// Decode every complete Zstandard frame, KEEPING the prefix when the stream
/// errors partway. The writer appends one frame per batch, so a concurrent
/// read can catch the final frame half-written — failing the whole decode
/// there would make a LIVE session vanish from the list until the next flush.
/// The JSONL walk upstream skips whatever broken tail line the cut leaves.
fn decode_zstd_frames_prefix(bytes: &[u8]) -> Option<String> {
    use std::io::Read as _;
    let mut decoder = zstd::stream::read::Decoder::with_buffer(bytes).ok()?;
    let mut decoded = Vec::new();
    // On Err, `read_to_end` has already appended every byte that decoded
    // cleanly before the failure — exactly the prefix to keep.
    let _ = decoder.read_to_end(&mut decoded);
    // Lossy: one bad byte must not drop the whole session; the JSON lines
    // that decode cleanly still parse.
    Some(String::from_utf8_lossy(&decoded).into_owned())
}

fn parse_session_log(session_dir: &Path, attachments: Option<&Path>) -> Option<SessionParse> {
    Some(parse_session_events(
        &read_session_log_text(session_dir)?,
        attachments,
    ))
}

/// Byte ceiling for inlining one attachment into a history turn.
///
/// Under upstream's stock policy this never fires: the store holds NORMALIZED
/// bytes, and normalization re-encodes (and downscales, repeatedly) until the
/// object fits `normalizedImageMaxBytes` / `normalizedImageMaxDimension` —
/// 4 MiB and 2048px by default, refusing the write outright if it cannot. The
/// far larger `maxImageBytes` (20 MB) and `maxImageDimension` (8192) are
/// INPUT-admission limits, i.e. what a prompt may hand in, not what lands on
/// disk.
///
/// It exists because both of those are deployment-configurable and because
/// base64 inflates whatever it does find by ~4/3 into a single conversation
/// payload. Over the ceiling the block degrades to a text note rather than
/// disappearing — the same trade `attachment_placeholder` makes for a missing
/// object.
const DEEPSEEK_MAX_INLINE_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

/// One `image` content block → the block history should show.
///
/// The log stores only an `ImageAttachmentRef` (`{attachmentId, mediaType,
/// bytes, width, height, name?}`); the pixels live in the content-addressed
/// store at `$DSH_HOME/attachments/v1/objects/<first two hex>/<sha256 hex>`,
/// where `attachmentId` is the string `sha256:<hex>`.
///
/// `attachments` is `None` on the LIST path: building a summary never renders
/// blocks, so reading (and base64-ing) every image in every session just to
/// count turns would be pure cost. Both paths still produce a block, because
/// the alternative is worse than an unrendered image — an image-only prompt
/// ("what is wrong with this screenshot?") carries no text at all, and dropping
/// its block would drop the whole user turn, leaving an answer to a question
/// that is not there.
fn attachment_image_block(attachment: Option<&Value>, attachments: Option<&Path>) -> ContentBlock {
    let Some(attachment) = attachment else {
        return attachment_placeholder(None, None);
    };
    let media_type = attachment
        .get("mediaType")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|m| !m.is_empty());
    let name = attachment
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty());
    let placeholder = || attachment_placeholder(name, media_type);

    let (Some(mime), Some(root)) = (media_type, attachments) else {
        return placeholder();
    };
    let Some(hex) = attachment
        .get("attachmentId")
        .and_then(Value::as_str)
        .and_then(attachment_object_hex)
    else {
        return placeholder();
    };
    if attachment
        .get("bytes")
        .and_then(Value::as_u64)
        .is_some_and(|bytes| bytes > DEEPSEEK_MAX_INLINE_IMAGE_BYTES)
    {
        return placeholder();
    }
    let object = root.join("objects").join(&hex[..2]).join(&hex);
    // Size the file on disk BEFORE reading it. The log's own `bytes` field is
    // checked above, but it is optional and it is the log's claim — a truncated
    // or corrupt object would otherwise be read into memory in full and only
    // then rejected, which is the one allocation the ceiling is meant to stop.
    let Ok(meta) = fs::metadata(&object) else {
        return placeholder();
    };
    if meta.len() == 0 || meta.len() > DEEPSEEK_MAX_INLINE_IMAGE_BYTES {
        return placeholder();
    }
    let Ok(bytes) = fs::read(&object) else {
        return placeholder();
    };
    // The file can grow between the stat and the read; the store is
    // content-addressed and its writes are atomic renames, so this is
    // belt-and-braces rather than an expected path.
    if bytes.is_empty() || bytes.len() as u64 > DEEPSEEK_MAX_INLINE_IMAGE_BYTES {
        return placeholder();
    }
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    ContentBlock::Image {
        data: STANDARD.encode(&bytes),
        mime_type: mime.to_string(),
        // The store is content-addressed, so the id is a digest, not a name.
        // Handing it over as the `uri` would label the thumbnail with 64 hex
        // characters; `None` lets the frontend derive `image.<ext>` from the
        // mime instead. A ref that kept the user's own file name uses that.
        uri: name.map(String::from),
    }
}

/// The stand-in for an image whose pixels are not being inlined — the list
/// path, an object that is gone (the store is prunable independently of the
/// logs), or one too large to inline. Names whatever the ref knew so the turn
/// still says what was attached.
fn attachment_placeholder(name: Option<&str>, media_type: Option<&str>) -> ContentBlock {
    let detail = match (name, media_type) {
        (Some(name), _) => format!(" {name}"),
        (None, Some(media_type)) => format!(" {media_type}"),
        (None, None) => String::new(),
    };
    ContentBlock::Text {
        text: format!("[image{detail}]"),
    }
}

/// The object's sha256 hex, or `None` when the id is not the `sha256:<64 hex>`
/// form the local store mints. Validated rather than split because the hex goes
/// straight into a path: a `../…` id must not become a directory traversal.
fn attachment_object_hex(attachment_id: &str) -> Option<String> {
    let hex = attachment_id.strip_prefix("sha256:")?;
    // Lowercase only: upstream formats the digest with `toString('hex')`, and
    // accepting other spellings would invent ids the store never wrote.
    (hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .then(|| hex.to_string())
}

/// Tool-call arguments arrive as the model's raw JSON string. Store it
/// verbatim when small; over the cap, re-serialize with string VALUES capped so
/// the stored preview stays VALID JSON (the delegation card `JSON.parse`s it to
/// recover `task` / `agent_type` — an opaque byte truncation would corrupt it).
/// Unparseable arguments (a model that emitted broken JSON) fall back to the
/// plain truncation, which cannot make them any less parseable.
const DEEPSEEK_TOOL_INPUT_CAP: usize = 8_000;

fn deepseek_tool_input_preview(arguments: &str) -> Option<String> {
    let trimmed = arguments.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.len() <= DEEPSEEK_TOOL_INPUT_CAP {
        return Some(trimmed.to_string());
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => crate::parsers::grok::cap_json_to_budget(&value, DEEPSEEK_TOOL_INPUT_CAP),
        Err(_) => Some(truncate_str(trimmed, DEEPSEEK_TOOL_INPUT_CAP)),
    }
}

fn event_millis(value: &Value) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp_millis(value.get("time")?.as_i64()?)
}

/// Render every `{type:"image"}` part of a message `content[]` array, in wire
/// order. Empty when the message carries none, which is every message written
/// before 0.6.0 and most written after it.
fn user_image_blocks(content: Option<&Value>, attachments: Option<&Path>) -> Vec<ContentBlock> {
    let Some(items) = content.and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("image"))
        .map(|item| attachment_image_block(item.get("attachment"), attachments))
        .collect()
}

/// Concatenate the `text` of every `{type:"text"}` part in a message
/// `content[]` array.
fn collect_text_parts(content: Option<&Value>) -> String {
    let mut out = String::new();
    if let Some(items) = content.and_then(Value::as_array) {
        for item in items {
            if item.get("type").and_then(Value::as_str) == Some("text") {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(text);
                }
            }
        }
    }
    out
}

fn add_usage(a: TurnUsage, b: TurnUsage) -> TurnUsage {
    TurnUsage {
        input_tokens: a.input_tokens.saturating_add(b.input_tokens),
        output_tokens: a.output_tokens.saturating_add(b.output_tokens),
        cache_creation_input_tokens: a
            .cache_creation_input_tokens
            .saturating_add(b.cache_creation_input_tokens),
        cache_read_input_tokens: a
            .cache_read_input_tokens
            .saturating_add(b.cache_read_input_tokens),
    }
}

/// Map one step's `usage` object onto `TurnUsage`; `None` when all counters
/// are absent or zero. `reasoningTokens` is a subset of `outputTokens` (the
/// adapter reports both), so it is deliberately not added anywhere.
fn usage_from_step(usage: Option<&Value>) -> Option<TurnUsage> {
    let usage = usage?;
    let field = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let input = field("inputTokens");
    let output = field("outputTokens");
    let cache_read = field("cacheReadTokens");
    if input == 0 && output == 0 && cache_read == 0 {
        return None;
    }
    Some(TurnUsage {
        input_tokens: input,
        output_tokens: output,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: cache_read,
    })
}

/// What a compaction's opening and summary events told us, held until its
/// `compaction/end` closes the transaction and the marker can be emitted.
#[derive(Default)]
struct OpenCompaction {
    started_at: Option<DateTime<Utc>>,
    /// `/compact` rather than the automatic window-pressure trigger.
    manual: bool,
    /// Heuristic token price of the range the summary replaced.
    pre_tokens: Option<u64>,
    /// Tokens the summarization call emitted, i.e. the replacement's size.
    post_tokens: Option<u64>,
}

/// The `compactionId` correlating a compaction's three events.
fn compaction_id(data: &Value) -> Option<String> {
    data.get("compactionId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(String::from)
}

fn parse_session_events(text: &str, attachments: Option<&Path>) -> SessionParse {
    let mut sp = SessionParse::default();

    // The open assistant turn for the CURRENT log turn: index into `sp.turns`,
    // the accumulated per-step usage, and the `turn/start` clock.
    let mut open_assistant: Option<usize> = None;
    let mut pending_usage: Option<TurnUsage> = None;
    let mut turn_started_at: Option<DateTime<Utc>> = None;
    // Compactions between their `start` and `end` markers, by `compactionId`.
    // A map rather than a single slot because the markers bracket a model call:
    // context injected while the summary runs may sit between the pair, and
    // nothing in the log format promises the brackets never interleave.
    let mut open_compactions: HashMap<String, OpenCompaction> = HashMap::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let event_type = value.get("type").and_then(Value::as_str).unwrap_or("");
        // Raw stream chunks duplicate `assistant/message` content (and the
        // compacted `*-chunks` rows carry `seq0`/`time0` instead of `time`).
        //
        // `deepseek-acp` 0.9.0 stopped writing these rows — the compacted
        // stream rides inside `assistant/message` now. The skip STAYS: logs
        // written before that upgrade still hold them, and they are still the
        // same duplicate content.
        if matches!(
            event_type,
            "assistant/chunk" | "tool-call-chunks" | "reasoning-chunks" | "text-chunks"
        ) {
            continue;
        }
        let ts = event_millis(&value);
        if let Some(ts) = ts {
            sp.first_ts.get_or_insert(ts);
            sp.last_ts = Some(ts);
        }
        let data = value.get("data");

        match event_type {
            "session" => {
                sp.cwd = value
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                sp.created_at = value
                    .get("createdAt")
                    .and_then(Value::as_i64)
                    .and_then(DateTime::from_timestamp_millis);
                sp.delegation_depth = value
                    .get("delegationDepth")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
            }
            "turn/start" => {
                // Defensive: a dangling assistant turn from a log missing its
                // `turn/end` must not absorb the next turn's steps.
                finalize_assistant(
                    &mut sp.turns,
                    &mut open_assistant,
                    &mut pending_usage,
                    None,
                    turn_started_at,
                );
                turn_started_at = ts;
            }
            "turn/end" => {
                finalize_assistant(
                    &mut sp.turns,
                    &mut open_assistant,
                    &mut pending_usage,
                    ts,
                    turn_started_at,
                );
                turn_started_at = None;
            }
            "user/message" => {
                let Some(data) = data else { continue };
                // Plugin-injected runtime-context snapshots (sandbox/approval
                // policy, file-change notices, …) are model plumbing, not
                // conversation content.
                let kind = data
                    .pointer("/source/kind")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if kind != "user" {
                    continue;
                }
                let text = collect_text_parts(data.get("content"));
                // Since 0.6.0 a prompt may be images only ("what is wrong with
                // this screenshot?"), so empty text no longer means an empty
                // turn — check for renderable blocks instead. Skipping on text
                // alone would delete the question and leave its answer.
                let images = user_image_blocks(data.get("content"), attachments);
                if text.trim().is_empty() && images.is_empty() {
                    continue;
                }
                if sp.first_user_text.is_none() && !text.trim().is_empty() {
                    sp.first_user_text = Some(title_from_user_text(text.trim()));
                }
                sp.content_events += 1;
                sp.message_count += 1;
                let ts = ts.or(sp.last_ts).unwrap_or_else(Utc::now);
                // Text first, then the images: upstream keeps the two
                // interleaved in wire order for the model, but a history turn
                // renders attachments as a strip beside the prose, so the
                // ordering within the block list is not what the reader sees.
                let mut blocks = Vec::with_capacity(images.len() + 1);
                if !text.trim().is_empty() {
                    blocks.push(ContentBlock::Text { text });
                }
                blocks.extend(images);
                sp.turns.push(MessageTurn {
                    id: format!("turn-{}", sp.turns.len()),
                    role: TurnRole::User,
                    blocks,
                    timestamp: ts,
                    usage: None,
                    duration_ms: None,
                    model: None,
                    completed_at: Some(ts),
                    agent_message_id: None,
                });
            }
            "request/header" => {
                if let Some(model) = data
                    .and_then(|d| d.pointer("/header/config/model"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    sp.model = Some(model.to_string());
                }
            }
            "request/context" => {
                if let Some(window) = data
                    .and_then(|d| d.get("contextWindow"))
                    .and_then(Value::as_u64)
                    .filter(|w| *w > 0)
                {
                    sp.context_window = Some(window);
                }
            }
            "session/title" => {
                if let Some(title) = data
                    .and_then(|d| d.get("title"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    sp.title = Some(title.to_string());
                }
            }
            "assistant/message" => {
                let Some(data) = data else { continue };
                let step_usage = usage_from_step(data.get("usage"));
                if let Some(usage) = &step_usage {
                    sp.last_step_input_side = Some(
                        usage
                            .input_tokens
                            .saturating_add(usage.cache_read_input_tokens),
                    );
                    pending_usage = Some(match pending_usage.take() {
                        Some(prev) => add_usage(prev, usage.clone()),
                        None => usage.clone(),
                    });
                }
                let message = data.get("message");
                let source_model = message
                    .and_then(|m| m.pointer("/source/model"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                if sp.model.is_none() {
                    sp.model.clone_from(&source_model);
                }
                // The log's own persistent identity for this message, which
                // deepseek-acp 0.8.0 accepts as an AIR fork point (see
                // `acp::fork`). Upstream keeps it stable "across every
                // representation boundary" precisely for clients that read the
                // JSONL, and matching it resolves to the message's log TURN —
                // which is the granularity a coaxi bubble already has.
                let message_id = message
                    .and_then(|m| m.get("id"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let Some(blocks) = message
                    .and_then(|m| m.get("content"))
                    .and_then(Value::as_array)
                else {
                    continue;
                };
                // The list-view activity count grows once per assistant
                // MESSAGE with text, not once per text block inside it.
                let mut counted_text = false;
                for block in blocks {
                    let rendered = match block.get("type").and_then(Value::as_str).unwrap_or("") {
                        "text" => {
                            let text = block
                                .get("text")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            if text.trim().is_empty() {
                                continue;
                            }
                            if !counted_text {
                                sp.message_count += 1;
                                counted_text = true;
                            }
                            ContentBlock::Text {
                                text: text.to_string(),
                            }
                        }
                        "reasoning" => {
                            let text = block
                                .get("text")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            if text.trim().is_empty() {
                                continue;
                            }
                            ContentBlock::Thinking {
                                text: text.to_string(),
                            }
                        }
                        "tool-call" => ContentBlock::ToolUse {
                            tool_use_id: block.get("id").and_then(Value::as_str).map(String::from),
                            tool_name: block
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown")
                                .to_string(),
                            input_preview: block
                                .get("arguments")
                                .and_then(Value::as_str)
                                .and_then(deepseek_tool_input_preview),
                            status: None,
                            meta: None,
                        },
                        // Upstream declares `image` role-neutral and says
                        // assistant-side rendering is forward compatibility —
                        // today's adapters are all text-only on output, so no
                        // real log reaches this arm.
                        //
                        // RECORDED, not yet rendered: the frontend surfaces
                        // images from USER turns only (`ai-elements-adapter`
                        // extracts them into `userImages`; an assistant-turn
                        // image block falls to its `default: return null`). So
                        // this arm's job is to get the block into the stored
                        // turn rather than drop it on the floor — the day an
                        // adapter emits one, the data is already there and only
                        // the renderer has to learn it.
                        "image" => attachment_image_block(block.get("attachment"), attachments),
                        _ => continue,
                    };
                    sp.content_events += 1;
                    let ts = ts.or(sp.last_ts).unwrap_or_else(Utc::now);
                    let turn = ensure_assistant(
                        &mut sp.turns,
                        &mut open_assistant,
                        ts,
                        sp.model.clone().or(source_model.clone()),
                    );
                    // The turn's fork point is the message that OPENS it, so
                    // the later steps of a multi-step turn must not overwrite
                    // it. Any of them resolves to the same log turn upstream,
                    // and naming the first matches how the Claude parser picks
                    // a turn's id. Set here rather than above so a message
                    // whose blocks are all skipped never conjures an empty
                    // bubble just to hold an id.
                    if let Some(id) = message_id {
                        turn.agent_message_id.get_or_insert_with(|| id.to_string());
                    }
                    turn.blocks.push(rendered);
                }
            }
            "tool/result" => {
                let Some(data) = data else { continue };
                let result = data
                    .pointer("/message/content")
                    .and_then(Value::as_array)
                    .and_then(|items| {
                        items
                            .iter()
                            .find(|i| i.get("type").and_then(Value::as_str) == Some("tool-result"))
                    });
                let tool_use_id = result
                    .and_then(|r| r.get("toolCallId"))
                    .and_then(Value::as_str)
                    .or_else(|| {
                        data.pointer("/message/source/callId")
                            .and_then(Value::as_str)
                    })
                    .map(String::from);
                let output = result
                    .map(|r| collect_text_parts(r.get("content")))
                    .unwrap_or_default();
                // `data.error` is the tool's internal failure identity; the
                // model-facing `isError` flag is the primary signal.
                let is_error = result
                    .and_then(|r| r.get("isError"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    || data.get("error").is_some_and(|e| !e.is_null());
                sp.content_events += 1;
                let ts = ts.or(sp.last_ts).unwrap_or_else(Utc::now);
                let model = sp.model.clone();
                let turn = ensure_assistant(&mut sp.turns, &mut open_assistant, ts, model);
                turn.blocks.push(ContentBlock::ToolResult {
                    tool_use_id,
                    output_preview: (!output.trim().is_empty()).then_some(output),
                    is_error,
                    agent_stats: None,
                    images: Vec::new(),
                });
            }
            // ── Context compaction (0.6.0) ───────────────────────────────
            //
            // Upstream writes one compaction as three events: `start` takes the
            // lock, `summary` records the summary and what it replaced, `end`
            // releases the lock (carrying `error` when the attempt failed).
            //
            // The LOG is the only place coaxi can see this. On the wire the
            // matching `compaction_update` / `compaction_summary_chunk` updates
            // are gated behind a `clientCapabilities.session.compaction` that
            // the pinned ACP schema crate cannot advertise, so the agent
            // correctly never sends them — see the DeepSeek entry in
            // `acp::registry`. Nothing is lost from the transcript either way:
            // the log is append-only, so the compacted turns are all still here
            // and history shows them. What the marker adds is WHY the model
            // stopped remembering them.
            "compaction/start" => {
                let Some(data) = data else { continue };
                let Some(id) = compaction_id(data) else { continue };
                let entry = open_compactions.entry(id).or_default();
                entry.started_at = ts;
                entry.manual = data.get("sourceCommandId").is_some();
            }
            "compaction/summary" => {
                let Some(data) = data else { continue };
                let Some(id) = compaction_id(data) else { continue };
                // `or_default` rather than a lookup: a log can begin AFTER the
                // opening marker (a fork seed, or a truncated prefix from a
                // half-written final frame), and a compaction with no visible
                // `start` still deserves its marker.
                let entry = open_compactions.entry(id).or_default();
                if data.get("sourceCommandId").is_some() {
                    entry.manual = true;
                }
                // What the replaced span cost, and what replaced it. `usage` is
                // the SUMMARIZATION request's own accounting, so its output side
                // is the summary's size — the two together are the reduction
                // this compaction bought.
                entry.pre_tokens = data.get("shadowedTokenCount").and_then(Value::as_u64);
                entry.post_tokens = data.pointer("/usage/outputTokens").and_then(Value::as_u64);
            }
            "compaction/end" => {
                let Some(data) = data else { continue };
                let Some(id) = compaction_id(data) else { continue };
                let entry = open_compactions.remove(&id).unwrap_or_default();
                let error = data
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|e| !e.is_empty());
                let mut marker = serde_json::Map::new();
                marker.insert("version".to_string(), Value::from(1));
                // `trigger` is adapter-defined vocabulary the card surfaces as a
                // tooltip; upstream distinguishes the two by whether a command
                // initiated the transaction. All THREE events repeat
                // `sourceCommandId`, and this is the only one guaranteed to be
                // in the log — reading it from the opening marker alone would
                // relabel a `/compact` as automatic whenever its `start` fell
                // outside a fork seed or a truncated prefix.
                let manual = entry.manual || data.get("sourceCommandId").is_some();
                marker.insert(
                    "trigger".to_string(),
                    Value::from(if manual { "manual" } else { "auto" }),
                );
                // Counts only for a SUCCESSFUL compaction: a failed attempt
                // replaced nothing, so reporting a reduction it did not make
                // would be a lie told in the failure's own label.
                if error.is_none() {
                    if let Some(pre) = entry.pre_tokens {
                        marker.insert("preTokens".to_string(), Value::from(pre));
                    }
                    if let Some(post) = entry.post_tokens {
                        marker.insert("postTokens".to_string(), Value::from(post));
                    }
                }
                if let (Some(started), Some(ended)) = (entry.started_at, ts) {
                    let elapsed = (ended - started).num_milliseconds();
                    if elapsed > 0 {
                        marker.insert("durationMs".to_string(), Value::from(elapsed));
                    }
                }
                if let Some(error) = error {
                    marker.insert("error".to_string(), Value::from(error));
                }

                sp.content_events += 1;
                let ts = ts.or(sp.last_ts).unwrap_or_else(Utc::now);
                let model = sp.model.clone();
                let turn = ensure_assistant(&mut sp.turns, &mut open_assistant, ts, model);
                // The compaction id is the log's own identity for the
                // transaction, so re-parsing the same log yields the same
                // tool_use_id. The pair is self-contained — a ToolUse with no
                // ToolResult would read as a call still running.
                turn.blocks.push(ContentBlock::ToolUse {
                    tool_use_id: Some(id.clone()),
                    tool_name: "context_compaction".to_string(),
                    input_preview: None,
                    status: None,
                    meta: Some(Value::Object(
                        [("contextCompaction".to_string(), Value::Object(marker))]
                            .into_iter()
                            .collect(),
                    )),
                });
                turn.blocks.push(ContentBlock::ToolResult {
                    tool_use_id: Some(id),
                    output_preview: None,
                    is_error: error.is_some(),
                    agent_stats: None,
                    images: Vec::new(),
                });
            }
            // `agent/inbox/spliced` (queue bookkeeping), `step/start`/`step/end`
            // (sub-turn markers), `sandbox/mode`, `todo/write` (the todo_write
            // TOOL call already renders), and `session/end-seed` (resume seed
            // boundary) carry no conversation content. `compaction/prune` is
            // the model-free prune variant's metering record — it has no
            // start/end bracket of its own, and the backend this bridge
            // composes (`dsh-compaction-basic`) only ever summarizes, so no log
            // coaxi reads contains one.
            _ => {}
        }
    }

    finalize_assistant(
        &mut sp.turns,
        &mut open_assistant,
        &mut pending_usage,
        None,
        turn_started_at,
    );
    sp
}

/// The open assistant turn for the current log turn, created on first use.
fn ensure_assistant<'a>(
    turns: &'a mut Vec<MessageTurn>,
    open_assistant: &mut Option<usize>,
    ts: DateTime<Utc>,
    model: Option<String>,
) -> &'a mut MessageTurn {
    let idx = match open_assistant {
        Some(idx) => *idx,
        None => {
            turns.push(MessageTurn {
                id: format!("turn-{}", turns.len()),
                role: TurnRole::Assistant,
                blocks: Vec::new(),
                timestamp: ts,
                usage: None,
                duration_ms: None,
                model,
                completed_at: None,
                agent_message_id: None,
            });
            let idx = turns.len() - 1;
            *open_assistant = Some(idx);
            idx
        }
    };
    &mut turns[idx]
}

/// Close the open assistant turn: attach the summed step usage, and — when the
/// log recorded a `turn/end` — the completion clock and the `turn/start`-based
/// duration. A turn missing its `turn/end` (killed mid-flight) keeps honest
/// `None` clocks; `backfill_turn_durations` tiles an estimate later.
fn finalize_assistant(
    turns: &mut [MessageTurn],
    open_assistant: &mut Option<usize>,
    pending_usage: &mut Option<TurnUsage>,
    ended_at: Option<DateTime<Utc>>,
    started_at: Option<DateTime<Utc>>,
) {
    let Some(idx) = open_assistant.take() else {
        *pending_usage = None;
        return;
    };
    let Some(turn) = turns.get_mut(idx) else {
        *pending_usage = None;
        return;
    };
    if let Some(usage) = pending_usage.take() {
        turn.usage = Some(match turn.usage.take() {
            Some(existing) => add_usage(existing, usage),
            None => usage,
        });
    }
    if let Some(end) = ended_at {
        turn.completed_at = Some(end);
        if let Some(start) = started_at {
            let ms = (end - start).num_milliseconds();
            if ms > 0 {
                turn.duration_ms = Some(ms as u64);
            }
        }
    }
}

/// List immediate sub-directories of `dir` (empty when `dir` is missing or not
/// a directory). Shallow by design — the layout is exactly two levels deep.
fn read_subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

