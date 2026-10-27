use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::acp::types::PromptInputBlock;
use crate::models::agent::{is_valid_custom_agent_id, BUILTIN_AGENT_TYPES};

// Reserved at every Coaxi prompt ingress before any routing frame is appended.
// The parser may therefore treat a structurally valid RS-bounded frame as
// Coaxi-authored without deleting an API caller's control-character text.
const ROUTE_FRAME_SEPARATOR: char = '\u{001e}';
const ROUTE_FRAME_KIND: &str = "coaxi_internal_agent_routes";
/// Opening bytes of the frame's descriptor line — the anchor used to find a
/// frame in a transcript that no longer has the separators around it (see
/// [`find_unseparated_frame`]). `kind` is the descriptor's first field, so this
/// is a prefix of every rendered body; a field reorder that broke that is what
/// `the_body_anchor_is_what_the_renderer_starts_with` guards.
const ROUTE_FRAME_BODY_PREFIX: &str = "{\"kind\":\"coaxi_internal_agent_routes\"";
const LEGACY_ROUTE_PREFIX: &str = "{\"kind\":\"codeg_internal_agent_routes\"";
const ROUTE_FRAME_VERSION: u8 = 3;
const MAX_ROUTE_FRAME_BYTES: usize = 16 * 1024;
/// Distinct agents one frame may route, applied AFTER deduplication.
const MAX_AGENT_ROUTES: usize = 16;
const MAX_AGENT_REFERENCE_OCCURRENCES: usize = 256;

/// One routed agent inside the frame — nothing but the wire slug.
///
/// The frame is derived from the prompt's own visible `coaxi://agent/...`
/// links, so there is no out-of-band claim to carry or verify. Serializing as
/// `{"agentType":"..."}` is load-bearing: the frame's bytes are its identity
/// (see [`render_internal_agent_routes`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentRoute {
    agent_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InternalAgentRoutes {
    kind: String,
    version: u8,
    nonce: String,
    #[serde(skip)]
    routes: Vec<AgentRoute>,
}

fn agent_reference_regex() -> &'static Regex {
    // Agent wire slugs are built-ins or `custom:<slug>`, where the slug is
    // limited to ASCII alphanumeric, `-`, `_`, and `.`. Requiring a visible
    // label keeps this in lockstep with the frontend reference tokenizer.
    static PATTERN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\[(?:\\.|[^\]\\\r\n])+\]\(coaxi://agent/([A-Za-z0-9][A-Za-z0-9._:-]{0,127})\)")
            .expect("agent reference regex is valid")
    });
    &PATTERN
}

fn visible_agent_reference_types(blocks: &[PromptInputBlock]) -> Vec<String> {
    let mut references = Vec::new();
    for block in blocks {
        let PromptInputBlock::Text { text } = block else {
            continue;
        };
        for capture in agent_reference_regex().captures_iter(text) {
            if references.len() >= MAX_AGENT_REFERENCE_OCCURRENCES {
                return references;
            }
            if let Some(agent_type) = capture.get(1) {
                references.push(agent_type.as_str().to_string());
            }
        }
    }
    references
}

