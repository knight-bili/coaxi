//! 只向保存问题时的原工具调用投递一次；连接写入与用户答复分别记账。
use super::*;
use super::{questions::NativeQuestionTarget, scope::RunLease};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

pub(super) fn require_target(
    connection: &Connection,
    id: &str,
    lease: &RunLease,
    expected: &NativeQuestionTarget,
) -> Result<()> {
    let stored:Option<(String,String,i64)>=connection.query_row("SELECT q.target_digest,r.agent_run_id,r.execution_epoch FROM conversation_tool_question q JOIN conversation_tool_request r ON r.id=q.request_id WHERE q.request_id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    let (stored, run, epoch) = stored.context("conversation_tools.question_unavailable")?;
    ensure!(
        run == lease.run_id
            && epoch == lease.epoch
            && stored == digest(&serde_json::to_value(expected)?)?,
        "conversation_tools.original_native_call_changed"
    );
    Ok(())
}
/// 仅供认证后的原MCP调用轮询。已经领取的结果不再次返回可发送正文。
pub(crate) fn claim(
    connection: &mut Connection,
    lease: &RunLease,
    id: &str,
    expected: &NativeQuestionTarget,
) -> Result<Value> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    require_target(&tx, id, lease, expected)?;
    questions::close_unavailable(&tx)?;
    let status = requests::status(&tx, lease, id)?;
    if status["state"] != "answered" {
        tx.commit()?;
        return Ok(json!({"requestId":id,"state":status["state"]}));
    }
    let current = scope::require_run(&tx, &lease.run_id, lease.epoch)?;
    ensure!(
        current.frozen == lease.frozen,
        "conversation_tools.frozen_run_changed"
    );
    questions::target_guard(&tx, lease, expected)?;
    let(state,result_digest):(String,String)=tx.query_row("SELECT state,result_digest FROM conversation_tool_delivery WHERE request_id=?1 AND agent_run_id=?2 AND execution_epoch=?3 AND bridge_instance_id=?4 AND native_binding_id=?5 AND native_binding_generation=?6",params![id,lease.run_id,lease.epoch,expected.bridge_instance_id,expected.native_binding_id,expected.native_binding_generation],|row|Ok((row.get(0)?,row.get(1)?)))?;
    if state != "pending" {
        tx.commit()?;
        return Ok(
            json!({"requestId":id,"state":if state=="delivered"{"applied"}else{"outcome_unknown"}}),
        );
    }
    let outcome = status["outcome"].clone();
    validate("QuestionOutcomeV1", &outcome)?;
    ensure!(
        digest(&outcome)? == result_digest,
        "conversation_tools.answer_digest_changed"
    );
    let receipt = digest(&json!({"requestId":id,"target":expected,"resultDigest":result_digest}))?;
    ensure!(tx.execute("UPDATE conversation_tool_delivery SET state='claimed',receipt_digest=?2,updated_at=?3 WHERE request_id=?1 AND state='pending'",params![id,receipt,chrono::Utc::now().to_rfc3339()])?==1,"conversation_tools.delivery_changed");
    tx.commit()?;
    Ok(
        json!({"requestId":id,"state":"claimed","receiptDigest":receipt,"resultDigest":result_digest,"result":result(outcome,false)?}),
    )
}
/// 连接已flush后才提交written；未知只保留原记录，不能把新prompt当作补发。
pub(crate) fn receipt(
    connection: &mut Connection,
    lease: &RunLease,
    id: &str,
    expected: &NativeQuestionTarget,
    receipt_digest: &str,
    result_digest: &str,
    written: bool,
) -> Result<Value> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    require_target(&tx, id, lease, expected)?;
    let(state,original_receipt,original_result):(String,Option<String>,String)=tx.query_row("SELECT state,receipt_digest,result_digest FROM conversation_tool_delivery WHERE request_id=?1 AND bridge_instance_id=?2",params![id,expected.bridge_instance_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
    ensure!(
        original_receipt.as_deref() == Some(receipt_digest) && original_result == result_digest,
        "conversation_tools.delivery_receipt_changed"
    );
    ensure!(
        matches!(state.as_str(), "claimed" | "delivered" | "outcome_unknown"),
        "conversation_tools.delivery_not_claimed"
    );
    if state != "delivered" {
        let now = chrono::Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE conversation_tool_delivery SET state=?2,updated_at=?3 WHERE request_id=?1",
            params![
                id,
                if written {
                    "delivered"
                } else {
                    "outcome_unknown"
                },
                now
            ],
        )?;
        // 迟到的原连接回执仍是事实；停止/撤销不能抹掉此前已经写出的答案。
        tx.execute("UPDATE conversation_tool_request SET state=?2,revision=revision+1,updated_at=?3 WHERE id=?1 AND state IN ('answered','outcome_unknown')",params![id,if written{"applied"}else{"outcome_unknown"},now])?;
    }
    let status = requests::status(&tx, lease, id)?;
    tx.commit()?;
    Ok(status)
}
pub(crate) fn revoke(connection: &Connection, lease: &RunLease) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    connection.execute("UPDATE conversation_tool_run SET revoked_at=coalesce(revoked_at,?3) WHERE agent_run_id=?1 AND execution_epoch=?2",params![lease.run_id,lease.epoch,now])?;
    connection.execute("UPDATE conversation_tool_request SET state=CASE WHEN EXISTS(SELECT 1 FROM conversation_tool_delivery d WHERE d.request_id=conversation_tool_request.id AND d.state IN ('claimed','outcome_unknown')) THEN 'outcome_unknown' ELSE 'cancelled' END,revision=revision+1,updated_at=?3 WHERE agent_run_id=?1 AND execution_epoch=?2 AND state IN ('prepared','awaiting_answer','answered')",params![lease.run_id,lease.epoch,now])?;
    connection.execute("UPDATE conversation_tool_delivery SET state=CASE WHEN state='pending' THEN 'cancelled' ELSE 'outcome_unknown' END,updated_at=?3 WHERE agent_run_id=?1 AND execution_epoch=?2 AND state IN ('pending','claimed')",params![lease.run_id,lease.epoch,now])?;
    Ok(())
}
/// Core重启不接管原stdio调用；已创建的Mission/Automation及冻结事实保持原回执。
pub(crate) fn recover_after_restart(connection: &Connection) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    connection.execute(
        "UPDATE conversation_tool_run SET revoked_at=coalesce(revoked_at,?1)",
        [&now],
    )?;
    connection.execute("UPDATE conversation_tool_request SET state=CASE WHEN tool_name='ask_user_question' AND NOT EXISTS(SELECT 1 FROM conversation_tool_delivery d WHERE d.request_id=conversation_tool_request.id AND d.state IN ('claimed','outcome_unknown')) THEN 'cancelled' ELSE 'outcome_unknown' END,revision=revision+1,updated_at=?1 WHERE state IN ('prepared','awaiting_answer','answered')",[&now])?;
    connection.execute("UPDATE conversation_tool_delivery SET state=CASE WHEN state='pending' THEN 'cancelled' ELSE 'outcome_unknown' END,updated_at=?1 WHERE state IN ('pending','claimed')",[&now])?;
    Ok(())
}
