pub mod antigravity;
pub mod claude;
pub mod cline;
pub mod codebuddy;
pub mod cursor;
pub mod deepseek;
pub mod gemini;
pub mod grok;
pub mod hermes;
pub mod kimi_code;
pub mod openclaw;
pub mod opencode;
mod opencode_context_window;
pub mod pi;
mod summary_cache;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;

/// A root of external agent-CLI transcript data, archived under
/// `external/<agent>/` by the optional "include conversation content" toggle.
/// These paths are owned by the respective CLIs — codeg only reads them.
#[derive(Clone)]
pub struct ExternalSource {
    /// Stable directory name inside the archive (`external/<agent>/`).
    pub agent: &'static str,
    /// Live source path (a directory, or a single file when `is_file`).
    pub root: PathBuf,
    pub is_file: bool,
    /// This source's `.db` files are SQLite databases owned by the agent CLI.
    ///
    /// Backup then snapshots each one with a read-only page copy — never
    /// `VACUUM` (it renumbers implicit rowids, and we cannot audit a
    /// third-party schema for tables that treat rowid as a key) and never a
    /// read-write open (that runs WAL recovery inside someone else's live
    /// store). Exactly one self-contained file per database enters the
    /// archive, never a `-wal`/`-shm`; restore deletes the live sidecars
    /// before the rename so a stale WAL can never be replayed onto a database
    /// it does not belong to. See `commands::backup::external`.
    pub sqlite: bool,
    /// When `Some`, only entries whose first path component (relative to
    /// `root`) is in this allowlist are archived. Used to keep the backup to
    /// transcript/session data and exclude sibling credential/config/cache
    /// files in shared base dirs (e.g. `~/.gemini/oauth_creds.json`). `None`
    /// means the whole root is already transcript-scoped.
    pub include_top: Option<&'static [&'static str]>,
    /// The append-only JSONL indexes among this source's files. Restore
    /// merges one into the live copy instead of skipping or replacing it —
    /// see [`AppendOnlyIndex`].
    pub indexes: &'static [AppendOnlyIndex],
}

/// A file the agent CLI only ever appends to, one JSON object per line, the
/// newest usable line for a key winning (codex's `session_index.jsonl`,
/// kimi's).
///
/// The live copy usually exists when a backup is restored — every session the
/// CLI created since is in it — so neither conflict policy fits a whole-file
/// restore: skipping drops the restored sessions' lines, replacing drops every
/// line written since the backup. Restore appends the backup's lines for the
/// keys the live copy lacks instead (`commands::backup::external`).
#[derive(Clone, Copy, Debug)]
pub struct AppendOnlyIndex {
    /// Path relative to the source's [`ExternalSource::restore_base`],
    /// `/`-separated.
    pub path: &'static str,
    /// The string field naming the record a line describes.
    pub key: &'static str,
    /// The string field a line exists to set.
    pub value: &'static str,
    /// Whether a value is one the index's reader acts on — the reader's own
    /// test, so the merge and the reader agree on every line. A line the
    /// reader skips (an empty name, say) never counts as a key's newest line.
    pub usable: fn(&str) -> bool,
}

impl ExternalSource {
    /// The base directory a `external/<agent>/<rest>` entry restores under.
    /// For file sources that is the file's parent; for dir sources, the root.
    pub fn restore_base(&self) -> PathBuf {
        if self.is_file {
            self.root
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| self.root.clone())
        } else {
            self.root.clone()
        }
    }
}

/// Enumerate every external transcript source, resolved against the current
/// environment (honoring `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, etc.). Sources
/// whose root does not exist are still listed; callers skip missing roots.

use chrono::{DateTime, Utc};
use regex::Regex;

