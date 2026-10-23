//! 原请求意图先持久化；业务写入必须在第二个事务核对同一原意图后完成。
use super::*;
use super::scope::RunLease;
use rusqlite::{Connection,OptionalExtension,params,TransactionBehavior};

fn context(id:&str,part:&str)->String{format!("conversation-tool:{id}:{part}")}
pub(crate) fn status(connection:&Connection,lease:&RunLease,id:&str)->Result<Value>{
    uuid::Uuid::parse_str(id)?;
    let row:Option<(String,i64,String,Option<Vec<u8>>)>=connection.query_row("SELECT agent_run_id,execution_epoch,state,result_cipher FROM conversation_tool_request WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    let Some((run,epoch,state,cipher))=row else{return Ok(json!({"request_id":id,"state":"not_seen"}))};
    ensure!(run==lease.run_id&&epoch==lease.epoch,"conversation_tools.request_owner_changed");
    let mut value=json!({"request_id":id,"state":state});
    if let Some(cipher)=cipher{value["outcome"]=crate::member_environment::decrypt::<Value>(connection,&context(id,"result"),&cipher)?;}
    validate("RequestStatusV1",&value)?;Ok(value)
}
pub(crate) fn begin(connection:&mut Connection,lease:&RunLease,name:&str,input:&Value)->Result<Value>{
    validate_tool(name,input)?;ensure!(lease.frozen.flags.permits(name)&&name!="get_session_info","conversation_tools.not_assigned");
    ensure!(input["operation"]!="status","conversation_tools.status_is_read_only");
    let id=input["request_id"].as_str().context("conversation_tools.request_id_required")?;
    let input_digest=digest(input)?;let tx=connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current=scope::require_run(&tx,&lease.run_id,lease.epoch)?;
    ensure!(current.frozen==lease.frozen,"conversation_tools.frozen_run_changed");
    let prior:Option<(String,i64,String,String)>=tx.query_row("SELECT agent_run_id,execution_epoch,tool_name,input_digest FROM conversation_tool_request WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    if let Some((run,epoch,tool,stored))=prior{
        ensure!(run==lease.run_id&&epoch==lease.epoch&&tool==name&&stored==input_digest,"conversation_tools.request_payload_changed");
        return status(&tx,lease,id)
    }
    let blocked:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tool_request WHERE agent_run_id=?1 AND execution_epoch=?2 AND tool_name=?3 AND (state IN ('prepared','outcome_unknown') OR (?3='ask_user_question' AND state IN ('awaiting_answer','answered'))))",params![lease.run_id,lease.epoch,name],|row|row.get(0))?;
    ensure!(!blocked,"conversation_tools.original_request_unresolved");
    let cipher=crate::member_environment::encrypt(&tx,&context(id,"input"),input)?;
    let now=chrono::Utc::now();let expires=now+chrono::Duration::seconds(7200);
    tx.execute("INSERT INTO conversation_tool_request(id,agent_run_id,execution_epoch,tool_name,input_digest,payload_cipher,state,created_at,expires_at_ms,updated_at) VALUES(?1,?2,?3,?4,?5,?6,'prepared',?7,?8,?7)",params![id,lease.run_id,lease.epoch,name,input_digest,cipher,now.to_rfc3339(),expires.timestamp_millis()])?;
    let result=status(&tx,lease,id)?;tx.commit()?;Ok(result)
}
/// 调用方的业务事务与本函数共享一个连接/事务；取消墓碑胜出后迟到写入被拒绝。
pub(crate) fn prepared(connection:&Connection,lease:&RunLease,id:&str,name:&str)->Result<Value>{
    let current=scope::require_run(connection,&lease.run_id,lease.epoch)?;ensure!(current.frozen==lease.frozen&&lease.frozen.flags.permits(name),"conversation_tools.not_assigned");
    let(state,expires,cipher):(String,i64,Vec<u8>)=connection.query_row("SELECT state,expires_at_ms,payload_cipher FROM conversation_tool_request WHERE id=?1 AND agent_run_id=?2 AND execution_epoch=?3 AND tool_name=?4",params![id,lease.run_id,lease.epoch,name],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
    ensure!(state=="prepared"&&chrono::Utc::now().timestamp_millis()<expires,"conversation_tools.request_no_longer_prepared");
    let input=crate::member_environment::decrypt(connection,&context(id,"input"),&cipher)?;validate_tool(name,&input)?;Ok(input)
}
pub(crate) fn finish(connection:&Connection,id:&str,kind:&str,result:&Value)->Result<()>{
    validate(if kind=="question"{"QuestionOutcomeV1"}else{"AuthoringOutcomeV1"},result)?;
    let cipher=crate::member_environment::encrypt(connection,&context(id,"result"),result)?;
    ensure!(connection.execute("UPDATE conversation_tool_request SET state='applied',revision=revision+1,result_kind=?2,result_cipher=?3,updated_at=?4 WHERE id=?1 AND state IN ('prepared','answered')",params![id,kind,cipher,chrono::Utc::now().to_rfc3339()])?==1,"conversation_tools.request_changed");Ok(())
}
pub(crate) fn cancel_remaining(connection:&Connection,id:&str)->Result<bool>{
    uuid::Uuid::parse_str(id)?;
    // SQLite 写事务串行裁决；已落地业务的 applied 回执不会被写成撤销。
    let changed=connection.execute("UPDATE conversation_tool_request SET state='cancelled',revision=revision+1,updated_at=?2 WHERE id=?1 AND state IN ('prepared','awaiting_answer','outcome_unknown') AND NOT EXISTS(SELECT 1 FROM conversation_tool_delivery WHERE request_id=?1 AND state IN ('claimed','delivered','outcome_unknown'))",params![id,chrono::Utc::now().to_rfc3339()])?;
    if changed==1{connection.execute("UPDATE conversation_tool_delivery SET state='cancelled',updated_at=?2 WHERE request_id=?1 AND state='pending'",params![id,chrono::Utc::now().to_rfc3339()])?;}Ok(changed==1)
}
