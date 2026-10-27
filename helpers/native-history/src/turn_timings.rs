//! Per-turn wall-clock observations recorded by coaxi's own ACP connection
//! layer, for agents whose native session store carries **no per-turn
//! timestamps**.
//!
//! Cursor's ACP `store.db` is the motivating case (verified exhaustively on
//! real sessions): messages carry no timestamps at all, the root record's
//! only epoch field (`f26`) is a one-shot stamp set mid-first-turn (title
//! generation) that never updates, and the CLI store's `turn_timings` (root
//! `f14`) is absent from ACP sessions. Tool calls carry start/end stamps
//! (`f59`/`f60`), so tool-ful turns get a synthesized clock — but a
//! thinking+text-only turn is natively clockless, and its message footer
//! rendered nothing.
//!
//! Every ACP session of such an agent runs THROUGH coaxi, so the connection
//! layer is a first-party witness of each turn's real span: it stamps the
//! prompt send and observes the turn's completion. This module persists that
//! observation as one JSONL line per completed turn, keyed by the agent's own
//! session id, and the history parser merges it back for turns whose native
//! store yields no clock. The data is honest — coaxi's own measurement, not a
//! fabrication from unrelated fields.
//!
//! ## File layout
//!
//! `<paths::coaxi_turn_timings_root()>/<agent>/<session-id>.jsonl`, one JSON
//! object per line:
//!
//! ```jsonc
//! {"v":1,"ord":3,"conn":"<connection uuid>","prompt_sha":"<16 hex chars>",
//!  "started_at_ms":...,"ended_at_ms":...}
//! ```
//!
//! `prompt_sha` is the first 8 bytes (16 hex chars) of the SHA-256 of the
//! prompt's concatenated text blocks — enough to correlate, and it avoids
//! persisting prompt text outside the agent's own store. `(conn, ord)` is
//! the structural position anchor: `ord` counts EVERY prompt turn of the
//! recording connection (journaled or not), and ordinals are comparable only
//! within one `conn`.
//!
//! ## Matching (reader side)
//!
//! Alignment is defense-in-depth; every layer degrades to "no clock", never
//! a wrong clock (each closes a concrete Codex-review counterexample):
//!
//! 1. **Line validity** — schema version, non-inverted span, `ord ≥ 1`,
//!    non-empty `conn`; invalid lines are skipped.
//! 2. **Whole-file order gate** — any `started_at_ms` regression rejects the
//!    ENTIRE journal (a landing-order anomaly means positions can't be
//!    trusted; keeping a prefix would leave a stale pseudo-tail).
//! 3. **Trailing `(conn, ord)`-contiguity trim** — only the journal's tail
//!    run of same-connection, strictly consecutive ordinals is trusted; a
//!    hole (dropped line, skipped non-`end_turn` turn) or a connection
//!    boundary ends trust, so the walk can never slide across a gap onto an
//!    older same-hash entry.
//! 4. **Count guard** — a trusted run longer than the store's user-turn
//!    count means phantom lines; reject everything.
//! 5. **Tail-anchored hash walk** — last user turn ↔ last trusted line,
//!    stepping backwards while `prompt_hash(user text)` matches, stopping at
//!    the first mismatch.
//! 6. **Same-hash run-length equality guard** — identical-prompt runs must
//!    have EQUAL length on both sides or nothing in the run is assigned.
//!
//! Accepted residuals (Codex-reviewed; closing either would need a
//! store-side persistence/tail anchor, which Cursor does not expose):
//!
//! * A turn that reported a clean `end_turn` but was never actually
//!   persisted by Cursor shifts alignment within a trusted run;
//!   misassignment additionally requires an identical repeated prompt at
//!   exactly the shifted offset.
//! * A missing journal SUFFIX — the session's newest turns all lost their
//!   lines (canceled/empty turns are deliberately unjournaled; queue-full
//!   drops) — leaves an older line as the journal tail. If the store's
//!   newest turn hash-collides with that stale tail (and the same-hash run
//!   lengths happen to align), the old span is attributed to the newer turn.
//!   Pinned by `parsers::cursor` test
//!   `accepted_residual_missing_tail_lines_can_misattribute_span`.
//!
//! Sessions never run through this coaxi install (copied from another
//! machine) simply have no journal file.
//!
//! ## Growth
//!
//! ~100 bytes per completed turn, one file per session. There is no GC —
//! the footprint is negligible next to the agent's own session store (a
//! single Cursor `store.db`-wal outweighs thousands of journal lines), and
//! deleting a journal would silently strip history clocks.

