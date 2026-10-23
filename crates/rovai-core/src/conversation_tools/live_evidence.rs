//! 从已有可见消息的原 Run 读取工具名与实际计数；不输出工具参数、结果或最新会话上下文。
use super::*;
use rusqlite::{Connection, OptionalExtension, params};

pub(crate) fn tools(
    connection: &Connection,
    source: &sources::Source,
    run: Option<&str>,
    at: &str,
) -> Result<(Vec<String>, bool)> {
    let Some(run) = run else {
        return Ok((vec![], false));
    };
    let sources::Locator::Live { conversation_id } = &source.locator else {
        anyhow::bail!("conversation_tools.live_source_required")
    };
    let visible:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM agent_run r JOIN conversation c ON c.id=r.conversation_id WHERE r.id=?1 AND r.camp_id=?2 AND ((?3='public' AND c.kind='camp_member')OR(?3<>'public' AND c.id=?4)))",params![run,source.owner_thread_id,source.source_kind,conversation_id],|row|row.get(0))?;
    if !visible {
        return Ok((vec![], false));
    }
    let mut query=connection.prepare("SELECT id,execution_epoch,event_type,kind,phase,payload_preview_json FROM agent_run_execution_evidence WHERE agent_run_id=?1 AND kind IN ('tool_call','tool_result') AND julianday(occurred_at)<=julianday(?2) ORDER BY sequence LIMIT 513")?;
    let mut rows = query
        .query_map(params![run, at], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut truncated = rows.len() > 512;
    rows.truncate(512);
    let mut names = Vec::new();
    for (id, epoch, event, kind, phase, raw) in rows {
        let Ok(payload) = serde_json::from_str::<Value>(&raw) else {
            truncated = true;
            continue;
        };
        let facts = crate::canonical_activity::classify_evidence(
            run, epoch, &id, &event, &kind, &phase, &payload,
        );
        if let Some(name) = facts.tool_name {
            let name = name
                .chars()
                .filter(|character| !character.is_control())
                .take(256)
                .collect::<String>();
            if !name.is_empty() && !names.contains(&name) {
                if names.len() < 128 {
                    names.push(name)
                } else {
                    truncated = true;
                }
            }
        }
    }
    Ok((names, truncated))
}

pub(crate) fn stats(connection: &Connection, runs: &[(String, String)]) -> Result<Value> {
    let mut totals = [0i64; 4];
    let mut observed = [false; 4];
    let mut query=connection.prepare("SELECT u.prompt_input_total_tokens,u.output_tokens,u.cache_read_tokens,u.cache_write_tokens FROM runtime_usage_run_summary u JOIN runtime_usage_collection_state state ON state.collection_epoch=u.collection_epoch WHERE u.agent_run_id=?1 AND u.finalized_at IS NOT NULL AND julianday(u.last_observed_at)<=julianday(?2)")?;
    for (run, at) in runs {
        let usage: Option<[Option<i64>; 4]> = query
            .query_row(params![run, at], |row| {
                Ok([row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?])
            })
            .optional()?;
        if let Some(usage) = usage {
            for (index, count) in usage.into_iter().enumerate() {
                if let Some(count) = count {
                    if !(0..=9_007_199_254_740_991).contains(&count) {
                        continue;
                    }
                    let Some(next) = totals[index]
                        .checked_add(count)
                        .filter(|value| *value <= 9_007_199_254_740_991)
                    else {
                        continue;
                    };
                    totals[index] = next;
                    observed[index] = true;
                }
            }
        }
    }
    let mut usage = serde_json::Map::new();
    for (index, key) in [
        "input_tokens",
        "output_tokens",
        "cache_read_tokens",
        "cache_write_tokens",
    ]
    .iter()
    .enumerate()
    {
        if observed[index] {
            usage.insert((*key).into(), json!(totals[index]));
        }
    }
    Ok(if usage.is_empty() {
        json!({"total_usage":null,"coverage":"unavailable"})
    } else {
        json!({"total_usage":usage,"coverage":"partial"})
    })
}
