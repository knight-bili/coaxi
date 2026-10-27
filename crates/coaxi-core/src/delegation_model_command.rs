//! Model commands use Core-attested identity and the original command receipt transaction.
//! No process launch, directory observation or task-blob preparation belongs in this lock.
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    command::{
        ActorRef, CommandEnvelope, CommandExecution, CommandHandlerResult, DomainCommand,
        DomainCommandGateway, canonical_json_digest, sealed,
    },
    db::Database,
    delegation::FrozenRequest,
    delegation_admission::{ParentStartAdmission, WorkspaceObservation, authenticate_operation},
    delegation_contract::{CancelParams, ResumeParams, StartParams},
    delegation_factory::ChildRunPlan,
    team_tool::{AuthenticatedTeamToolRun, BuiltinToolBindingCredential},
};

// Deliberately no Debug/Serialize: the lease credential must never leave Core.
pub(crate) struct Invocation<'a, T> {
    pub binding: &'a BuiltinToolBindingCredential,
    pub source: &'a AuthenticatedTeamToolRun,
    pub tool_call: &'a str,
    pub input: &'a T,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Payload {
    binding_id: String,
    binding_generation: i64,
    binding_compatibility_digest: String,
    credential_digest: String,
    tool_call: String,
    input: Value,
}
#[derive(Serialize)]
#[serde(transparent)]
struct Start(Payload);
impl sealed::Sealed for Start {}
impl DomainCommand for Start {
    const TYPE: &'static str = "delegation.start";
    const EXACT_REPLAY: bool = true;
}
#[derive(Serialize)]
#[serde(transparent)]
struct Cancel(Payload);
impl sealed::Sealed for Cancel {}
impl DomainCommand for Cancel {
    const TYPE: &'static str = "delegation.cancel";
    const EXACT_REPLAY: bool = true;
}
#[derive(Serialize)]
#[serde(transparent)]
struct Resume(Payload);
impl sealed::Sealed for Resume {}
impl DomainCommand for Resume {
    const TYPE: &'static str = "delegation.resume";
    const EXACT_REPLAY: bool = true;
}

fn envelope<T: Serialize, C: DomainCommand>(
    connection: &Connection,
    invocation: &Invocation<'_, T>,
    wrap: impl FnOnce(Payload) -> C,
) -> Result<CommandEnvelope<C>> {
    let Invocation {
        binding,
        source,
        tool_call,
        input,
    } = invocation;
    ensure!(
        !tool_call.is_empty() && tool_call.len() <= 512,
        "delegation.invalid_tool_call"
    );
    ensure!(
        uuid::Uuid::parse_str(&binding.native_binding_id).is_ok()
            && binding.native_binding_generation > 0
            && !binding.binding_credential.is_empty()
            && source.execution_epoch > 0,
        "delegation.invalid_invocation"
    );
    // Immutable Run ownership, not today's member/default/Binding state. This
    // enables receipt-only recovery after stop, takeover or Binding replacement.
    let (camp, agent): (String,String) = connection.query_row(
        "SELECT c.camp_id,c.agent_id FROM agent_run r JOIN conversation c ON c.id=r.conversation_id
         WHERE r.id=?1", [&source.agent_run_id],
        |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional()?.context("delegation.source_not_found")?;
    ensure!(
        camp == source.camp_id && agent == source.agent_id,
        "delegation.source_scope_mismatch"
    );
    let identity = canonical_json_digest(&json!({"domain":"coaxi.delegation-command.v1",
        "operation":C::TYPE,"binding":binding.native_binding_id,"generation":binding.native_binding_generation,
        "run":source.agent_run_id,"epoch":source.execution_epoch,"toolCall":tool_call}))?;
    Ok(CommandEnvelope {
        command_id: format!("delegation-{identity}"),
        actor: ActorRef::Agent {
            agent_id: agent,
            source_agent_run_id: source.agent_run_id.clone(),
        },
        camp_id: Some(camp),
        expected_versions: Vec::new(),
        execution_epoch: Some(source.execution_epoch),
        payload: wrap(Payload {
            binding_id: binding.native_binding_id.clone(),
            binding_generation: binding.native_binding_generation,
            binding_compatibility_digest: binding.binding_compatibility_digest.clone(),
            credential_digest: canonical_json_digest(
                &json!({"credential":binding.binding_credential}),
            )?,
            tool_call: (*tool_call).into(),
            input: serde_json::to_value(input)?,
        }),
    })
}

#[derive(Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReceiptObservation {
    pub command_id: String,
    pub request_digest: String,
}
impl ReceiptObservation {
    pub(crate) fn valid(&self) -> bool {
        let hash =
            |value: &str| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
        self.command_id
            .strip_prefix("delegation-")
            .is_some_and(hash)
            && hash(&self.request_digest)
    }
}

