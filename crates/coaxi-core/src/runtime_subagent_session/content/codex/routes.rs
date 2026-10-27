// Adapted from fixed Codeg aff1e6c (Apache-2.0).
// Source ranges and changes: integration/inventory/native-codex-content-port.json.
use serde::{Deserialize, Serialize};
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
    BUILTIN_AGENT_WIRES.contains(&agent_type)
        || agent_type
            .strip_prefix("custom:")
            .is_some_and(is_valid_custom_agent_id)
}
fn parse_internal_agent_routes(candidate: &str) -> Option<InternalAgentRoutes> {
    if !candidate.starts_with(ROUTE_FRAME_SEPARATOR) || !candidate.ends_with(ROUTE_FRAME_SEPARATOR)
    {
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

/// The record contains one or more valid internal frames and no user-visible
/// content. Used by the Codex parser to avoid phantom turns and promotion
/// coverage from adapters that persist ACP text blocks separately.
pub(crate) fn contains_only_internal_agent_routes(input: &str) -> bool {
    contains_internal_agent_routes(input) && strip_internal_agent_routes(input).trim().is_empty()
}

const BUILTIN_AGENT_WIRES: &[&str] = &[
    "claude_code",
    "codex",
    "open_code",
    "gemini",
    "open_claw",
    "cline",
    "hermes",
    "code_buddy",
    "kimi_code",
    "pi",
    "grok",
    "cursor",
    "deepseek",
    "qoder",
    "antigravity",
];
pub fn is_valid_custom_agent_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        // A leading dot would produce a hidden directory (and `.`/`..` would
        // escape one); reject the whole class rather than special-case it.
        && !id.starts_with('.')
        // Must not collide with a built-in wire name, or `from_wire` would be
        // ambiguous and the custom agent could shadow a real one.
        && !matches!(
            id,
            "claude_code"
                | "codex"
                | "open_code"
                | "gemini"
                | "open_claw"
                | "cline"
                | "hermes"
                | "code_buddy"
                | "kimi_code"
                | "pi"
                | "grok"
                | "cursor"
                | "deepseek"
                | "qoder"
                | "antigravity"
        )
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