use std::io::Write as _;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// One observed turn span. `prompt_sha` correlates the entry to the user
/// turn that started it (see the module docs for the matching rules).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurnTiming {
    /// Schema version; bump on incompatible changes so old lines are skipped
    /// rather than misread.
    pub v: u32,
    /// 1-based turn ordinal within the recording CONNECTION: incremented for
    /// EVERY prompt turn the connection runs (journaled or not), so two
    /// adjacent ordinals prove two ADJACENT turns. This is the reader's
    /// contiguity anchor (Codex review R5): only the journal's trailing
    /// strictly-consecutive ordinal run is trusted for tail alignment — an
    /// ordinal hole (a dropped line, a skipped non-`end_turn` turn) or a
    /// restart (a resumed session's new connection starts back at 1) ends
    /// the trusted run, so the reverse walk can never slide across a gap
    /// onto an older same-hash entry. Lines with `ord == 0` (pre-ordinal
    /// writers) are skipped as invalid.
    #[serde(default)]
    pub ord: u64,
    /// The recording connection's id. Ordinals are only comparable WITHIN
    /// one connection — numerically consecutive ordinals from DIFFERENT
    /// connections prove nothing (Codex review R6: old connection journals
    /// ord 1, a resumed connection's ord-1 turn is canceled-unjournaled and
    /// its ord 2 lands next → `1 → 2` looks contiguous across the boundary).
    /// The reader's trim additionally requires equal `conn` while walking.
    /// Lines with an empty `conn` are skipped as invalid.
    #[serde(default)]
    pub conn: String,
    /// First 16 hex chars of SHA-256 over the prompt's concatenated text
    /// blocks ("" hashes the empty string — image-only prompts still match).
    pub prompt_sha: String,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
}

pub const TURN_TIMING_SCHEMA_VERSION: u32 = 1;

/// Journal agent key for Cursor sessions — shared by the connection-layer
/// writer and the `parsers::cursor` reader so the two can't drift.
pub const CURSOR_JOURNAL_AGENT: &str = "cursor";

/// Hash a prompt's text for journal correlation: SHA-256, first 8 bytes,
/// lower hex. Writer hashes the concatenated text blocks it sends; the reader
/// hashes the user turn text it parsed — the two match exactly when the agent
/// stored the text verbatim, and a mismatch merely leaves the turn clockless.
pub fn prompt_hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// Journal ids come from the agent's own session identifiers (UUID-shaped in
/// practice). Reject anything that could traverse out of the journal root —
/// defensive only, but this string does originate from filesystem/wire data.
fn safe_component(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && !s.starts_with('.')
}

/// `<root>/<agent>/<session_id>.jsonl`, or `None` when either component is
/// unsafe as a file name.
fn journal_path_in(root: &std::path::Path, agent: &str, session_id: &str) -> Option<PathBuf> {
    if !safe_component(agent) || !safe_component(session_id) {
        return None;
    }
    Some(root.join(agent).join(format!("{session_id}.jsonl")))
}

/// One queued journal append. `ack` fires (best-effort) once the line is on
/// disk so the enqueuer can bound-wait for determinism before emitting
/// TurnComplete.
struct JournalJob {
    root: PathBuf,
    agent: String,
    session_id: String,
    timing: TurnTiming,
    ack: tokio::sync::oneshot::Sender<()>,
}

/// Bound on queued appends. One line per completed turn across all sessions —
/// 256 outstanding means the writer thread has been stuck for a very long
/// time; further lines are dropped (missing entries only ever degrade turns
/// to "no clock").
const JOURNAL_QUEUE_CAP: usize = 256;

/// The journal's single-writer front door. ALL production appends flow
/// through one dedicated OS thread consuming a FIFO channel, which is what
/// makes file line order structurally equal to enqueue order (Codex review):
/// a stalled write can NEVER be overtaken by a later turn's line — the later
/// job simply waits in the queue — so no mutex-fairness, timestamp-tie, or
/// wall-clock-rollback combination can land same-hash spans swapped. A hung
/// filesystem blocks only this thread (never Tokio's async or blocking
/// pools); the queue then fills and further lines are DROPPED with a debug
/// log — degradation is always "missing entry", never "reordered entry".
static JOURNAL_TX: std::sync::OnceLock<std::sync::mpsc::SyncSender<JournalJob>> =
    std::sync::OnceLock::new();