// Observation only: the digest cannot authorize or dispatch a command.
pub(crate) fn receipt_observation<T: Serialize>(
    db: &Database,
    invocation: &Invocation<'_, T>,
    operation: &str,
) -> Result<ReceiptObservation> {
    fn key<C: DomainCommand>(envelope: CommandEnvelope<C>) -> Result<ReceiptObservation> {
        Ok(ReceiptObservation {
            request_digest: canonical_json_digest(&serde_json::to_value(&envelope)?)?,
            command_id: envelope.command_id,
        })
    }
    match operation {
        "delegation.start" => key(envelope(db.connection(), invocation, Start)?),
        "delegation.cancel" => key(envelope(db.connection(), invocation, Cancel)?),
        "delegation.resume" => key(envelope(db.connection(), invocation, Resume)?),
        _ => anyhow::bail!("delegation.invalid_receipt_operation"),
    }
}

fn execute_observed<C: DomainCommand>(
    db: &mut Database,
    command: &CommandEnvelope<C>,
    observation: Option<&str>,
    handler: impl FnOnce(&Transaction<'_>) -> Result<CommandHandlerResult>,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(db, command, |tx| {
        let result = handler(tx)?;
        if let Some(tool_call) = observation {
            crate::collaboration::append_domain_event(
                tx,
                "delegation.tool_receipt_confirmed",
                command.camp_id.as_deref(),
                None,
                &command.actor,
                command.execution_epoch,
                &json!({"toolCallId":tool_call,"commandId":command.command_id,
                    "requestDigest":canonical_json_digest(&serde_json::to_value(command)?)?}),
            )?;
        }
        Ok(result)
    })
}

pub(crate) fn replay_start(
    db: &Database,
    invocation: &Invocation<'_, StartParams>,
) -> Result<Option<CommandExecution>> {
    DomainCommandGateway.replay_if_recorded(db, &envelope(db.connection(), invocation, Start)?)
}

pub(crate) fn replay_resume(
    db: &Database,
    invocation: &Invocation<'_, ResumeParams>,
) -> Result<Option<CommandExecution>> {
    DomainCommandGateway.replay_if_recorded(db, &envelope(db.connection(), invocation, Resume)?)
}
pub(crate) fn replay_cancel(
    db: &Database,
    invocation: &Invocation<'_, CancelParams>,
) -> Result<Option<CommandExecution>> {
    DomainCommandGateway.replay_if_recorded(db, &envelope(db.connection(), invocation, Cancel)?)
}

pub(crate) fn resume(
    db: &mut Database,
    invocation: &Invocation<'_, ResumeParams>,
    prepared: &crate::delegation_resume::PreparedResume,
    directory: &crate::delegation_resume::DirectoryObservation,
) -> Result<CommandExecution> {
    resume_observed(db, invocation, prepared, directory, None)
}
pub(crate) fn resume_observed(
    db: &mut Database,
    invocation: &Invocation<'_, ResumeParams>,
    prepared: &crate::delegation_resume::PreparedResume,
    directory: &crate::delegation_resume::DirectoryObservation,
    observation: Option<&str>,
) -> Result<CommandExecution> {
    let command = envelope(db.connection(), invocation, Resume)?;
    execute_observed(db, &command, observation, |tx| {
        prepared.commit(
            tx,
            invocation,
            directory,
            &command.actor,
            &chrono::Utc::now().to_rfc3339(),
        )
    })
}

pub(crate) struct PreparedStart<'a> {
    pub admission: &'a ParentStartAdmission,
    pub observation: &'a WorkspaceObservation,
    pub request: &'a FrozenRequest<'a>,
    pub plan: &'a ChildRunPlan<'a>,
}

