//! 固定 Coaxi aff1e6c 的原用量事实选择与聚合，保留逐轮和整会话回退语义。
use super::models::*;
use super::{NativeUsageDetail, UsageFact, UsageFactRow};
use crate::runtime_subagent_session::content::model::MessageTurn;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Timelike, Utc};
use std::collections::{HashMap, HashSet};
const MAX_DENSE_BUCKETS: usize = 3000;
const TOP_CONVERSATIONS: usize = 8;
fn clamp_i64(v: u64) -> i64 {
    v.min(i64::MAX as u64) as i64
}

/// Turn a parsed conversation's turns into storable facts.
///
/// Only turns that actually report usage are kept — a zero-token turn would add
/// a row that contributes nothing but inflates the turn count.
///
/// `occurred_at` is the turn's **own** timestamp — when the assistant spoke and
/// the usage was reported — not `completed_at`. The two differ by more than
/// they look: a turn absorbs the tool results that follow it, so `completed_at`
/// is the moment the *last tool* finished, and one stalled tool (a long build,
/// a command left waiting overnight) drags it arbitrarily far from the API call
/// that actually spent the tokens. Real transcripts hold gaps of more than a
/// day, which is exactly how a turn's spend lands on a calendar day it had
/// nothing to do with. The turn's own timestamp is bounded by the turn itself,
/// and it keeps the series in transcript order.
///
/// `session_model` backfills turns whose parser records no per-turn model.
/// Codex is the motivating case: its rollout files carry the model in
/// `turn_context` events, which the parser folds into the *session* summary
/// while every turn stays `model: None` — without the fallback, all of a Codex
/// session's spend lands in the dashboard's "unknown model" slice. A turn that
/// does name its own model always wins, so mid-session switches recorded by a
/// parser are never overwritten.
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

/// `turn_key` of the whole-session fallback row. Distinct enough from any
/// parser's turn id to be recognizable when auditing the table.
pub(crate) const SESSION_TOTAL_TURN_KEY: &str = "__session_total__";

/// Every fact a parsed conversation contributes.
///
/// Normally that is one row per usage-bearing turn. Some agents, though, only
/// know their token counts at the session level: Hermes reads cumulative
/// `input/output/cache` columns off its own SQLite `sessions` row and leaves
/// every message's `usage` as `None`, precisely because its per-message rows
/// carry no input/output split. Counting only per-turn usage would report those
/// conversations as **zero tokens** — a confident wrong number, not a missing
/// one.
///
/// So when a conversation yields no per-turn facts but does carry a session
/// total, that total is written as a single row. The trade-off is explicit: the
/// session's whole spend lands in one bucket (its last turn's) instead of being
/// spread across the turns that produced it, which is right for every total and
/// breakdown and coarse only on the time axis. The fallback cannot double-count
/// — it fires only when the per-turn pass produced nothing at all.
///
/// Agents that record no token data anywhere (Cursor) still contribute nothing,
/// which is the honest answer for them.
pub(crate) fn facts_from_detail(
    detail: &NativeUsageDetail,
    fallback_at: DateTime<Utc>,
) -> Vec<UsageFact> {
    let session_model = detail.summary.model.as_deref();
    let per_turn = facts_from_turns(&detail.turns, session_model);
    if !per_turn.is_empty() {
        return per_turn;
    }

    let Some(stats) = detail.session_stats.as_ref() else {
        return Vec::new();
    };
    let Some(usage) = stats.total_usage.as_ref() else {
        return Vec::new();
    };
    let input = clamp_i64(usage.input_tokens);
    let output = clamp_i64(usage.output_tokens);
    let cache_create = clamp_i64(usage.cache_creation_input_tokens);
    let cache_read = clamp_i64(usage.cache_read_input_tokens);
    if input == 0 && output == 0 && cache_create == 0 && cache_read == 0 {
        return Vec::new();
    }

    // The end of the conversation is the least-wrong single instant for a
    // session-wide total; `fallback_at` (the row's `updated_at`) covers a
    // conversation whose transcript has stats but no turns at all.
    let occurred_at = detail
        .turns
        .last()
        .map(|t| t.completed_at.unwrap_or(t.timestamp))
        .unwrap_or(fallback_at);

    vec![UsageFact {
        turn_key: SESSION_TOTAL_TURN_KEY.to_string(),
        occurred_at,
        model: detail
            .summary
            .model
            .as_ref()
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty()),
        input_tokens: input,
        output_tokens: output,
        cache_creation_tokens: cache_create,
        cache_read_tokens: cache_read,
        duration_ms: clamp_i64(stats.total_duration_ms),
    }]
}