use crate::models::{
    AgentType, ContentBlock, ConversationDetail, ConversationSummary, MessageTurn, SessionStats,
    TurnRole, TurnUsage,
};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("Conversation not found: {0}")]
    ConversationNotFound(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

pub trait AgentParser {
    fn list_conversations(&self) -> Result<Vec<ConversationSummary>, ParseError>;
    fn get_conversation(&self, conversation_id: &str) -> Result<ConversationDetail, ParseError>;
}

/// The ONE place a history parser is constructed.
///
/// Every caller goes through here so the internal `@agent` routing frame is
/// stripped from every agent's history — not just the one agent whose parser
/// happens to know about it. `codeg-mcp` is injected into every MCP-capable
/// agent, so the frame lands in each of their native transcripts; a per-parser
/// fix would silently miss whichever parser was written next.
pub fn build_parser_at(agent:&str,base:PathBuf)->Result<Box<dyn AgentParser>,ParseError>{
 let inner:Box<dyn AgentParser>=match agent{
  "gemini"=>Box::new(gemini::GeminiParser::with_base_dir(base)),
  "openclaw"=>Box::new(openclaw::OpenClawParser::with_base_dir(base)),
  "hermes"=>Box::new(hermes::HermesParser::with_base_dir(base)),
  "codebuddy"=>Box::new(codebuddy::CodeBuddyParser::with_base_dir(base)),
  "kimi_code"=>Box::new(kimi_code::KimiCodeParser::with_base_dir(base)),
  "pi"=>Box::new(pi::PiParser::with_base_dir(base)),
  "deepseek"=>Box::new(deepseek::DeepSeekParser::with_base_dir(base)),
  "cursor"=>Box::new(cursor::CursorParser::with_base_dir(base)),
  "opencode"=>Box::new(opencode::OpenCodeParser::with_base_dir(base)),
  "cline"=>Box::new(cline::ClineParser::with_base_dir(base)),
  "antigravity"=>Box::new(antigravity::AntigravityParser::with_base_dir(base)),
  _=>return Err(ParseError::InvalidData("history.unsupported_adapter".into())),
 };Ok(Box::new(RouteSanitized(inner)))
}

/// Removes Codeg's internal `@agent` routing frame from whatever a parser read
/// back out of an agent's own transcript.
///
/// Only complete frames that re-render byte-for-byte are touched (see
/// [`crate::acp::agent_mentions::strip_internal_agent_routes`]), and the
/// separator that opens one is scrubbed from every prompt at ingress, so
/// look-alike user prose is never eligible. A BLOCK left with no content after
/// stripping carried nothing but the frame and is dropped; a user turn left
/// with no content at all was a transport-only record and is dropped rather
/// than rendered as a phantom turn.
///
/// Not every agent hands the frame back the way it was sent: Antigravity's ACP
/// server joins the prompt's text blocks with a space and replaces each
/// separator with one, so its trajectory holds the frame's body with the
/// separators gone. That form is matched on the body alone and removed here
/// too — which is also why the [`sanitize_text`] trim below is not optional for
/// those agents, since the spaces that replaced the separators survive the
/// strip.
///
/// Dropping the emptied block — not just emptying it — is what keeps an
/// `@`-mention turn from growing a blank band when it is reopened from history.
/// `append_agent_routes` appends the frame as its OWN prompt block, and the
/// parsers that keep one block per recorded text item (claude's
/// `extract_user_content`, `acp_native`'s `prompt_blocks`) therefore hand back
/// `[Text(prose), Text(frame)]`. Emptying the second in place left a zero-height
/// text part that still takes a `space-y-4` gap in the bubble.
///
/// The Codex parser additionally handles route-only records STRUCTURALLY
/// (canonical-channel coverage is positional and cannot be repaired after the
/// fact); this pass is idempotent on top of that.
struct RouteSanitized(Box<dyn AgentParser>);

impl AgentParser for RouteSanitized {
    fn list_conversations(&self) -> Result<Vec<ConversationSummary>, ParseError> {
        let mut summaries = self.0.list_conversations()?;
        for summary in &mut summaries {
            sanitize_summary(summary);
        }
        Ok(summaries)
    }

    fn get_conversation(&self, conversation_id: &str) -> Result<ConversationDetail, ParseError> {
        let mut detail = self.0.get_conversation(conversation_id)?;
        let before = detail.turns.len();
        detail.turns.retain_mut(|turn| {
            if !matches!(turn.role, TurnRole::User) {
                return true;
            }
            turn.blocks.retain_mut(|block| {
                let ContentBlock::Text { text } = block else {
                    return true;
                };
                // Only a block that actually HELD a frame is eligible to go: an
                // empty text block a parser produced for some other reason is
                // left exactly as it was found.
                !(sanitize_text(text) && text.trim().is_empty())
            });
            // A turn whose ONLY content was the frame carried no user message.
            !turn.blocks.iter().all(|block| match block {
                ContentBlock::Text { text } => text.trim().is_empty(),
                _ => false,
            })
        });
        // Keep the count the sidebar shows in step with the turns actually
        // rendered; the summary rides along inside the detail.
        let dropped = (before - detail.turns.len()) as u32;
        detail.summary.message_count = detail.summary.message_count.saturating_sub(dropped);
        sanitize_summary(&mut detail.summary);
        Ok(detail)
    }
}

fn sanitize_summary(summary: &mut ConversationSummary) {
    if let Some(title) = summary.title.as_mut() {
        sanitize_text(title);
        // Titles are capped by their parser BEFORE reaching here, so a frame can
        // straddle the cut and survive `sanitize_text`, which only removes whole
        // frames. Anything from a leftover marker on is truncated frame.
        crate::acp::agent_mentions::cut_at_route_frame_marker(title);
        if title.trim().is_empty() {
            summary.title = None;
        }
    }
}

/// Strip every complete frame from `text`, reporting whether one was there.
///
/// The trailing trim only runs when a frame WAS removed, and it is load-bearing
/// for the agents that join a prompt's text blocks into one record with a blank
/// line: `strip_internal_agent_routes` absorbs a single newline adjacent to the
/// frame, so `"prose\n\n<frame>"` comes back as `"prose\n"` — which
/// `whitespace-pre-wrap` paints as an extra blank line under the message. Text
/// with no frame in it is never touched.
fn sanitize_text(text: &mut String) -> bool {
    // Single scan short-circuit: history with no frame pays one memchr per
    // string, not a parse attempt.
    if !crate::acp::agent_mentions::contains_internal_agent_routes(text) {
        return false;
    }
    *text = crate::acp::agent_mentions::strip_internal_agent_routes(text);
    text.truncate(text.trim_end().len());
    true
}

/// Expand a leading `~` in a relocation env var, for the agents whose OWN
/// resolver does that.
///
/// Handles a bare `~` and a `~/` — or, on Windows, `~\` — prefix.
///
/// A `~user` form is left VERBATIM, which is a deliberate divergence rather
/// than a match: Python's `os.path.expanduser` does attempt a passwd lookup for
/// it on unix, and Node's does not. Resolving another account's home here would
/// mean duplicating that lookup to guess at a directory, so the value is passed
/// through instead — and because a literal `~user/...` is not an absolute path,
/// every consumer of this treats it as unresolvable and fails closed (the fs
/// sandbox refuses the slot; the Antigravity settings sync refuses the write).
/// The cost is that this rare form is unsupported, not that it resolves wrongly.
///
/// Deliberately NOT applied everywhere. Antigravity runs `os.path.expanduser`
/// on `GEMINI_HOME` (`acp_server/paths.py`) and DeepSeek expands `DSH_HOME`
/// (`dsh-home-paths`' `expandHomePath`), but Hermes's `get_hermes_home` is a
/// bare `Path(val.strip())` and Codex, Claude and the rest are likewise
/// verbatim. Expanding for one of those would point codeg at `$HOME/...` while
/// the agent used a literal `~` directory — and, in the fs sandbox, would hand
/// out `$HOME` as a writable root the user never selected.
///
/// Shared so the rule lives in ONE place: it is mirrored by
/// `acp::file_system_runtime`'s root table (`EXPANDS_TILDE`), and a copy that
/// drifts from the resolver it mirrors is exactly how the agent ends up unable
/// to write its own directory.
pub fn expand_home_prefix(value: &str, home_dir: Option<&PathBuf>) -> PathBuf {
    let Some(home) = home_dir else {
        return PathBuf::from(value);
    };
    if value == "~" {
        return home.clone();
    }
    if let Some(rest) = value.strip_prefix("~/").or_else(|| value.strip_prefix("~\\")) {
        return home.join(rest);
    }
    PathBuf::from(value)
}

/// Truncate a string to `max_len` characters, appending "..." if truncated.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

/// True when `id` is safe to embed as a single filename component beneath a
/// session's `subagents/` directory (Claude Code's and CodeBuddy's sub-agent
/// transcript layout). The id is read straight from transcript JSON
/// (`agentId` / `subAgent.sessionId`), so a corrupted or hostile transcript
/// could otherwise smuggle a path that escapes the directory once it is joined
/// and a file is opened.
///
/// Rejects: empty, a path separator (`/` or `\`), a parent ref (`..`), a colon
/// (Windows drive prefix `C:` / NTFS alternate-data-stream), or a NUL. The
/// checks are conservative and platform-independent — we reject `:` and `\`
/// even on Unix (where they are legal filename chars) so the same id can never
/// escape if the transcript is later read on Windows, where `Path::join("C:x")`
/// silently replaces the whole base path.
pub fn is_safe_subagent_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains('/')
        && !id.contains('\\')
        && !id.contains("..")
        && !id.contains(':')
        && !id.contains('\0')
}

/// Punctuation the serializer escapes with a leading backslash inside a
/// reference label (mirrors `escapeMarkdownText` in `src/lib/reference-text.ts`
/// and the class in the frontend `unescapeReferenceLabel`).
fn is_escapable_reference_punct(c: char) -> bool {
    matches!(
        c,
        '\\' | '`' | '*' | '_' | '~' | '[' | ']' | '(' | ')' | '<' | '>'
    )
}