/// Append prompt-local routing for every agent the user visibly linked, when
/// this connection actually received the coaxi-mcp companion's delegation
/// group.
///
/// The prompt IS the input: a visible `[label](coaxi://agent/<type>)` link is
/// the whole criterion. That link is text the agent reads either way, and the
/// frame grants nothing — it only names the channel a delegation should take —
/// so there is deliberately no out-of-band claim to authenticate. Deriving from
/// the text instead means copy-paste, draft restore, queue editing and a
/// remounted composer all behave identically, because they all preserve the
/// link.
///
/// No per-agent special-casing lives here either. `delegation_enabled` is the
/// single launch-time fact this needs: the connection exposed the
/// `delegate_to_agent` tool group, which already implies the agent forwards MCP
/// over the wire (`AcpAgentMeta::supports_mcp` + `agent_delivers_wire_mcp` —
/// that pair is what gates the injection). Whether the named agent is installed
/// is NOT checked here: answering that costs a filesystem sweep per launch, and
/// the delegation spawner is the hard gate for it anyway. A mention that cannot
/// be served comes back as a `delegate_to_agent` error instead.
///
/// The original blocks remain untouched until the connection loop, so
/// broadcasts, previews, and optimistic user messages never contain this
/// internal block.
pub(crate) fn append_agent_routes(blocks: &mut Vec<PromptInputBlock>, delegation_enabled: bool) {
    if !delegation_enabled {
        return;
    }

    let mut seen = BTreeSet::new();
    let routes: Vec<AgentRoute> = visible_agent_reference_types(blocks)
        .into_iter()
        .filter(|agent_type| valid_agent_wire_syntax(agent_type))
        // Dedup first, then bound: a prompt repeating one agent 200 times still
        // routes it, and the cap only bites on distinct agents.
        .filter(|agent_type| seen.insert(agent_type.clone()))
        .take(MAX_AGENT_ROUTES)
        .map(|agent_type| AgentRoute { agent_type })
        .collect();
    if routes.is_empty() {
        return;
    }

    let frame = InternalAgentRoutes {
        kind: ROUTE_FRAME_KIND.to_string(),
        version: ROUTE_FRAME_VERSION,
        nonce: uuid::Uuid::new_v4().to_string(),
        routes,
    };
    blocks.push(PromptInputBlock::Text {
        text: render_internal_agent_routes(&frame),
    });
}

/// The frame's exact bytes ARE its identity: `parse_internal_agent_routes`
/// only accepts a candidate it can re-render byte-for-byte, which is what lets
/// the parsers strip a Coaxi-authored frame without ever deleting look-alike
/// user prose.
///
/// CONSEQUENCE: editing this string orphans every frame already written into an
/// agent's on-disk transcript — they stop round-tripping and become visible
/// history. Any wording change must therefore bump [`ROUTE_FRAME_VERSION`];
/// `route_frame_wording_is_pinned_to_its_version` guards that.
///
/// No compatibility renderer exists yet: `parse_internal_agent_routes` accepts
/// the current version only, so a bump deliberately abandons the frames already
/// on disk. That is a pre-release call — there is no shipped history worth
/// preserving. Once there is, a wording change also has to keep the previous
/// version's renderer alive for PARSING, and dispatch on the candidate's own
/// `version` when round-tripping, so old transcripts keep cleaning up.
///
/// The instruction is narrow on purpose. Agents already discover
/// `delegate_to_agent` from the companion's tool schema; what they do wrong is
/// reach for their own sub-agent mechanism instead. This frame only binds the
/// channel — it does not tell the agent whether to delegate at all.
fn render_internal_agent_routes(frame: &InternalAgentRoutes) -> String {
    format!(
        "{ROUTE_FRAME_SEPARATOR}{}{ROUTE_FRAME_SEPARATOR}",
        render_internal_agent_routes_body(frame)
    )
}

/// The frame WITHOUT its separators: exactly three newline-terminated lines.
///
/// Split out because the separators are the one part of the frame an agent may
/// not give back. They are reserved and scrubbed at ingress, but that only
/// guarantees coaxi never *sends* a stray one — it says nothing about what the
/// agent writes into its own store. Antigravity's ACP server, for one, joins
/// the prompt's text blocks with a space and replaces each separator with one,
/// so its trajectory holds this body and no separators at all.
///
/// Everything the identity argument needs is in here: the nonce, the routes,
/// and the full instruction wording. See [`find_unseparated_frame`].
fn render_internal_agent_routes_body(frame: &InternalAgentRoutes) -> String {
    let descriptor = serde_json::to_string(frame).expect("internal route frame is serializable");
    let routes = serde_json::to_string(&frame.routes).expect("agent routes are serializable");
    format!(
        "{descriptor}\n\
Coaxi composer routing metadata (authoritative): {routes}\n\
When you delegate any of this work, route it through the `delegate_to_agent` \
tool from the `coaxi-mcp` server with `agent_type` taken from the matching \
route: do not substitute your own native sub-agent, task, or spawn mechanism, \
and do not route it through any other delegation tool.\n"
    )
}

