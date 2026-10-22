//! 导入原生历史的派生用量索引。原始材料只读，和融合运行统计不相加。
use super::*;
use std::collections::{BTreeMap,HashMap};
use chrono::{DateTime,Utc};
use serde::{Deserialize,Serialize};
use crate::runtime_subagent_session::content::model::{MessageTurn,SessionStats};
mod aggregate;
pub(crate) mod models;
mod facts;
use facts::{UsageFact,UsageFactRow};

const CACHE_VERSION:u32=1;
const MAX_FACTS:usize=500_000;
const MAX_ARCHIVES:usize=5000;
#[derive(Deserialize,Default)]
struct NativeSummary {#[serde(default)]model:Option<String>}
struct NativeUsageDetail {summary:NativeSummary,turns:Vec<MessageTurn>,session_stats:Option<SessionStats>}
#[derive(Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Cached {version:u32,import_id:String,source_digest:String,transcript_digest:String,source_identity:String,provider:String,indexed_at:String,session_total_fallback:bool,facts:Vec<UsageFact>}
fn checked_json(path:&Path,limit:u64)->Result<Value>{reject_link(path)?;ensure!(fs::metadata(path)?.is_file()&&fs::metadata(path)?.len()<=limit,"history.usage_file_too_large");Ok(serde_json::from_slice(&fs::read(path)?)?)}
fn manifests(directory:&Path)->Result<Vec<(String,Value)>>{
    if !directory.exists(){return Ok(Vec::new())}reject_link(directory)?;let mut rows=Vec::new();let mut bytes=0u64;
    for entry in fs::read_dir(directory)?{let entry=entry?;let name=entry.file_name().to_string_lossy().into_owned();let Some(id)=name.strip_suffix(".manifest.json")else{continue};identifier(id)?;bytes=bytes.checked_add(fs::symlink_metadata(entry.path())?.len()).context("history.usage_manifest_limit")?;ensure!(bytes<=32*1024*1024,"history.usage_manifest_limit");let value=checked_json(&entry.path(),1024*1024)?;ensure!(value["id"]==id,"history.manifest_identity_changed");rows.push((id.to_string(),value));ensure!(rows.len()<=MAX_ARCHIVES,"history.usage_archive_limit");}
    rows.sort_by(|a,b|a.0.cmp(&b.0));Ok(rows)
}
fn cache_path(directory:&Path,id:&str)->PathBuf{directory.join("usage-projection-v1").join(format!("{id}.json"))}
fn read_cache(directory:&Path,id:&str,manifest:&Value)->Result<Cached>{
    let saved:Cached=serde_json::from_value(checked_json(&cache_path(directory,id),32*1024*1024)?)?;
    ensure!(saved.version==CACHE_VERSION&&saved.import_id==id&&saved.source_digest==manifest["sourceDigest"].as_str().unwrap_or_default()&&saved.facts.len()<=100_000,"history.usage_cache_changed");
    Ok(saved)
}
pub(super) fn index(directory:&Path,id:&str,force:bool)->Result<Value>{
    identifier(id)?;let manifest_path=directory.join(format!("{id}.manifest.json"));let manifest=checked_json(&manifest_path,1024*1024)?;
    if !force{if let Ok(cache)=read_cache(directory,id,&manifest){return Ok(json!({"id":id,"indexed":true,"unchanged":true,"facts":cache.facts.len()}));}}
    let transcript_path=directory.join(format!("{id}.transcript.json"));let transcript=checked_json(&transcript_path,128*1024*1024)?;
    let transcript_digest=crate::command::canonical_json_digest(&transcript)?;
    let summary=serde_json::from_value(transcript.get("summary").filter(|value|value.is_object()).cloned().unwrap_or_else(||json!({})))?;
    let detail=NativeUsageDetail{summary,turns:serde_json::from_value(transcript["turns"].clone())?,session_stats:serde_json::from_value(transcript.get("session_stats").cloned().unwrap_or(Value::Null))?};
    let fallback=manifest["summary"]["ended_at"].as_str().or_else(||manifest["summary"]["started_at"].as_str()).or_else(||manifest["importedAt"].as_str()).context("history.usage_time_unavailable")?;
    let fallback=DateTime::parse_from_rfc3339(fallback)?.with_timezone(&Utc);
    let facts=aggregate::facts_from_detail(&detail,fallback);ensure!(facts.len()<=100_000,"history.usage_fact_limit");
    let provider=manifest["provider"].as_str().context("history.invalid_provider")?.to_string();
    let native=if manifest["snapshotRoot"].is_null(){None}else{checked_json(&directory.join(format!("{id}.native.json")),1024*1024).ok()};
    let identity=if let Some(native)=native.as_ref().filter(|value|value["format"]=="codeg-native-snapshot-v1"){json!({"provider":provider,"session":native["nativeSessionId"],"originalRoot":manifest["originalRoot"]})}else{json!({"provider":provider,"originalRoot":manifest["originalRoot"],"sourceName":manifest["sourceName"]})};
    let cache=Cached{version:CACHE_VERSION,import_id:id.into(),source_digest:manifest["sourceDigest"].as_str().context("history.source_digest_missing")?.into(),transcript_digest:transcript_digest.clone(),source_identity:crate::command::canonical_json_digest(&identity)?,provider,indexed_at:Utc::now().to_rfc3339(),session_total_fallback:facts.iter().any(|fact|fact.turn_key==aggregate::SESSION_TOTAL_TURN_KEY),facts};
    ensure!(crate::command::canonical_json_digest(&checked_json(&transcript_path,128*1024*1024)?)?==transcript_digest&&checked_json(&manifest_path,1024*1024)?==manifest,"history.usage_source_changed");
    let target=cache_path(directory,id);crate::platform::private_storage::prepare_private_directory(target.parent().context("history.invalid_path")?)?;crate::platform::private_storage::atomic_write_private_json(&target,&cache)?;
    Ok(json!({"id":id,"indexed":true,"unchanged":false,"facts":cache.facts.len(),"sessionTotalFallback":cache.session_total_fallback}))
}

#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request {
    #[serde(default="report_operation")]operation:String,
    #[serde(default)]filter:models::TokenUsageFilter,
    #[serde(default)]project_roots:Option<Vec<String>>,
    #[serde(default)]include_archived:bool,
    #[serde(default)]after:Option<String>,#[serde(default)]force:bool,
}
fn report_operation()->String{"report".into()}
fn sync(directory:&Path,request:&Request)->Result<Value>{
    if let Some(after)=&request.after{identifier(after)?}
    let rows=manifests(directory)?;let selected=rows.iter().filter(|(id,_)|request.after.as_ref().is_none_or(|after|id>after)).take(8).collect::<Vec<_>>();
    let mut results=Vec::new();for(id,_)in &selected{match index(directory,id,request.force){Ok(value)=>results.push(value),Err(error)=>results.push(json!({"id":id,"indexed":false,"error":error.to_string(),"existingFactsPreserved":cache_path(directory,id).exists()}))}}
    let last=selected.last().map(|(id,_)|id.as_str());let more=last.is_some_and(|last|rows.iter().any(|(id,_)|id.as_str()>last));
    Ok(json!({"results":results,"next":if more{last}else{None},"totalArchives":rows.len(),"processedThrough":last,"source":"imported_native_history","originalsUntouched":true}))
}
fn report(directory:&Path,request:Request,projects:&HashMap<String,String>)->Result<Value>{
    let filter=&request.filter;ensure!((-840..=840).contains(&filter.tz_offset_minutes)&&filter.folder_ids.is_none(),"history.usage_invalid_filter");
    ensure!(request.project_roots.as_ref().is_none_or(|rows|rows.len()<=500&&rows.iter().all(|value|value.len()<=4096)),"history.usage_invalid_filter");
    if let(Some(start),Some(end))=(filter.start,filter.end){ensure!(start<end,"history.usage_invalid_range");}
    let mut manifests=manifests(directory)?;let mut archives=Vec::new();let mut missing=Vec::new();let mut loaded_facts=0usize;let mut scan_truncated=false;
    manifests.sort_by(|a,b|b.1["importedAt"].as_str().cmp(&a.1["importedAt"].as_str()).then(a.0.cmp(&b.0)));
    for(id,manifest)in manifests{if !request.include_archived&&manifest["archived"]==true{continue}if loaded_facts>=MAX_FACTS{scan_truncated=true;break}match read_cache(directory,&id,&manifest){Ok(mut cache)=>{let remaining=MAX_FACTS-loaded_facts;if cache.facts.len()>remaining{cache.facts.sort_by(|a,b|b.occurred_at.cmp(&a.occurred_at));cache.facts.truncate(remaining);scan_truncated=true}loaded_facts+=cache.facts.len();archives.push((id,manifest,cache))},Err(_)=>missing.push(id)}}
    // 同一原生来源多次导入时只取最后一次；旧快照继续留库，不重复计费。
    archives.sort_by(|a,b|b.1["importedAt"].as_str().cmp(&a.1["importedAt"].as_str()).then(a.0.cmp(&b.0)));
    let mut identities=std::collections::HashSet::new();let mut superseded=0;
    archives.retain(|(_,_,cache)|{let keep=identities.insert(cache.source_identity.clone());if !keep{superseded+=1}keep});
    let mut folder_ids=BTreeMap::<String,i32>::new();for(_,manifest,_)in &archives{let root=manifest["linkedThreadId"].as_str().and_then(|id|projects.get(id)).cloned().unwrap_or_default();folder_ids.entry(root).or_insert(0);}
    for(index,(_,id))in folder_ids.iter_mut().enumerate(){*id=i32::try_from(index+1)?;}
    let folder_labels=folder_ids.iter().map(|(path,id)|(*id,if path.is_empty(){"未记录工程".into()}else{path.clone()})).collect::<HashMap<_,_>>();
    let mut rows=Vec::new();let mut titles=HashMap::<i32,String>::new();let mut import_ids=BTreeMap::<i32,String>::new();let mut no_usage=0;let mut fallback_sessions=0;let mut truncated=scan_truncated;let mut provenance=Vec::new();
    for(index,(id,manifest,cache))in archives.iter().enumerate(){
        let project=manifest["linkedThreadId"].as_str().and_then(|id|projects.get(id)).cloned().unwrap_or_default();
        if request.project_roots.as_ref().is_some_and(|selected|!selected.contains(&project))||filter.agent_types.as_ref().is_some_and(|selected|!selected.contains(&cache.provider)){continue}
        let conversation=i32::try_from(index+1)?;let folder=*folder_ids.get(&project).context("history.usage_folder_missing")?;
        if cache.facts.is_empty(){no_usage+=1}if cache.session_total_fallback{fallback_sessions+=1}
        titles.insert(conversation,manifest["summary"]["title"].as_str().or_else(||manifest["sourceName"].as_str()).unwrap_or("导入历史").to_string());import_ids.insert(conversation,id.clone());
        provenance.push(json!({"conversationId":conversation,"importId":id,"sourceDigest":cache.source_digest,"transcriptDigest":cache.transcript_digest,"sourceIdentity":cache.source_identity,"provider":cache.provider,"indexedAt":cache.indexed_at,"importedAt":manifest["importedAt"],"indexedFactRowsIncludedInScan":cache.facts.len(),"sessionTotalFallback":cache.session_total_fallback,"archived":manifest["archived"].as_bool().unwrap_or(false)}));
        for fact in &cache.facts{if filter.models.as_ref().is_some_and(|selected|!fact.model.as_ref().is_some_and(|model|selected.contains(model))){continue}if rows.len()>=MAX_FACTS{truncated=true;break}
            ensure!([fact.input_tokens,fact.output_tokens,fact.cache_creation_tokens,fact.cache_read_tokens,fact.duration_ms].iter().all(|value|*value>=0),"history.usage_invalid_cached_counter");
            rows.push(UsageFactRow{conversation_id:conversation,folder_id:folder,agent_type:cache.provider.clone(),model:fact.model.clone(),occurred_at:fact.occurred_at,input_tokens:fact.input_tokens,output_tokens:fact.output_tokens,cache_creation_tokens:fact.cache_creation_tokens,cache_read_tokens:fact.cache_read_tokens,total_tokens:fact.total_tokens(),duration_ms:fact.duration_ms});
        }
    }
    rows.sort_by(|a,b|b.occurred_at.cmp(&a.occurred_at));
    // 在交给原 u64 聚合前检测总和，损坏记录不能产生溢出或看似正确的小数值。
    for column in 0..6{let sum=rows.iter().map(|row|match column{0=>row.input_tokens,1=>row.output_tokens,2=>row.cache_creation_tokens,3=>row.cache_read_tokens,4=>row.total_tokens,_=>row.duration_ms}as u128).sum::<u128>();ensure!(sum<=u64::MAX.into(),"history.usage_counter_overflow");}
    let selected=rows.iter().filter(|row|filter.start.is_none_or(|start|row.occurred_at>=start)&&filter.end.is_none_or(|end|row.occurred_at<end)).cloned().collect::<Vec<_>>();
    let previous=if filter.compare_previous{if let(Some(start),Some(end))=(filter.start,filter.end){rows.iter().filter(|row|row.occurred_at>=start-(end-start)&&row.occurred_at<start).cloned().collect::<Vec<_>>()}else{Vec::new()}}else{Vec::new()};
    let options=aggregate::AggregateOptions{bucket:filter.bucket,tz_offset_minutes:filter.tz_offset_minutes,range_start:filter.start,range_end:filter.end,folder_labels:&folder_labels,truncated,compare_previous:filter.compare_previous&&filter.start.is_some()&&filter.end.is_some()};
    let mut report=aggregate::aggregate_report(&selected,&previous,&options);
    for item in &mut report.top_conversations{item.title=titles.get(&item.conversation_id).cloned();}
    Ok(json!({"schemaVersion":1,"source":"imported_native_history","report":report,"folderKeys":folder_ids.iter().map(|(path,id)|json!({"id":id,"path":path})).collect::<Vec<_>>(),"importIds":import_ids,"missingIndex":missing,"supersededSnapshots":superseded,"sessionsWithoutUsage":no_usage,"sessionTotalFallbacks":fallback_sessions,"nativeCountersAreSourceReported":true,"mergedWithCoreUsage":false,"generatedAt":Utc::now().to_rfc3339(),"projection":{"version":CACHE_VERSION,"sourceRevision":"aff1e6c278453ed8d0b96a5f664edcdca621aa3f","archiveLimit":MAX_ARCHIVES,"factRowLimit":MAX_FACTS,"scanOrder":"latest_imported_archive_first_then_latest_fact_within_archive","globallyNewestFactsGuaranteed":!scan_truncated,"sameSourcePolicy":"latest_imported_snapshot_only","includesArchived":request.include_archived},"provenance":provenance}))
}

impl Core {
    pub(in crate::application) async fn history_usage(&self,local:bool,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value)?;let directory=self.data_dir.join("imported-history");
        if request.operation=="sync"{ensure!(local,"history.usage_local_owner_required");let _guard=workbench::MUTATIONS.lock().await;return tokio::task::spawn_blocking(move||sync(&directory,&request)).await?;}
        ensure!(request.operation=="report","history.usage_invalid_operation");
        let projects={let database=self.database.lock().await;let mut statement=database.connection().prepare("SELECT id,project_path FROM camp WHERE project_path IS NOT NULL")?;statement.query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?.collect::<rusqlite::Result<HashMap<_,_>>>()?};
        tokio::task::spawn_blocking(move||report(&directory,request,&projects)).await?
    }
}