// ─── Local-time bucket math ─────────────────────────────────────────────

fn local_naive(dt: DateTime<Utc>, offset_minutes: i32) -> NaiveDateTime {
    dt.naive_utc() + Duration::minutes(offset_minutes as i64)
}

fn utc_from_local_naive(n: NaiveDateTime, offset_minutes: i32) -> DateTime<Utc> {
    DateTime::<Utc>::from_naive_utc_and_offset(n - Duration::minutes(offset_minutes as i64), Utc)
}

/// The first local calendar day of the bucket `date` falls in.
fn bucket_start_date(date: NaiveDate, bucket: TokenUsageBucket) -> NaiveDate {
    match bucket {
        TokenUsageBucket::Day => date,
        TokenUsageBucket::Week => {
            date - Duration::days(date.weekday().num_days_from_monday() as i64)
        }
        // `with_day(1)` on a valid date always yields a valid date.
        TokenUsageBucket::Month => date.with_day(1).unwrap_or(date),
    }
}

/// The bucket immediately after the one starting at `date`.
fn next_bucket_date(date: NaiveDate, bucket: TokenUsageBucket) -> NaiveDate {
    match bucket {
        TokenUsageBucket::Day => date + Duration::days(1),
        TokenUsageBucket::Week => date + Duration::days(7),
        TokenUsageBucket::Month => {
            let (y, m) = if date.month() == 12 {
                (date.year() + 1, 1)
            } else {
                (date.year(), date.month() + 1)
            };
            NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(date + Duration::days(31))
        }
    }
}

fn bucket_key(date: NaiveDate, bucket: TokenUsageBucket) -> String {
    match bucket {
        TokenUsageBucket::Month => format!("{:04}-{:02}", date.year(), date.month()),
        _ => date.format("%Y-%m-%d").to_string(),
    }
}

// ─── Aggregation ────────────────────────────────────────────────────────

/// Running sums for one group (a bucket, or one slice of a breakdown).
#[derive(Debug, Default, Clone)]
struct Acc {
    input: u64,
    output: u64,
    cache_create: u64,
    cache_read: u64,
    total: u64,
    duration_ms: u64,
    turns: u64,
    conversations: HashSet<i32>,
}

impl Acc {
    fn add(&mut self, row: &UsageFactRow) {
        // Stored counters are non-negative by construction (`clamp_i64` on a
        // u64), but read them defensively: a hand-edited DB must not be able to
        // produce a negative total that reads as "less than nothing".
        self.input += row.input_tokens.max(0) as u64;
        self.output += row.output_tokens.max(0) as u64;
        self.cache_create += row.cache_creation_tokens.max(0) as u64;
        self.cache_read += row.cache_read_tokens.max(0) as u64;
        self.total += row.total_tokens.max(0) as u64;
        self.duration_ms += row.duration_ms.max(0) as u64;
        self.turns += 1;
        self.conversations.insert(row.conversation_id);
    }

    fn into_totals(self, active_days: u32) -> TokenUsageTotals {
        TokenUsageTotals {
            input_tokens: self.input,
            output_tokens: self.output,
            cache_creation_tokens: self.cache_create,
            cache_read_tokens: self.cache_read,
            total_tokens: self.total,
            turn_count: self.turns,
            conversation_count: self.conversations.len() as u64,
            duration_ms: self.duration_ms,
            active_days,
        }
    }