pub(crate) fn start(
    db: &mut Database,
    invocation: &Invocation<'_, StartParams>,
    prepared: PreparedStart<'_>,
) -> Result<CommandExecution> {
    start_inner(db, invocation, prepared, None, None)
}

pub(crate) fn start_with_candidate(
    db: &mut Database,
    invocation: &Invocation<'_, StartParams>,
    prepared: PreparedStart<'_>,
    candidate: &crate::managed_blob::PreparedBlobCandidate,
    observation: Option<&str>,
) -> Result<CommandExecution> {
    start_inner(db, invocation, prepared, Some(candidate), observation)
}

fn start_inner(
    db: &mut Database,
    invocation: &Invocation<'_, StartParams>,
    prepared: PreparedStart<'_>,
    candidate: Option<&crate::managed_blob::PreparedBlobCandidate>,
    observation: Option<&str>,
) -> Result<CommandExecution> {
    let command = envelope(db.connection(), invocation, Start)?;
    execute_observed(db, &command, observation, |tx| {
        ensure!(
            prepared.admission.matches_invocation(
                invocation.binding,
                invocation.source,
                invocation.tool_call
            ),
            "delegation.preparation_scope_mismatch"
        );
        ensure!(
            prepared.plan.task == invocation.input.task.as_str()
                && prepared.plan.target == &invocation.input.agent_type
                && prepared
                    .request
                    .working_dir
                    .map(|p| p.as_path().as_os_str())
                    == invocation
                        .input
                        .working_dir
                        .as_ref()
                        .map(|p| p.as_path().as_os_str()),
            "delegation.preparation_input_mismatch"
        );
        tx.execute_batch("SAVEPOINT delegation_model_start;")?;
        if let Some(candidate) = candidate {
            ensure!(
                candidate.id() == prepared.request.task_blob_id,
                "delegation.task_candidate_mismatch"
            );
            candidate.register(tx)?;
        }
        let at = chrono::Utc::now().to_rfc3339();
        let registered = if prepared.observation.requires_user_permission() {
            prepared
                .admission
                .register_initial_awaiting_workspace_permission(
                    tx,
                    prepared.request,
                    prepared.plan,
                    prepared.observation,
                    &at,
                )
        } else {
            prepared.admission.register_initial_within_parent(
                tx,
                prepared.request,
                prepared.plan,
                prepared.observation,
                &at,
            )
        };
        let registered = match registered {
            Ok(value) => value,
            Err(error) => {
                tx.execute_batch(
                    "ROLLBACK TO delegation_model_start; RELEASE delegation_model_start;",
                )?;
                // Storage faults roll back the entire command; never turn a
                // broken database into a durable business rejection.
                if error.chain().any(|cause| cause.is::<rusqlite::Error>()) {
                    return Err(error);
                }
                return Ok(CommandHandlerResult::rejected(
                    "delegation.start_rejected",
                    json!({"status":"failed","error_code":"delegation.start_rejected"}),
                ));
            }
        };
        if candidate.is_some() {
            crate::managed_blob::attach_gc_candidate(tx, prepared.request.task_blob_id)?;
        }
        tx.execute_batch("RELEASE delegation_model_start;")?;
        let report = task_report(
            tx,
            &registered.identity.id,
            prepared.admission.conversation_id(),
        )?;
        crate::collaboration::append_domain_event(
            tx,
            "delegation.started",
            command.camp_id.as_deref(),
            Some(("delegation", &registered.identity.id)),
            &command.actor,
            command.execution_epoch,
            &report,
        )?;
        Ok(CommandHandlerResult::accepted(
            "delegation.started",
            report,
            None,
        ))
    })
}

pub(crate) fn reject_start(
    db: &mut Database,
    invocation: &Invocation<'_, StartParams>,
    code: &str,
    observation: Option<&str>,
) -> Result<CommandExecution> {
    ensure!(
        code.starts_with("delegation.")
            && code
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_'),
        "delegation.invalid_rejection"
    );
    let command = envelope(db.connection(), invocation, Start)?;
    execute_observed(db, &command, observation, |tx| {
        authenticate_operation(
            tx,
            invocation.binding,
            invocation.source,
            invocation.tool_call,
            "delegation.status",
        )?;
        Ok(CommandHandlerResult::rejected(
            code,
            json!({"status":"failed","error_code":code}),
        ))
    })
}