/// Reverse the serializer's label escaping: drop the backslash from each escaped
/// inline-significant punctuation char so the recovered label reads literally.
/// Mirrors the frontend `unescapeReferenceLabel`.
fn unescape_reference_label(label: &[char]) -> String {
    let mut out = String::with_capacity(label.len());
    let mut i = 0;
    while i < label.len() {
        if label[i] == '\\' && i + 1 < label.len() && is_escapable_reference_punct(label[i + 1]) {
            out.push(label[i + 1]);
            i += 2;
        } else {
            out.push(label[i]);
            i += 1;
        }
    }
    out
}

/// Mirror ECMAScript's `/\s/` — the whitespace class the frontend
/// `foldReferenceLinks` (`src/lib/reference-link.ts`) scans destinations with —
/// so this port stays in step with it. It deliberately differs from Rust's
/// `char::is_whitespace()` in exactly two code points: `U+FEFF` (BOM) is
/// whitespace to JS but not to Rust, and `U+0085` (NEL) is whitespace to Rust
/// but not to JS. The set is ECMAScript WhiteSpace + LineTerminator.
fn is_markdown_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'..='\u{000D}'      // tab, LF, VT, FF, CR
            | '\u{0020}'             // space
            | '\u{00A0}'             // no-break space
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'             // line separator
            | '\u{2029}'             // paragraph separator
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'             // zero-width no-break space (BOM)
    )
}

/// Whether the backslash at `k` escapes the next character. CommonMark never
/// lets a backslash escape whitespace, so `\` + whitespace ENDS (not extends) a
/// label/destination scan — only `\` + a non-whitespace char is a real escape.
fn reference_escapes_next(chars: &[char], k: usize) -> bool {
    chars.get(k) == Some(&'\\') && chars.get(k + 1).is_some_and(|c| !is_markdown_whitespace(*c))
}

/// If a well-formed `(destination)` begins at `start`, return the index just
/// past its closing `)`; otherwise `None`. Mirrors the frontend `destinationEnd`
/// and the serializer's two forms: a `<…>`-wrapped destination (interior `\`,
/// `<`, `>` backslash-escaped) or a bare run with no `(`, `)`, whitespace, `<` or
/// `>`.
fn reference_destination_end(chars: &[char], start: usize) -> Option<usize> {
    let n = chars.len();
    if start >= n || chars[start] != '(' {
        return None;
    }
    let mut k = start + 1;
    if chars.get(k) == Some(&'<') {
        k += 1;
        while k < n {
            if reference_escapes_next(chars, k) {
                k += 2;
                continue;
            }
            match chars[k] {
                '>' => {
                    return if chars.get(k + 1) == Some(&')') {
                        Some(k + 2)
                    } else {
                        None
                    };
                }
                // An unescaped `<` or a line break is forbidden inside `<…>`;
                // bailing here also bounds the scan so a missing `>` stops at the
                // next `<` instead of running to EOF (keeps adversarial input
                // linear).
                '<' | '\n' | '\r' => return None,
                _ => k += 1,
            }
        }
        return None;
    }
    while k < n {
        if reference_escapes_next(chars, k) {
            k += 2;
            continue;
        }
        let c = chars[k];
        if c == ')' {
            return Some(k + 1);
        }
        if c == '(' || c == '<' || c == '>' || is_markdown_whitespace(c) {
            return None;
        }
        k += 1;
    }
    None
}

/// Replace every inline `[label](destination)` reference link in `text` with its
/// unescaped `label`, leaving all other prose (including malformed `[…]`/`(…)`
/// fragments and invocation tokens like `@Codex`) untouched.
///
/// This is the Rust counterpart of the frontend canonical fold
/// (`foldReferenceLinks` in `src/lib/reference-link.ts`) and MUST stay in step
/// with it: a single O(n) left-to-right scan over a stack of unmatched `[`
/// positions, matching each `]` against the most recent opener so a balanced
/// nested label closes at the right bracket, requiring a non-empty label and a
/// well-formed `(dest)` for a link, and recovering later links after a
/// stray/unbalanced `[`. Used to derive conversation titles from a user's first
/// message: folding BEFORE truncation means a long `file://` destination can
/// never be sliced mid-link into an unterminable `[label](file://…` fragment.
pub fn fold_reference_links(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(text.len());
    // Start of the pending prose run; flushed before each link and at the end.
    let mut text_start = 0usize;
    // Indices of `[` seen but not yet matched by a `]` (most recent on top).
    let mut openers: Vec<usize> = Vec::new();
    let mut i = 0usize;

    while i < n {
        if reference_escapes_next(&chars, i) {
            // `\[` / `\]` (and any `\x`) is literal — skip both chars.
            i += 2;
            continue;
        }
        match chars[i] {
            '[' => {
                openers.push(i);
                i += 1;
            }
            ']' if !openers.is_empty() => {
                let open = openers.pop().expect("openers is non-empty");
                match reference_destination_end(&chars, i + 1) {
                    // A link needs a well-formed `(dest)` right after `]` and a
                    // non-empty label between the brackets.
                    Some(end) if i > open + 1 => {
                        out.extend(chars[text_start..open].iter());
                        out.push_str(&unescape_reference_label(&chars[open + 1..i]));
                        // Everything up to `open` is committed, so any still-open
                        // outer `[` can no longer span a link.
                        openers.clear();
                        i = end;
                        text_start = end;
                    }
                    // Not a link: keep the brackets in the pending prose run and
                    // keep scanning so a later valid link is still found.
                    _ => i += 1,
                }
            }
            _ => i += 1,
        }
    }
    out.extend(chars[text_start..n].iter());
    out
}

/// Derive a conversation title from a user's first message: fold inline
/// reference links to their labels, then cap the length. Folding first ensures a
/// `[name](file://<long path>)` mention becomes `name` instead of a raw — and,
/// once truncated, unterminable — Markdown link.
pub fn title_from_user_text(text: &str) -> String {
    truncate_str(&fold_reference_links(text), 100)
}

/// Widen one projected prompt block into a rendered turn's block type.
///
/// The projection itself is [`crate::acp::types::project_user_prompt_block`] —
/// the SINGLE rule shared with the live broadcast. This only carries the result
/// across into `models::message`, keeping the image `uri` that the live wire
/// type has nowhere to put but the frontend uses for an image's display name.
pub fn user_turn_block(block: &crate::acp::types::PromptInputBlock) -> ContentBlock {
    match crate::acp::types::project_user_prompt_block(block) {
        crate::acp::types::UserTurnBlock::Text { text } => ContentBlock::Text { text },
        crate::acp::types::UserTurnBlock::Image {
            data,
            mime_type,
            uri,
        } => ContentBlock::Image {
            data,
            mime_type,
            uri,
        },
    }
}

