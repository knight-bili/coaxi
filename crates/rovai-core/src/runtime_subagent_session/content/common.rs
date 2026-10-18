// Adapted from Codeg aff1e6c, Apache-2.0. Pure normalization only.
use super::model::*;
use chrono::{DateTime, Utc};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
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
pub fn relocate_orphaned_tool_results(turns: &mut Vec<MessageTurn>) {
    // Build map: tool_use_id → turn index
    let mut tool_use_turn: HashMap<String, usize> = HashMap::new();
    for (idx, turn) in turns.iter().enumerate() {
        for block in &turn.blocks {
            if let ContentBlock::ToolUse {
                tool_use_id: Some(id),
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
                tool_use_id: Some(id),
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
                tool_use_id: Some(id),
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
                tool_use_id: Some(id),
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
                tool_use_id: Some(id),
                tool_name,
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
                ContentBlock::ToolResult { tool_use_id: Some(id), .. }
                if read_tool_ids.contains(id)
            );
            if !is_read_result {
                continue;
            }
            if let ContentBlock::ToolResult { output_preview, .. } = block {
                if let Some(text) = output_preview {
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

pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}
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
    chars.get(k) == Some(&'\\')
        && chars
            .get(k + 1)
            .is_some_and(|c| !is_markdown_whitespace(*c))
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