pub(crate) fn cancel(
    db: &mut Database,
    invocation: &Invocation<'_, CancelParams>,
) -> Result<CommandExecution> {
    cancel_observed(db, invocation, None)
}
pub(crate) fn cancel_observed(
    db: &mut Database,
    invocation: &Invocation<'_, CancelParams>,
    observation: Option<&str>,
) -> Result<CommandExecution> {
    let command = envelope(db.connection(), invocation, Cancel)?;
    let mut stopped = Vec::new();
    let result = execute_observed(db, &command, observation, |tx| {
        let scope = authenticate_operation(
            tx,
            invocation.binding,
            invocation.source,
            invocation.tool_call,
            "delegation.cancel",
        )?;
        let id = invocation.input.task_id.as_str();
        let Some(_) = crate::delegation::find_for_parent(tx, id, &scope.conversation)? else {
            return Ok(CommandHandlerResult::applied(
                "delegation.unknown",
                unknown(id),
                None,
            ));
        };
        let control = crate::delegation::control(tx, id)?;
        let status: String = tx.query_row(
            "SELECT r.status FROM delegation_attempt a JOIN agent_run r ON r.id=a.agent_run_id
            WHERE a.delegation_id=?1 AND a.attempt_no=?2",
            params![id, control.current_attempt_no],
            |r| r.get(0),
        )?;
        if control.takeover_at.is_some()
            || !matches!(status.as_str(), "queued" | "running" | "waiting")
        {
            return Ok(CommandHandlerResult::applied(
                "delegation.already_terminal",
                task_report(tx, id, &scope.conversation)?,
                None,
            ));
        }
        stopped = crate::delegation_control::cancel_tree(
            tx,
            id,
            &command.actor,
            "parent_requested_delegation_stop",
            &chrono::Utc::now().to_rfc3339(),
        )?;
        let report = task_report(tx, id, &scope.conversation)?;
        crate::collaboration::append_domain_event(
            tx,
            "delegation.cancelled",
            command.camp_id.as_deref(),
            Some(("delegation", id)),
            &command.actor,
            command.execution_epoch,
            &report,
        )?;
        Ok(CommandHandlerResult::applied(
            "delegation.cancelled",
            report,
            None,
        ))
    })?;
    crate::runtime::pump_targets_after_runs_terminal(db, &stopped)?;
    Ok(result)
}

fn unknown(id: &str) -> Value {
    json!({"task_id":id,"status":"unknown"})
}

// Shared by inline reports, fixed-result paging and final-only history. A raw
// agent message or a late User result is never a delegated result by itself.
pub(crate) const FIXED_FINAL: &str = "r.status='succeeded' AND r.terminal_resolution_source='runtime_terminal'
    AND COALESCE(r.terminal_reason_code,'') NOT IN ('auth_required','authentication_required','rejected','refusal','max_tokens','unknown_stop')
    AND r.invocation_kind IN ('delegation','delegation_user') AND r.operation_policy='delegation_child_v1' AND r.operation_policy_version=1
    AND r.response_delivery='conversation_message' AND r.destination_conversation_id=m.conversation_id
    AND r.conversation_id=m.conversation_id AND r.final_conversation_message_id=m.id
    AND m.author_type='agent' AND m.agent_run_id=r.id AND m.source_agent_run_id=r.id";

pub(crate) struct FixedFinal {
    pub message: String,
    pub version: String,
    pub body: String,
}