/// Queue one observed turn for the session's journal, returning a receiver
/// that resolves once the line has landed (or errors if the job was dropped —
/// callers treat both the same and bound their wait). `root` is injected so
/// tests write into a temp tree through the very same writer path.
pub fn enqueue_turn_timing(
    root: PathBuf,
    agent: String,
    session_id: String,
    timing: TurnTiming,
) -> tokio::sync::oneshot::Receiver<()> {
    let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
    let tx = JOURNAL_TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::sync_channel::<JournalJob>(JOURNAL_QUEUE_CAP);
        let spawned = std::thread::Builder::new()
            .name("turn-timings-writer".into())
            .spawn(move || {
                for job in rx {
                    append_turn_timing_in(&job.root, &job.agent, &job.session_id, &job.timing);
                    let _ = job.ack.send(());
                }
            });
        if let Err(e) = spawned {
            // No consumer: the channel fills to cap and every later enqueue
            // drops. Journaling is best-effort by contract.
            tracing::debug!("[turn-timings] failed to spawn writer thread: {e}");
        }
        tx
    });
    if tx
        .try_send(JournalJob {
            root,
            agent,
            session_id,
            timing,
            ack: ack_tx,
        })
        .is_err()
    {
        tracing::debug!("[turn-timings] journal queue full or closed; entry dropped");
    }
    ack_rx
}

/// Append one observed turn to the session's journal. Best-effort by
/// contract: any failure (unresolvable path, dir/file I/O) is logged at
/// debug and swallowed. Called by the writer thread (production) and
/// directly by tests building fixture journals — production code must go
/// through [`enqueue_turn_timing`] so the single-writer ordering invariant
/// holds.
pub fn append_turn_timing_in(
    root: &std::path::Path,
    agent: &str,
    session_id: &str,
    timing: &TurnTiming,
) {
    let Some(path) = journal_path_in(root, agent, session_id) else {
        tracing::debug!("[turn-timings] skipping journal append: unsafe id agent={agent} session={session_id}");
        return;
    };
    let Ok(line) = serde_json::to_string(timing) else {
        return;
    };
    let write = || -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            crate::scoped_fs::create_dir_all(dir)?;
        }
        let mut f = crate::scoped_fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        // Single `write_all` of "line\n" keeps the append atomic enough for
        // this single-writer file (one live connection per session).
        f.write_all(format!("{line}\n").as_bytes())
    };
    if let Err(e) = write() {
        tracing::debug!("[turn-timings] journal append failed for {}: {e}", path.display());
    }
}

/// Read a session's journal, oldest → newest. Missing file → empty. Malformed
/// or wrong-version lines are skipped so one bad write can't poison the rest.
///
/// Chronological-order gate: if the surviving lines are NOT monotonically
/// non-decreasing by `started_at_ms`, the WHOLE journal is rejected (empty
/// result), not just a suffix. Line order is the alignment contract — the
/// parser tail-anchors its matching on the journal's LAST entry — and a
/// disorder means some write landed out of turn order (deep write pile-up
/// with unfair mutex wakeup, two processes sharing a data dir, a hand-edited
/// file). Keeping the ordered prefix is NOT safe: its last entry becomes a
/// stale pseudo-tail that a later same-hash turn (image-only prompts all
/// hash "") can pair with across the gap (Codex review). Rejecting
/// everything degrades the session to "no clock", never a wrong clock.
/// Strict `<` keeps legitimately tied starts; the in-process append lock
/// (see [`append_turn_timing_in`]) makes disorder rare to begin with.
pub fn read_turn_timings(agent: &str, session_id: &str) -> Vec<TurnTiming> {
    let root = crate::paths::coaxi_turn_timings_root();
    if journal_path_in(&root, agent, session_id).is_some_and(|path| path.exists()) {
        return read_turn_timings_in(&root, agent, session_id);
    }
    // 已保存的计时仅作为旧历史读取；新记录始终写入 Coaxi 目录。
    read_turn_timings_in(&crate::scoped_fs::root().join(".codeg-turn-timings"), agent, session_id)
}

/// Root-injectable core of [`read_turn_timings`].
pub fn read_turn_timings_in(
    root: &std::path::Path,
    agent: &str,
    session_id: &str,
) -> Vec<TurnTiming> {
    let Some(path) = journal_path_in(root, agent, session_id) else {
        return Vec::new();
    };
    let Ok(content) = crate::scoped_fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut out: Vec<TurnTiming> = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(t) = serde_json::from_str::<TurnTiming>(line) else {
            continue;
        };
        if t.v != TURN_TIMING_SCHEMA_VERSION
            || t.ended_at_ms < t.started_at_ms
            || t.ord == 0
            || t.conn.is_empty()
        {
            continue;
        }
        // Order gate: any regression in start time invalidates the whole
        // journal (see the doc comment on [`read_turn_timings`]).
        if let Some(prev) = out.last() {
            if t.started_at_ms < prev.started_at_ms {
                tracing::debug!(
                    "[turn-timings] journal for {agent}/{session_id} is out of order; rejecting all entries"
                );
                return Vec::new();
            }
        }
        out.push(t);
    }
    out
}

/// Current wall clock in epoch milliseconds — the journal's time base.
pub fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

