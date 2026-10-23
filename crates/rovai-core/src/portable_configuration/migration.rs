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
    let mut providers=serde_json::Map::new();let mut customs=serde_json::Map::new();let mut policy_values=serde_json::Map::new();let mut file_targets=BTreeSet::new();
    for(entry,id)in targets.as_object().context("configuration.targets_invalid")?{let id=id.as_str().context("configuration.target_invalid")?;
        match domains[entry].as_str(){Some("modelProviders")=>{providers.insert(id.into(),provider_basis(connection,id)?);},Some("customAgents")=>{customs.insert(id.into(),custom_basis(connection,id)?);},Some("preferences")=>{if policies::owns(id){policy_values.insert(id.into(),policies::basis(connection,id)?);}else{file_targets.insert(id.to_owned());}},Some(domain)=>if let Some(target)=library::file_target(domain){file_targets.insert(target.to_owned());},None=>anyhow::bail!("configuration.domain_required")}
    }
    let mut member_values=serde_json::Map::new();for member in members{let id=text(member,"agentId")?;member_values.insert(id.into(),member_basis(connection,id)?);}
    Ok(json!({"providers":providers,"customAgents":customs,"members":member_values,"policies":policy_values,"files":files::basis(root,&file_targets.into_iter().collect::<Vec<_>>())?}))
}
pub(crate) fn preview(connection:&Connection,root:&Path,client:&str,input:&Value)->Result<Value>{
    validate_operation("configuration.migration.preview",input)?;
    let archive=archive_store::retained(connection,root,client,text(input,"archiveId")?,text(input,"sourceDigest")?)?;
    let entries=archive.entries()?;let mut selections=BTreeMap::new();
    for selected in input["selections"].as_array().context("configuration.selections_invalid")?{
        let id=text(selected,"entryId")?;ensure!(selected["decision"]["entryId"]==id&&selections.insert(id.to_owned(),selected.clone()).is_none(),"configuration.duplicate_selection");
        ensure!(entries.iter().any(|entry|entry.id==id&&entry.domain==selected["domain"]),"configuration.source_entry_changed");
    }
    let mut targets=serde_json::Map::new();let mut domains=serde_json::Map::new();let mut seen_targets=BTreeSet::new();let mut preference_fields=BTreeSet::new();let mut rows=Vec::new();
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
            }else if entry.domain=="preferences"{
                match preferences::selected_patch(&entry.value,decision.context("configuration.decision_required")?){
                    Ok((target,patch))=>{
                        let policy=policies::owns(&target);let original=if policy{policies::basis(connection,&target)?}else{json!({"version":1,"value":files::read(root,&target)?})};
                        for field in patch.as_object().context("configuration.preference_patch_invalid")?.keys(){
                            if !preference_fields.insert(format!("{target}.{field}")){row["status"]=json!("conflict");row["reason"]=json!("多个来源条目指向同一个设置字段，请只保留一个选择。");}
                        }
                        let before=&original["value"];row["before"]=preferences::before_fields(before,&patch);row["after"]=patch;
                        let key=text(&entry.value,"key")?;let existing=row["before"].as_object().is_some_and(|map|map.values().any(|value|!value.is_null()));
                        let decision=decision.context("configuration.decision_required")?;
                        if !channel_preferences::owns(&target)&&((action=="create"&&(existing||!decision["targetId"].is_null()||!decision["expectedRevision"].is_null()))||(action=="replace"&&(!existing||decision["targetId"]!=key||decision["expectedRevision"]!=original["version"]))){
                            row["status"]=json!("conflict");row["reason"]=json!("请明确选择替换对应偏好，已有值不会按名称自动覆盖。");
                        }
                        if channel_preferences::owns(&target)&&before.is_null(){row["status"]=json!("conflict");row["reason"]=json!("所选本机渠道已不存在，请重新选择准确目标。");}
                        targets.insert(entry.id.clone(),json!(target));domains.insert(entry.id.clone(),json!(entry.domain));
                    },
                    Err(_)=>{row["status"]=json!("unsupported");row["reason"]=json!("原值已保留；此设置需要对应策略或当前主机支持，尚未应用。");},
                }
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
pub(crate) struct Confirm{#[serde(skip_deserializing,default)]pub client:DraftClient,pub schema_version:u32,pub preview_id:String,pub preview_digest:String,#[serde(skip)]pub core_instance:String}
impl sealed::Sealed for Confirm{}
impl DomainCommand for Confirm{const TYPE:&'static str="configuration.migration.confirm";const EXACT_REPLAY:bool=true;fn draft_client(&self)->Option<&DraftClient>{Some(&self.client)}}
pub(crate) fn receipt(connection:&Connection,client:&str,id:&str)->Result<Value>{
    uuid::Uuid::parse_str(id)?;let raw:Option<String>=connection.query_row("SELECT value_json FROM configuration_import_receipt WHERE command_id=?1 AND client_id=?2",params![id,client],|row|row.get(0)).optional()?;
    let Some(raw)=raw else{anyhow::bail!("configuration.receipt_not_found")};let mut value:Value=serde_json::from_str(&raw)?;value["files"]=json!(files::statuses(connection,id)?);validate("MigrationReceiptV1",&value)?;Ok(value)
}

fn target_basis(connection:&Connection,root:&Path,domain:&str,id:&str)->Result<Value>{
    if domain=="preferences"&&policies::owns(id){return policies::basis(connection,id)}
    match domain{
        "modelProviders"=>provider_basis(connection,id),"customAgents"=>custom_basis(connection,id),"memberConnection"=>member_basis(connection,id),
        "quickMessages"|"taskTemplates"|"agentSettings"|"preferences"=>Ok(files::basis(root,&[id.to_owned()])?[id].clone()),
        _=>anyhow::bail!("configuration.undo_domain_unknown"),
    }
}
pub(crate) fn undo_preview(connection:&Connection,root:&Path,client:&str,input:&Value)->Result<Value>{
    validate_operation("configuration.migration.undoPreview",input)?;let command=text(input,"originalCommandId")?;
    let original=receipt(connection,client,command)?;ensure!(original["state"]=="applied","configuration.original_not_applied");
    let(archive,kind):(String,String)=connection.query_row("SELECT p.archive_id,p.kind FROM configuration_import_receipt r JOIN configuration_import_preview p ON p.id=r.preview_id WHERE r.command_id=?1 AND r.client_id=?2",params![command,client],|row|Ok((row.get(0)?,row.get(1)?)))?;
    ensure!(kind=="apply","configuration.undo_requires_original_import");
    let mut statement=connection.prepare("SELECT domain,target_id,before_json,after_revision FROM configuration_import_before WHERE command_id=?1 ORDER BY domain,target_id")?;
    let records=statement.query_map([command],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;drop(statement);
    ensure!(!records.is_empty()&&records.len()<=8192,"configuration.undo_records_unavailable");
    let mut items=Vec::new();let mut rows=Vec::new();let mut local=Vec::new();let files_complete=original["files"].as_array().is_some_and(|files|files.iter().all(|file|file["state"]=="applied"));
    for(domain,id,before,expected)in records{
        let before:Value=serde_json::from_str(&before)?;let current=target_basis(connection,root,&domain,&id)?;
        let valid=files_complete&&digest(&current)?==expected;
        rows.push(json!({"entryId":digest(&json!({"original":command,"domain":domain,"targetId":id}))?,"domain":domain,"name":id,"action":"replace","status":if valid{"ready"}else{"conflict"},"before":if current.is_object(){current.clone()}else{json!({"value":current})},"after":if before.is_object(){before.clone()}else{json!({"value":before})},"reason":if valid{Value::Null}else{json!("目标已发生后续修改或文件尚未完成应用；本次撤销不会覆盖它。")}}));
        local.push(current);items.push(json!({"domain":domain,"targetId":id,"before":before,"afterRevision":expected}));
    }
    let now=chrono::Utc::now();let expires=now+chrono::Duration::minutes(10);let id=uuid::Uuid::new_v4().to_string();
    let source:String=connection.query_row("SELECT source_digest FROM configuration_import_archive WHERE id=?1 AND client_id=?2",params![archive,client],|row|row.get(0))?;
    let mut preview=json!({"schemaVersion":1,"previewId":id,"previewDigest":"","expiresAt":expires.to_rfc3339(),"archiveId":archive,"sourceDigest":source,"localDigest":digest(&json!(local))?,"selections":[],"members":[],"canApply":rows.iter().all(|row|row["status"]=="ready"),"rows":rows});
    preview["previewDigest"]=json!(digest(&preview)?);validate("MigrationPreviewV1",&preview)?;
    let private=json!({"items":items});ensure!(serde_json::to_vec(&private)?.len()<=MAX_REQUEST,"configuration.preview_too_large");
    connection.execute("INSERT INTO configuration_import_preview(id,client_id,archive_id,kind,original_command_id,digest,value_json,local_basis_json,expires_at_ms,created_at) VALUES(?1,?2,?3,'undo',?4,?5,?6,?7,?8,?9)",params![id,client,archive,command,text(&preview,"previewDigest")?,serde_json::to_string(&preview)?,serde_json::to_string(&private)?,expires.timestamp_millis(),now.to_rfc3339()])?;Ok(preview)
}
fn undo_on(tx:&Transaction<'_>,root:&Path,command:&str,private:&Value)->Result<Value>{
    let items=private["items"].as_array().context("configuration.undo_records_unavailable")?;
    for item in items{let current=target_basis(tx,root,text(item,"domain")?,text(item,"targetId")?)?;ensure!(digest(&current)?==text(item,"afterRevision")?,"configuration.undo_conflict");}
    let mut counts=BTreeMap::<String,u64>::new();let mut changed=Vec::new();
    for item in items{
        let domain=text(item,"domain")?;let id=text(item,"targetId")?;let before=&item["before"];
        match domain{
            "modelProviders"=>{if before.is_null(){tx.execute("UPDATE portable_provider_head SET removed=1 WHERE id=?1",[id])?;}else{tx.execute("UPDATE portable_provider_head SET revision=?2,removed=?3 WHERE id=?1",params![id,before["revision"].as_i64(),before["removed"].as_bool()])?;}},
            "customAgents"=>{if before.is_null(){tx.execute("UPDATE custom_runtime_head SET removed=1 WHERE id=?1",[id])?;}else{tx.execute("UPDATE custom_runtime_head SET revision=?2,removed=?3,registry_id=?4 WHERE id=?1",params![id,before["revision"].as_i64(),before["removed"].as_bool(),before["registryId"].as_str()])?;}},
            "memberConnection"=>{
                let current=member_basis(tx,id)?;
                if before["binding"].is_null(){tx.execute("DELETE FROM portable_member_connection WHERE agent_id=?1",[id])?;}else{
                    let revision=current["binding"]["revision"].as_i64().context("configuration.undo_member_binding_missing")?.checked_add(1).context("configuration.revision_exhausted")?;
                    tx.execute("UPDATE portable_member_connection SET revision=?2,reference_json=?3,environment_json=?4,updated_at=?5 WHERE agent_id=?1",params![id,revision,before["binding"]["referenceJson"].as_str(),before["binding"]["environmentJson"].as_str(),chrono::Utc::now().to_rfc3339()])?;
                }
                tx.execute("UPDATE agent_profile SET version=version+1,default_model_selection_json=?2,updated_at=?3 WHERE id=?1",params![id,before["modelJson"].as_str(),chrono::Utc::now().to_rfc3339()])?;changed.push(id.to_owned());
            },
            "quickMessages"|"taskTemplates"|"agentSettings"|"preferences"=>{
                if domain=="preferences"&&policies::owns(id){let current=policies::basis(tx,id)?;policies::apply(tx,id,&current,&before["value"])?;}
                else{let current=files::read(root,id)?;files::enqueue(tx,root,command,domain,id,&current,&before["value"])?;}
            },
            _=>anyhow::bail!("configuration.undo_domain_unknown"),
        }
        *counts.entry(domain.into()).or_default()+=1;
    }
    Ok(json!({"counts":counts,"changedMembers":changed,"files":files::statuses(tx,command)?}))
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(transparent)]
pub(crate) struct UndoConfirm(pub Confirm);
impl sealed::Sealed for UndoConfirm{}
impl DomainCommand for UndoConfirm{const TYPE:&'static str="configuration.migration.undoConfirm";const EXACT_REPLAY:bool=true;fn draft_client(&self)->Option<&DraftClient>{Some(&self.0.client)}}
pub(crate) fn undo_confirm(database:&mut crate::db::Database,root:&Path,envelope:&CommandEnvelope<UndoConfirm>)->Result<CommandExecution>{
    DomainCommandGateway.execute(database,envelope,|tx|{
        ensure!(matches!(&envelope.actor,ActorRef::User{..})&&envelope.camp_id.is_none(),"configuration.user_required");let input=&envelope.payload.0;
        validate_operation(UndoConfirm::TYPE,&json!({"schemaVersion":input.schema_version,"commandId":envelope.command_id,"previewId":input.preview_id,"previewDigest":input.preview_digest}))?;
        let(raw,private,expires,stored,original):(String,String,i64,String,String)=tx.query_row("SELECT value_json,local_basis_json,expires_at_ms,digest,original_command_id FROM configuration_import_preview WHERE id=?1 AND client_id=?2 AND kind='undo'",params![input.preview_id,input.client.id()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).context("configuration.preview_not_owned")?;
        ensure!(stored==input.preview_digest,"configuration.preview_changed");let preview:Value=serde_json::from_str(&raw)?;let private:Value=serde_json::from_str(&private)?;
        let mut result=json!({"schemaVersion":1,"commandId":envelope.command_id,"previewId":input.preview_id,"previewDigest":stored,"state":"prepared","counts":{},"changedMembers":[],"errorCode":null,"historyPreserved":true,"files":[]});
        let now=chrono::Utc::now().to_rfc3339();tx.execute("INSERT INTO configuration_import_receipt(command_id,client_id,core_instance_id,preview_id,request_digest,state,value_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'prepared',?6,?7,?7)",params![envelope.command_id,input.client.id(),input.core_instance,input.preview_id,digest(&serde_json::to_value(input)?)?,serde_json::to_string(&result)?,now])?;
        tx.execute_batch("SAVEPOINT portable_configuration_undo")?;
        let applied=(||->Result<_>{ensure!(chrono::Utc::now().timestamp_millis()<expires&&preview["canApply"]==true,"configuration.preview_not_applicable");let current=receipt(tx,input.client.id(),&original)?;ensure!(current["state"]=="applied"&&current["files"].as_array().is_some_and(|files|files.iter().all(|file|file["state"]=="applied")),"configuration.original_files_unsettled");undo_on(tx,root,&envelope.command_id,&private)})();
        match applied{
            Ok(applied)=>{tx.execute_batch("RELEASE portable_configuration_undo")?;result["state"]=json!("applied");result["counts"]=applied["counts"].clone();result["changedMembers"]=applied["changedMembers"].clone();result["files"]=applied["files"].clone();},
            Err(_)=>{tx.execute_batch("ROLLBACK TO portable_configuration_undo; RELEASE portable_configuration_undo")?;result["state"]=json!("rejected");result["errorCode"]=json!("configuration.undo_conflict");},
        }
        validate("MigrationReceiptV1",&result)?;tx.execute("UPDATE configuration_import_receipt SET state=?2,value_json=?3,updated_at=?4 WHERE command_id=?1",params![envelope.command_id,text(&result,"state")?,serde_json::to_string(&result)?,chrono::Utc::now().to_rfc3339()])?;
        Ok(if result["state"]=="applied"{CommandHandlerResult::applied("configuration.undo_applied",result,None)}else{CommandHandlerResult::rejected("configuration.undo_rejected",result)})
    })
}
fn remember(tx:&Transaction<'_>,command:&str,domain:&str,id:&str,before:&Value,after:&Value)->Result<()>{
    tx.execute("INSERT INTO configuration_import_before(command_id,domain,target_id,before_json,after_revision,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![command,domain,id,serde_json::to_string(before)?,digest(after)?,chrono::Utc::now().to_rfc3339()])?;Ok(())
}
fn apply_on(tx:&Transaction<'_>,root:&Path,client:&str,command:&str,preview:&Value,private:&Value)->Result<Value>{
    let members=preview["members"].as_array().context("configuration.members_invalid")?;
    ensure!(basis(tx,root,&private["targets"],&private["domains"],members)?==private["basis"],"configuration.local_configuration_changed");
    let archive=archive_store::retained(tx,root,client,text(preview,"archiveId")?,text(preview,"sourceDigest")?)?;
    let entries=archive.entries()?;let mut references=BTreeMap::new();let mut counts=BTreeMap::<String,u64>::new();let mut file_updates=BTreeMap::<String,(String,Value)>::new();let mut policy_updates=BTreeMap::<String,Value>::new();
    let selections=preview["selections"].as_array().context("configuration.selections_invalid")?;
    for entry in &entries{
        let Some(selected)=selections.iter().find(|selected|selected["entryId"]==entry.id)else{continue};
        if selected["decision"]["action"]=="skip"{continue}
        let source=text(selected,"entryId")?;ensure!(entry.domain==selected["domain"],"configuration.source_entry_changed");
        let id=private["targets"][source].as_str().context("configuration.target_required")?;
        *counts.entry(entry.domain.clone()).or_default()+=1;
        if entry.domain=="preferences"{
            if let Some((target,patch))=policies::patch(&entry.value)?{
                ensure!(id==target,"configuration.preference_target_changed");
                let original=policy_updates.get(target).cloned().unwrap_or_else(||private["basis"]["policies"][target]["value"].clone());
                policy_updates.insert(target.into(),policies::merge(target,&original,&patch)?);continue
            }
            let(target,patch)=preferences::selected_patch(&entry.value,&selected["decision"])?;ensure!(id==target.as_str(),"configuration.preference_target_changed");
            let original=file_updates.get(&target).map(|(_,value)|value.clone()).unwrap_or_else(||private["basis"]["files"][target.as_str()]["value"].clone());
            let next=preferences::merge_at(root,&target,&original,&patch)?;file_updates.insert(target,("preferences".into(),next));continue
        }
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
        files::enqueue(tx,root,command,&domain,&target,&before["value"],&after)?;remember(tx,command,&domain,&target,before,&after_basis)?;
    }
    for(target,after)in policy_updates{
        let before=&private["basis"]["policies"][&target];let saved=policies::apply(tx,&target,before,&after)?;
        remember(tx,command,"preferences",&target,before,&saved)?;
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
