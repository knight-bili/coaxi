// Adapted from fixed Codeg aff1e6c (Apache-2.0).
// Source ranges and changes: integration/inventory/native-codex-content-port.json.
pub(super) use super::super::common::{
    backfill_turn_durations, compute_session_stats, relocate_orphaned_tool_results,
    structurize_read_tool_output, title_from_user_text, truncate_str,
};
use super::super::model::*;
use chrono::{DateTime, Utc};
pub(crate) fn turn_error_message(
    id: String,
    message: &str,
    timestamp: DateTime<Utc>,
) -> Option<UnifiedMessage> {
    let message = message.trim();
    if message.is_empty() {
        return None;
    }
    Some(UnifiedMessage {
        id,
        role: MessageRole::System,
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
pub(crate) const COMPACTION_SUMMARY_META_KEY: &str = "coaxi.compactionSummary";

/// Hand a synthesized compaction divider — the `_meta.contextCompaction`
/// ToolUse and its paired ToolResult that every parser emits for a compaction —
/// the summary its agent retained, so the reopened divider opens onto the same
/// "Summary" the live one does: the result carries the text, the call carries
/// the claim.
///
/// Returns `false`, touching nothing, unless `blocks` is such a pair still
/// waiting for its summary; a divider takes exactly one.
pub(crate) fn attach_compaction_summary(blocks: &mut [ContentBlock], summary: String) -> bool {
    let [
        ContentBlock::ToolUse {
            meta: Some(serde_json::Value::Object(meta)),
            ..
        },
        ContentBlock::ToolResult { output_preview, .. },
    ] = blocks
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

/// Original patch geometry over an explicitly authorized Core line reader.
pub fn resolve_patch_line_numbers(
    turns: &mut [MessageTurn],
    cwd: Option<&str>,
    load: &mut impl FnMut(&str, Option<&str>) -> Option<Vec<String>>,
) {
    for turn in turns.iter_mut() {
        for block in turn.blocks.iter_mut() {
            if let ContentBlock::ToolUse {
                tool_name,
                input_preview,
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
                if let Some(text) = input_preview {
                    if text.contains("@@\n") || text.contains("@@\r\n") {
                        if let Some(resolved) = resolve_patch_text(text, cwd, load) {
                            *input_preview = Some(resolved);
                        }
                    }
                }
            }
        }
    }
}

/// Resolve a single patch text, replacing bare `@@` with `@@ -N,M +N,M @@`.
pub fn resolve_patch_text(
    patch: &str,
    cwd: Option<&str>,
    load: &mut impl FnMut(&str, Option<&str>) -> Option<Vec<String>>,
) -> Option<String> {
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
            file_lines = load(path, cwd);
            output.push_str(line);
            output.push('\n');
            i += 1;
            continue;
        }

        // Detect bare @@ hunk header (no line numbers)
        if line == "@@" {
            if let (Some(fl), true) = (&file_lines, current_file_path.is_some()) {
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

    if any_resolved { Some(output) } else { None }
}

/// Load file lines from disk, trying both absolute path and cwd-relative.
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
pub(crate) fn displayed_session_title(raw: &str) -> Option<String> {
    // What AIR's pattern calls white space: Java's ASCII `\s`.
    const AIR_WHITESPACE: [char; 6] = [' ', '\t', '\n', '\u{0B}', '\u{0C}', '\r'];
    let mut visible = raw.trim();
    while let Some(rest) = visible.strip_prefix("[archived]") {
        let title = rest.trim_start_matches(AIR_WHITESPACE);
        if title.len() == rest.len() || title.trim().is_empty() {
            break;
        }
        // `trim` again: the adapter collapses and trims what it publishes, so
        // a Unicode space left after the marker never reaches the wire.
        visible = title.trim();
    }
    if visible.is_empty() {
        None
    } else {
        Some(truncate_str(visible, 100))
    }
}
pub(crate) fn tool_result_content_text(content: &serde_json::Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return (!text.is_empty()).then(|| text.to_string());
    }
    let mut out = String::new();
    for item in content.as_array()? {
        if item.get("type").and_then(serde_json::Value::as_str) == Some("text") {
            if let Some(text) = item.get("text").and_then(serde_json::Value::as_str) {
                out.push_str(text);
            }
        }
    }
    (!out.is_empty()).then_some(out)
}