// 仅用于读取固定历史格式，不向新任务发出旧协议。
fn render_legacy_agent_routes_body(frame: &InternalAgentRoutes) -> String {
    let descriptor = serde_json::to_string(frame).expect("internal route frame is serializable");
    let routes = serde_json::to_string(&frame.routes).expect("agent routes are serializable");
    format!(
        "{descriptor}\n\
Codeg composer routing metadata (authoritative): {routes}\n\
When you delegate any of this work, route it through the `delegate_to_agent` \
tool from the `codeg-mcp` server with `agent_type` taken from the matching \
route: do not substitute your own native sub-agent, task, or spawn mechanism, \
and do not route it through any other delegation tool.\n"
    )
}

fn valid_agent_wire_syntax(agent_type: &str) -> bool {
    BUILTIN_AGENT_TYPES
        .iter()
        .any(|candidate| candidate.as_wire() == agent_type)
        || agent_type
            .strip_prefix(crate::models::agent::CUSTOM_AGENT_WIRE_PREFIX)
            .is_some_and(is_valid_custom_agent_id)
}

fn parse_internal_agent_routes(candidate: &str) -> Option<InternalAgentRoutes> {
    if !candidate.starts_with(ROUTE_FRAME_SEPARATOR) || !candidate.ends_with(ROUTE_FRAME_SEPARATOR) {
        return None;
    }
    parse_internal_agent_routes_body(
        candidate
            .strip_prefix(ROUTE_FRAME_SEPARATOR)?
            .strip_suffix(ROUTE_FRAME_SEPARATOR)?,
    )
}

/// Validate a candidate against [`render_internal_agent_routes_body`].
///
/// Shared by both finders so the separated and unseparated forms can never
/// drift into accepting different things.
fn parse_internal_agent_routes_body(candidate: &str) -> Option<InternalAgentRoutes> {
    if candidate.len() > MAX_ROUTE_FRAME_BYTES {
        return None;
    }
    let (descriptor, remainder) = candidate.split_once('\n')?;
    let mut frame: InternalAgentRoutes = serde_json::from_str(descriptor).ok()?;
    let legacy = frame.kind == "codeg_internal_agent_routes";
    let metadata_prefix = if legacy { "Codeg composer routing metadata (authoritative): " } else { "Coaxi composer routing metadata (authoritative): " };
    let routes_json = remainder
        .lines()
        .next()?
        .strip_prefix(metadata_prefix)?;
    frame.routes = serde_json::from_str(routes_json).ok()?;
    if (!legacy && frame.kind != ROUTE_FRAME_KIND)
        || frame.version != ROUTE_FRAME_VERSION
        || uuid::Uuid::parse_str(&frame.nonce).is_err()
        || frame.routes.is_empty()
        || frame.routes.len() > MAX_AGENT_ROUTES
        || frame
            .routes
            .iter()
            .any(|route| !valid_agent_wire_syntax(&route.agent_type))
        || (if legacy { render_legacy_agent_routes_body(&frame) } else { render_internal_agent_routes_body(&frame) }) != candidate
    {
        return None;
    }
    Some(frame)
}

/// The next frame at or after `from`, in whichever form the agent stored it.
///
/// Both forms are searched and the EARLIER start wins. That ordering is what
/// keeps the separators inside the removed span whenever they survived: a
/// separated frame's own body is also a valid unseparated match one byte later,
/// so the separated hit is always the earlier one and the stray control
/// characters go with it.
fn find_internal_agent_routes(input: &str, from: usize) -> Option<(usize, usize)> {
    match (
        find_separated_frame(input, from),
        find_unseparated_frame(input, from),
    ) {
        (Some(separated), Some(bare)) if bare.0 < separated.0 => Some(bare),
        (Some(separated), _) => Some(separated),
        (None, bare) => bare,
    }
}

