//! 普通消息续办使用新预览和原消息命令事务，不能和 MCP 双通道消费。
use super::*;
use chrono::{DateTime,Utc};
use rusqlite::{params,Connection,Transaction};
use crate::command::{CommandHandlerResult,CommandResultStatus};

#[derive(Debug,Clone,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Reference{pub schema_version:u32,pub feedback_id:String,pub preview_id:String,pub preview_digest:String}
#[derive(Debug,Clone)]pub struct Input{pub reference:Reference,pub scope:Scope,pub agent_id:String,pub body:String}
fn eligible(connection:&Connection,id:&str)->Result<(Receipt,String)>{
    uuid::Uuid::parse_str(id)?;let receipt=storage::receipt(connection,id)?;
    ensure!(matches!(receipt.status.as_str(),"pending"|"requires_user_review")&&receipt.delivery_channel=="none"&&receipt.bridge_instance_id.is_none()&&receipt.result_digest.is_none(),"feedback.normal_not_eligible");
    let attempted:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM ui_feedback_attempt a,json_each(a.feedback_ids_json) ids WHERE ids.value=?1)",[id],|row|row.get(0))?;ensure!(!attempted,"feedback.normal_not_eligible");
    let text:String=connection.query_row("SELECT text FROM ui_feedback_entry WHERE id=?1",[id],|row|row.get(0))?;Ok((receipt,text))
}
pub fn preview(connection:&mut Connection,scope:&Scope,agent:&str,feedback_id:&str)->Result<Value>{
    storage::scope(connection,scope,agent)?;
    let tx=connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;let(receipt,text)=eligible(&tx,feedback_id)?;
    ensure!(receipt.scope==*scope&&receipt.agent_id==agent,"scope_changed");
    let full=normal_message(feedback_id,&receipt.source_agent_run_id,&text);let id=uuid::Uuid::new_v4().to_string();let now=Utc::now();let expires=(now+chrono::Duration::minutes(10)).to_rfc3339();
    let guard=json!({"kind":"normal_message","feedbackId":feedback_id,"originalTextDigest":receipt.text_digest});
    let preview_digest=digest(&json!({"previewId":id,"scope":scope,"agentId":agent,"text":full,"guard":guard,"expiresAt":expires}))?;
    let result=PreviewResult{schema_version:1,preview_id:id.clone(),preview_digest:preview_digest.clone(),scope:scope.clone(),agent_id:agent.into(),agent_run_id:receipt.source_agent_run_id.clone(),execution_epoch:receipt.source_execution_epoch,text:full,expires_at:expires.clone()};
    let source:String=tx.query_row("SELECT source_json FROM ui_feedback_entry WHERE id=?1",[feedback_id],|row|row.get(0))?;
    tx.execute("INSERT INTO ui_feedback_preview(id,request_json,result_json,binding_guard_json,expires_at,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id,source,serde_json::to_string(&result)?,serde_json::to_string(&guard)?,expires,now.to_rfc3339()])?;
    let reference=Reference{schema_version:1,feedback_id:feedback_id.into(),preview_id:id,preview_digest};tx.commit()?;Ok(json!({"preview":result,"reference":reference}))
}
pub fn validate(connection:&Connection,input:&Input)->Result<()>{
    ensure!(input.reference.schema_version==1,"feedback.invalid_version");uuid::Uuid::parse_str(&input.reference.preview_id)?;
    storage::scope(connection,&input.scope,&input.agent_id)?;let(receipt,text)=eligible(connection,&input.reference.feedback_id)?;
    ensure!(receipt.scope==input.scope&&receipt.agent_id==input.agent_id,"scope_changed");
    let(raw,guard,expiry):(String,String,String)=connection.query_row("SELECT result_json,binding_guard_json,expires_at FROM ui_feedback_preview WHERE id=?1",[&input.reference.preview_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
    let preview:PreviewResult=serde_json::from_str(&raw)?;let guard:Value=serde_json::from_str(&guard)?;
    ensure!(guard["kind"]=="normal_message"&&guard["feedbackId"]==input.reference.feedback_id&&guard["originalTextDigest"]==receipt.text_digest,"feedback.normal_preview_changed");
    ensure!(preview.preview_digest==input.reference.preview_digest&&preview.scope==input.scope&&preview.agent_id==input.agent_id&&DateTime::parse_from_rfc3339(&expiry)?.with_timezone(&Utc)>Utc::now(),"feedback.normal_preview_changed");
    ensure!(preview.text==input.body&&input.body==normal_message(&receipt.feedback_id,&receipt.source_agent_run_id,&text),"feedback.normal_body_changed");Ok(())
}
pub fn complete(transaction:&Transaction<'_>,input:&Input,command_id:&str,result:&CommandHandlerResult)->Result<()>{
    if result.status==CommandResultStatus::Rejected{return Ok(())}
    validate(transaction,input)?;
    let mut receipt=storage::receipt(transaction,&input.reference.feedback_id)?;
    let message=result.payload["conversationMessageId"].as_str().or_else(||result.payload["messageId"].as_str()).or_else(||result.payload["campMessageId"].as_str());let pending=result.payload["pendingInputId"].as_str();
    ensure!(message.is_some()||pending.is_some(),"feedback.normal_result_unavailable");
    receipt.status="continued_as_message".into();receipt.delivery_channel="normal_message".into();receipt.continued_message_id=message.map(str::to_string);
    transaction.execute("INSERT INTO ui_feedback_continuation(feedback_id,command_id,preview_id,pending_input_id,message_id,result_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![receipt.feedback_id,command_id,input.reference.preview_id,pending,message,serde_json::to_string(result)?,Utc::now().to_rfc3339()])?;
    let changed=transaction.execute("UPDATE ui_feedback_entry SET status='continued_as_message',receipt_json=?1 WHERE id=?2 AND status IN ('pending','requires_user_review')",params![serde_json::to_string(&receipt)?,receipt.feedback_id])?;ensure!(changed==1,"feedback.normal_already_consumed");Ok(())
}
