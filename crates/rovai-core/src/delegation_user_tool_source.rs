//! User-only parent tool presentation. Stored receipts and attempts, never the
//! runtime's task id or display name, select the original child and task.
use super::*;
use crate::{execution_evidence::ExecutionEvidenceService, managed_blob::ManagedBlobStore};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Params {
    thread_id: crate::delegation_contract::NonEmpty,
    evidence_id: crate::delegation_contract::NonEmpty,
}

pub(crate) fn read(
    db: &Database,
    directory: &Path,
    actor: &ActorRef,
    input: &Params,
) -> Result<Value> {
    reader(actor)?;
    require_installed(db.connection())?;
    let evidence_id = input.evidence_id.as_str();
    let thread = input.thread_id.as_str();
    let (run, epoch, parent): (String, i64, String) = db
        .connection()
        .query_row(
            "SELECT e.agent_run_id,e.execution_epoch,c.id FROM agent_run_execution_evidence e
         JOIN agent_run r ON r.id=e.agent_run_id JOIN conversation c ON c.id=r.conversation_id
         JOIN camp ON camp.id=c.camp_id
         WHERE e.id=?1 AND c.camp_id=?2 AND camp.deletion_operation_id IS NULL",
            params![evidence_id, thread],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .context("delegation.source_unavailable")?;
    let source = json!({"threadId":thread,"evidenceId":evidence_id,"agentRunId":run,
        "executionEpoch":epoch,"parentConversationId":parent});
    let unresolved = || json!({"state":"unresolved","source":source});
    let payload = ExecutionEvidenceService
        .read_full_payload(db, &ManagedBlobStore::new(directory), thread, evidence_id)
        .map_err(|_| anyhow::anyhow!("delegation.source_unavailable"))?;
    let operation = payload["canonicalTool"].as_str().unwrap_or("");
    if payload["sourceAuthority"] != "core"
        || !matches!(operation, "delegation.start" | "delegation.resume")
    {
        return Ok(unresolved());
    }
    // The Core observation number must identify this one stored row. A copied
    // payload or duplicate observation cannot re-label an unrelated native row.
    let unique: bool = db.connection().query_row(
        "SELECT count(*)=1 AND min(id)=?1 FROM agent_run_execution_evidence
         WHERE agent_run_id=?2 AND execution_epoch=?3
           AND json_extract(payload_preview_json,'$.sourceAuthority')='core'
           AND json_extract(payload_preview_json,'$.canonicalTool')=?4
           AND json_extract(payload_preview_json,'$.toolCallId')=?5",
        params![
            evidence_id,
            run,
            epoch,
            operation,
            payload["toolCallId"].as_str()
        ],
        |row| row.get(0),
    )?;
    if !unique {
        return Ok(unresolved());
    }
    let Some(receipt) =
        crate::delegation_dispatch::recovery::source_receipt(db, evidence_id, &payload)?
    else {
        return Ok(unresolved());
    };
    ensure!(
        receipt.operation == operation,
        "delegation.source_unavailable"
    );
    let Some(request) = payload["coreRequestId"]
        .as_str()
        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
    else {
        return Ok(unresolved());
    };
    let call = format!("agent-run:{run}:builtin-cli:{request}");
    let kind = if operation == "delegation.start" {
        "initial"
    } else {
        "resume"
    };
    let binding: Option<(String, i64, Option<String>)> = db
        .connection()
        .query_row(
            "SELECT a.delegation_id,a.attempt_no,a.interruption_reason FROM delegation_attempt a
         JOIN delegation d ON d.id=a.delegation_id
         WHERE a.trigger_parent_run_id=?1 AND a.trigger_parent_epoch=?2
           AND a.trigger_tool_call_id=?3 AND a.kind=?4
           AND d.camp_id=?5 AND d.parent_conversation_id=?6",
            params![run, epoch, call, kind, thread, parent],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((task_id, attempt_no, reason)) = binding else {
        // A confirmed refusal is not a revived child, even when its result
        // mentions a real task. Do not enrich it from that other task's data.
        if receipt.payload["error_code"].is_string() || receipt.payload["status"] == "unknown" {
            return Ok(
                json!({"state":"refused","source":source,"operation":operation,
                "code":receipt.payload["error_code"].as_str().unwrap_or("delegation.unknown")}),
            );
        }
        return Ok(unresolved());
    };
    let id = identity(db.connection(), &task_id)?.context("delegation.source_unavailable")?;
    ensure!(
        id.camp_id == thread && id.parent_conversation_id == parent,
        "delegation.source_unavailable"
    );
    let (digest, task): (String, String) = db.connection().query_row(
        "SELECT b.sha256,m.body FROM delegation d JOIN managed_blob b ON b.id=d.task_blob_id
         JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=1
         JOIN agent_run r ON r.id=a.agent_run_id
         JOIN conversation_message m ON m.id=r.trigger_conversation_message_id
           AND m.conversation_id=d.child_conversation_id
         WHERE d.id=?1 AND m.author_type='system' AND m.author_id='delegation-parent'",
        [&task_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    ensure!(
        format!("{:x}", Sha256::digest(task.as_bytes())) == digest,
        "delegation.task_changed"
    );
    Ok(
        json!({"state":"bound","source":source,"operation":operation,"attemptNo":attempt_no,
        "detail":detail(db.connection(),&id)?,"task":task,"reason":reason}),
    )
}