    fn into_breakdown(self, key: String, label: String) -> TokenUsageBreakdownItem {
        TokenUsageBreakdownItem {
            key,
            label,
            input_tokens: self.input,
            output_tokens: self.output,
            cache_creation_tokens: self.cache_create,
            cache_read_tokens: self.cache_read,
            total_tokens: self.total,
            turn_count: self.turns,
            conversation_count: self.conversations.len() as u64,
        }
    }
}

/// Everything [`aggregate_report`] needs that isn't a fact row.
pub(crate) struct AggregateOptions<'a> {
    pub bucket: TokenUsageBucket,
    pub tz_offset_minutes: i32,
    /// Explicit lower bound from the filter; `None` clamps to the first fact.
    pub range_start: Option<DateTime<Utc>>,
    /// Explicit, exclusive upper bound; `None` clamps to just past the last fact.
    pub range_end: Option<DateTime<Utc>>,
    pub folder_labels: &'a HashMap<i32, String>,
    pub truncated: bool,
    /// A comparison window was requested. Kept separate from "are there
    /// previous rows": a window that was asked for and came back empty is a
    /// real answer (the UI renders it as "new"), and collapsing it to `None`
    /// would make a first-ever period look like one with no comparison at all.
    pub compare_previous: bool,
}

/// Fold filtered facts into everything one dashboard render needs.
///
/// One pass over `rows` feeds every output at once — buckets, the three
/// breakdowns, the weekday×hour grid, the per-conversation tally and the active
/// day set — because a second pass would just re-walk the same vector.
pub(crate) fn aggregate_report(
    rows: &[UsageFactRow],
    previous_rows: &[UsageFactRow],
    opts: &AggregateOptions<'_>,
) -> TokenUsageReport {
    let tz = opts.tz_offset_minutes;
    let bucket = opts.bucket;

    let mut totals = Acc::default();
    let mut by_bucket: HashMap<NaiveDate, Acc> = HashMap::new();
    let mut by_folder: HashMap<i32, Acc> = HashMap::new();
    let mut by_agent: HashMap<String, Acc> = HashMap::new();
    let mut by_model: HashMap<String, Acc> = HashMap::new();
    let mut heat: HashMap<(u8, u8), (u64, u64)> = HashMap::new();
    let mut per_conversation: HashMap<i32, (u64, u64, DateTime<Utc>, String, i32)> = HashMap::new();
    let mut active_dates: HashSet<NaiveDate> = HashSet::new();
    let mut first_activity: Option<DateTime<Utc>> = None;
    let mut last_activity: Option<DateTime<Utc>> = None;

    for row in rows {
        let local = local_naive(row.occurred_at, tz);
        let date = local.date();

        totals.add(row);
        by_bucket
            .entry(bucket_start_date(date, bucket))
            .or_default()
            .add(row);
        by_folder.entry(row.folder_id).or_default().add(row);
        by_agent.entry(row.agent_type.clone()).or_default().add(row);
        // A turn whose transcript never named a model still spent tokens, so it
        // is counted under an explicit "unknown" slice rather than dropped.
        by_model
            .entry(
                row.model
                    .clone()
                    .unwrap_or_else(|| UNKNOWN_MODEL.to_string()),
            )
            .or_default()
            .add(row);

        let cell = heat
            .entry((
                local.weekday().num_days_from_monday() as u8,
                local.hour() as u8,
            ))
            .or_insert((0, 0));
        cell.0 += row.total_tokens.max(0) as u64;
        cell.1 += 1;

        let entry = per_conversation.entry(row.conversation_id).or_insert((
            0,
            0,
            row.occurred_at,
            row.agent_type.clone(),
            row.folder_id,
        ));
        entry.0 += row.total_tokens.max(0) as u64;
        entry.1 += 1;
        if row.occurred_at > entry.2 {
            entry.2 = row.occurred_at;
        }

        active_dates.insert(date);
        if first_activity.is_none_or(|f| row.occurred_at < f) {
            first_activity = Some(row.occurred_at);
        }
        if last_activity.is_none_or(|l| row.occurred_at > l) {
            last_activity = Some(row.occurred_at);
        }
    }

    let active_days = active_dates.len() as u32;
    let series = build_series(&by_bucket, opts, first_activity, last_activity);

    let mut folder_items: Vec<TokenUsageBreakdownItem> = by_folder
        .into_iter()
        .map(|(id, acc)| {
            let label = opts
                .folder_labels
                .get(&id)
                .cloned()
                .unwrap_or_else(|| format!("#{id}"));
            acc.into_breakdown(id.to_string(), label)
        })
        .collect();
    sort_breakdown(&mut folder_items);

    let mut agent_items: Vec<TokenUsageBreakdownItem> = by_agent
        .into_iter()
        .map(|(key, acc)| {
            let label = key.clone();
            acc.into_breakdown(key, label)
        })
        .collect();
    sort_breakdown(&mut agent_items);

    let mut model_items: Vec<TokenUsageBreakdownItem> = by_model
        .into_iter()
        .map(|(key, acc)| {
            let label = key.clone();
            acc.into_breakdown(key, label)
        })
        .collect();
    sort_breakdown(&mut model_items);

    let mut heatmap: Vec<TokenUsageHeatCell> = heat
        .into_iter()
        .map(
            |((weekday, hour), (total_tokens, turn_count))| TokenUsageHeatCell {
                weekday,
                hour,
                total_tokens,
                turn_count,
            },
        )
        .collect();
    heatmap.sort_by_key(|a| (a.weekday, a.hour));

    let mut top: Vec<(i32, u64, u64, DateTime<Utc>, String, i32)> = per_conversation
        .into_iter()
        .map(|(id, (tokens, turns, last, agent, folder_id))| {
            (id, tokens, turns, last, agent, folder_id)
        })
        .collect();
    // Ties broken by id so the list is stable across identical requests.
    top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    top.truncate(TOP_CONVERSATIONS);
    let top_conversations: Vec<TokenUsageConversationItem> = top
        .into_iter()
        .map(
            |(id, tokens, turns, last, agent, folder_id)| TokenUsageConversationItem {
                conversation_id: id,
                // Filled by the command layer, which owns the DB handle.
                title: None,
                agent_type: agent,
                folder_label: opts.folder_labels.get(&folder_id).cloned(),
                total_tokens: tokens,
                turn_count: turns,
                last_activity_at: last,
            },
        )
        .collect();

    let previous_totals = opts.compare_previous.then(|| {
        let mut prev = Acc::default();
        let mut prev_dates: HashSet<NaiveDate> = HashSet::new();
        for row in previous_rows {
            prev.add(row);
            prev_dates.insert(local_naive(row.occurred_at, tz).date());
        }
        prev.into_totals(prev_dates.len() as u32)
    });

    TokenUsageReport {
        range_start: opts.range_start.or(first_activity),
        // Always exclusive, so a consumer that prints "the last day counted"
        // can subtract one instant unconditionally. When the filter had no
        // upper bound this is one tick past the last recorded turn.
        range_end: opts
            .range_end
            .or_else(|| last_activity.map(|t| t + Duration::milliseconds(1))),
        bucket,
        totals: totals.into_totals(active_days),
        previous_totals,
        series,
        by_folder: folder_items,
        by_agent: agent_items,
        by_model: model_items,
        heatmap,
        top_conversations,
        streak: compute_streak(&active_dates),
        first_activity_at: first_activity,
        last_activity_at: last_activity,
        truncated: opts.truncated,
    }
}

