//! Coaxi aff1e6c278453ed8d0b96a5f664edcdca621aa3f, qoder.rs:358-370,399-602.
//! Apache-2.0: integration/licenses/component-a/LICENSE. Original replay selection;
//! display fallbacks do not by themselves grant a native agent file.
use serde_json::Value;
use std::collections::{HashMap, HashSet};

fn is_content_record(value: &Value) -> bool {
    matches!(
        value.get("type").and_then(Value::as_str),
        Some("user" | "assistant")
    )
}

fn is_sidechain(value: &Value) -> bool {
    value
        .get("isSidechain")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Indices of the `user`/`assistant` records that make up the conversation as
/// Qoder itself would replay it, in chronological order.
///
/// Qoder's own reader resolves history by walking `parentUuid` back from a leaf
/// and reversing, tolerating cycles and broken links (it records them as
/// diagnostics and stops). This mirrors that, with two choices worth naming:
///
/// * **Which leaf.** The newest `active-leaf` record wins when it appears after
///   the last content record — that is the marker Qoder rewrites on every
///   commit, and it is the only thing that knows about a rewind the user made
///   without sending anything afterwards. Otherwise the last content record is
///   the leaf, which is what a rewind-then-continue produces anyway (the new
///   message descends from the rewound leaf, so the abandoned branch is simply
///   not among its ancestors).
/// * **When NOT to trust the walk.** Every fallback here errs toward showing
///   MORE history, because hiding a message the user really sent is the only
///   unrecoverable outcome. So append order wins whenever the graph does not
///   fully explain the file: no `parentUuid` links anywhere (an older writer),
///   a walk that dies on a dangling parent or a cycle instead of reaching a
///   root, or a tail-chosen leaf that is itself a root while older content
///   exists (a writer that chained only some records — a genuine rewind to the
///   session's first message can only be selected by an explicit `active-leaf`).
///   A rewind never produces any of those: the abandoned records stay in the
///   file, so the live chain still resolves cleanly to the root.
///
/// `attachment` and `system` records are indexed but not returned: they sit ON
/// the chain between content records, so the walk has to be able to step
/// through them even though they render nothing here.
pub(super) fn active_branch(records: &[Value]) -> Vec<usize> {
    let append_order: Vec<usize> = records
        .iter()
        .enumerate()
        .filter(|(_, v)| is_content_record(v) && !is_sidechain(v))
        .map(|(i, _)| i)
        .collect();

    let mut by_uuid: HashMap<&str, usize> = HashMap::new();
    let mut has_links = false;
    for (i, value) in records.iter().enumerate() {
        if is_sidechain(value) {
            continue;
        }
        if let Some(uuid) = value.get("uuid").and_then(Value::as_str) {
            by_uuid.entry(uuid).or_insert(i);
        }
        has_links |= value
            .get("parentUuid")
            .and_then(Value::as_str)
            .is_some_and(|p| !p.is_empty());
    }
    let tail = append_order.last().copied();

    // `leafUuid: null` is Qoder saying the branch is EMPTY — not that the marker
    // is broken. The record schema declares the field nullable
    // (`gpr(A,"leafUuid")`, the same nullable-string check `parentUuid` gets),
    // and the rewind path persists whatever it retained:
    // `recordActiveLeaf(n.retainedLeafUuid, {explicit:true, rewound:true})`,
    // which is null exactly when the user rewound the FIRST prompt and kept
    // nothing. Qoder then replays an empty conversation; treating the marker as
    // malformed and falling back to the tail would re-display the prompt, the
    // reply, the title and the token usage that the user just cleared.
    //
    // Two conditions keep this from ever blanking a live session — the one
    // outcome worse than showing too much. The marker must be NEWER than every
    // content record (a stale null followed by fresh messages must not erase
    // them), and `leafUuid` must be PRESENT and literally null: a missing or
    // malformed key is a writer this parser does not understand, and those still
    // fall through to showing history.
    let newest_marker = records
        .iter()
        .enumerate()
        .rev()
        .find(|(_, value)| value.get("type").and_then(Value::as_str) == Some("active-leaf"));
    if let Some((marker_pos, marker)) = newest_marker {
        if matches!(marker.get("leafUuid"), Some(Value::Null))
            && tail.is_none_or(|tail_idx| marker_pos > tail_idx)
        {
            return Vec::new();
        }
    }

    if !has_links {
        return append_order;
    }

    let marker = records.iter().enumerate().rev().find_map(|(pos, value)| {
        if value.get("type").and_then(Value::as_str) != Some("active-leaf") {
            return None;
        }
        let leaf = value.get("leafUuid").and_then(Value::as_str)?;
        by_uuid.get(leaf).copied().map(|idx| (pos, idx))
    });
    let (leaf, chosen_by_marker) = match (marker, tail) {
        // The marker only overrides the tail when it was written LATER: a stale
        // `active-leaf` followed by fresh messages must not truncate them away.
        (Some((marker_pos, marker_idx)), Some(tail_idx)) => {
            if marker_pos > tail_idx {
                (marker_idx, true)
            } else {
                (tail_idx, false)
            }
        }
        (Some((_, marker_idx)), None) => (marker_idx, true),
        (None, Some(tail_idx)) => (tail_idx, false),
        (None, None) => return append_order,
    };

    let mut chain = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut reached_root = false;
    let mut cursor = Some(leaf);
    while let Some(index) = cursor {
        if !seen.insert(index) {
            break; // parent cycle — the graph is not trustworthy
        }
        chain.push(index);
        // `logicalParentUuid` is how the chain survives a COMPACTION. Qoder
        // writes the `system`/`compact_boundary` record with `parentUuid: null`
        // on purpose — that is what makes the post-compaction segment its own
        // root for replay — and stashes the real predecessor in
        // `logicalParentUuid`. Following only `parentUuid` would stop dead at
        // the boundary and hide every message the user sent before it.
        match records[index]
            .get("parentUuid")
            .and_then(Value::as_str)
            .filter(|parent| !parent.is_empty())
            .or_else(|| {
                records[index]
                    .get("logicalParentUuid")
                    .and_then(Value::as_str)
                    .filter(|parent| !parent.is_empty())
            }) {
            // `null`, absent or empty on BOTH: this record starts the session.
            None => {
                reached_root = true;
                break;
            }
            Some(parent) => cursor = by_uuid.get(parent).copied(),
        }
    }
    if !reached_root {
        return append_order;
    }

    chain.reverse();
    chain.retain(|&i| is_content_record(&records[i]));

    if chain.is_empty() || (!chosen_by_marker && chain.len() < 2 && append_order.len() > 1) {
        return append_order;
    }
    reattach_parallel_tool_results(records, &mut chain, &by_uuid);
    chain
}

/// Re-attach the tool results that hang OFF the chain instead of sitting on it.
///
/// A batch of PARALLEL tool calls is not one record. The 1.1.23 bundle writes
/// one assistant record per `tool_use` and then one `user` record per result —
/// `…map(A => WMA([A], o.get(A.tool_use_id) ?? …))`, where `o` maps a
/// `tool_use.id` to the uuid of the assistant record that issued it, and that
/// uuid becomes the result's `sourceToolAssistantUUID` and hence its
/// `parentUuid`. The results are therefore SIBLINGS: each descends from its own
/// call, and only the last one appended is an ancestor of whatever the model
/// says next.
///
/// A pure ancestor walk consequently keeps exactly one result per parallel
/// batch and silently drops the others, leaving their `tool_use` cards looking
/// like calls that never came back. Anything reading two files at once — a
/// completely ordinary turn — hits this.
///
/// Gating on `sourceToolAssistantUUID` is what keeps the repair from
/// resurrecting an abandoned branch: a rewind re-roots on a HUMAN prompt, which
/// carries no such marker, and the tool results re-run under it point at the new
/// assistant records, not the live ones.
fn reattach_parallel_tool_results(
    records: &[Value],
    chain: &mut Vec<usize>,
    by_uuid: &HashMap<&str, usize>,
) {
    let on_chain: HashSet<usize> = chain.iter().copied().collect();
    let mut recovered: Vec<usize> = records
        .iter()
        .enumerate()
        .filter(|(index, value)| {
            !on_chain.contains(index)
                && is_content_record(value)
                && !is_sidechain(value)
                && value
                    .get("sourceToolAssistantUUID")
                    .and_then(Value::as_str)
                    .and_then(|uuid| by_uuid.get(uuid))
                    .is_some_and(|issuer| on_chain.contains(issuer))
        })
        .map(|(index, _)| index)
        .collect();
    if recovered.is_empty() {
        return;
    }
    chain.append(&mut recovered);
    // The transcript is append-only, so file order IS turn order: sorting the
    // merged set puts every recovered result back beside its siblings.
    chain.sort_unstable();
}
