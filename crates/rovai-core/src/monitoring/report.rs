//! Codeg 看板的 Core 投影。只统计原去重流程已经接纳的增量，不改原统计口径。
use super::*;

pub(super) fn record_activity(tx:&rusqlite::Transaction<'_>,epoch:&str,run:&RuntimeUsageRun,delta:&UsageCounters,bucket:&str)->Result<()> {
    let project:Option<String>=tx.query_row("SELECT json_extract(workspace_json,'$.executionRoot') FROM agent_run WHERE id=?1",[&run.key.agent_run_id],|row|row.get(0)).optional()?.flatten();
    tx.execute(r#"INSERT INTO runtime_usage_activity(collection_epoch,agent_run_id,bucket_start_at,runtime_kind,provider_key,model_key,project_root,prompt_input_total_tokens,uncached_input_tokens,cache_read_tokens,cache_write_tokens,output_tokens)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
        ON CONFLICT(collection_epoch,agent_run_id,bucket_start_at,runtime_kind,provider_key,model_key) DO UPDATE SET
        prompt_input_total_tokens=CASE WHEN excluded.prompt_input_total_tokens IS NULL THEN runtime_usage_activity.prompt_input_total_tokens ELSE COALESCE(runtime_usage_activity.prompt_input_total_tokens,0)+excluded.prompt_input_total_tokens END,
        uncached_input_tokens=CASE WHEN excluded.uncached_input_tokens IS NULL THEN runtime_usage_activity.uncached_input_tokens ELSE COALESCE(runtime_usage_activity.uncached_input_tokens,0)+excluded.uncached_input_tokens END,
        cache_read_tokens=CASE WHEN excluded.cache_read_tokens IS NULL THEN runtime_usage_activity.cache_read_tokens ELSE COALESCE(runtime_usage_activity.cache_read_tokens,0)+excluded.cache_read_tokens END,
        cache_write_tokens=CASE WHEN excluded.cache_write_tokens IS NULL THEN runtime_usage_activity.cache_write_tokens ELSE COALESCE(runtime_usage_activity.cache_write_tokens,0)+excluded.cache_write_tokens END,
        output_tokens=CASE WHEN excluded.output_tokens IS NULL THEN runtime_usage_activity.output_tokens ELSE COALESCE(runtime_usage_activity.output_tokens,0)+excluded.output_tokens END"#,
        params![epoch,run.key.agent_run_id,bucket,run.runtime_kind.as_str(),run.provider_key.as_deref().unwrap_or_default(),run.model_key.as_deref().unwrap_or_default(),project,delta.prompt_input_total_tokens,delta.uncached_input_tokens,delta.cache_read_tokens,delta.cache_write_tokens,delta.output_tokens])?;
    Ok(())
}

pub(super) fn retain_activity(tx:&rusqlite::Transaction<'_>,now:DateTime<Utc>)->Result<usize>{
    Ok(tx.execute("DELETE FROM runtime_usage_activity WHERE rowid IN(SELECT rowid FROM runtime_usage_activity WHERE bucket_start_at<?1 ORDER BY bucket_start_at LIMIT 1000)",[(now-Duration::days(366)).to_rfc3339()])?)
}
