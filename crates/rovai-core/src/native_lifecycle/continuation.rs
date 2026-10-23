//! 原生工作副本沿用 Core 的 Thread 和私聊权威；导入快照只读。
use super::*;
use crate::{agent_profile::{FrozenAgentRuntimeConfig,ResolvedRuntimeBinding},camp_id::ThreadId};
use rusqlite::{Connection,OptionalExtension,Transaction,params};
use std::{fs,io::Read,path::{Path,PathBuf},str::FromStr};

pub(crate) struct ImportedSource {
    pub reference:NativeSourceReferenceV1,
    pub location:source::Location,
    pub turns:Vec<crate::runtime_subagent_session::content::model::MessageTurn>,
    pub snapshot_digest:String,
}
fn read_private(path:&Path,limit:u64)->Result<Vec<u8>>{
    source::no_link(path)?;
    let mut file=crate::platform::private_storage::open_private_read_file(path)?;
    ensure!(file.metadata()?.is_file()&&file.metadata()?.len()<=limit,"native_lifecycle.import_size_limit");
    let mut bytes=Vec::new();file.by_ref().take(limit+1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64<=limit,"native_lifecycle.import_size_limit");Ok(bytes)
}
pub(crate) fn imported(connection:&Connection,data:&Path,owner:&str,id:&str,revision:&str)->Result<ImportedSource>{
    ThreadId::parse(owner)?;
    ensure!(id.len()==64&&id.bytes().all(|byte|byte.is_ascii_hexdigit()),"native_lifecycle.import_id_invalid");
    let directory=data.join("imported-history");source::no_link(&directory)?;
    let manifest:Value=serde_json::from_slice(&read_private(&directory.join(format!("{id}.manifest.json")),2*1024*1024)?)?;
    ensure!(manifest["id"]==id&&manifest["linkedThreadId"]==owner&&manifest["archived"]!=true,"native_lifecycle.import_unavailable");
    ensure!(crate::command::canonical_json_digest(&manifest)?==revision,"native_lifecycle.import_changed");
    let reference=source::latest(connection,owner,id,revision)?.context("native_lifecycle.source_reselection_required")?;
    ensure!(manifest["provider"]==reference.provider,"native_lifecycle.provider_changed");
    let location=source::resolve(connection,owner,&reference)?;
    let native=read_private(&directory.join(format!("{id}.native.json")),128*1024*1024)?;
    use sha2::{Digest,Sha256};
    ensure!(manifest["sourceDigest"].as_str()==Some(format!("{:x}",Sha256::digest(&native)).as_str()),"native_lifecycle.import_snapshot_changed");
    if location.relative.is_some(){ensure!(source::native_id(&reference.provider,&native)?==reference.native_session_id,"native_lifecycle.native_id_changed");}
    let bytes=read_private(&directory.join(format!("{id}.transcript.json")),128*1024*1024)?;
    let transcript:Value=serde_json::from_slice(&bytes)?;
    let turns=serde_json::from_value(transcript["turns"].clone())?;
    Ok(ImportedSource{reference,location,turns,snapshot_digest:format!("sha256:{:x}",Sha256::digest(&bytes))})
}
/// 工程必须取自 Core 中已存在的来源 Thread，原生日志的 cwd 不授予路径权限。
pub(crate) fn workspace(connection:&Connection,owner:&str)->Result<(PathBuf,String)>{
    let(kind,path):(String,String)=connection.query_row("SELECT project_binding_kind,project_path FROM camp WHERE id=?1 AND activation_state='active'",[owner],|row|Ok((row.get(0)?,row.get(1)?))).context("native_lifecycle.source_thread_unavailable")?;
    ensure!(kind=="directory","native_lifecycle.directory_project_required");
    let path=PathBuf::from(path);ensure!(path.is_absolute(),"native_lifecycle.project_path_invalid");
    source::no_link(&path)?;let canonical=fs::canonicalize(&path)?;
    ensure!(canonical.is_dir(),"native_lifecycle.project_unavailable");
    let digest=digest(&json!({"projectBindingKind":kind,"projectPath":canonical}))?;Ok((canonical,digest))
}
pub(crate) fn member(connection:&Connection,id:&str,expected:i64)->Result<FrozenAgentRuntimeConfig>{
    let(status,version,adapter,installation,model,permissions):(String,i64,Option<String>,Option<String>,Option<String>,Option<String>)=connection.query_row("SELECT profile_status,version,selected_runtime_adapter_kind,default_runtime_installation_id,default_model_selection_json,default_permission_config_json FROM agent_profile WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))).context("native_lifecycle.member_missing")?;
    ensure!(status=="present"&&version==expected,"native_lifecycle.member_changed");
    let adapter=adapter.context("native_lifecycle.member_runtime_missing")?;
    let binding=ResolvedRuntimeBinding{
        custom_runtime_ref:if adapter=="custom-acp"{crate::custom_runtime::storage::member_reference(connection,id)?}else{None},
        adapter_kind:crate::agent_profile::AdapterKind::from_str(&adapter)?,
        installation_id:installation.context("native_lifecycle.member_installation_missing")?,
        model:serde_json::from_str(&model.context("native_lifecycle.member_model_missing")?)?,
        permissions:serde_json::from_str(&permissions.context("native_lifecycle.member_permissions_missing")?)?,
    };
    if let Some(reference)=&binding.custom_runtime_ref{
        let current=crate::custom_runtime::storage::latest(connection,&reference.id)?;
        ensure!(current.is_some_and(|value|value["enabled"]==true&&value["definitionDigest"]==reference.definition_digest),"native_lifecycle.member_runtime_changed");
    }
    crate::agent_profile::resolve_frozen_runtime_for_member_binding(connection,id,&binding)?.map_err(|blocker|anyhow::anyhow!("native_lifecycle.runtime_blocked: {}",blocker.code))
}

/// 同一确认事务中创建唯一工作副本；没有用户消息，也不创建伪造 AgentRun。
pub(crate) fn create_working_copy(tx:&Transaction<'_>,preview:&Value,client:&crate::draft_client::DraftClient)->Result<(ThreadId,String)>{
    use crate::{collaboration::{CreateThreadCommand,ProjectBindingKind,ThreadActivationState,ThreadCollaborationMode},command::{ActorRef,CommandResultStatus}};
    let owner=preview["sourceThreadId"].as_str().context("native_lifecycle.source_thread_missing")?;
    let (path,workspace_digest)=workspace(tx,owner)?;ensure!(preview["workspaceDigest"]==workspace_digest,"native_lifecycle.project_changed");
    let thread=ThreadId::parse(preview["newThreadId"].as_str().context("native_lifecycle.new_thread_missing")?)?;
    let conversation=preview["newConversationId"].as_str().context("native_lifecycle.new_conversation_missing")?.to_owned();uuid::Uuid::parse_str(&conversation)?;
    let agent=preview["agentId"].as_str().context("native_lifecycle.member_missing")?.to_owned();
    let command=CreateThreadCommand{name:Some(preview["title"].as_str().context("native_lifecycle.title_missing")?.into()),project_binding_kind:ProjectBindingKind::Directory,project_path:path.to_string_lossy().into_owned(),member_agent_ids:vec![agent.clone()],default_lead_agent_id:agent.clone(),collaboration_mode:ThreadCollaborationMode::Peer,activation_state:ThreadActivationState::Active};
    let actor=ActorRef::User{user_id:"local_user".into()};
    let result=crate::collaboration::create_camp_in_tx(tx,&actor,None,&command,&thread)?;
    ensure!(result.status==CommandResultStatus::Applied,"native_lifecycle.thread_creation_rejected: {}",result.code);
    let now=chrono::Utc::now().to_rfc3339();
    tx.execute("INSERT INTO conversation(id,camp_id,agent_id,kind,summary_through_message_sequence,last_message_sequence,last_accepted_public_boundary_sequence,last_input_sequence,native_binding_generation,version,created_at,updated_at) VALUES(?1,?2,?3,'single_chat',0,0,0,0,0,1,?4,?4)",params![conversation,thread,agent,now])?;
    tx.execute("INSERT INTO single_chat_composer_draft(conversation_id,revision,source_attachments_json,updated_at,client_id) VALUES(?1,0,'[]',?2,?3)",params![conversation,now,client.id()])?;
    crate::collaboration::append_domain_event(tx,"single_chat.opened",Some(thread.as_str()),Some(("conversation",&conversation)),&actor,None,&json!({"agentId":agent,"conversationVersion":1}))?;
    Ok((thread,conversation))
}

pub(crate) fn receipt(connection:&Connection,command:&str,client:&str,core_instance:&str)->Result<Value>{
    let(kind,stored_client,instance,state,raw):(String,String,String,String,String)=connection.query_row("SELECT kind,client_id,core_instance_id,state,value_json FROM native_lifecycle_intent WHERE command_id=?1",[command],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).context("native_lifecycle.command_missing")?;
    ensure!(kind=="continuation"&&stored_client==client,"native_lifecycle.command_scope_mismatch");
    let mut value:Value=serde_json::from_str(&raw)?;
    if matches!(state.as_str(),"prepared"|"starting")&&(instance!=core_instance||!super::steer_delivery::active(command)){
        value["state"]=json!("unknown");value["error"]=json!("原生创建结果待核对，禁止自动重复创建");
    }
    validate_shape("NativeContinuationReceiptV1",&value)?;Ok(value)
}

pub(crate) fn reserve(tx:&Transaction<'_>,command:&str,source:&NativeSourceReferenceV1)->Result<()>{
    let occupied:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM native_lifecycle_source_lease WHERE provider=?1 AND native_session_id=?2 AND state<>'released')",params![source.provider,source.native_session_id],|row|row.get(0))?;
    ensure!(!occupied,"native_lifecycle.source_already_in_use");
    let changed=tx.execute("INSERT INTO native_lifecycle_source_lease(source_handle_id,provider,native_session_id,command_id,state) VALUES(?1,?2,?3,?4,'reserved') ON CONFLICT(source_handle_id,provider,native_session_id) DO UPDATE SET command_id=excluded.command_id,state='reserved' WHERE native_lifecycle_source_lease.state='released'",params![source.source_handle_id,source.provider,source.native_session_id,command])?;
    ensure!(changed==1,"native_lifecycle.source_already_in_use");Ok(())
}
pub(crate) fn runtime_for_conversation(connection:&Connection,conversation:&str,agent:&str,base:&FrozenAgentRuntimeConfig)->Result<Option<FrozenAgentRuntimeConfig>>{
    if storage::absent(connection)?{return Ok(None)}
    let stored:Option<(String,String,String,String)>=connection.query_row("SELECT n.base_runtime_digest,n.runtime_json,i.state,c.agent_id FROM native_lifecycle_continuation n JOIN native_lifecycle_intent i ON i.command_id=n.command_id JOIN conversation c ON c.id=n.conversation_id WHERE n.conversation_id=?1",[conversation],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    let Some((expected,raw,state,owner))=stored else{return Ok(None)};
    ensure!(state=="binding_recorded","native_lifecycle.preparation_not_confirmed");
    ensure!(owner==agent,"native_lifecycle.prepared_runtime_changed");
    let frozen:FrozenAgentRuntimeConfig=serde_json::from_str(&raw)?;
    let origin=frozen.native_origin.as_ref().context("native_lifecycle.origin_missing")?;origin.validate()?;
    let started:bool=connection.query_row("SELECT last_input_sequence>0 FROM conversation WHERE id=?1",[conversation],|row|row.get(0))?;
    if !started{ensure!(base.config_digest==expected,"native_lifecycle.prepared_runtime_changed");return Ok(Some(frozen))}
    // 创建时准备的首次输入沿用原冻结；以后的新消息按当前开关冻结，旧 Run 不回写。
    let mut next=base.clone();
    super::freeze_intent(&mut next,Some(origin.clone()),storage::freeze_policy(connection,agent)?)?;
    Ok(Some(next))
}

/// 释放仅发生于 Core 已结束该工作副本且不存在活动执行时；unknown 不按重试释放。
pub(crate) fn release_closed(connection:&Connection)->Result<()> {
    if storage::absent(connection)?{return Ok(())}
    connection.execute("UPDATE native_lifecycle_source_lease SET state='released' WHERE state='bound' AND command_id IN (SELECT n.command_id FROM native_lifecycle_continuation n LEFT JOIN conversation c ON c.id=n.conversation_id LEFT JOIN camp t ON t.id=n.thread_id WHERE (c.id IS NULL OR c.ended_at IS NOT NULL OR t.id IS NULL) AND NOT EXISTS(SELECT 1 FROM agent_run r WHERE r.conversation_id=n.conversation_id AND r.ended_at IS NULL))",[])?;Ok(())
}