/// Read one recorded ACP content block off disk and project it the way the
/// live path projects the same prompt. `None` when the block has nothing to
/// render — see [`crate::acp::types::prompt_block_from_wire`].
///
/// Every history parser that reconstructs a user turn from raw ACP content
/// goes through here, so "how an attachment appears in a user message" is
/// decided once rather than per agent.
pub fn user_turn_block_from_wire(item: &serde_json::Value) -> Option<ContentBlock> {
    crate::acp::types::prompt_block_from_wire(item).map(|b| user_turn_block(&b))
}

/// The record an agent wrote for a turn that FAILED, as the message closing
/// its round: a `System` message holding the agent's account of why as one
/// [`ContentBlock::TurnError`] — `None` when it gives no account.
///
/// A `System` turn, never an assistant one: the error is not the model's
/// reply, so rendering it as one put it in the model's mouth (the reason the
/// parsers used to drop these records), and every reply count —
/// `syncTurnMetadata`'s alignment of live replies with parsed ones, the
/// window's history baseline — counts assistant turns only, so a failure the
/// live view and the transcript disagree about can never shift those.
pub(crate) fn turn_error_message(
    id: String,
    message: &str,
    timestamp: DateTime<Utc>,
) -> Option<crate::models::UnifiedMessage> {
    let message = message.trim();
    if message.is_empty() {
        return None;
    }
    Some(crate::models::UnifiedMessage {
        id,
        role: crate::models::MessageRole::System,
        content: vec![ContentBlock::TurnError {
            message: message.to_string(),
        }],
        timestamp,
        usage: None,
        duration_ms: None,
        model: None,
        completed_at: Some(timestamp),
        agent_message_id: None,
    })
}

/// How many of `turns` are somebody's message: all but a failed turn's line
/// ([`turn_error_message`]), which reports a failure rather than saying
/// anything — the list summaries never counted its record either.
pub(crate) fn message_turn_count(turns: &[MessageTurn]) -> u32 {
    turns
        .iter()
        .filter(|turn| {
            !(matches!(turn.role, TurnRole::System)
                && !turn.blocks.is_empty()
                && turn
                    .blocks
                    .iter()
                    .all(|block| matches!(block, ContentBlock::TurnError { .. })))
        })
        .count() as u32
}

/// Fill in `duration_ms` for assistant turns whose agent reports no timing of
/// its own, by *tiling* the conversation timeline: a reply took as long as the
/// span between the end of the previous activity and its own completion.
///
/// Why tiling rather than "elapsed since the prompt": most agents split one
/// reply into several assistant turns (a thinking block, each tool round, the
/// final message), and the UI merges those sub-turns back into one card by
/// **summing** their durations (`mergeAdjacentToolGroups` in
/// `message-list-view.tsx`), as do `compute_session_stats` and the token
/// dashboard. Durations must therefore partition the turn, not each restate it
/// from the same origin; measuring every sub-turn from the prompt made a
/// 9-minute Codex reply report ~49 minutes.
///
/// Each assistant turn's span starts at the latest of: the previous turn's
/// completion (so the user's thinking time between a reply and their next
/// prompt is charged to nobody) and any `turn_starts` marker at or before this
/// turn's end. `turn_starts` is for agents that log an explicit start-of-turn
/// event (Codex's `task_started` / `turn_context`); it keeps the measurement
/// anchored even when the prompt itself never made it into the parsed turns.
///
/// Turns that already carry a duration from the agent are left untouched — an
/// agent-reported span always beats an inferred one.
pub fn backfill_turn_durations(turns: &mut [MessageTurn], turn_starts: &[DateTime<Utc>]) {
    // End of the previous turn / latest start marker seen so far. `None` until
    // the first boundary: a transcript that opens mid-reply has nothing to
    // measure that first turn against, and a guess there would be unbounded.
    let mut cursor: Option<DateTime<Utc>> = None;
    let mut next_start = 0usize;

    for turn in turns.iter_mut() {
        let end = turn.completed_at.unwrap_or(turn.timestamp);

        // Adopt every marker that opened at or before this turn ended. Markers
        // are chronological, so this walks each one once across the whole scan.
        while next_start < turn_starts.len() && turn_starts[next_start] <= end {
            advance_duration_cursor(&mut cursor, turn_starts[next_start]);
            next_start += 1;
        }

        if matches!(turn.role, TurnRole::Assistant) && turn.duration_ms.is_none() {
            if let Some(start) = cursor {
                let ms = (end - start).num_milliseconds();
                if ms > 0 {
                    turn.duration_ms = Some(ms as u64);
                }
            }
        }

        advance_duration_cursor(&mut cursor, end);
    }
}

/// Move `cursor` forward to `candidate`, never backward — an out-of-order
/// record (skewed clocks, a tool result written after the reply that follows
/// it) must not rewind the boundary and hand the next turn an inflated span.
fn advance_duration_cursor(cursor: &mut Option<DateTime<Utc>>, candidate: DateTime<Utc>) {
    if cursor.is_none_or(|current| candidate > current) {
        *cursor = Some(candidate);
    }
}

/// Aggregate turn-level usage and duration into a single `SessionStats`.
///
/// The two halves stand on their own: an agent that times its turns but reports
/// no token split still gets its duration (mirrors `acp_native::session_stats`,
/// which already had to hand-roll this). Token totals stay `None` in that case
/// rather than becoming a row of zeros — the footer reads `None` as "this agent
/// doesn't say" and omits the breakdown, where zeros would read as "it says the
/// reply was free".
pub fn compute_session_stats(turns: &[MessageTurn]) -> Option<SessionStats> {
    let mut total_in = 0u64;
    let mut total_out = 0u64;
    let mut total_cache_create = 0u64;
    let mut total_cache_read = 0u64;
    let mut total_duration = 0u64;
    let mut has_usage = false;

    for turn in turns {
        if let Some(ref u) = turn.usage {
            total_in += u.input_tokens;
            total_out += u.output_tokens;
            total_cache_create += u.cache_creation_input_tokens;
            total_cache_read += u.cache_read_input_tokens;
            has_usage = true;
        }
        if let Some(d) = turn.duration_ms {
            total_duration += d;
        }
    }

    if !has_usage && total_duration == 0 {
        return None;
    }

    Some(SessionStats {
        total_usage: has_usage.then_some(TurnUsage {
            input_tokens: total_in,
            output_tokens: total_out,
            cache_creation_input_tokens: total_cache_create,
            cache_read_input_tokens: total_cache_read,
        }),
        total_tokens: has_usage
            .then_some(total_in + total_out + total_cache_create + total_cache_read),
        total_duration_ms: total_duration,
        context_window_used_tokens: None,
        context_window_max_tokens: None,
        context_window_usage_percent: None,
    })
}

fn model_capacity_suffix_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\[\s*([0-9]+(?:\.[0-9]+)?)\s*([km])\s*\]\s*$")
            .expect("valid model capacity regex")
    })
}

