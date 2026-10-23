//! Read-only receipt reconciliation. Never invokes a business command or Runtime.
use super::*;
use crate::{
    command::canonical_json_digest,
    execution_evidence::{AgentRunExecutionEvidence, ExecutionEvidenceService},
};
use rusqlite::{OptionalExtension, params};
use serde_json::json;

pub(crate) struct Cursor {
    pending: std::collections::VecDeque<String>,
}
pub(crate) struct Batch {
    pub updated: Vec<AgentRunExecutionEvidence>,
    pub failures: usize,
}
impl Cursor {
    // Capture exact IDs before readiness. SQLite rowids can be reused after a
    // deletion, so a rowid watermark cannot safely exclude later invocations.
    pub(crate) fn capture(db: &Database) -> Result<Self> {
        Ok(Self {
            pending: db.connection().prepare(
                "SELECT id FROM agent_run_execution_evidence
                 WHERE phase IN ('started','updated','progress')
                 AND json_extract(payload_preview_json,'$.sourceAuthority')='core'
                 AND json_extract(payload_preview_json,'$.kind')='builtin_tool_invocation'
                 AND json_extract(payload_preview_json,'$.canonicalTool') IN
                     ('delegation.start','delegation.cancel','delegation.resume','delegation.status','delegation.read','delegation.history')
                 ORDER BY rowid"
            )?.query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<_>>()?,
        })
    }
    pub(crate) fn done(&self) -> bool {
        self.pending.is_empty()
    }
    pub(crate) fn next(&mut self, db: &mut Database, directory: &Path) -> Result<Batch> {
        let mut batch = Batch {
            updated: Vec::new(),
            failures: 0,
        };
        for _ in 0..64 {
            let Some(id) = self.pending.pop_front() else {
                break;
            };
            match reconcile(db, directory, &id) {
                Ok(Some(evidence)) => batch.updated.push(evidence),
                Ok(None) => {}
                Err(_error) => {
                    #[cfg(test)]
                    eprintln!("Fixture reconciliation diagnostic: {_error:#}");
                    batch.failures += 1;
                    eprintln!("Core delegation observation reconciliation deferred: {id}");
                }
            }
        }
        Ok(batch)
    }
}