pub(crate) fn fixed_final(
    connection: &Connection,
    run: &str,
    conversation: &str,
    through: i64,
) -> Result<Option<FixedFinal>> {
    let row:Option<(String,i64,String)>=connection.query_row(&format!(
        "SELECT m.id,m.sequence,m.body FROM agent_run r JOIN conversation_message m ON m.id=r.final_conversation_message_id
         WHERE r.id=?1 AND m.conversation_id=?2 AND m.sequence<=?3 AND {FIXED_FINAL}"),
        params![run,conversation,through],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    row.map(|(message, sequence, body)| {
        // conversation_message has no mutable numeric version column. Its
        // exact persisted content digest is the cursor's fixed result version.
        let version = crate::command::canonical_json_digest_with(|writer| {
            serde_json::to_writer(writer, &(&message, sequence, &body))?;
            Ok(())
        })?;
        Ok(FixedFinal {
            message,
            version,
            body,
        })
    })
    .transpose()
}

pub(crate) fn blocked_summary(
    connection: &rusqlite::Connection,
    run: &str,
    epoch: i64,
) -> Result<Option<Value>> {
    let permission:Option<String>=connection.query_row(
        "SELECT p.id FROM approval p JOIN action_execution x ON x.id=p.action_id
         WHERE x.agent_run_id=?1 AND x.source_agent_run_execution_epoch=?2 AND x.status='prepared' AND x.action_kind='runtime_permission_grant'
           AND p.status='pending' AND p.requested_for_user_id=?3 ORDER BY p.requested_at,p.id LIMIT 1",
        params![run,epoch,crate::current_user::CURRENT_USER_ID],|r|r.get(0)).optional()?;
    if let Some(permission) = permission {
        return Ok(Some(
            json!({"kind":"permission","request_id":permission,"title":"需要用户授权"}),
        ));
    }
    crate::runtime_interaction::storage::blocked_summary(connection, run, epoch)
}

/// Only the current attempt's persisted final is a result. Never use the latest
/// message in the child, which may belong to a later User after takeover.
pub(crate) fn task_report(connection: &Connection, id: &str, parent: &str) -> Result<Value> {
    let Some(identity) = crate::delegation::find_for_parent(connection, id, parent)? else {
        return Ok(unknown(id));
    };
    let (run, status, runtime, epoch, started, ended, boundary): (
        String,
        String,
        String,
        i64,
        Option<String>,
        Option<String>,
        Option<i64>,
    ) = connection.query_row(
        "SELECT r.id,r.status,d.requested_runtime,r.execution_epoch,r.started_at,r.ended_at,
            c.takeover_message_sequence
         FROM delegation d JOIN delegation_control c ON c.delegation_id=d.id
         JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=c.current_attempt_no
         JOIN agent_run r ON r.id=a.agent_run_id WHERE d.id=?1",
        [id],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        },
    )?;
    let mut report = json!({"task_id":id,"child_conversation_id":identity.child_conversation_id,"agent_type":runtime});
    match status.as_str() {
        "queued" | "running" | "waiting" => {
            report["status"] = json!("running");
            if status == "queued" {
                report["message"] = json!("等待资源");
            }
            if let Some(blocked) = blocked_summary(connection, &run, epoch)? {
                report["message"] = json!(match blocked["kind"].as_str() {
                    Some("question") => "等待用户回答",
                    Some("plan_approval") => "等待用户确认计划",
                    _ => "等待用户授权",
                });
                report["blocked_on"] = blocked;
            }
        }
        "cancelled" => {
            report["status"] = json!("canceled");
            if crate::delegation_control::cleanup_pending(connection, id)? {
                report["cleanup_pending"] = json!(true);
            }
        }
        "succeeded" => {
            if let Some(final_result) = fixed_final(
                connection,
                &run,
                &identity.child_conversation_id,
                boundary.unwrap_or(i64::MAX),
            )? {
                let mut body = final_result.body;
                let truncated = body.len() > 256 * 1024;
                if truncated {
                    let mut end = 256 * 1024 - "…".len();
                    while !body.is_char_boundary(end) {
                        end -= 1;
                    }
                    body.truncate(end);
                    body.push('…');
                }
                report["status"] = json!("completed");
                report["text"] = json!(body);
                report["result_ref"] = json!({"conversation_id":identity.child_conversation_id,"message_id":final_result.message,"truncated":truncated});
            } else {
                report["status"] = json!("failed");
                report["error_code"] = json!("delegation.result_unavailable");
            }
        }
        _ => {
            report["status"] = json!("failed");
            report["error_code"] = json!("delegation.failed");
        }
    }
    if let (Some(start), Some(end)) = (started, ended) {
        if let (Ok(start), Ok(end)) = (
            chrono::DateTime::parse_from_rfc3339(&start),
            chrono::DateTime::parse_from_rfc3339(&end),
        ) {
            let ms = (end - start).num_milliseconds();
            if ms >= 0 {
                report["duration_ms"] = json!(ms);
            }
        }
    }
    Ok(report)
}