/// A frame the agent gave back exactly as it was sent, separators and all.
fn find_separated_frame(input: &str, from: usize) -> Option<(usize, usize)> {
    let mut search_from = from;
    while search_from < input.len() {
        let start = search_from + input[search_from..].find(ROUTE_FRAME_SEPARATOR)?;
        let after_start = start + ROUTE_FRAME_SEPARATOR.len_utf8();
        let mut bounded_end = (after_start + MAX_ROUTE_FRAME_BYTES).min(input.len());
        while !input.is_char_boundary(bounded_end) {
            bounded_end -= 1;
        }
        if let Some(close_rel) = input[after_start..bounded_end].find(ROUTE_FRAME_SEPARATOR) {
            let end = after_start + close_rel + ROUTE_FRAME_SEPARATOR.len_utf8();
            if parse_internal_agent_routes(&input[start..end]).is_some() {
                return Some((start, end));
            }
        }
        // Preserve an invalid separator and retry at the next byte. This is
        // what prevents an unmatched user opening marker from consuming a
        // later genuine frame and all prose between them.
        search_from = after_start;
    }
    None
}

/// A frame whose separators the agent dropped or rewrote, matched on its body.
///
/// WHY THIS EXISTS. Separators are the cheapest possible proof that a frame is
/// Coaxi-authored, but they only reach a transcript intact if the agent stores
/// the prompt verbatim — and one does not. Antigravity's ACP server joins the
/// prompt's text blocks with a space and replaces each separator with one, so a
/// routed turn lands in `conversations/<id>.db` as
/// `…（使用coaxi-mcp工具）  {"kind":…delegation tool.\n ` — the same body, both
/// separators now spaces. The separated pass cannot see that, and the whole
/// frame renders inside the user's bubble when the session is reopened.
///
/// The body's own NEWLINES do survive that rewrite, which is load-bearing:
/// [`unseparated_frame_end`] delimits a candidate by counting exactly three of
/// them. Only the separators were observed to be rewritten — an agent that
/// flattened newlines too would need a different anchor, not a wider one.
///
/// What stands in for the separators as evidence is the body itself: three
/// lines that must re-render byte-for-byte from their own parsed contents,
/// nonce and full instruction wording included. Prose that reproduces all of
/// that was copied out of a frame, not typed. The residue an agent leaves in
/// place of the separators is left to the callers' existing trims — it is
/// whitespace, and the frame is always the last thing in the record.
fn find_unseparated_frame(input: &str, from: usize) -> Option<(usize, usize)> {
    let mut search_from = from;
    while search_from < input.len() {
        let remaining = &input[search_from..];
        let start = search_from + [remaining.find(ROUTE_FRAME_BODY_PREFIX), remaining.find(LEGACY_ROUTE_PREFIX)].into_iter().flatten().min()?;
        if let Some(end) = unseparated_frame_end(input, start) {
            if parse_internal_agent_routes_body(&input[start..end]).is_some() {
                return Some((start, end));
            }
        }
        // Same rule as the separated scan: a look-alike opening must not hide a
        // real frame further along.
        search_from = start + 1;
    }
    None
}

/// End of an unseparated candidate: the body is exactly three newline-terminated
/// lines, so there is nothing to search for — the third newline IS the close.
fn unseparated_frame_end(input: &str, start: usize) -> Option<usize> {
    let mut bounded_end = (start + MAX_ROUTE_FRAME_BYTES).min(input.len());
    while !input.is_char_boundary(bounded_end) {
        bounded_end -= 1;
    }
    let mut cursor = start;
    for _ in 0..3 {
        cursor += input.get(cursor..bounded_end)?.find('\n')? + 1;
    }
    Some(cursor)
}

/// Remove only complete frames that can be parsed and reproduced byte-for-byte.
/// Coaxi scrubs the reserved separator from every prompt at ingress, so a frame
/// that still round-trips can only have been appended at the final boundary —
/// true for any agent's transcript, not just the one this was first written for.
///
/// A frame the agent stored without its separators is removed too, on the
/// strength of its body alone (see [`find_unseparated_frame`]); only the body
/// goes, and the whitespace an agent put where the separators were is left to
/// the caller's trim.
pub(crate) fn strip_internal_agent_routes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some((start, end)) = find_internal_agent_routes(input, cursor) {
        let prefix = &input[cursor..start];
        let prefix_end = if start > cursor
            && input.as_bytes()[start - 1] == b'\n'
            && !prefix.contains(ROUTE_FRAME_SEPARATOR)
        {
            start - 1
        } else {
            start
        };
        output.push_str(&input[cursor..prefix_end]);
        cursor = end;
        if prefix_end == start
            && cursor < input.len()
            && input.as_bytes().get(cursor) == Some(&b'\n')
        {
            cursor += 1;
        }
    }
    output.push_str(&input[cursor..]);
    output
}

