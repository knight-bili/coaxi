//! Core SQLite 拥有用户原文、预览、命令身份和每次投递尝试。
use super::*;
use chrono::{DateTime,Utc};
use rusqlite::{params,Connection,OptionalExtension,TransactionBehavior};

pub fn policy(connection:&Connection)->Result<Policy>{let raw:String=connection.query_row("SELECT value_json FROM ui_feedback_policy WHERE singleton=1",[],|row|row.get(0))?;let value:Policy=serde_json::from_str(&raw)?;value.validate()?;Ok(value)}
pub fn set_policy(connection:&mut Connection,expected:u64,members:Vec<MemberPolicy>)->Result<Policy>{
    ensure!(expected<MAX_SAFE_INTEGER,"feedback.invalid_revision");let value=Policy{schema_version:1,revision:expected+1,members};value.validate()?;
    let tx=connection.transaction_with_behavior(TransactionBehavior::Immediate)?;let current=policy(&tx)?;
    if serde_json::to_value(&current)?==serde_json::to_value(&value)?{return Ok(current)}
    ensure!(current.revision==expected,"policy_changed");
    for member in &value.members{let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM agent_profile WHERE id=?1)",[&member.agent_id],|row|row.get(0))?;ensure!(exists,"member_changed")}
    let revision=i64::try_from(value.revision).context("feedback.invalid_revision")?;
    tx.execute("UPDATE ui_feedback_policy SET revision=?1,value_json=?2 WHERE singleton=1",params![revision,serde_json::to_string(&value)?])?;
    // 撤权后的未写出原文保留，不允许旧凭据领取另一份新反馈。
    tx.execute("UPDATE ui_feedback_entry SET receipt_json=json_set(receipt_json,'$.status',CASE WHEN status='pending' THEN 'requires_user_review' ELSE 'unknown' END),status=CASE WHEN status='pending' THEN 'requires_user_review' ELSE 'unknown' END WHERE status IN ('pending','response_prepared')",[])?;
    tx.commit()?;Ok(value)
}
pub fn scope(connection:&Connection,target:&Scope,agent:&str)->Result<()>{
    target.validate()?;
    let allowed:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM conversation c JOIN camp ON camp.id=c.camp_id JOIN camp_member m ON m.camp_id=camp.id AND m.agent_id=c.agent_id WHERE camp.id=?1 AND c.agent_id=?2 AND c.ended_at IS NULL AND camp.activation_state='active' AND camp.deletion_operation_id IS NULL AND m.status='active' AND m.leave_requested_at IS NULL AND ((?3='public' AND c.kind='camp_member' AND ?4 IS NULL) OR(c.kind=?3 AND c.id=?4)))",params![target.thread_id,agent,target.scope_kind,target.conversation_id],|row|row.get(0))?;
    ensure!(allowed,"scope_changed");Ok(())
}
pub fn run_guard(connection:&Connection,target:&Scope,agent:&str,run:&str,epoch:u64)->Result<Value>{
    scope(connection,target,agent)?;ensure!(epoch<=MAX_SAFE_INTEGER,"epoch_changed");let epoch=i64::try_from(epoch).context("epoch_changed")?;let current_policy=policy(connection)?;ensure!(current_policy.enabled(agent),"feature_disabled");
    let guard:Option<Value>=connection.query_row(r#"
        SELECT c.native_binding_id,c.native_binding_generation,c.native_session_id,camp.membership_generation,m.version
        FROM agent_run r JOIN conversation c ON c.id=r.conversation_id JOIN camp ON camp.id=c.camp_id
        JOIN camp_member m ON m.camp_id=camp.id AND m.agent_id=c.agent_id LEFT JOIN camp_turn t ON t.id=r.camp_turn_id
        WHERE r.id=?1 AND r.execution_epoch=?2 AND r.status='running' AND r.cancel_requested_at IS NULL
        AND camp.id=?3 AND c.agent_id=?4 AND c.ended_at IS NULL AND camp.activation_state='active'
        AND m.status='active' AND m.leave_requested_at IS NULL AND m.version=CAST(json_extract(r.effective_config_json,'$.campMemberVersion') AS INTEGER)
        AND ((?5='public' AND c.kind='camp_member' AND ?6 IS NULL) OR(c.kind=?5 AND c.id=?6))
        AND (r.camp_turn_id IS NULL OR(t.status IN ('running','waiting') AND t.cancel_requested_at IS NULL AND t.execution_budget_exhausted_at IS NULL
        AND(t.execution_budget_deadline_at>?7 OR(t.execution_budget_deadline_at IS NULL AND t.execution_budget_schema_version=2))))
    "#,params![run,epoch,target.thread_id,agent,target.scope_kind,target.conversation_id,crate::execution_budget::camp_turn_execution_budget_now().to_rfc3339()],|row|Ok(json!({"nativeBindingId":row.get::<_,Option<String>>(0)?,"nativeBindingGeneration":row.get::<_,i64>(1)?,"nativeSessionId":row.get::<_,Option<String>>(2)?,"membershipGeneration":row.get::<_,i64>(3)?,"memberVersion":row.get::<_,i64>(4)?,"policyRevision":current_policy.revision}))).optional()?;
    guard.context("run_not_active")
}
/// 用户确认和实际工具领取必须属于已冻结了反馈服务器的任务。
/// 工具 catalog 的启动握手仍可早于 ContextManifest，不在该阶段塞入旧任务。
pub fn receiving_guard(connection:&Connection,target:&Scope,agent:&str,run:&str,epoch:u64)->Result<Value>{
    let guard=run_guard(connection,target,agent,run,epoch)?;
    let admitted:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM context_manifest m,json_each(m.mcp_exposure_json,'$.servers') s WHERE m.agent_run_id=?1 AND json_extract(s.value,'$.serverId')=?2)",params![run,SERVER_NAME],|row|row.get(0))?;
    ensure!(admitted,"runtime_unavailable");Ok(guard)
}
pub fn require_binding(connection:&Connection,binding:&Binding,core_instance:&str)->Result<()>{
    validate_shape("binding",&serde_json::to_value(binding)?)?;
    ensure!(binding.core_instance_id==core_instance&&DateTime::parse_from_rfc3339(&binding.expires_at)?.with_timezone(&Utc)>Utc::now(),"binding_changed");
    ensure!(binding.catalog_digest==digest(&catalog())?,"binding_changed");
    let guard=run_guard(connection,&binding.scope,&binding.agent_id,&binding.agent_run_id,binding.execution_epoch)?;
    ensure!(guard["policyRevision"]==binding.policy_revision,"policy_changed");
    ensure!(guard["nativeBindingId"]==binding.native_binding_id&&guard["nativeBindingGeneration"]==binding.native_binding_generation&&guard["membershipGeneration"]==binding.membership_generation,"binding_changed");
    if let Some(session)=&binding.native_session_id{ensure!(guard["nativeSessionId"]==*session,"binding_changed")}
    Ok(())
}
pub fn preview(connection:&mut Connection,input:PreviewRequest)->Result<PreviewResult>{
    let text=preview_text(&input)?;
    let tx=connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let guard=receiving_guard(&tx,&input.scope,&input.agent_id,&input.agent_run_id,input.execution_epoch)?;
    if let Source::GeneratedCard{origin}=&input.source{super::cards::require_card_fields(&tx,&input,origin)?}
    let id=uuid::Uuid::new_v4().to_string();let now=Utc::now();let expires=(now+chrono::Duration::minutes(10)).to_rfc3339();
    let request_json=serde_json::to_string(&input)?;
    let preview_digest=digest(&json!({"previewId":id,"input":input,"guard":guard,"text":text,"expiresAt":expires}))?;
    let result=PreviewResult{schema_version:1,preview_id:id.clone(),preview_digest,scope:input.scope.clone(),agent_id:input.agent_id.clone(),agent_run_id:input.agent_run_id.clone(),execution_epoch:input.execution_epoch,text,expires_at:expires.clone()};
    tx.execute("DELETE FROM ui_feedback_preview WHERE expires_at<?1 AND NOT EXISTS(SELECT 1 FROM ui_feedback_entry e WHERE e.preview_id=ui_feedback_preview.id)",[now.to_rfc3339()])?;
    let count:i64=tx.query_row("SELECT count(*) FROM ui_feedback_preview WHERE expires_at>=?1",[now.to_rfc3339()],|row|row.get(0))?;ensure!(count<4096,"feedback.preview_capacity");
    tx.execute("INSERT INTO ui_feedback_preview(id,request_json,result_json,binding_guard_json,expires_at,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id,request_json,serde_json::to_string(&result)?,serde_json::to_string(&guard)?,expires,now.to_rfc3339()])?;
    tx.commit()?;Ok(result)
}
pub fn receipt(connection:&Connection,id:&str)->Result<Receipt>{let raw:String=connection.query_row("SELECT receipt_json FROM ui_feedback_entry WHERE id=?1",[id],|row|row.get(0))?;let result:Receipt=serde_json::from_str(&raw)?;validate_shape("receipt",&serde_json::to_value(&result)?)?;Ok(result)}
pub fn submit(connection:&mut Connection,input:SubmitRequest)->Result<Receipt>{
    validate_shape("submitRequest",&serde_json::to_value(&input)?)?;let input_digest=digest(&serde_json::to_value(&input)?)?;
    let tx=connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let previous:Option<(String,String)>=tx.query_row("SELECT input_digest,feedback_id FROM ui_feedback_command WHERE command_id=?1",[&input.command_id],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
    if let Some((original,id))=previous{ensure!(original==input_digest,"feedback.command_changed");return receipt(&tx,&id)}
    let(raw,result,original_guard,expires):(String,String,String,String)=tx.query_row("SELECT request_json,result_json,binding_guard_json,expires_at FROM ui_feedback_preview WHERE id=?1",[&input.preview_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)))?;
    let source:PreviewRequest=serde_json::from_str(&raw)?;let result:PreviewResult=serde_json::from_str(&result)?;
    ensure!(result.preview_digest==input.preview_digest&&DateTime::parse_from_rfc3339(&expires)?.with_timezone(&Utc)>Utc::now(),"feedback.preview_changed");
    let guard=receiving_guard(&tx,&source.scope,&source.agent_id,&source.agent_run_id,source.execution_epoch)?;ensure!(serde_json::from_str::<Value>(&original_guard)?==guard,"binding_changed");
    if let Source::GeneratedCard{origin}=&source.source{super::cards::require_card_fields(&tx,&source,origin)?}
    let existing:Option<String>=tx.query_row("SELECT id FROM ui_feedback_entry WHERE preview_id=?1",[&input.preview_id],|row|row.get(0)).optional()?;
    let response=if let Some(id)=existing{receipt(&tx,&id)?}else{
        let execution_epoch=i64::try_from(source.execution_epoch).context("epoch_changed")?;
        let pending:i64=tx.query_row("SELECT count(*) FROM ui_feedback_entry WHERE agent_run_id=?1 AND execution_epoch=?2 AND status IN ('pending','response_prepared')",params![source.agent_run_id,execution_epoch],|row|row.get(0))?;ensure!(pending<MAX_PENDING as i64,"feedback.pending_capacity");
        let id=uuid::Uuid::new_v4().to_string();let created=Utc::now().to_rfc3339();let source_kind=match source.source{Source::LiveNote=>"live_note",Source::GeneratedCard{..}=>"generated_card"};
        let record=Receipt{schema_version:1,feedback_id:id.clone(),scope:source.scope.clone(),agent_id:source.agent_id.clone(),source_agent_run_id:source.agent_run_id.clone(),source_execution_epoch:source.execution_epoch,source_kind:source_kind.into(),text_digest:digest(&Value::String(result.text.clone()))?,status:"pending".into(),delivery_channel:"none".into(),bridge_instance_id:None,transport_rpc_id:Value::Null,native_tool_call_id:None,result_digest:None,written_at:None,continued_message_id:None};
        tx.execute("INSERT INTO ui_feedback_entry(id,preview_id,thread_id,conversation_id,scope_kind,agent_id,agent_run_id,execution_epoch,text,created_at,status,receipt_json,source_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'pending',?11,?12)",params![id,input.preview_id,source.scope.thread_id,source.scope.conversation_id,source.scope.scope_kind,source.agent_id,source.agent_run_id,execution_epoch,result.text,created,serde_json::to_string(&record)?,raw])?;record
    };
    tx.execute("INSERT INTO ui_feedback_command(command_id,input_digest,feedback_id,created_at) VALUES(?1,?2,?3,?4)",params![input.command_id,input_digest,response.feedback_id,Utc::now().to_rfc3339()])?;
    tx.commit()?;Ok(response)
}
pub fn list(connection:&Connection,target:&Scope,agent:&str,after:Option<&str>)->Result<Value>{
    scope(connection,target,agent)?;
    if let Some(id)=after{uuid::Uuid::parse_str(id)?;}
    let mut query=connection.prepare("SELECT id,text,created_at FROM ui_feedback_entry WHERE thread_id=?1 AND scope_kind=?2 AND conversation_id IS ?3 AND agent_id=?4 AND(?5 IS NULL OR(created_at,id)>(SELECT created_at,id FROM ui_feedback_entry WHERE id=?5 AND thread_id=?1 AND scope_kind=?2 AND conversation_id IS ?3 AND agent_id=?4)) ORDER BY created_at,id LIMIT 51")?;
    let mut rows=query.query_map(params![target.thread_id,target.scope_kind,target.conversation_id,agent,after],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let more=rows.len()>50;rows.truncate(50);let next=if more{rows.last().map(|row|row.0.clone())}else{None};
    let items=rows.into_iter().map(|(id,text,created)|Ok(json!({"receipt":receipt(connection,&id)?,"text":text,"createdAt":created}))).collect::<Result<Vec<_>>>()?;
    Ok(json!({"items":items,"next":next}))
}