/// User views may read a terminal observation whose raw request claim was
/// intentionally removed from the public payload. Recover only the claim
/// already committed with this exact Core observation, never a caller's id.
pub(crate) fn source_receipt(
    db: &Database,
    evidence_id: &str,
    payload: &Value,
) -> Result<Option<Outcome>> {
    let claims = db.connection().prepare(
        "SELECT json_extract(link.payload_json,'$.commandId'),json_extract(link.payload_json,'$.requestDigest')
         FROM agent_run_execution_evidence e JOIN agent_run r ON r.id=e.agent_run_id
         JOIN conversation c ON c.id=r.conversation_id
         JOIN event_log link ON link.event_type='delegation.tool_receipt_confirmed'
         WHERE e.id=?1 AND link.actor_type='agent' AND link.actor_id=c.agent_id
           AND link.source_agent_run_id=e.agent_run_id AND link.execution_epoch=e.execution_epoch
           AND link.camp_id=c.camp_id AND json_extract(link.payload_json,'$.toolCallId')=?2
         ORDER BY link.rowid LIMIT 2",
    )?.query_map(params![evidence_id,payload["toolCallId"].as_str()], |row|
        Ok(model::ReceiptObservation { command_id: row.get(0)?, request_digest: row.get(1)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let [claim] = claims.as_slice() else {
        return Ok(None);
    };
    let mut observed = payload.clone();
    observed["coreCommandReceipt"] = serde_json::to_value(claim)?;
    confirmed(db, evidence_id, &observed)
}

fn confirmed(db: &Database, evidence_id: &str, payload: &Value) -> Result<Option<Outcome>> {
    let Some(proof) = payload
        .get("coreCommandReceipt")
        .cloned()
        .and_then(|value| serde_json::from_value::<model::ReceiptObservation>(value).ok())
    else {
        return Ok(None);
    };
    if !proof.valid() {
        return Ok(None);
    }
    let operation = payload["canonicalTool"]
        .as_str()
        .context("Missing canonical operation")?;
    // Both records commit atomically. Matching a later retry's receipt alone
    // cannot prove this particular attempt executed it.
    let matched: bool = db.connection().query_row(
        "SELECT EXISTS(SELECT 1 FROM agent_run_execution_evidence e
         JOIN agent_run r ON r.id=e.agent_run_id JOIN conversation c ON c.id=r.conversation_id
         JOIN event_log receipt ON receipt.event_type='command.result' AND receipt.command_id=?2
         JOIN event_log link ON link.event_type='delegation.tool_receipt_confirmed'
         WHERE e.id=?1 AND receipt.command_type=?3 AND receipt.actor_type='agent'
         AND receipt.actor_id=c.agent_id AND receipt.source_agent_run_id=e.agent_run_id
         AND receipt.execution_epoch=e.execution_epoch AND receipt.camp_id=c.camp_id
         AND receipt.request_digest_version=1 AND receipt.request_digest=?4
         AND link.actor_type='agent' AND link.actor_id=receipt.actor_id
         AND link.source_agent_run_id=e.agent_run_id AND link.execution_epoch=e.execution_epoch
         AND link.camp_id=c.camp_id
         AND json_extract(link.payload_json,'$.toolCallId')=?5
         AND json_extract(link.payload_json,'$.commandId')=receipt.command_id
         AND json_extract(link.payload_json,'$.requestDigest')=receipt.request_digest)",
        params![
            evidence_id,
            proof.command_id,
            operation,
            proof.request_digest,
            payload["toolCallId"].as_str()
        ],
        |r| r.get(0),
    )?;
    if !matched {
        return Ok(None);
    }
    let result = crate::command::load_stored_result(db.connection(), &proof.command_id)?
        .context("Confirmed command receipt disappeared")?;
    let operation =
        contract::canonical_operation(operation).context("Unknown recovered operation")?;
    Ok(Some(Outcome::command(
        operation,
        crate::command::CommandExecution {
            result,
            replayed: false,
        },
    )))
}

fn reconcile(
    db: &mut Database,
    directory: &Path,
    id: &str,
) -> Result<Option<AgentRunExecutionEvidence>> {
    use crate::builtin_tool_transport::{
        BuiltinToolError, BuiltinToolInvocationEnvelope, BuiltinToolRecovery,
    };
    let store = ManagedBlobStore::new(directory);
    let phase: Option<String> = db
        .connection()
        .query_row(
            "SELECT phase FROM agent_run_execution_evidence WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    if !matches!(phase.as_deref(), Some("started" | "updated" | "progress")) {
        return Ok(None);
    }
    let mut payload = ExecutionEvidenceService.read_full_payload_unscoped(db, &store, id)?;
    let operation = payload["canonicalTool"]
        .as_str()
        .context("Missing operation")?
        .to_owned();
    let request = payload["coreRequestId"]
        .as_str()
        .context("Missing request identity")?
        .to_owned();
    let outcome = confirmed(db, id, &payload)?;
    let envelope = if let Some(outcome) = outcome {
        crate::delegation_catalog::validate_output(&operation, &outcome.payload)?;
        let digest = canonical_json_digest(&outcome.payload)?;
        let projection = crate::builtin_tool_evidence_projection::complete_saved_invocation(
            &operation,
            &payload["operationProjection"],
            &outcome.payload,
        )?;
        ensure!(
            projection["inputDigest"] == payload["rawInputDigest"],
            "Recovered input digest changed"
        );
        payload["status"] = json!("completed");
        payload["rawOutputDigest"] = json!(digest);
        payload["operationProjection"] = projection;
        payload["receiptId"] = json!(outcome.receipt_id);
        payload["errorCode"] = Value::Null;
        payload["coreReconciliation"] = json!("receipt_confirmed");
        BuiltinToolInvocationEnvelope::success(&operation, &request, outcome.payload)?
    } else {
        payload["status"] = json!("failed");
        payload["errorCode"] = json!("builtin_tool.result_unconfirmed");
        payload["coreReconciliation"] = json!("result_unconfirmed");
        BuiltinToolInvocationEnvelope::rejected(&operation, &request, BuiltinToolError {
            code: "builtin_tool.result_unconfirmed".into(),
            message: "Core restarted before this invocation result could be confirmed; inspect the original task before retrying".into(),
            recovery: BuiltinToolRecovery::ConfirmOutcome, details: None,
        })?
    };
    payload["coreEnvelope"] = serde_json::to_value(envelope)?;
    Ok(ExecutionEvidenceService
        .record_recovered_builtin_result(db, &store, id, &payload)?
        .map(|recorded| recorded.into_evidence()))
}
