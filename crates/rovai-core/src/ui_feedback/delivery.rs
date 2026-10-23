//! 读取先冻结，实际写到智能体连接后才确认。重试始终引用原编号。
use super::*;
use chrono::Utc;
use rusqlite::{params,Connection,OptionalExtension,TransactionBehavior};

fn prepared(id:&str,binding_digest:&str,ids:&Value,result:&Value,result_digest:&str)->Value{json!({"schemaVersion":1,"attemptId":id,"bindingDigest":binding_digest,"feedbackIds":ids,"resultDigest":result_digest,"result":result})}
pub fn prepare(connection:&mut Connection,binding:&Binding,core_instance:&str,rpc_id:Value,arguments_digest:&str)->Result<Value>{
    let binding_digest=digest(&serde_json::to_value(binding)?)?;
    validate_shape("privatePrepareRequest",&json!({"schemaVersion":1,"bindingDigest":binding_digest,"transportRpcId":rpc_id,"argumentsDigest":arguments_digest}))?;
    let tx=connection.transaction_with_behavior(TransactionBehavior::Immediate)?;storage::require_binding(&tx,binding,core_instance)?;
    let guard=storage::receiving_guard(&tx,&binding.scope,&binding.agent_id,&binding.agent_run_id,binding.execution_epoch)?;
    ensure!(guard["nativeSessionId"].is_string(),"binding_changed");
    let rpc_json=serde_json::to_string(&rpc_id)?;
    let previous:Option<(String,String,String,String,String,String)>=tx.query_row("SELECT id,binding_digest,arguments_digest,feedback_ids_json,result_json,result_digest FROM ui_feedback_attempt WHERE bridge_instance_id=?1 AND rpc_id_json=?2",params![binding.bridge_instance_id,rpc_json],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))).optional()?;
    if let Some((id,original_binding,original_arguments,ids,result,result_digest))=previous{
        ensure!(original_binding==binding_digest&&original_arguments==arguments_digest,"feedback.rpc_identity_changed");
        return Ok(prepared(&id,&binding_digest,&serde_json::from_str::<Value>(&ids)?,&serde_json::from_str::<Value>(&result)?,&result_digest))
    }
    let attempts:i64=tx.query_row("SELECT count(*) FROM ui_feedback_attempt WHERE bridge_instance_id=?1",[&binding.bridge_instance_id],|row|row.get(0))?;ensure!(attempts<16384,"original_request_pending");
    let execution_epoch=i64::try_from(binding.execution_epoch).context("binding_changed")?;
    let candidates={let mut query=tx.prepare("SELECT id,text,created_at FROM ui_feedback_entry WHERE agent_run_id=?1 AND execution_epoch=?2 AND status='pending' AND thread_id=?3 AND conversation_id IS ?4 AND scope_kind=?5 AND agent_id=?6 ORDER BY created_at,id LIMIT 256")?;query.query_map(params![binding.agent_run_id,execution_epoch,binding.scope.thread_id,binding.scope.conversation_id,binding.scope.scope_kind,binding.agent_id],|row|Ok(ModelNote{id:row.get(0)?,text:row.get(1)?,created_at:row.get(2)?}))?.collect::<rusqlite::Result<Vec<_>>>()?};
    let mut notes=Vec::new();let mut result=render_feedback_result(&json!({"feedback":[]}));
    for note in candidates{
        validate_shape("modelNote",&serde_json::to_value(&note)?)?;notes.push(note);
        let next=render_feedback_result(&json!({"feedback":notes}));
        // 为最外层 JSON RPC id 和封套留出空间，按实际 UTF8 字节判断。
        if serde_json::to_vec(&json!({"jsonrpc":"2.0","id":rpc_id,"result":next}))?.len()>MAX_FRAME_BYTES{notes.pop();break}
        result=next;
    }
    let id=uuid::Uuid::new_v4().to_string();let ids=Value::Array(notes.iter().map(|note|Value::String(note.id.clone())).collect());let result_digest=digest(&result)?;
    for note in &notes{
        let mut receipt=storage::receipt(&tx,&note.id)?;receipt.status="response_prepared".into();receipt.delivery_channel="mcp".into();receipt.bridge_instance_id=Some(binding.bridge_instance_id.clone());receipt.transport_rpc_id=rpc_id.clone();receipt.result_digest=Some(result_digest.clone());
        let changed=tx.execute("UPDATE ui_feedback_entry SET status='response_prepared',receipt_json=?1 WHERE id=?2 AND status='pending'",params![serde_json::to_string(&receipt)?,note.id])?;ensure!(changed==1,"original_request_pending");
    }
    tx.execute("INSERT INTO ui_feedback_attempt(id,bridge_instance_id,binding_json,binding_digest,rpc_id_json,arguments_digest,feedback_ids_json,result_json,result_digest,write_status,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'prepared',?10)",params![id,binding.bridge_instance_id,serde_json::to_string(binding)?,binding_digest,rpc_json,arguments_digest,serde_json::to_string(&ids)?,serde_json::to_string(&result)?,result_digest,Utc::now().to_rfc3339()])?;
    tx.commit()?;Ok(prepared(&id,&binding_digest,&ids,&result,&result_digest))
}
pub fn wrote(connection:&mut Connection,binding:&Binding,attempt:&str,binding_digest:&str,result_digest:&str,status:&str)->Result<Value>{
    validate_shape("privateWriteReceipt",&json!({"schemaVersion":1,"attemptId":attempt,"bindingDigest":binding_digest,"resultDigest":result_digest,"writeStatus":status}))?;
    ensure!(digest(&serde_json::to_value(binding)?)?==binding_digest,"binding_changed");
    let tx=connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let(original_binding,original_result,ids,previous):(String,String,String,String)=tx.query_row("SELECT binding_digest,result_digest,feedback_ids_json,write_status FROM ui_feedback_attempt WHERE id=?1 AND bridge_instance_id=?2",params![attempt,binding.bridge_instance_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)))?;
    ensure!(original_binding==binding_digest&&original_result==result_digest,"feedback.receipt_changed");
    if previous=="written"{return Ok(json!({"schemaVersion":1,"attemptId":attempt,"writeStatus":"written"}))}
    let ids:Vec<String>=serde_json::from_str(&ids)?;let now=Utc::now().to_rfc3339();
    for id in &ids{
        let mut receipt=storage::receipt(&tx,id)?;
        ensure!(receipt.bridge_instance_id.as_deref()==Some(binding.bridge_instance_id.as_str())&&receipt.result_digest.as_deref()==Some(result_digest),"feedback.receipt_changed");
        receipt.status=if status=="written"{"connection_written"}else{"unknown"}.into();receipt.written_at=if status=="written"{Some(now.clone())}else{None};
        tx.execute("UPDATE ui_feedback_entry SET status=?1,receipt_json=?2 WHERE id=?3",params![receipt.status,serde_json::to_string(&receipt)?,id])?;
    }
    tx.execute("UPDATE ui_feedback_attempt SET write_status=?1,written_at=?2 WHERE id=?3",params![if status=="written"{"written"}else{"unknown"},if status=="written"{Some(now.as_str())}else{None},attempt])?;
    tx.commit()?;Ok(json!({"schemaVersion":1,"attemptId":attempt,"writeStatus":status}))
}
pub fn revoke(connection:&Connection,binding:&Binding)->Result<()>{
    let execution_epoch=i64::try_from(binding.execution_epoch).context("binding_changed")?;
    connection.execute("UPDATE ui_feedback_entry SET receipt_json=json_set(receipt_json,'$.status',CASE WHEN status='pending' THEN 'requires_user_review' ELSE 'unknown' END),status=CASE WHEN status='pending' THEN 'requires_user_review' ELSE 'unknown' END WHERE agent_run_id=?1 AND execution_epoch=?2 AND status IN ('pending','response_prepared')",params![binding.agent_run_id,execution_epoch])?;Ok(())
}
pub fn recover_after_restart(connection:&Connection)->Result<()>{
    connection.execute("UPDATE ui_feedback_entry SET receipt_json=json_set(receipt_json,'$.status',CASE WHEN status='pending' THEN 'requires_user_review' ELSE 'unknown' END),status=CASE WHEN status='pending' THEN 'requires_user_review' ELSE 'unknown' END WHERE status IN ('pending','response_prepared')",[])?;
    connection.execute("UPDATE ui_feedback_attempt SET write_status='unknown' WHERE write_status='prepared'",[])?;Ok(())
}