/// Matches the SDK's *id* spelling of Anthropic's 1M-context lane, where `1m`
/// is its own delimited token (`claude-opus-4-6-1m`). `\b1m\b` is the same
/// test claude-agent-acp's `inferContextWindowFromModel` applies, and it
/// deliberately does not match embedded runs like `10m`.
fn claude_one_million_id_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b1m\b").expect("valid claude 1m id regex"))
}

/// Whether `model` names Anthropic's 1M-context lane through the SDK's id
/// spelling (`claude-opus-4-6-1m`). The CLI's *display* spelling
/// (`claude-sonnet-5[1m]`) carries the same meaning but is handled by
/// [`parse_model_capacity_suffix`], which reads the bracketed number directly.
fn is_claude_one_million_context_id(model: &str) -> bool {
    claude_one_million_id_regex().is_match(model)
}

/// `claude-<family>-<major>[-<minor>]`, then a snapshot date, a gateway
/// suffix or nothing. The minor may be dotted (`claude-opus-4.7`), as gateways
/// that rename Anthropic's ids write it. It is at most two digits, so a
/// snapshot date such as `-20251001` is never read as one.
fn claude_family_version_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^claude-(opus|sonnet|haiku|fable|mythos)-(\d{1,2})(?:[-.](\d{1,2}))?(?:$|\D)")
            .expect("valid claude family regex")
    })
}

/// Whether `model` (lowercased, provider prefix stripped) is a Claude model
/// whose DEFAULT window is 1M, with no opt-in. This follows the `native_1m`
/// flag of the model catalog baked into Claude Code (CLI 2.1.293): every Fable
/// and Mythos, Opus from 4.7, Sonnet from 5, and Haiku from 5.5. Opus 4.6 and
/// Sonnet 4.5 / 4.6 reach 1M only through the beta lane, which an id spells as
/// `-1m` ([`is_claude_one_million_context_id`]).
fn is_claude_native_one_million_model(model: &str) -> bool {
    let Some(captures) = claude_family_version_regex().captures(model) else {
        return false;
    };
    let major: u32 = captures[2].parse().unwrap_or(0);
    let minor: u32 = captures
        .get(3)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);
    match &captures[1] {
        "fable" | "mythos" => true,
        "opus" => (major, minor) >= (4, 7),
        "sonnet" => major >= 5,
        "haiku" => (major, minor) >= (5, 5),
        _ => false,
    }
}

fn parse_model_capacity_suffix(model: &str) -> Option<u64> {
    let captures = model_capacity_suffix_regex().captures(model.trim())?;
    let value = captures.get(1)?.as_str().parse::<f64>().ok()?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }

    let unit = captures
        .get(2)
        .map(|m| m.as_str().to_ascii_lowercase())
        .unwrap_or_default();
    let multiplier = match unit.as_str() {
        "m" => 1_000_000.0,
        "k" => 1_000.0,
        _ => return None,
    };

    Some((value * multiplier) as u64)
}

pub fn infer_context_window_max_tokens(model: Option<&str>) -> Option<u64> {
    let raw = model?.trim();
    if raw.is_empty() {
        return None;
    }

    if let Some(suffixed_limit) = parse_model_capacity_suffix(raw) {
        return Some(suffixed_limit);
    }

    let normalized = raw
        .rsplit('/')
        .next()
        .unwrap_or(raw)
        .split(':')
        .next()
        .unwrap_or(raw)
        .trim()
        .to_ascii_lowercase();

    // The current Claude models are natively 1M (Opus from 4.7, Sonnet from 5,
    // Haiku from 5.5, every Fable and Mythos). For the older ones the default
    // lane is 200K, and the opt-in 1M lane shows up in the model id itself.
    // Agents other than Claude Code record the id the way their backend named
    // it, so the marker survives here — unlike Claude Code's own transcripts,
    // where it is stripped (see
    // `claude::claude_context_window_max_tokens_for_model`).
    if normalized.starts_with("claude") {
        if is_claude_one_million_context_id(&normalized)
            || is_claude_native_one_million_model(&normalized)
        {
            return Some(1_000_000);
        }
        return Some(200_000);
    }
    // gemini-cli's own `tokenLimit()` (packages/core/src/core/tokenLimits.ts,
    // 0.60.0): 1 << 20 for every Gemini model, a separate 256K bucket for the
    // Gemma family. The round 1_000_000 that used to sit here reported the
    // gauge ~4.9% high.
    if normalized.starts_with("gemini") {
        return Some(1_048_576);
    }
    if normalized.starts_with("gemma") {
        return Some(256_000);
    }
    if normalized.starts_with("kimi") {
        // The k3 family is the 1M lane; k2.x and everything older is 256K.
        // Source of truth is the models.dev catalog kimi-code bundles itself
        // (`app/kosongConfig/builtInModelsDev.ts`): under Moonshot's own
        // `moonshotai` provider, `kimi-k3` is 1048576 while `kimi-k2.6` /
        // `kimi-k2.7-code` / `kimi-k2.7-code-highspeed` are all 262144, and
        // across every third-party provider in that catalog the `kimi-k3*` ids
        // cluster on 1048576 (a handful round it to 1000000). Only the HISTORY
        // gauge lands here — a live Kimi session gets the real window from the
        // agent's own `usage_update {used, size}` frame.
        if normalized.starts_with("kimi-k3") {
            return Some(1_048_576);
        }
        return Some(262_144);
    }
    if normalized.starts_with("grok") {
        // Context windows per x.ai docs (docs.x.ai/developers/models, 2026-07).
        // Grok's model names churn, so match the known families before the
        // generic fallback, most-specific first:
        //   grok-4.5              → 500K
        //   grok-4.3 / grok-4.20  → 1M
        //   grok-build-* / grok-code-fast-1 → 256K (coding models; the latter
        //                           is 256K despite the "fast" in its name)
        //   general -fast (grok-4-fast) → 2M
        // Default any unknown grok model to the conservative 256K rather than
        // guessing high.
        //
        // This is the LAST resort. Grok publishes every model's real window —
        // `availableModels[]._meta.totalContextTokens` on the wire, and
        // `models_cache.json` / a BYO `[model.<id>].context_window` on disk — so
        // both the live path and `grok::build_detail` consult those first and
        // only land here for a model nothing on this machine names.
        if normalized.contains("4.5") {
            return Some(500_000);
        }
        if normalized.contains("4.3") || normalized.contains("4.20") {
            return Some(1_000_000);
        }
        if normalized.contains("code") || normalized.contains("build") {
            return Some(256_000);
        }
        // The 2M lane is grok's OWN `-fast` line. `grok-composer-2.5-fast` is
        // Cursor's Composer, merely offered in grok's model list, and shares
        // none of that capacity — it takes the conservative default below.
        if normalized.contains("fast") && !normalized.contains("composer") {
            return Some(2_000_000);
        }
        return Some(256_000);
    }

    match normalized.as_str() {
        "gpt-5.2-codex" | "gpt-5.1-codex-max" | "gpt-5.1-codex-mini" | "gpt-5.2" => Some(258_000),
        "gpt-5.1" | "gpt-5.1-codex" | "gpt-4o" | "gpt-4o-mini" | "gpt-4-turbo" | "o1-mini"
        | "o1-preview" => Some(128_000),
        "gpt-4" => Some(8_192),
        "o3" | "o3-mini" | "o1" => Some(200_000),
        _ => {
            // 258K is the *effective* window OpenAI's own catalog advertises for
            // this whole generation: `context_window: 272000` with
            // `effective_context_window_percent: 95`. gpt-6 shares that profile
            // byte for byte (see `resources/codex/bundled-catalog.json`), so it
            // rides the same lane rather than falling through to `None` and
            // leaving those sessions with no context meter at all.
            if normalized.starts_with("gpt-5") || normalized.starts_with("gpt-6") {
                Some(258_000)
            } else if normalized.starts_with("gpt-4o")
                || normalized.starts_with("gpt-4.1")
                || normalized.starts_with("gpt-4-turbo")
            {
                Some(128_000)
            } else if normalized.starts_with("o3") || normalized == "o1" {
                Some(200_000)
            } else if normalized.starts_with("o1-mini") || normalized.starts_with("o1-preview") {
                Some(128_000)
            } else {
                None
            }
        }
    }
}

