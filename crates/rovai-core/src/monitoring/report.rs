//! Codeg 看板的 Core 投影。只统计原去重流程已经接纳的增量，不改原统计口径。
use super::*;

pub(super) fn record_activity(tx:&rusqlite::Transaction<'_>,epoch:&str,run:&RuntimeUsageRun,delta:&UsageCounters,bucket:&str)->Result<()> {
    let project:Option<String>=tx.query_row("SELECT json_extract(workspace_json,'$.executionRoot') FROM agent_run WHERE id=?1",[&run.key.agent_run_id],|row|row.get(0)).optional()?.flatten();
    tx.execute(r#"INSERT INTO runtime_usage_activity(collection_epoch,agent_run_id,bucket_start_at,runtime_kind,provider_key,model_key,project_root,prompt_input_total_tokens,uncached_input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,partial_observations)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
        ON CONFLICT(collection_epoch,agent_run_id,bucket_start_at,runtime_kind,provider_key,model_key) DO UPDATE SET
        partial_observations=runtime_usage_activity.partial_observations+excluded.partial_observations,
        prompt_input_total_tokens=CASE WHEN excluded.prompt_input_total_tokens IS NULL THEN runtime_usage_activity.prompt_input_total_tokens ELSE COALESCE(runtime_usage_activity.prompt_input_total_tokens,0)+excluded.prompt_input_total_tokens END,
        uncached_input_tokens=CASE WHEN excluded.uncached_input_tokens IS NULL THEN runtime_usage_activity.uncached_input_tokens ELSE COALESCE(runtime_usage_activity.uncached_input_tokens,0)+excluded.uncached_input_tokens END,
        cache_read_tokens=CASE WHEN excluded.cache_read_tokens IS NULL THEN runtime_usage_activity.cache_read_tokens ELSE COALESCE(runtime_usage_activity.cache_read_tokens,0)+excluded.cache_read_tokens END,
        cache_write_tokens=CASE WHEN excluded.cache_write_tokens IS NULL THEN runtime_usage_activity.cache_write_tokens ELSE COALESCE(runtime_usage_activity.cache_write_tokens,0)+excluded.cache_write_tokens END,
        output_tokens=CASE WHEN excluded.output_tokens IS NULL THEN runtime_usage_activity.output_tokens ELSE COALESCE(runtime_usage_activity.output_tokens,0)+excluded.output_tokens END"#,
        params![epoch,run.key.agent_run_id,bucket,run.runtime_kind.as_str(),run.provider_key.as_deref().unwrap_or_default(),run.model_key.as_deref().unwrap_or_default(),project,delta.prompt_input_total_tokens,delta.uncached_input_tokens,delta.cache_read_tokens,delta.cache_write_tokens,delta.output_tokens,i64::from(delta.prompt_input_total_tokens.is_none()||delta.output_tokens.is_none())])?;
    Ok(())
}

pub(super) fn retain_activity(tx:&rusqlite::Transaction<'_>,now:DateTime<Utc>)->Result<usize>{
    Ok(tx.execute("DELETE FROM runtime_usage_activity WHERE rowid IN(SELECT rowid FROM runtime_usage_activity WHERE bucket_start_at<?1 ORDER BY bucket_start_at LIMIT 1000)",[(now-Duration::days(366)).to_rfc3339()])?)
}

use chrono::{Datelike, NaiveDate};
use anyhow::ensure;