pub(crate) fn contains_internal_agent_routes(input: &str) -> bool {
    find_internal_agent_routes(input, 0).is_some()
}

/// Cut a string at a frame marker left behind by TRUNCATION, for the title path.
///
/// [`strip_internal_agent_routes`] only removes a byte-exact *complete* frame,
/// which is right for turn text but not for a title: parsers derive one by
/// joining a prompt's text blocks and capping the result (80 chars in
/// `acp_native`, 100 in [`title_from_user_text`]), and that cap lands wherever it
/// lands. A short first prompt therefore truncates mid-frame, leaving an opening
/// marker and half a JSON descriptor with no close — which the strip pass skips,
/// putting internal route metadata in the sidebar. The frame's body is 453
/// bytes even for the shortest agent slug, well past either cap, so a title that
/// reaches a frame at all is always sliced inside one.
///
/// Two markers, because either one can be the first thing left of a frame.
/// Cutting at the SEPARATOR is sound because RS is reserved:
/// `strip_route_separator_from_prompt` removes it from every prompt field at
/// ingress, so a separator that survives into persisted history can only be one
/// Coaxi appended itself. The descriptor prefix carries no such guarantee — it
/// is ordinary text a user could in principle type — but it is the only handle
/// left on the agents that rewrite the separators away (Antigravity), and the
/// blast radius of a false positive is one truncated sidebar title, against a
/// certainty of leaking route metadata into every routed conversation's title.
/// Turn text keeps demanding the full round-trip either way; the asymmetry is
/// the same one the separator cut has always had.
///
/// [`title_from_user_text`]: crate::parsers::title_from_user_text
pub(crate) fn cut_at_route_frame_marker(text: &mut String) {
    let Some(cut) = text
        .find(ROUTE_FRAME_SEPARATOR)
        .into_iter()
        .chain(text.find(ROUTE_FRAME_BODY_PREFIX))
        .chain(text.find(LEGACY_ROUTE_PREFIX))
        .chain(find_halved_body_prefix(text))
        .min()
    else {
        return;
    };
    text.truncate(cut);
    text.truncate(text.trim_end().len());
}

/// Start of a descriptor prefix the CAP itself cut in half.
///
/// The separator is one character and cannot be halved: either it is inside the
/// capped window or no frame content is. The descriptor prefix is 37 bytes and
/// very much can be, so a first prompt whose prose length falls in a 37-wide
/// band leaves `…prose  {"kind":"coaxi_int...` — a fragment [`str::find`] cannot
/// see, and the leak this whole cut exists to stop.
///
/// Matched at the END of the string with a trailing run of `.` discarded,
/// because both title paths cap with [`truncate_str`], which marks the cut by
/// appending `...`.
///
/// A fragment shorter than `{"kind":"c` is deliberately left alone: what
/// survives at that length is a generic JSON opening rather than anything that
/// identifies coaxi, and matching it would start trimming ordinary prose that
/// happens to end in `{`.
///
/// [`truncate_str`]: crate::parsers::truncate_str
fn find_halved_body_prefix(text: &str) -> Option<usize> {
    const SHORTEST_FRAGMENT: usize = "{\"kind\":\"c".len();
    let capped = text.trim_end_matches('.');
    // Longest fragment first, so the cut lands at the earliest byte. The full
    // marker is excluded — `find` already covers that case.
    (SHORTEST_FRAGMENT..ROUTE_FRAME_BODY_PREFIX.len())
        .rev()
        .find(|&length| capped.ends_with(&ROUTE_FRAME_BODY_PREFIX[..length]))
        .map(|length| capped.len() - length)
}