pub fn latest_turn_total_usage_tokens(turns: &[MessageTurn]) -> Option<u64> {
    turns.iter().rev().find_map(|turn| {
        turn.usage.as_ref().map(|usage| {
            usage
                .input_tokens
                .saturating_add(usage.output_tokens)
                .saturating_add(usage.cache_creation_input_tokens)
                .saturating_add(usage.cache_read_input_tokens)
        })
    })
}

/// Context-window occupancy for agents whose transcripts carry ANTHROPIC-SHAPED
/// usage counters (`input_tokens` + `cache_creation_input_tokens` +
/// `cache_read_input_tokens` are the whole prompt; `output_tokens` is the reply).
///
/// Deliberately NOT [`latest_turn_total_usage_tokens`], which adds
/// `output_tokens` too: for this counter shape the reply is not resident in the
/// prompt window that produced it, so including it over-reports the gauge by
/// the last turn's output. Claude Code, Qoder and OpenCode all write this
/// shape (OpenCode's `tokens.input` already has the cached part taken out).
pub fn latest_turn_prompt_usage_tokens(turns: &[MessageTurn]) -> Option<u64> {
    turns.iter().rev().find_map(|turn| {
        turn.usage.as_ref().and_then(|usage| {
            let used = usage
                .input_tokens
                .saturating_add(usage.cache_creation_input_tokens)
                .saturating_add(usage.cache_read_input_tokens);
            (used > 0).then_some(used)
        })
    })
}

pub fn merge_context_window_stats(
    stats: Option<SessionStats>,
    used_tokens: Option<u64>,
    max_tokens: Option<u64>,
) -> Option<SessionStats> {
    if used_tokens.is_none() && max_tokens.is_none() {
        return stats;
    }

    let usage_percent = match (used_tokens, max_tokens) {
        (Some(used), Some(max)) if max > 0 => Some((used as f64 / max as f64) * 100.0),
        _ => None,
    };

    match stats {
        Some(mut s) => {
            s.context_window_used_tokens = used_tokens;
            s.context_window_max_tokens = max_tokens;
            s.context_window_usage_percent = usage_percent;
            Some(s)
        }
        None => Some(SessionStats {
            total_usage: None,
            total_tokens: None,
            total_duration_ms: 0,
            context_window_used_tokens: used_tokens,
            context_window_max_tokens: max_tokens,
            context_window_usage_percent: usage_percent,
        }),
    }
}

/// Stamp a context-window occupancy the AGENT stated directly, overriding
/// whatever [`merge_context_window_stats`] recomputed from used/max.
///
/// Most agents publish token counts and codeg derives the percentage. Qoder
/// publishes the percentage (`usage.context_usage_ratio`) and, for its own
/// hosted models, redacts the token counters to zero — so for those sessions
/// the stated figure is the ONLY occupancy signal that exists, and
/// `merge_context_window_stats` has nothing to divide. It wins even when the
/// counters ARE present, because a recomputation would divide by a window this
/// parser had to back-derive or guess.
///
/// `None` leaves `stats` untouched. A non-finite value is dropped and an
/// out-of-range one is clamped rather than dropped: a gauge is drawn from this,
/// and "no ring" is a worse answer than "pinned at 100%".
pub fn with_reported_context_percent(
    stats: Option<SessionStats>,
    percent: Option<f64>,
) -> Option<SessionStats> {
    let Some(percent) = percent.filter(|p| p.is_finite()) else {
        return stats;
    };
    let percent = percent.clamp(0.0, 100.0);
    match stats {
        Some(mut s) => {
            s.context_window_usage_percent = Some(percent);
            Some(s)
        }
        None => Some(SessionStats {
            total_usage: None,
            total_tokens: None,
            total_duration_ms: 0,
            context_window_used_tokens: None,
            context_window_max_tokens: None,
            context_window_usage_percent: Some(percent),
        }),
    }
}

/// `_meta` key claiming that a context-compaction call's result IS the summary
/// the agent kept of the history it dropped.
///
/// The live reader stamps it on every call it translates from the ACP
/// compaction lifecycle (`acp::connection::session_compaction_event`); a history
/// parser stamps it through [`attach_compaction_summary`] on a divider whose
/// summary its transcript still holds. Codeg-namespaced (like
/// `codeg.delegation`) rather than nested in `contextCompaction`, whose members
/// are the adapters' reserved vocabulary. The frontend twin is
/// `COMPACTION_SUMMARY_META_KEY` in `src/lib/context-compaction.ts`, which shows
/// a compaction call's output as its summary only under this claim.
pub(crate) const COMPACTION_SUMMARY_META_KEY: &str = "codeg.compactionSummary";

/// Hand a synthesized compaction divider — the `_meta.contextCompaction`
/// ToolUse and its paired ToolResult that every parser emits for a compaction —
/// the summary its agent retained, so the reopened divider opens onto the same
/// "Summary" the live one does: the result carries the text, the call carries
/// the claim.
///
/// Returns `false`, touching nothing, unless `blocks` is such a pair still
/// waiting for its summary; a divider takes exactly one.
pub(crate) fn attach_compaction_summary(blocks: &mut [ContentBlock], summary: String) -> bool {
    let [ContentBlock::ToolUse {
        meta: Some(serde_json::Value::Object(meta)),
        ..
    }, ContentBlock::ToolResult { output_preview, .. }] = blocks
    else {
        return false;
    };
    if !meta.contains_key("contextCompaction") || output_preview.is_some() {
        return false;
    }
    meta.insert(
        COMPACTION_SUMMARY_META_KEY.to_string(),
        serde_json::Value::Bool(true),
    );
    *output_preview = Some(summary);
    true
}

