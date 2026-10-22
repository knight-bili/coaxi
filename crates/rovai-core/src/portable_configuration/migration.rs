//! 精确源行预览与整批数据库事务；导入本身不启动工作或改写历史 Run。
use super::*;
use crate::{command::{ActorRef,CommandEnvelope,CommandExecution,CommandHandlerResult,DomainCommand,DomainCommandGateway,sealed},draft_client::DraftClient};
use rusqlite::{Connection,OptionalExtension,Transaction,params};
use std::{collections::{BTreeMap,BTreeSet},path::Path};

fn text<'a>(value:&'a Value,key:&str)->Result<&'a str>{value[key].as_str().context("configuration.field_required")}
fn provider_basis(connection:&Connection,id:&str)->Result<Value>{
    let row:Option<(i64,bool,String)>=connection.query_row("SELECT h.revision,h.removed,r.digest FROM portable_provider_head h JOIN portable_provider_revision r ON r.id=h.id AND r.revision=h.revision WHERE h.id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    Ok(row.map(|(revision,removed,digest)|json!({"revision":revision,"removed":removed,"digest":digest})).unwrap_or(Value::Null))
}
fn custom_basis(connection:&Connection,id:&str)->Result<Value>{
    let row:Option<(i64,bool,String,String)>=connection.query_row("SELECT h.revision,h.removed,h.registry_id,d.digest FROM custom_runtime_head h JOIN custom_runtime_definition d ON d.id=h.id AND d.revision=h.revision WHERE h.id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    Ok(row.map(|(revision,removed,registry,digest)|json!({"revision":revision,"removed":removed,"registryId":registry,"digest":digest})).unwrap_or(Value::Null))
}
fn member_basis(connection:&Connection,id:&str)->Result<Value>{
    let(version,kind,installation,model,status):(i64,Option<String>,Option<String>,Option<String>,String)=connection.query_row("SELECT version,selected_runtime_adapter_kind,default_runtime_installation_id,default_model_selection_json,profile_status FROM agent_profile WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).context("configuration.member_missing")?;
    let binding:Option<(i64,String,String)>=connection.query_row("SELECT revision,reference_json,environment_json FROM portable_member_connection WHERE agent_id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    let generation=installation.as_deref().map(|id|connection.query_row("SELECT generation FROM adapter_installation WHERE id=?1 AND enabled=1",[id],|row|row.get::<_,i64>(0))).transpose()?;
    Ok(json!({"version":version,"kind":kind,"installation":installation,"installationGeneration":generation,"modelJson":model,"status":status,"binding":binding.map(|(revision,reference,environment)|json!({"revision":revision,"referenceJson":reference,"environmentJson":environment}))}))
}
fn expected_kind(agent:&str)->Result<&'static str>{Ok(match agent{"claude_code"=>"claude-code-cli","codex"=>"codex-cli","gemini"=>"gemini-cli",_=>anyhow::bail!("configuration.agent_type_unsupported")})}
fn public_provider(definition:&Value,has_secret:bool)->Result<Value>{
    let mut value=definition.clone();let mut url=reqwest::Url::parse(text(definition,"apiUrl")?)?;
    if url.query().is_some(){url.set_query(Some("[已隐藏查询参数]"));}
    value["apiUrl"]=json!(url.to_string());value["rawAgentTypesJson"]=json!("原文已加密保留");value["rawModel"]=json!("原文已加密保留");value["credential"]=json!(if has_secret{"已包含"}else{"需要填写"});Ok(value)
}
fn source_name(entry:&archive::Entry)->String{
    entry.value.get("name").or_else(||entry.value.get("title")).or_else(||entry.value.get("key")).and_then(Value::as_str).unwrap_or(&entry.domain).chars().take(512).collect()
}
fn basis(connection:&Connection,root:&Path,targets:&Value,domains:&Value,members:&[Value])->Result<Value>{
    let mut providers=serde_json::Map::new();let mut customs=serde_json::Map::new();let mut file_targets=BTreeSet::new();
    for(entry,id)in targets.as_object().context("configuration.targets_invalid")?{let id=id.as_str().context("configuration.target_invalid")?;
        match domains[entry].as_str(){Some("modelProviders")=>{providers.insert(id.into(),provider_basis(connection,id)?);},Some("customAgents")=>{customs.insert(id.into(),custom_basis(connection,id)?);},Some(domain)=>if let Some(target)=library::file_target(domain){file_targets.insert(target.to_owned());},None=>anyhow::bail!("configuration.domain_required")}
    }
    let mut member_values=serde_json::Map::new();for member in members{let id=text(member,"agentId")?;member_values.insert(id.into(),member_basis(connection,id)?);}
    Ok(json!({"providers":providers,"customAgents":customs,"members":member_values,"files":files::basis(root,&file_targets.into_iter().collect::<Vec<_>>())?}))
}
pub(crate) fn preview(connection:&Connection,root:&Path,client:&str,input:&Value)->Result<Value>{
    validate_operation("configuration.migration.preview",input)?;
    let archive=archive_store::retained(connection,root,client,text(input,"archiveId")?,text(input,"sourceDigest")?)?;
    let entries=archive.entries()?;let mut selections=BTreeMap::new();
    for selected in input["selections"].as_array().context("configuration.selections_invalid")?{
        let id=text(selected,"entryId")?;ensure!(selected["decision"]["entryId"]==id&&selections.insert(id.to_owned(),selected.clone()).is_none(),"configuration.duplicate_selection");
        ensure!(entries.iter().any(|entry|entry.id==id&&entry.domain==selected["domain"]),"configuration.source_entry_changed");
    }
    let mut targets=serde_json::Map::new();let mut domains=serde_json::Map::new();let mut seen_targets=BTreeSet::new();let mut rows=Vec::new();
    for entry in &entries{
        let selected=selections.get(&entry.id);let decision=selected.map(|item|&item["decision"]);let action=decision.and_then(|item|item["action"].as_str()).unwrap_or("skip");
        let mut row=json!({"entryId":entry.id,"domain":entry.domain,"name":source_name(entry),"action":action,"status":"ready","before":{},"after":{},"reason":null});
        if action!="skip"{
            if entry.domain=="modelProviders"{
                let decision=decision.context("configuration.decision_required")?;
                let target=if action=="create"{ensure!(decision["targetId"].is_null()&&decision["expectedRevision"].is_null(),"configuration.create_target_invalid");uuid::Uuid::new_v4().to_string()}else{text(decision,"targetId")?.to_owned()};
                uuid::Uuid::parse_str(&target)?;ensure!(seen_targets.insert(target.clone()),"configuration.duplicate_target");
                let before=provider_basis(connection,&target)?;
                if (action=="create"&&!before.is_null())||(action=="replace"&&(before.is_null()||decision["expectedRevision"]!=before["revision"])){
                    row["status"]=json!("conflict");row["reason"]=json!("目标配置已改变，请重新选择。");
                }
                row["before"]=if before.is_null(){json!({})}else{before};
                match normalize::provider(&target,&entry.value){
                    Ok(normalized)=>{row["after"]=public_provider(&normalized.definition,normalized.credential.is_some())?;if !normalized.warnings.is_empty(){row["status"]=json!("needs_environment");row["reason"]=json!(normalized.warnings.join("；"));}},
                    Err(_)=>{row["status"]=json!("unsupported");row["reason"]=json!("原服务商或模型字段无法完整转换；完整原文已保留。");},
                }
                targets.insert(entry.id.clone(),json!(target));
                domains.insert(entry.id.clone(),json!(entry.domain));
            }else if entry.domain=="customAgents"||library::file_target(&entry.domain).is_some(){
                let decision=decision.context("configuration.decision_required")?;
                let target=if action=="create"{ensure!(decision["targetId"].is_null()&&decision["expectedRevision"].is_null(),"configuration.create_target_invalid");uuid::Uuid::new_v4().to_string()}else{text(decision,"targetId")?.to_owned()};
                uuid::Uuid::parse_str(&target)?;ensure!(seen_targets.insert(format!("{}:{target}",entry.domain)),"configuration.duplicate_target");
                let before=if entry.domain=="customAgents"{custom_basis(connection,&target)?}else{
                    let library=files::read(root,library::file_target(&entry.domain).context("configuration.library_domain_unknown")?)?;
                    library::item(&entry.domain,&library,&target)?.cloned().unwrap_or(Value::Null)
                };
                let revision=if before.is_null(){None}else{Some(before["revision"].as_i64().unwrap_or(1))};
                if(action=="create"&&revision.is_some())||(action=="replace"&&revision!=decision["expectedRevision"].as_i64()){
                    row["status"]=json!("conflict");row["reason"]=json!("目标库条目已经改变，请重新选择。");
                }
                row["before"]=if entry.domain=="customAgents"{json!({"id":target,"revision":revision})}else if before.is_null(){json!({})}else{before};
                let converted=if entry.domain=="customAgents"{
                    normalize::custom_agent(&target,&entry.value).map(|definition|json!({"id":target,"name":definition["name"],"registryId":definition["registryId"],"distributionKind":definition["distributionKind"],"version":definition["version"],"installation":"等待单独安装","automaticExecutionAllowed":false}))
                }else{library::candidate(connection,client,&entry.domain,&target,&entry.value,false)};
                match converted{Ok(value)=>row["after"]=value,Err(_)=>{row["status"]=json!("unsupported");row["reason"]=json!("此条原配置无法完整转换；原文保留，请补齐字段后重新预览。");}}
                targets.insert(entry.id.clone(),json!(target));domains.insert(entry.id.clone(),json!(entry.domain));
            }else{
                row["status"]=json!("unsupported");row["reason"]=json!("此域的应用接线尚在编写；完整原文已保留，不能标作已应用。");
            }
        }
        rows.push(row);
    }
    let members=input["members"].as_array().context("configuration.members_invalid")?;let mut seen_members=BTreeSet::new();
    for member in members{
        let id=text(member,"agentId")?;ensure!(seen_members.insert(id),"configuration.duplicate_member");
        let source=text(member,"providerEntryId")?;let entry=entries.iter().find(|entry|entry.id==source&&entry.domain=="modelProviders").context("configuration.provider_entry_required")?;
        ensure!(targets.contains_key(source),"configuration.member_provider_not_selected");
        let before=member_basis(connection,id)?;
        let valid=before["version"]==member["expectedVersion"]&&before["status"]=="present"&&before["installation"]==member["installationId"]&&before["installationGeneration"]==member["expectedInstallationGeneration"]&&before["kind"].as_str()==expected_kind(text(&entry.value,"agentType")?).ok();
        let _:crate::agent_profile::ModelSelection=serde_json::from_value(member["modelSelection"].clone()).map_err(|_|anyhow::anyhow!("configuration.model_selection_invalid"))?;
        if let Some(row)=rows.iter_mut().find(|row|row["entryId"]==source){
            if !valid{row["status"]=json!("conflict");row["reason"]=json!("所选队员、程序或安装代次已经改变。");}
            if let Some(environment_id)=member["environmentEntryId"].as_str(){
                let valid_environment=(||->Result<_>{
                    let environment=entries.iter().find(|entry|entry.id==environment_id&&entry.domain=="agentSettings").context("configuration.environment_entry_required")?;
                    let setting:archive::CodegAgentSetting=serde_json::from_value(environment.value.clone())?;
                    ensure!(environment.value["agentType"]==entry.value["agentType"],"configuration.environment_agent_mismatch");
                    if let Some(provider)=setting.provider{ensure!(provider.agent_type==text(&entry.value,"agentType")?&&provider.name==text(&entry.value,"name")?,"configuration.provider_source_ambiguous");}
                    if let Some(raw)=setting.env_json{crate::member_environment::parse(&raw,cfg!(windows))?;}Ok(())
                })();
                if valid_environment.is_err(){row["status"]=json!("conflict");row["reason"]=json!("环境设置与所选服务商不匹配，请明确选择原文件中的对应条目。");}
            }
        }
    }
    let targets=Value::Object(targets);let domains=Value::Object(domains);let local=basis(connection,root,&targets,&domains,members)?;
    let id=uuid::Uuid::new_v4().to_string();let now=chrono::Utc::now();let expires=now+chrono::Duration::minutes(10);
    let can_apply=rows.iter().any(|row|row["action"]!="skip")&&rows.iter().all(|row|row["action"]=="skip"||row["status"]=="ready");
    let mut preview=json!({"schemaVersion":1,"previewId":id,"previewDigest":"","expiresAt":expires.to_rfc3339(),"archiveId":input["archiveId"],"sourceDigest":input["sourceDigest"],"localDigest":digest(&local)?,"selections":input["selections"],"members":members,"rows":rows,"canApply":can_apply});
    preview["previewDigest"]=json!(digest(&preview)?);validate("MigrationPreviewV1",&preview)?;
    let private=json!({"basis":local,"targets":targets,"domains":domains});ensure!(serde_json::to_vec(&private)?.len()<=MAX_REQUEST,"configuration.preview_too_large");
    connection.execute("INSERT INTO configuration_import_preview(id,client_id,archive_id,kind,digest,value_json,local_basis_json,expires_at_ms,created_at) VALUES(?1,?2,?3,'apply',?4,?5,?6,?7,?8)",params![id,client,text(input,"archiveId")?,text(&preview,"previewDigest")?,serde_json::to_string(&preview)?,serde_json::to_string(&private)?,expires.timestamp_millis(),now.to_rfc3339()])?;Ok(preview)
}

#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct Confirm{#[serde(skip_deserializing,default)]pub client:DraftClient,pub schema_version:u32,pub preview_id:String,pub preview_digest:String,#[serde(skip_deserializing,default)]pub core_instance:String}
impl sealed::Sealed for Confirm{}
impl DomainCommand for Confirm{const TYPE:&'static str="configuration.migration.confirm";const EXACT_REPLAY:bool=true;fn draft_client(&self)->Option<&DraftClient>{Some(&self.client)}}
pub(crate) fn receipt(connection:&Connection,client:&str,id:&str)->Result<Value>{
    uuid::Uuid::parse_str(id)?;let raw:Option<String>=connection.query_row("SELECT value_json FROM configuration_import_receipt WHERE command_id=?1 AND client_id=?2",params![id,client],|row|row.get(0)).optional()?;
    let Some(raw)=raw else{anyhow::bail!("configuration.receipt_not_found")};let value:Value=serde_json::from_str(&raw)?;validate("MigrationReceiptV1",&value)?;Ok(value)
}
fn remember(tx:&Transaction<'_>,command:&str,domain:&str,id:&str,before:&Value,after:&Value)->Result<()>{
    tx.execute("INSERT INTO configuration_import_before(command_id,domain,target_id,before_json,after_revision,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![command,domain,id,serde_json::to_string(before)?,digest(after)?,chrono::Utc::now().to_rfc3339()])?;Ok(())
}
fn apply_on(tx:&Transaction<'_>,root:&Path,client:&str,command:&str,preview:&Value,private:&Value)->Result<Value>{
    let members=preview["members"].as_array().context("configuration.members_invalid")?;
    ensure!(basis(tx,root,&private["targets"],&private["domains"],members)?==private["basis"],"configuration.local_configuration_changed");
    let archive=archive_store::retained(tx,root,client,text(preview,"archiveId")?,text(preview,"sourceDigest")?)?;
    let entries=archive.entries()?;let mut references=BTreeMap::new();let mut counts=BTreeMap::<String,u64>::new();let mut file_updates=BTreeMap::<String,(String,Value)>::new();
    for selected in preview["selections"].as_array().context("configuration.selections_invalid")?{
        if selected["decision"]["action"]=="skip"{continue}
        let source=text(selected,"entryId")?;let entry=entries.iter().find(|entry|entry.id==source&&entry.domain==selected["domain"]).context("configuration.source_entry_changed")?;
        let id=private["targets"][source].as_str().context("configuration.target_required")?;
        *counts.entry(entry.domain.clone()).or_default()+=1;
        if entry.domain=="customAgents"{
            let before=custom_basis(tx,id)?;let definition=normalize::custom_agent(id,&entry.value)?;
            let saved=crate::custom_runtime::storage::upsert_on(tx,before["revision"].as_i64().unwrap_or(0),&definition)?;
            ensure!(saved.status==crate::command::CommandResultStatus::Applied,"configuration.custom_agent_not_saved");
            remember(tx,command,"customAgents",id,&before,&custom_basis(tx,id)?)?;continue
        }
        if let Some(target)=library::file_target(&entry.domain){
            if !file_updates.contains_key(target){let original=&private["basis"]["files"][target]["value"];file_updates.insert(target.into(),(entry.domain.clone(),library::initial(&entry.domain,original)?));}
            let(_,next)=file_updates.get_mut(target).context("configuration.library_missing")?;
            let candidate=library::candidate(tx,client,&entry.domain,id,&entry.value,true)?;
            library::merge(&entry.domain,next,candidate,text(&selected["decision"],"action")?,selected["decision"]["expectedRevision"].as_i64())?;continue
        }
        ensure!(entry.domain=="modelProviders","configuration.domain_application_unavailable");let before=provider_basis(tx,id)?;
        let mut normalized=normalize::provider(id,&entry.value)?;ensure!(normalized.warnings.is_empty(),"configuration.source_conversion_incomplete");
        if let Some(plain)=normalized.credential{normalized.definition["credential"]=serde_json::to_value(secrets::import_on(tx,client,"provider_key",&plain)?)?;}
        let saved=providers::upsert_on(tx,client,before["revision"].as_i64().unwrap_or(0),&normalized.definition)?;
        references.insert(source.to_owned(),providers::reference(&saved)?);remember(tx,command,"modelProviders",id,&before,&provider_basis(tx,id)?)?;
    }
    let mut changed=Vec::new();
    for member in members{
        let id=text(member,"agentId")?;let before=member_basis(tx,id)?;let reference=references.get(text(member,"providerEntryId")?).context("configuration.member_provider_not_selected")?;
        members::set_on(tx,id,member["expectedVersion"].as_i64().context("configuration.member_version_required")?,Some(reference))?;
        if let Some(source)=member["environmentEntryId"].as_str(){
            let entry=entries.iter().find(|entry|entry.id==source&&entry.domain=="agentSettings").context("configuration.environment_entry_required")?;
            let candidate=library::candidate(tx,client,"agentSettings",&uuid::Uuid::new_v4().to_string(),&entry.value,true)?;
            tx.execute("UPDATE portable_member_connection SET environment_json=?2 WHERE agent_id=?1",params![id,serde_json::to_string(&candidate["environment"]) ?])?;
        }
        tx.execute("UPDATE agent_profile SET default_model_selection_json=?2 WHERE id=?1",params![id,serde_json::to_string(&member["modelSelection"]) ?])?;
        remember(tx,command,"memberConnection",id,&before,&member_basis(tx,id)?)?;changed.push(id.to_owned());
    }
    for(target,(domain,after))in file_updates{
        let before=&private["basis"]["files"][&target];let mut after_basis=before.clone();after_basis["value"]=after.clone();
        files::enqueue(tx,command,&domain,&target,&before["value"],&after)?;remember(tx,command,&domain,&target,before,&after_basis)?;
    }
    Ok(json!({"counts":counts,"changedMembers":changed,"files":files::statuses(tx,command)?}))
}
pub(crate) fn confirm(database:&mut crate::db::Database,root:&Path,envelope:&CommandEnvelope<Confirm>)->Result<CommandExecution>{
    DomainCommandGateway.execute(database,envelope,|tx|{
        ensure!(matches!(&envelope.actor,ActorRef::User{..})&&envelope.camp_id.is_none(),"configuration.user_required");let input=&envelope.payload;
        validate_operation(Confirm::TYPE,&json!({"schemaVersion":input.schema_version,"commandId":envelope.command_id,"previewId":input.preview_id,"previewDigest":input.preview_digest}))?;
        let(raw,private,expires,stored):(String,String,i64,String)=tx.query_row("SELECT value_json,local_basis_json,expires_at_ms,digest FROM configuration_import_preview WHERE id=?1 AND client_id=?2 AND kind='apply'",params![input.preview_id,input.client.id()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).context("configuration.preview_not_owned")?;
        ensure!(stored==input.preview_digest,"configuration.preview_changed");let preview:Value=serde_json::from_str(&raw)?;let private:Value=serde_json::from_str(&private)?;
        let mut result=json!({"schemaVersion":1,"commandId":envelope.command_id,"previewId":input.preview_id,"previewDigest":stored,"state":"prepared","counts":{},"changedMembers":[],"errorCode":null,"historyPreserved":true,"files":[]});
        let now=chrono::Utc::now().to_rfc3339();
        tx.execute("INSERT INTO configuration_import_receipt(command_id,client_id,core_instance_id,preview_id,request_digest,state,value_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'prepared',?6,?7,?7)",params![envelope.command_id,input.client.id(),input.core_instance,input.preview_id,digest(&serde_json::to_value(input)?)?,serde_json::to_string(&result)?,now])?;
        tx.execute_batch("SAVEPOINT portable_configuration_apply")?;
        let applied=(||->Result<_>{ensure!(chrono::Utc::now().timestamp_millis()<expires&&preview["canApply"]==true,"configuration.preview_not_applicable");apply_on(tx,root,input.client.id(),&envelope.command_id,&preview,&private)})();
        match applied{
            Ok(applied)=>{tx.execute_batch("RELEASE portable_configuration_apply")?;result["state"]=json!("applied");result["counts"]=applied["counts"].clone();result["changedMembers"]=applied["changedMembers"].clone();result["files"]=applied["files"].clone();},
            Err(_)=>{tx.execute_batch("ROLLBACK TO portable_configuration_apply; RELEASE portable_configuration_apply")?;result["state"]=json!("rejected");result["errorCode"]=json!("configuration.batch_not_applied");},
        }
        validate("MigrationReceiptV1",&result)?;
        tx.execute("UPDATE configuration_import_receipt SET state=?2,value_json=?3,updated_at=?4 WHERE command_id=?1",params![envelope.command_id,text(&result,"state")?,serde_json::to_string(&result)?,chrono::Utc::now().to_rfc3339()])?;
        Ok(if result["state"]=="applied"{CommandHandlerResult::applied("configuration.migration_applied",result,None)}else{CommandHandlerResult::rejected("configuration.migration_rejected",result)})
    })
}
