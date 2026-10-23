use super::*;
use crate::{agent_profile::{AdapterKind,FrozenAgentRuntimeConfig},agent_runtime_adapter::{AgentRuntimeAdapterRegistry,ExternalMcpProjection}};
use rusqlite::{Connection,OptionalExtension,params};
use std::path::Path;

pub fn catalog(flags:FlagsV1)->Result<Vec<Value>>{
    let tools=CONTRACT["modelTools"].as_array().context("conversation_tools.catalog_invalid")?.iter().filter(|tool|tool["name"].as_str().is_some_and(|name|flags.permits(name))).cloned().collect::<Vec<_>>();
    ensure!(tools.len()<=4,"conversation_tools.catalog_invalid");Ok(tools)
}
pub(crate) fn attach(runtime:&mut FrozenAgentRuntimeConfig,frozen:Option<FrozenConversationToolsV1>)->Result<()>{
    if let Some(frozen)=&frozen{
        frozen.validate()?;
        let identity=digest(&json!({"policyRevision":frozen.policy_revision,"policyDigest":frozen.policy_digest,"scope":frozen.scope,"agentId":frozen.agent_id,"flags":frozen.flags,"feedbackGate":frozen.feedback_gate,"catalogDigest":frozen.catalog_digest}))?;
        runtime.binding_compatibility_digest=crate::command::canonical_json_digest(&json!({"native":runtime.binding_compatibility_digest,"conversationTools":identity}))?;
    }
    runtime.conversation_tools=frozen;runtime.refresh_config_digest()?;Ok(())
}
pub(crate) fn freeze_for_conversation(connection:&Connection,conversation:&str,member:&str,runtime:&mut FrozenAgentRuntimeConfig)->Result<()>{
    let policy=policy::get(connection)?;let mut flags=policy.member_flags(member);
    if !flags.any()&&policy.feedback_gate.is_none(){return Ok(())}
    let capability=AgentRuntimeAdapterRegistry::default().mcp_projection_for_runtime(runtime);
    if !capability.supports_stdio||capability.external_mcp_projection==ExternalMcpProjection::Unsupported||matches!(runtime.adapter_kind,AdapterKind::Pi|AdapterKind::CursorAgent){flags=FlagsV1::default();}
    let(camp,kind,path,revision):(String,String,String,i64)=connection.query_row("SELECT c.camp_id,c.kind,camp.project_path,camp.version FROM conversation c JOIN camp ON camp.id=c.camp_id WHERE c.id=?1 AND c.agent_id=?2 AND c.ended_at IS NULL AND camp.deletion_operation_id IS NULL",params![conversation,member],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)))?;
    let scope_kind=match kind.as_str(){"camp_member"=>ScopeKindV1::Public,"single_chat"=>ScopeKindV1::SingleChat,"delegation_child"=>ScopeKindV1::DelegationChild,_=>anyhow::bail!("conversation_tools.scope_changed")};
    let scope=ScopeV1{thread_id:camp.clone(),conversation_id:if matches!(scope_kind,ScopeKindV1::Public){None}else{Some(conversation.into())},scope_kind};
    let mut project_grants=Vec::new();
    if flags.automations||flags.work_tasks{
        let mut query=connection.prepare("SELECT m.agent_id FROM camp_member m JOIN agent_profile a ON a.id=m.agent_id WHERE m.camp_id=?1 AND m.status='active' AND m.leave_requested_at IS NULL AND a.profile_status='present' ORDER BY m.agent_id")?;
        let members=query.query_map([&camp],|row|row.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        project_grants.push(ProjectGrantV1{project_id:uuid::Uuid::new_v4().to_string(),project_path:path,revision,member_agent_ids:members});
    }
    let frozen=FrozenConversationToolsV1{schema_version:1,policy_revision:policy.revision,policy_digest:digest(&serde_json::to_value(&policy)?)?,scope,agent_id:member.into(),flags,feedback_gate:policy.feedback_gate,source_grants:vec![],project_grants,catalog_digest:digest(&json!(catalog(flags)?))?};
    attach(runtime,Some(frozen))
}
/// 只接收当前真实User消息中的来源引用；调用方不得传入Agent或系统消息正文。
pub(crate) fn select_user_sources(connection:&Connection,runtime:&mut FrozenAgentRuntimeConfig,user_text:&str)->Result<()>{
    let Some(mut frozen)=runtime.conversation_tools.clone()else{return Ok(())};
    if !frozen.flags.session_info{return Ok(())}
    let pattern=regex::Regex::new(r"rovai-source:([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})")?;
    let ids=pattern.captures_iter(user_text).take(129).map(|capture|capture[1].to_lowercase()).collect::<Vec<_>>();
    frozen.source_grants=grants::selected(connection,&frozen.scope,&ids)?;
    // 原文中的资料引用是本Run输入事实；不改变已经冻结的MCP目录身份。
    runtime.conversation_tools=Some(frozen);
    runtime.refresh_config_digest()?;Ok(())
}
pub(crate) fn authorize(connection:&mut Connection,id:&str,epoch:i64)->Result<scope::RunLease>{
    let tx=connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let raw:String=tx.query_row("SELECT effective_config_json FROM agent_run WHERE id=?1 AND execution_epoch=?2 AND status='running' AND cancel_requested_at IS NULL",params![id,epoch],|row|row.get(0)).context("conversation_tools.run_unavailable")?;
    let value:Value=serde_json::from_str(&raw)?;let frozen:FrozenConversationToolsV1=serde_json::from_value(value.pointer("/runtime/conversationTools").context("conversation_tools.not_assigned")?.clone())?;
    scope::register_run(&tx,id,epoch,&frozen)?;let lease=scope::require_run(&tx,id,epoch)?;tx.commit()?;Ok(lease)
}
pub(crate) fn session_info(connection:&Connection,root:&Path,lease:&scope::RunLease,input:&Value)->Result<Value>{
    validate_tool("get_session_info",input)?;let current=scope::require_run(connection,&lease.run_id,lease.epoch)?;
    ensure!(current.frozen==lease.frozen&&lease.frozen.flags.session_info,"conversation_tools.not_assigned");
    let requested=&input["session_id"];let absent=||json!({"found":false,"session_id":requested});
    let matches=lease.frozen.source_grants.iter().filter(|grant|requested.as_str()==Some(grant.source_session_id.as_str())||requested.as_i64().is_some_and(|id|grant.legacy_session_id==Some(id))).collect::<Vec<_>>();
    if matches.len()!=1{return Ok(absent())}let grant=matches[0];
    if grants::require_current(connection,&lease.frozen.scope,grant).is_err(){return Ok(absent())}
    let source=match sources::load(connection,&grant.source_session_id){Ok(source)=>source,Err(_)=>return Ok(absent())};
    let snapshot=match source_projection::read(connection,root,&source,Some(grant.through_sequence)){Ok(snapshot) if snapshot.source_digest==grant.source_digest=>snapshot,_=>return Ok(absent())};
    source_projection::info(&source,&snapshot,requested,input["max_messages"].as_u64().unwrap_or(20)as usize)
}
pub fn feedback_allowed(runtime:&FrozenAgentRuntimeConfig)->bool{runtime.conversation_tools.as_ref().is_none_or(|frozen|frozen.feedback_gate!=Some(false))}