/// Relocate orphaned tool_result blocks to the turn that contains their matching tool_use.
///
/// After `group_into_turns` splits assistant rounds, async tool execution can cause
/// a tool_result to land in a later turn than its corresponding tool_use.
/// This post-processing step moves such orphaned results back.
pub fn relocate_orphaned_tool_results(turns: &mut Vec<MessageTurn>) {
    // Build map: tool_use_id → turn index
    let mut tool_use_turn: HashMap<String, usize> = HashMap::new();
    for (idx, turn) in turns.iter().enumerate() {
        for block in &turn.blocks {
            if let ContentBlock::ToolUse {
                tool_use_id: Some(ref id),
                ..
            } = block
            {
                tool_use_turn.insert(id.clone(), idx);
            }
        }
    }

    if tool_use_turn.is_empty() {
        return;
    }

    // Collect (source_turn, target_turn, block) for orphaned results
    let mut relocations: Vec<(usize, usize, ContentBlock)> = Vec::new();
    for (turn_idx, turn) in turns.iter().enumerate() {
        for block in &turn.blocks {
            if let ContentBlock::ToolResult {
                tool_use_id: Some(ref id),
                ..
            } = block
            {
                if let Some(&target) = tool_use_turn.get(id) {
                    if target != turn_idx {
                        relocations.push((turn_idx, target, block.clone()));
                    }
                }
            }
        }
    }

    if relocations.is_empty() {
        return;
    }

    // Build set of (turn_idx, tool_use_id) to remove
    let remove_set: HashMap<usize, Vec<String>> = {
        let mut map: HashMap<usize, Vec<String>> = HashMap::new();
        for (from, _, block) in &relocations {
            if let ContentBlock::ToolResult {
                tool_use_id: Some(ref id),
                ..
            } = block
            {
                map.entry(*from).or_default().push(id.clone());
            }
        }
        map
    };

    // Remove from source turns
    for (&turn_idx, ids) in &remove_set {
        turns[turn_idx].blocks.retain(|block| {
            if let ContentBlock::ToolResult {
                tool_use_id: Some(ref id),
                ..
            } = block
            {
                !ids.contains(id)
            } else {
                true
            }
        });
    }

    // Append to target turns
    for (_, target, block) in relocations {
        turns[target].blocks.push(block);
    }

    // Remove turns that became empty after relocation
    turns.retain(|turn| !turn.blocks.is_empty());
}

/// Convert Read tool output from numbered-line format to `{"start_line":N,"content":"..."}`.
///
/// Claude Code embeds line numbers in Read output like `   115→content`.
/// This splits on the `→` delimiter (or tab for older `cat -n` format),
/// extracts the starting line number, and returns clean content.
pub fn structurize_read_tool_output(turns: &mut [MessageTurn]) {
    let mut read_tool_ids: HashSet<String> = HashSet::new();
    for turn in turns.iter() {
        for block in &turn.blocks {
            if let ContentBlock::ToolUse {
                tool_use_id: Some(ref id),
                ref tool_name,
                ..
            } = block
            {
                let name = tool_name.to_lowercase();
                if matches!(
                    name.as_str(),
                    "read" | "read_file" | "readfile" | "read file" | "cat" | "view"
                ) {
                    read_tool_ids.insert(id.clone());
                }
            }
        }
    }

    for turn in turns.iter_mut() {
        for block in turn.blocks.iter_mut() {
            let is_read_result = matches!(
                block,
                ContentBlock::ToolResult { tool_use_id: Some(ref id), .. }
                if read_tool_ids.contains(id)
            );
            if !is_read_result {
                continue;
            }
            if let ContentBlock::ToolResult {
                ref mut output_preview,
                ..
            } = block
            {
                if let Some(ref text) = output_preview {
                    if let Some(json) = strip_numbered_lines(text) {
                        *output_preview = Some(json);
                    }
                }
            }
        }
    }
}

/// Known delimiters between line number and content.
const LINE_NUM_DELIMITERS: &[&str] = &["→", "\t"];

/// Try to split a line at a known delimiter, returning (line_number, content).
fn split_line_number(line: &str) -> Option<(u64, &str)> {
    for delim in LINE_NUM_DELIMITERS {
        if let Some(pos) = line.find(delim) {
            let prefix = line[..pos].trim();
            if let Ok(num) = prefix.parse::<u64>() {
                let content_start = pos + delim.len();
                return Some((num, &line[content_start..]));
            }
        }
    }
    None
}

/// If most lines have a recognized line-number prefix, strip them all
/// and return `{"start_line":N,"content":"clean text"}`.
pub fn strip_numbered_lines(text: &str) -> Option<String> {
    let raw_lines: Vec<&str> = text.lines().collect();
    if raw_lines.len() < 2 {
        return None;
    }

    let matched = raw_lines
        .iter()
        .filter(|l| l.is_empty() || split_line_number(l).is_some())
        .count();
    if matched < raw_lines.len() * 4 / 5 {
        return None;
    }

    let mut start_line: u64 = 1;
    let mut first = true;
    let stripped: Vec<&str> = raw_lines
        .iter()
        .map(|line| {
            if let Some((num, content)) = split_line_number(line) {
                if first {
                    start_line = num;
                    first = false;
                }
                content
            } else {
                first = false;
                *line
            }
        })
        .collect();

    Some(
        serde_json::json!({
            "start_line": start_line,
            "content": stripped.join("\n")
        })
        .to_string(),
    )
}

/// Resolve line numbers for `*** Update File` / `*** Add File` style patches.
///
/// When a hunk header is just `@@` without `-N,M +N,M`, this reads the actual
/// file from disk and matches the context lines to calculate real line numbers.
/// Falls back gracefully if the file doesn't exist or context doesn't match.
pub fn resolve_patch_line_numbers(turns: &mut [MessageTurn], cwd: Option<&str>) {
    for turn in turns.iter_mut() {
        for block in turn.blocks.iter_mut() {
            if let ContentBlock::ToolUse {
                ref tool_name,
                ref mut input_preview,
                ..
            } = block
            {
                let name = tool_name.to_lowercase();
                if !matches!(
                    name.as_str(),
                    "apply_patch" | "edit" | "patch" | "applypatch"
                ) {
                    continue;
                }
                if let Some(ref text) = input_preview {
                    if text.contains("@@\n") || text.contains("@@\r\n") {
                        if let Some(resolved) = resolve_patch_text(text, cwd) {
                            *input_preview = Some(resolved);
                        }
                    }
                }
            }
        }
    }
}