#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Query {
    range:String,
    #[serde(default)]start:Option<String>,#[serde(default)]end:Option<String>,
    #[serde(default)]tz_offset_minutes:i32,
    #[serde(default="daily")]bucket:String,
    #[serde(default)]runtime_kind:Option<AdapterKind>,
    #[serde(default)]provider_key:Option<String>,#[serde(default)]model_key:Option<String>,
    #[serde(default)]project_root:Option<String>,
}
fn daily()->String{"day".into()}
struct Bounds {from:DateTime<Utc>,to:DateTime<Utc>,offset:Duration}
impl Query {
    fn bounds(&self,now:DateTime<Utc>,recording:DateTime<Utc>)->Result<Bounds>{
        ensure!((-840..=840).contains(&self.tz_offset_minutes),"usage.invalid_timezone");
        ensure!(["day","week","month"].contains(&self.bucket.as_str()),"usage.invalid_bucket");
        for value in [&self.provider_key,&self.model_key,&self.project_root].into_iter().flatten(){ensure!(!value.trim().is_empty()&&value.len()<=4096,"usage.invalid_filter");}
        let offset=Duration::minutes(self.tz_offset_minutes.into());
        let local=(now+offset).date_naive();
        let midnight=|date:NaiveDate|date.and_hms_opt(0,0,0).map(|value|value.and_utc()-offset).context("usage.invalid_date");
        let tomorrow=midnight(local.succ_opt().context("usage.invalid_date")?)?;
        let (start,end)=match self.range.as_str(){
            "24h"=>(now-Duration::hours(24),now),
            "7d"=>(tomorrow-Duration::days(7),tomorrow),
            "30d"=>(tomorrow-Duration::days(30),tomorrow),
            "90d"=>(tomorrow-Duration::days(90),tomorrow),
            "thisMonth"=>(midnight(local.with_day(1).context("usage.invalid_date")?)?,tomorrow),
            "thisYear"=>(midnight(NaiveDate::from_ymd_opt(local.year(),1,1).context("usage.invalid_date")?)?,tomorrow),
            "all"=>(recording.max(now-Duration::days(366)),tomorrow),
            "custom"=>{
                let parse=|value:Option<&str>|NaiveDate::parse_from_str(value.context("usage.date_required")?,"%Y-%m-%d").map_err(anyhow::Error::from);
                let from=parse(self.start.as_deref())?;let through=parse(self.end.as_deref())?;
                (midnight(from)?,midnight(through.succ_opt().context("usage.invalid_date")?)?)
            },
            _=>anyhow::bail!("usage.invalid_range"),
        };
        ensure!(start<end&&end-start<=Duration::days(367)&&end<=tomorrow,"usage.invalid_range");
        Ok(Bounds{from:start.max(recording).min(end),to:end,offset})
    }
    fn where_sql(&self,epoch:&str,bounds:&Bounds)->(String,Vec<SqlValue>){
        let mut sql=String::from("collection_epoch=? AND bucket_start_at>=? AND bucket_start_at<?");
        let mut params=vec![epoch.into(),bounds.from.to_rfc3339().into(),bounds.to.to_rfc3339().into()];
        let runtime=self.runtime_kind.map(|value|value.as_str().to_string());
        for (column,value)in [("runtime_kind",&runtime),("provider_key",&self.provider_key),("model_key",&self.model_key),("project_root",&self.project_root)]{
            if let Some(value)=value{sql.push_str(&format!(" AND {column}=?"));params.push(value.clone().into());}
        }(sql,params)
    }
}
const COUNTERS:&str="SUM(prompt_input_total_tokens),SUM(uncached_input_tokens),SUM(cache_read_tokens),SUM(cache_write_tokens),SUM(output_tokens),COALESCE(SUM(COALESCE(prompt_input_total_tokens,0)+COALESCE(output_tokens,0)),0),COALESCE(SUM(partial_observations),0),COUNT(*)";
fn counters(row:&rusqlite::Row<'_>,at:usize)->rusqlite::Result<Value>{
    Ok(json!({"inputTokens":row.get::<_,Option<i64>>(at)?,"uncachedInputTokens":row.get::<_,Option<i64>>(at+1)?,"cacheReadTokens":row.get::<_,Option<i64>>(at+2)?,"cacheWriteTokens":row.get::<_,Option<i64>>(at+3)?,"outputTokens":row.get::<_,Option<i64>>(at+4)?,"knownTotalTokens":row.get::<_,i64>(at+5)?,"partialObservations":row.get::<_,i64>(at+6)?,"observedBuckets":row.get::<_,i64>(at+7)?}))
}
fn summary(connection:&Connection,sql:&str,params:&[SqlValue])->Result<Value>{
    Ok(connection.query_row(&format!("SELECT {COUNTERS},COUNT(DISTINCT agent_run_id) FROM runtime_usage_activity WHERE {sql}"),params_from_iter(params.iter()),|row|{let mut value=counters(row,0)?;value["runs"]=json!(row.get::<_,i64>(8)?);Ok(value)})?)
}
fn ranked(connection:&Connection,sql:&str,params:&[SqlValue],dimension:&str)->Result<Vec<Value>>{
    // 只允许内部固定表达式参与拼接，筛选值始终绑定。
    let field=match dimension{"project"=>"COALESCE(project_root,'')","runtime"=>"runtime_kind","model"=>"provider_key||' / '||model_key",_=>anyhow::bail!("usage.invalid_dimension")};
    let query=format!("WITH grouped AS(SELECT {field} AS key,{COUNTERS} FROM runtime_usage_activity WHERE {sql} GROUP BY {field}) SELECT key FROM grouped ORDER BY 7 DESC,key LIMIT 7");
    let keys=connection.prepare(&query)?.query_map(params_from_iter(params.iter()),|row|row.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut result=Vec::new();
    for key in &keys{let mut bound=params.to_vec();bound.push(key.clone().into());let mut value=summary(connection,&format!("{sql} AND {field}=?"),&bound)?;value["key"]=json!(key);result.push(value);}
    if !keys.is_empty(){let mut bound=params.to_vec();bound.extend(keys.iter().cloned().map(SqlValue::Text));let placeholders=vec!["?";keys.len()].join(",");let mut other=summary(connection,&format!("{sql} AND {field} NOT IN({placeholders})"),&bound)?;if other["observedBuckets"].as_i64().unwrap_or(0)>0{other["key"]=json!("__other__");result.push(other);}}
    Ok(result)
}
fn bucket_day(date:NaiveDate,bucket:&str)->Result<NaiveDate>{Ok(match bucket{"week"=>date-Duration::days(date.weekday().num_days_from_monday().into()),"month"=>date.with_day(1).context("usage.invalid_date")?,_=>date})}

impl MonitoringService {
    pub fn report(database:&mut Database,value:Value)->Result<Value>{
        let input:Query=serde_json::from_value(value)?;let now=Utc::now();
        let (epoch,collection_started)=collection_identity(database)?;
        let connection=database.connection();
        let recording:String=connection.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ',applied_at) FROM schema_migration WHERE version=212",[],|row|row.get(0))?;
        let recording_at=parse_time(&recording)?.max(parse_time(&collection_started)?);
        let bounds=input.bounds(now,recording_at)?;let (sql,params)=input.where_sql(&epoch,&bounds);
        let totals=summary(connection,&sql,&params)?;
        let modifier=format!("{:+} minutes",input.tz_offset_minutes);
        let group=match input.bucket.as_str(){"month"=>format!("strftime('%Y-%m-01',bucket_start_at,'{modifier}')"),"week"=>format!("date(bucket_start_at,'{modifier}','-'||((CAST(strftime('%w',bucket_start_at,'{modifier}') AS INTEGER)+6)%7)||' days')"),_=>format!("date(bucket_start_at,'{modifier}')")};
        let mut query=connection.prepare(&format!("SELECT {group},{COUNTERS} FROM runtime_usage_activity WHERE {sql} GROUP BY 1 ORDER BY 1"))?;
        let mut by_day=query.query_map(params_from_iter(params.iter()),|row|Ok((row.get::<_,String>(0)?,counters(row,1)?)))?.collect::<rusqlite::Result<BTreeMap<_,_>>>()?;drop(query);
        let mut series=Vec::new();let mut day=bucket_day((bounds.from+bounds.offset).date_naive(),&input.bucket)?;
        let end=(bounds.to+bounds.offset-Duration::nanoseconds(1)).date_naive();
        while day<=end&&series.len()<370{let key=day.format("%Y-%m-%d").to_string();let mut item=by_day.remove(&key).unwrap_or_else(||json!({"inputTokens":null,"uncachedInputTokens":null,"cacheReadTokens":null,"cacheWriteTokens":null,"outputTokens":null,"knownTotalTokens":0,"partialObservations":0,"observedBuckets":0}));item["key"]=json!(key);series.push(item);day=match input.bucket.as_str(){"week"=>day+Duration::days(7),"month"=>day.checked_add_months(chrono::Months::new(1)).context("usage.invalid_date")?,_=>day.succ_opt().context("usage.invalid_date")?};}
        let mut heat=connection.prepare(&format!("SELECT (CAST(strftime('%w',bucket_start_at,'{modifier}') AS INTEGER)+6)%7,CAST(strftime('%H',bucket_start_at,'{modifier}') AS INTEGER),{COUNTERS} FROM runtime_usage_activity WHERE {sql} GROUP BY 1,2 ORDER BY 1,2"))?;
        let heatmap=heat.query_map(params_from_iter(params.iter()),|row|{let mut value=counters(row,2)?;value["weekday"]=json!(row.get::<_,i32>(0)?);value["hour"]=json!(row.get::<_,i32>(1)?);Ok(value)})?.collect::<rusqlite::Result<Vec<_>>>()?;drop(heat);
        let previous_to=bounds.from;let previous_from=(previous_to-(bounds.to-bounds.from)).max(recording_at).min(previous_to);
        let previous_bounds=Bounds{from:previous_from,to:previous_to,offset:bounds.offset};let(previous_sql,previous_params)=input.where_sql(&epoch,&previous_bounds);
        let previous=summary(connection,&previous_sql,&previous_params)?;
        let projects=ranked(connection,&sql,&params,"project")?;let runtimes=ranked(connection,&sql,&params,"runtime")?;let models=ranked(connection,&sql,&params,"model")?;
        let facets=|column:&str|->Result<Vec<String>>{Ok(connection.prepare(&format!("SELECT DISTINCT {column} FROM runtime_usage_activity WHERE collection_epoch=?1 AND {column} IS NOT NULL AND {column}<>'' ORDER BY {column} LIMIT 501"))?.query_map([&epoch],|row|row.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?)};
        let filter=MonitoringFilter{range:"30d".into(),runtime_kind:input.runtime_kind,provider_key:input.provider_key.clone(),model_key:input.model_key.clone(),cost_kind:None};
        let mut cost_scope=Scope::new(&epoch,bounds.from,bounds.to,&filter);
        if let Some(project)=&input.project_root{cost_scope.where_sql.push_str(" AND agent_run_id IN(SELECT id FROM agent_run WHERE json_extract(workspace_json,'$.executionRoot')=?)");cost_scope.params.push(project.clone().into());}
        let run_cost=load_run_cost(connection,&cost_scope)?.into_iter().map(money_json).collect::<Vec<_>>();
        Ok(json!({"schemaVersion":1,"source":"core_usage_activity","generatedAt":now.to_rfc3339(),"recordingStartedAt":recording,"retentionDays":366,"resolutionSeconds":3600,"range":{"from":bounds.from.to_rfc3339(),"to":bounds.to.to_rfc3339()},"bucket":input.bucket,"tzOffsetMinutes":input.tz_offset_minutes,"totals":totals,"previous":previous,"series":series,"heatmap":heatmap,"byProject":projects,"byRuntime":runtimes,"byModel":models,"facets":{"projects":facets("project_root")?,"runtimes":facets("runtime_kind")?,"providers":facets("provider_key")?,"models":facets("model_key")?,"limit":500},"runCost":run_cost,"costScope":"whole_runs_overlapping_range","includesImportedNativeHistory":false}))
    }
}