/// Model slice for turns whose transcript never named one. A sentinel rather
/// than a dropped row, so the by-model chart's slices still sum to the total.
pub(crate) const UNKNOWN_MODEL: &str = "__unknown__";

fn sort_breakdown(items: &mut [TokenUsageBreakdownItem]) {
    // Tokens descending; the key breaks ties so repeated requests agree.
    items.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens).then(a.key.cmp(&b.key)));
}

/// Emit one point per bucket between the range bounds, including buckets with
/// no activity, so the chart's x-axis carries real gaps instead of silently
/// closing them up.
fn build_series(
    by_bucket: &HashMap<NaiveDate, Acc>,
    opts: &AggregateOptions<'_>,
    first_activity: Option<DateTime<Utc>>,
    last_activity: Option<DateTime<Utc>>,
) -> Vec<TokenUsagePoint> {
    let tz = opts.tz_offset_minutes;
    let bucket = opts.bucket;

    let point_of = |date: NaiveDate, acc: Option<&Acc>| {
        let next = next_bucket_date(date, bucket);
        let acc = acc.cloned().unwrap_or_default();
        TokenUsagePoint {
            bucket_key: bucket_key(date, bucket),
            start: utc_from_local_naive(date.and_hms_opt(0, 0, 0).unwrap_or_default(), tz),
            end: utc_from_local_naive(next.and_hms_opt(0, 0, 0).unwrap_or_default(), tz),
            input_tokens: acc.input,
            output_tokens: acc.output,
            cache_creation_tokens: acc.cache_create,
            cache_read_tokens: acc.cache_read,
            total_tokens: acc.total,
            turn_count: acc.turns,
            conversation_count: acc.conversations.len() as u64,
        }
    };

    let sparse = |by_bucket: &HashMap<NaiveDate, Acc>| {
        let mut dates: Vec<NaiveDate> = by_bucket.keys().copied().collect();
        dates.sort_unstable();
        dates
            .into_iter()
            .map(|d| point_of(d, by_bucket.get(&d)))
            .collect::<Vec<_>>()
    };

    // With no explicit bounds the densified span is the data's own extent; an
    // explicit filter range wins so a deliberately empty window still renders
    // its full axis (that emptiness is the answer).
    let (Some(lo), Some(hi_inclusive)) = (
        opts.range_start.or(first_activity),
        // `range_end` is exclusive: step back an instant so a range ending
        // exactly on a bucket boundary doesn't render a trailing empty bucket.
        opts.range_end
            .map(|e| e - Duration::milliseconds(1))
            .or(last_activity),
    ) else {
        return sparse(by_bucket);
    };

    let first = bucket_start_date(local_naive(lo, tz).date(), bucket);
    let last = bucket_start_date(local_naive(hi_inclusive, tz).date(), bucket);
    if last < first {
        return sparse(by_bucket);
    }

    let mut out: Vec<TokenUsagePoint> = Vec::new();
    let mut cursor = first;
    while cursor <= last {
        out.push(point_of(cursor, by_bucket.get(&cursor)));
        if out.len() >= MAX_DENSE_BUCKETS {
            // Pathological span (daily buckets over many years). Fall back to
            // the sparse form rather than returning a truncated axis that would
            // silently hide the tail.
            return sparse(by_bucket);
        }
        cursor = next_bucket_date(cursor, bucket);
    }
    out
}

/// Longest and trailing runs of consecutive active local days.
fn compute_streak(active: &HashSet<NaiveDate>) -> TokenUsageStreak {
    if active.is_empty() {
        return TokenUsageStreak::default();
    }
    let mut dates: Vec<NaiveDate> = active.iter().copied().collect();
    dates.sort_unstable();

    let mut longest = 1u32;
    let mut run = 1u32;
    for pair in dates.windows(2) {
        if pair[1] == pair[0] + Duration::days(1) {
            run += 1;
        } else {
            run = 1;
        }
        longest = longest.max(run);
    }
    TokenUsageStreak {
        longest_days: longest,
        // `run` ends at the last date by construction.
        current_days: run,
        current_ends_on: dates.last().map(|d| d.format("%Y-%m-%d").to_string()),
    }
}

// ─── Report command ─────────────────────────────────────────────────────