/// Resolve a single patch text, replacing bare `@@` with `@@ -N,M +N,M @@`.
pub fn resolve_patch_text(patch: &str, cwd: Option<&str>) -> Option<String> {
    let mut output = String::with_capacity(patch.len() + 256);
    let mut current_file_path: Option<String> = None;
    let mut file_lines: Option<Vec<String>> = None;
    let mut any_resolved = false;

    let lines: Vec<&str> = patch.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Detect file markers
        if line.starts_with("*** Update File: ") || line.starts_with("*** Add File: ") {
            let marker_end = if line.starts_with("*** Update File: ") {
                17
            } else {
                14
            };
            let path = line[marker_end..].trim();
            current_file_path = Some(path.to_string());
            file_lines = load_file_lines(path, cwd);
            output.push_str(line);
            output.push('\n');
            i += 1;
            continue;
        }

        // Detect bare @@ hunk header (no line numbers)
        if line == "@@" {
            if let (Some(ref fl), true) = (&file_lines, current_file_path.is_some()) {
                // Collect context lines from this hunk to find match position
                let hunk_lines = collect_hunk_lines(&lines, i + 1);
                if let Some((old_start, old_count, new_count)) = find_hunk_position(fl, &hunk_lines)
                {
                    let new_start = old_start; // same start for context-based patches
                    output.push_str(&format!(
                        "@@ -{},{} +{},{} @@\n",
                        old_start, old_count, new_start, new_count
                    ));
                    any_resolved = true;
                    i += 1;
                    continue;
                }
            }
            // Fallback: keep bare @@
            output.push_str(line);
            output.push('\n');
            i += 1;
            continue;
        }

        output.push_str(line);
        output.push('\n');
        i += 1;
    }

    if any_resolved {
        Some(output)
    } else {
        None
    }
}

/// Load file lines from disk, trying both absolute path and cwd-relative.
pub fn load_file_lines(path: &str, cwd: Option<&str>) -> Option<Vec<String>> {
    use crate::scoped_fs as fs;
    use std::path::Path;

    let p = Path::new(path);
    if p.is_absolute() {
        if let Ok(content) = fs::read_to_string(p) {
            return Some(content.lines().map(|l| l.to_string()).collect());
        }
    }
    if let Some(base) = cwd {
        let full = Path::new(base).join(path);
        if let Ok(content) = fs::read_to_string(&full) {
            return Some(content.lines().map(|l| l.to_string()).collect());
        }
    }
    None
}

/// Collect lines belonging to a hunk (until next `@@` or `*** ` marker or end).
fn collect_hunk_lines<'a>(lines: &'a [&'a str], start: usize) -> Vec<&'a str> {
    let mut result = Vec::new();
    for &line in &lines[start..] {
        if line == "@@" || line.starts_with("*** ") {
            break;
        }
        result.push(line);
    }
    result
}

/// Find where a hunk's context lines match in the file, returning (start_line, old_count, new_count).
/// `start_line` is 1-based.
///
/// The file on disk may be in either pre-patch or post-patch state, and may
/// have been further modified. We try three strategies in order:
/// 1. Contiguous match of context+added lines (post-patch file, no further edits)
/// 2. Contiguous match of context+deleted lines (pre-patch file)
/// 3. Subsequence match of context-only lines (file has been further modified)
fn find_hunk_position(file_lines: &[String], hunk_lines: &[&str]) -> Option<(usize, usize, usize)> {
    let mut old_count = 0usize;
    let mut new_count = 0usize;
    for hl in hunk_lines {
        if hl.starts_with(' ') {
            old_count += 1;
            new_count += 1;
        } else if hl.starts_with('-') {
            old_count += 1;
        } else if hl.starts_with('+') {
            new_count += 1;
        }
    }

    // Strategy 1: contiguous match of context+added (post-patch)
    let new_view: Vec<&str> = hunk_lines
        .iter()
        .filter(|l| l.starts_with(' ') || l.starts_with('+'))
        .map(|l| &l[1..])
        .collect();
    if let Some(pos) = find_contiguous(file_lines, &new_view) {
        return Some((pos + 1, old_count, new_count));
    }

    // Strategy 2: contiguous match of context+deleted (pre-patch)
    let old_view: Vec<&str> = hunk_lines
        .iter()
        .filter(|l| l.starts_with(' ') || l.starts_with('-'))
        .map(|l| &l[1..])
        .collect();
    if let Some(pos) = find_contiguous(file_lines, &old_view) {
        return Some((pos + 1, old_count, new_count));
    }

    // Strategy 3: subsequence match of context-only lines (file further modified)
    let ctx_only: Vec<&str> = hunk_lines
        .iter()
        .filter(|l| l.starts_with(' '))
        .map(|l| &l[1..])
        .collect();
    if let Some(pos) = find_subsequence(file_lines, &ctx_only) {
        return Some((pos + 1, old_count, new_count));
    }

    None
}

/// Find contiguous `view` lines in `file_lines`. Returns 0-based start index.
fn find_contiguous(file_lines: &[String], view: &[&str]) -> Option<usize> {
    if view.is_empty() || view.len() > file_lines.len() {
        return None;
    }
    let first = view[0];
    for i in 0..=(file_lines.len() - view.len()) {
        if file_lines[i].as_str() != first {
            continue;
        }
        if view
            .iter()
            .enumerate()
            .all(|(j, v)| file_lines[i + j].as_str() == *v)
        {
            return Some(i);
        }
    }
    None
}

/// Find `needles` as an ordered subsequence in `file_lines` within a small window.
/// Returns 0-based index of the first needle's position.
fn find_subsequence(file_lines: &[String], needles: &[&str]) -> Option<usize> {
    if needles.is_empty() {
        return None;
    }
    let first = needles[0];
    for start in 0..file_lines.len() {
        if file_lines[start].as_str() != first {
            continue;
        }
        let mut cursor = start + 1;
        let mut all_found = true;
        for &needle in &needles[1..] {
            // Allow up to 10 lines gap between consecutive context lines
            let limit = std::cmp::min(cursor + 10, file_lines.len());
            match file_lines[cursor..limit]
                .iter()
                .position(|fl| fl.as_str() == needle)
            {
                Some(offset) => cursor = cursor + offset + 1,
                None => {
                    all_found = false;
                    break;
                }
            }
        }
        if all_found {
            return Some(start);
        }
    }
    None
}

/// Extract the last path component as the folder name.
pub fn folder_name_from_path(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// Normalize a filesystem path string for tolerant cross-platform comparison.
/// This intentionally does not hit the filesystem (no canonicalize), and only
/// normalizes separators/casing differences that commonly break exact matching.
pub fn normalize_path_for_matching(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/");

    #[cfg(target_os = "windows")]
    {
        if let Some(stripped) = normalized.strip_prefix("//?/") {
            normalized = stripped.to_string();
        }
        normalized = normalized.to_ascii_lowercase();
    }

    while normalized.ends_with('/') {
        if normalized == "/" {
            break;
        }
        // Keep Windows drive root such as "c:/" intact.
        if normalized.len() == 3
            && normalized.as_bytes().get(1) == Some(&b':')
            && normalized.as_bytes().get(2) == Some(&b'/')
        {
            break;
        }
        normalized.pop();
    }

    normalized
}

pub fn path_eq_for_matching(left: &str, right: &str) -> bool {
    normalize_path_for_matching(left) == normalize_path_for_matching(right)
}

