//! Core observations reuse the original execution lifecycle store.
use super::*;
use crate::{command::canonical_json_digest, execution_evidence::ExecutionEvidenceService};
use serde_json::json;

// No credentials, private directory or task copy; never model authority.
pub(super) struct Observation {
    source: AuthenticatedTeamToolRun,
    operation: &'static str,
    request_id: String,
    tool_call_id: String,
    input_digest: String,
    projection: Value,
}
impl Observation {
    pub(super) fn prepare(
        active: &ActiveInvocation<'_>,
        operation: &'static str,
        request_id: &str,
        input: &Value,
    ) -> Result<Self> {
        // Gateway alone owns mutation identity/replay. Calls reaching here are
        // fresh attempts: pre-commit failures must not poison a later retry's
        // input/result. Recorded mutations returned earlier with zero writes.
        let identity = json!({"runtimeToolCallId":active.tool_call,"tool":operation,
            "binding":active.authorized.native_binding.native_binding_id,
            "generation":active.authorized.native_binding.native_binding_generation,
            "epoch":active.source.execution_epoch,"observation":uuid::Uuid::new_v4().to_string()});
        let input_digest = canonical_json_digest(input)?;
        let projection = crate::builtin_tool_evidence_projection::project_builtin_tool_invocation(
            operation,
            input,
            None,
            &input_digest,
            None,
        )?;
        Ok(Self {
            source: active.source.clone(),
            operation,
            request_id: request_id.into(),
            tool_call_id: canonical_json_digest(&identity)?,
            input_digest,
            projection,
        })
    }
    pub(super) fn start(&self, db: &mut Database, directory: &Path) -> Result<()> {
        let payload = json!({"toolCallId":self.tool_call_id,"coreRequestId":self.request_id,"status":"started","kind":"builtin_tool_invocation",
            "title":self.operation,"sourceAuthority":"core","canonicalTool":self.operation,"authorizationDecision":"allowed",
            "rawInputDigest":self.input_digest,"rawOutputDigest":null,"errorCode":null,"idempotentReplay":false,"receiptId":null,"operationProjection":self.projection});
        ExecutionEvidenceService
            .record_builtin_tool_started(
                db,
                &ManagedBlobStore::new(directory),
                &self.source.agent_run_id,
                self.source.execution_epoch,
                &payload,
            )?
            .context("builtin_tool.run_not_bound")?;
        Ok(())
    }
    pub(super) fn finish(
        &self,
        db: &mut Database,
        directory: &Path,
        input: &Value,
        result: &Result<Outcome>,
    ) -> Result<()> {
        let envelope = operation_envelope(self.operation, &self.request_id, result)?;
        let raw_result = envelope.result.as_ref();
        let output_digest = raw_result.map(canonical_json_digest).transpose()?;
        let projection = crate::builtin_tool_evidence_projection::project_builtin_tool_invocation(
            self.operation,
            input,
            raw_result,
            &self.input_digest,
            output_digest.as_deref(),
        )?;
        let payload = json!({"toolCallId":self.tool_call_id,"coreRequestId":self.request_id,"status":if envelope.ok {"completed"} else {"failed"},"kind":"builtin_tool_invocation",
            "title":self.operation,"sourceAuthority":"core","canonicalTool":self.operation,"authorizationDecision":"allowed",
            "rawInputDigest":self.input_digest,"rawOutputDigest":output_digest,"errorCode":envelope.error.as_ref().map(|e|&e.code),
            "idempotentReplay":result.as_ref().is_ok_and(|outcome|outcome.replayed),"receiptId":result.as_ref().ok().and_then(|outcome|outcome.receipt_id.as_deref()),"operationProjection":projection,"coreEnvelope":envelope});
        // Original terminal-only evidence admission may settle this already
        // started row after source stop. It cannot dispatch any work.
        ExecutionEvidenceService
            .record_builtin_tool_result(
                db,
                &ManagedBlobStore::new(directory),
                &self.source.agent_run_id,
                self.source.execution_epoch,
                &payload,
            )?
            .context("delegation.activity_completion_not_admitted")?;
        Ok(())
    }
}