/// The record contains one or more valid internal frames and no user-visible
/// content. Used by the Codex parser to avoid phantom turns and promotion
/// coverage from adapters that persist ACP text blocks separately.
pub(crate) fn contains_only_internal_agent_routes(input: &str) -> bool {
    contains_internal_agent_routes(input) && strip_internal_agent_routes(input).trim().is_empty()
}

/// RS is reserved for the internal frame, so it is REMOVED from every prompt
/// at ingress — never used to reject the prompt.
///
/// The character is invisible and mostly arrives inside content the user did
/// not author (an attached file's bytes land in `Resource.text`; ASCII-delimited
/// data and some export formats carry RS legitimately). Rejecting made such a
/// message permanently unsendable with no way for the user to find the offending
/// byte, and it also let a model echo the frame back into a `delegate_to_agent`
/// task string and kill the child prompt. Stripping keeps the invariant that
/// motivated the check — a byte-exact frame inside a Coaxi-authored turn can
/// only have been appended by [`append_agent_routes`] — and makes it stronger,
/// since no user-supplied RS reaches the wire at all.
///
/// That last claim is only true if EVERY field is covered, so the nominally
/// base64 ones (`Image.data`, `Resource.blob`) are scrubbed too. Nothing
/// validates them as base64 before they are forwarded to the agent, and both are
/// plain `String`s an API caller can fill freely; scrubbing is a no-op on
/// well-formed input, since RS is outside the base64 alphabet.
///
/// Must run before any conversation DB, broadcast, title, ledger, or
/// command-channel side effect, so the stored / broadcast / on-the-wire copies
/// of the message stay identical.
pub(crate) fn strip_route_separator_from_prompt(blocks: &mut [PromptInputBlock]) -> bool {
    fn scrub(value: &mut String) -> bool {
        if !value.contains(ROUTE_FRAME_SEPARATOR) {
            return false;
        }
        value.retain(|c| c != ROUTE_FRAME_SEPARATOR);
        true
    }
    fn scrub_opt(value: &mut Option<String>) -> bool {
        value.as_mut().is_some_and(scrub)
    }

    let mut stripped = false;
    for block in blocks.iter_mut() {
        // Every textual field, not just `text`: a sentinel uri or an attachment
        // name is echoed back into the prompt by the composer's serialization.
        match block {
            PromptInputBlock::Text { text } => stripped |= scrub(text),
            PromptInputBlock::Image {
                data,
                mime_type,
                uri,
            } => {
                stripped |= scrub(data);
                stripped |= scrub(mime_type);
                stripped |= scrub_opt(uri);
            }
            PromptInputBlock::Resource {
                uri,
                mime_type,
                text,
                blob,
            } => {
                stripped |= scrub(uri);
                stripped |= scrub_opt(mime_type);
                stripped |= scrub_opt(text);
                stripped |= scrub_opt(blob);
            }
            PromptInputBlock::ResourceLink {
                uri,
                name,
                mime_type,
                description,
            } => {
                stripped |= scrub(uri);
                stripped |= scrub(name);
                stripped |= scrub_opt(mime_type);
                stripped |= scrub_opt(description);
            }
        }
    }
    stripped
}


#[cfg(test)]
mod coaxi_history_brand_tests {
    use super::*;
    #[test]
    fn legacy_frame_round_trip_is_strict_and_user_prose_is_preserved() {
        let frame = InternalAgentRoutes {
            kind: "codeg_internal_agent_routes".into(), version: 3,
            nonce: "1c160a4b-8a9f-454e-9ab3-849a47fb7c33".into(),
            routes: vec![AgentRoute { agent_type: "codex".into() }],
        };
        let old = render_legacy_agent_routes_body(&frame);
        assert_eq!(strip_internal_agent_routes(&format!("用户原文\n{old}")), "用户原文");
        assert!(contains_only_internal_agent_routes(&old));
        let tampered = old.replace("codeg-mcp", "other-mcp");
        assert_eq!(strip_internal_agent_routes(&tampered), tampered);
        let mixed = old.replace("Codeg composer", "Coaxi composer");
        assert_eq!(strip_internal_agent_routes(&mixed), mixed);
    }
}
