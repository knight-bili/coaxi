//! Same-task continuation. Core owns eligibility, the next attempt and exact
//! Session fences; no original task, accepted input or external action is replayed.
use crate::{
    agent_profile::FrozenAgentRuntimeConfig,
    command::{ActorRef, CommandHandlerResult, canonical_json_digest},
    db::Database,
    delegation::Identity,
    delegation_contract::ResumeParams,
    delegation_model_command::Invocation,
    runtime::AgentRunWorkspace,
};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExpectedSession {
    version: u8,
    delegation: String,
    attempt: i64,
    previous_run: String,
    previous_epoch: i64,
    delivery: String,
    session: String,
    binding: String,
    generation: i64,
    session_key: Option<String>,
    installation: String,
    previous_digest: String,
    runtime_digest: String,
}
struct Plan {
    identity: Identity,
    attempt: i64,
    control_version: i64,
    conversation_version: i64,
    sequence: i64,
    config: Value,
    runtime: FrozenAgentRuntimeConfig,
    workspace: AgentRunWorkspace,
    expected: ExpectedSession,
    fingerprint: String,
}
enum Preparation {
    Unknown,
    Denied(String),
    Ready(Box<Plan>),
}
pub(crate) struct PreparedResume {
    invocation: String,
    parent: String,
    task: String,
    state: Preparation,
}
pub(crate) struct DirectoryObservation {
    invocation: String,
    plan: Option<String>,
    error: Option<&'static str>,
}

fn invocation_digest(inv: &Invocation<'_, ResumeParams>) -> Result<String> {
    canonical_json_digest(
        &json!({"binding":inv.binding.native_binding_id,"generation":inv.binding.native_binding_generation,
        "compatibility":inv.binding.binding_compatibility_digest,"credential":inv.binding.binding_credential,
        "run":inv.source.agent_run_id,"epoch":inv.source.execution_epoch,"thread":inv.source.camp_id,
        "agent":inv.source.agent_id,"call":inv.tool_call,"input":inv.input}),
    )
}
fn reject(task: &str, code: &str) -> CommandHandlerResult {
    CommandHandlerResult::rejected(
        code,
        json!({"task_id":task,"status":"failed","error_code":code}),
    )
}
fn safe_code(error: &anyhow::Error) -> String {
    let code = error.to_string();
    if code.starts_with("delegation.")
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_')
    {
        code
    } else {
        "delegation.not_resumable".into()
    }
}
pub(crate) fn prepare(db: &Database, inv: &Invocation<'_, ResumeParams>) -> Result<PreparedResume> {
    prepare_on(db.connection(), inv)
}
fn prepare_on(
    connection: &Connection,
    inv: &Invocation<'_, ResumeParams>,
) -> Result<PreparedResume> {
    // Reader admission can safely record a refusal after global disable. Only
    // the Ready branch below admits new work and rechecks both write policies.
    let parent = crate::delegation_admission::authenticate_operation(
        connection,
        inv.binding,
        inv.source,
        inv.tool_call,
        "delegation.status",
    )?;
    let task = inv.input.task_id.as_str();
    let state = match crate::delegation::find_for_parent(connection, task, &parent.conversation)? {
        None => Preparation::Unknown,
        Some(identity) => match load_plan(connection, identity, &parent) {
            Ok(plan) => Preparation::Ready(Box::new(plan)),
            Err(error) => {
                if error.chain().any(|cause| cause.is::<rusqlite::Error>()) {
                    return Err(error);
                }
                Preparation::Denied(safe_code(&error))
            }
        },
    };
    Ok(PreparedResume {
        invocation: invocation_digest(inv)?,
        parent: parent.conversation,
        task: task.into(),
        state,
    })
}
fn load_plan(
    connection: &Connection,
    identity: Identity,
    parent: &crate::delegation_admission::OperationAdmission,
) -> Result<Plan> {
    ensure!(
        parent
            .policy
            .allows("delegation.resume", parent.settings.enabled),
        "delegation.start_disabled"
    );
    ensure!(
        identity.depth <= i64::from(parent.policy.depth_limit.min(parent.settings.depth_limit)),
        "delegation.depth_limit"
    );
    let (attempt,control_version,taken,conversation_version,sequence,ended,session,binding,generation,installation,session_key,digest,run,epoch,status,ack,recovery,config,workspace):
        (i64,i64,bool,i64,i64,bool,Option<String>,Option<String>,i64,Option<String>,Option<String>,Option<String>,String,i64,String,bool,bool,String,String)=connection.query_row(
        "SELECT control.current_attempt_no,control.control_version,control.takeover_at IS NOT NULL,c.version,c.last_message_sequence,
           c.ended_at IS NOT NULL,c.native_session_id,c.native_binding_id,c.native_binding_generation,c.native_adapter_installation_id,
           c.native_session_compatibility_key,c.native_binding_compatibility_digest,r.id,r.execution_epoch,r.status,
           r.cancel_acknowledged_at IS NOT NULL,r.runtime_recovery_required<>0,r.effective_config_json,r.workspace_json
         FROM delegation_control control JOIN delegation d ON d.id=control.delegation_id JOIN conversation c ON c.id=d.child_conversation_id
         JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=control.current_attempt_no
         JOIN agent_run r ON r.id=a.agent_run_id WHERE d.id=?1",[&identity.id],|r|
        Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?,r.get(11)?,r.get(12)?,r.get(13)?,r.get(14)?,r.get(15)?,r.get(16)?,r.get(17)?,r.get(18)?)))?;
    ensure!(
        !taken && !ended && status == "cancelled",
        "delegation.not_resumable"
    );
    ensure!(
        ack && !recovery && !crate::delegation_control::cleanup_pending(connection, &identity.id)?,
        "delegation.cleanup_pending"
    );
    ensure!(
        crate::delegation_workspace_permission::covers_run(connection, &run)?,
        "delegation.workspace_permission_required"
    );
    let config: Value = serde_json::from_str(&config)?;
    let mut unsigned = config.clone();
    let saved = unsigned
        .as_object_mut()
        .context("delegation.invalid_config")?
        .remove("configDigest")
        .context("delegation.invalid_config")?;
    ensure!(
        saved.as_str() == Some(canonical_json_digest(&unsigned)?.as_str()),
        "delegation.invalid_config"
    );
    let policy = crate::delegation_policy::FrozenPolicy::from_config(&config)?
        .context("delegation.frozen_policy_required")?;
    ensure!(
        policy.role == crate::delegation_policy::Role::Child,
        "delegation.not_resumable"
    );
    policy.validate_invocation("delegation")?;
    let member:i64=connection.query_row("SELECT version FROM camp_member WHERE camp_id=?1 AND agent_id=?2 AND status='active' AND leave_requested_at IS NULL",params![identity.camp_id,identity.owner_agent_id],|r|r.get(0))?;
    ensure!(
        config["campMemberVersion"].as_i64() == Some(member),
        "delegation.member_changed"
    );
    let frozen: FrozenAgentRuntimeConfig = serde_json::from_value(config["runtime"].clone())?;
    crate::runtime::validate_saved_frozen_runtime(connection, &run, &frozen)?;
    let bare = crate::agent_profile::resolve_rebound_runtime_on_connection(connection, &frozen)?
        .map_err(|_| anyhow::anyhow!("delegation.runtime_unavailable"))?;
    ensure!(
        crate::runtime::runtime_logical_identity_matches(&frozen, &bare)
            && frozen.custom_api == bare.custom_api,
        "delegation.session_incompatible"
    );
    // Registry re-resolution is provisional: it deliberately has no native
    // Session evidence. Retain the original frozen execution basis; the normal
    // dispatch preflight may rebind it only under require_rebound_runtime below.
    let runtime = frozen.clone();
    let session = session
        .filter(|s| !s.is_empty())
        .context("delegation.not_resumable")?;
    let binding = binding.context("delegation.not_resumable")?;
    let installation = installation.context("delegation.not_resumable")?;
    let previous_digest = digest.context("delegation.not_resumable")?;
    ensure!(
        generation > 0
            && runtime.adapter_kind == frozen.adapter_kind
            && runtime.installation_id == frozen.installation_id
            && runtime.installation_id == installation
            && runtime.auth_scope == frozen.auth_scope
            && runtime.model == frozen.model
            && runtime.permissions == frozen.permissions
            && runtime.native_session_compatibility_key == session_key
            && crate::runtime::native_session_adapter_kind(
                connection,
                &identity.child_conversation_id
            )? == Some(runtime.adapter_kind),
        "delegation.session_incompatible"
    );
    // Some original adapters (including Codex) persist a nullable compatibility
    // key. Their exact Session ID + adapter + accepted input is the saved basis;
    // the actual native load must still succeed, with cold fallback forbidden.
    let (delivery,request_digest):(String,String)=connection.query_row(
        "SELECT input.id,COALESCE(input.runtime_request_digest,'') FROM runtime_input_delivery input JOIN context_manifest m ON m.id=input.context_manifest_id
         WHERE input.agent_run_id=?1 AND input.execution_epoch=?2 AND input.status='accepted'
           AND input.native_binding_id=?3 AND input.native_binding_generation=?4 AND length(input.native_input_id)>0
           AND m.agent_run_id=input.agent_run_id
           AND m.native_binding_generation=input.native_binding_generation",params![run,epoch,binding,generation],|r|Ok((r.get(0)?,r.get(1)?))).optional()?.context("delegation.input_not_reconciled")?;
    ensure!(
        !request_digest.is_empty(),
        "delegation.input_not_reconciled"
    );
    let unsafe_state:bool=connection.query_row("SELECT
        EXISTS(SELECT 1 FROM agent_run r WHERE r.conversation_id=?1 AND (r.status IN ('queued','running','waiting') OR r.runtime_recovery_required<>0 OR (r.cancel_requested_at IS NOT NULL AND r.cancel_acknowledged_at IS NULL)))
        OR EXISTS(SELECT 1 FROM runtime_input_delivery input JOIN agent_run r ON r.id=input.agent_run_id WHERE r.conversation_id=?1 AND input.status IN ('prepared','delivery_unknown'))
        OR EXISTS(SELECT 1 FROM action_execution x JOIN agent_run r ON r.id=x.agent_run_id WHERE r.conversation_id=?1 AND (x.status IN ('prepared','executing') OR (x.status='unknown' AND x.unknown_disposition='active')))
        OR EXISTS(SELECT 1 FROM approval p JOIN action_execution x ON x.id=p.action_id JOIN agent_run r ON r.id=x.agent_run_id WHERE r.conversation_id=?1 AND p.status='pending')
        OR EXISTS(SELECT 1 FROM runtime_delivery_checkpoint checkpoint JOIN agent_run r ON r.id=checkpoint.agent_run_id WHERE r.conversation_id=?1 AND checkpoint.status IN ('pending','delivering','failed'))
        OR EXISTS(SELECT 1 FROM agent_run other JOIN conversation c ON c.id=other.conversation_id WHERE other.conversation_id<>?1 AND c.native_session_id=?2
           AND other.runtime_adapter_kind=?3 AND other.runtime_auth_scope=?4 AND (other.status IN ('queued','running','waiting') OR other.runtime_recovery_required<>0 OR (other.cancel_requested_at IS NOT NULL AND other.cancel_acknowledged_at IS NULL)))",
        params![identity.child_conversation_id,session,runtime.adapter_kind.as_str(),runtime.auth_scope],|r|r.get(0))?;
    ensure!(!unsafe_state, "delegation.execution_unsettled");
    let (task_sha,original):(String,String)=connection.query_row("SELECT b.sha256,m.body FROM delegation d JOIN managed_blob b ON b.id=d.task_blob_id
        JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=1 JOIN agent_run r ON r.id=a.agent_run_id
        JOIN conversation_message m ON m.id=r.trigger_conversation_message_id AND m.conversation_id=d.child_conversation_id
        WHERE d.id=?1 AND m.author_type='system' AND m.author_id='delegation-parent'",[&identity.id],|r|Ok((r.get(0)?,r.get(1)?)))?;
    ensure!(
        format!("{:x}", Sha256::digest(original.as_bytes())) == task_sha,
        "delegation.task_changed"
    );
    let workspace: AgentRunWorkspace = serde_json::from_str(&workspace)?;
    workspace.validate()?;
    let expected_workspace: String = connection.query_row(
        "SELECT resolved_workspace_json FROM delegation WHERE id=?1",
        [&identity.id],
        |r| r.get(0),
    )?;
    ensure!(
        serde_json::from_str::<Value>(&expected_workspace)? == serde_json::to_value(&workspace)?,
        "delegation.workspace_changed"
    );
    let next = attempt.checked_add(1).context("delegation.not_resumable")?;
    let expected = ExpectedSession {
        version: 1,
        delegation: identity.id.clone(),
        attempt: next,
        previous_run: run,
        previous_epoch: epoch,
        delivery,
        session,
        binding,
        generation,
        session_key,
        installation,
        previous_digest,
        runtime_digest: runtime.binding_compatibility_digest.clone(),
    };
    let fingerprint = canonical_json_digest(
        &json!({"expected":expected,"control":control_version,"conversation":conversation_version,
        "sequence":sequence,"config":config,"runtime":runtime,"workspace":workspace,"task":task_sha,"request":request_digest}),
    )?;
    Ok(Plan {
        identity,
        attempt: next,
        control_version,
        conversation_version,
        sequence,
        config,
        runtime,
        workspace,
        expected,
        fingerprint,
    })
}

impl PreparedResume {
    pub(crate) fn observe_directory(&self) -> DirectoryObservation {
        let (plan, error) = match &self.state {
            Preparation::Ready(plan) => {
                let root = std::path::Path::new(&plan.workspace.execution_root);
                let valid = std::fs::canonicalize(root)
                    .is_ok_and(|actual| actual.is_dir() && actual == root);
                (
                    Some(plan.fingerprint.clone()),
                    (!valid).then_some("delegation.directory_unavailable"),
                )
            }
            _ => (None, None),
        };
        DirectoryObservation {
            invocation: self.invocation.clone(),
            plan,
            error,
        }
    }
    pub(crate) fn commit(
        &self,
        tx: &Transaction<'_>,
        inv: &Invocation<'_, ResumeParams>,
        observation: &DirectoryObservation,
        actor: &ActorRef,
        at: &str,
    ) -> Result<CommandHandlerResult> {
        ensure!(
            self.invocation == invocation_digest(inv)? && self.invocation == observation.invocation,
            "delegation.preparation_scope_mismatch"
        );
        let current = prepare_on(tx, inv)?;
        ensure!(
            current.parent == self.parent,
            "delegation.preparation_scope_mismatch"
        );
        match &self.state {
            Preparation::Unknown => {
                return Ok(CommandHandlerResult::applied(
                    "delegation.unknown",
                    json!({"task_id":self.task,"status":"unknown"}),
                    None,
                ));
            }
            Preparation::Denied(code) => return Ok(reject(&self.task, code)),
            Preparation::Ready(_) => {}
        }
        let Preparation::Ready(plan) = &current.state else {
            let code = match &current.state {
                Preparation::Denied(code) => code.as_str(),
                _ => "delegation.not_resumable",
            };
            return Ok(reject(&self.task, code));
        };
        let Preparation::Ready(original) = &self.state else {
            unreachable!()
        };
        ensure!(
            observation.plan.as_deref() == Some(original.fingerprint.as_str()),
            "delegation.preparation_scope_mismatch"
        );
        if plan.fingerprint != original.fingerprint {
            return Ok(reject(&self.task, "delegation.resume_state_changed"));
        }
        if let Some(code) = observation.error {
            return Ok(reject(&self.task, code));
        }
        tx.execute_batch("SAVEPOINT delegation_resume;")?;
        let result = (|| -> Result<CommandHandlerResult> {
            let next_sequence = plan
                .sequence
                .checked_add(1)
                .context("delegation.not_resumable")?;
            ensure!(tx.execute("UPDATE delegation_control SET current_attempt_no=?2,control_version=control_version+1 WHERE delegation_id=?1 AND control_version=?3 AND takeover_at IS NULL",params![self.task,plan.attempt,plan.control_version])?==1,"delegation.control_changed");
            ensure!(tx.execute("UPDATE conversation SET last_message_sequence=?2,version=version+1,updated_at=?4 WHERE id=?1 AND version=?3 AND ended_at IS NULL",params![plan.identity.child_conversation_id,next_sequence,plan.conversation_version,at])?==1,"delegation.conversation_changed");
            let mut config = plan.config.clone();
            config
                .as_object_mut()
                .context("delegation.invalid_config")?
                .remove("configDigest");
            config["conversationVersion"] = json!(plan.conversation_version + 1);
            config["runtime"] = serde_json::to_value(&plan.runtime)?;
            config["runtimeAdapter"] = json!(plan.runtime.adapter_kind);
            config["provider"] = json!(plan.runtime.adapter_kind);
            config["model"] = json!(plan.runtime.model.model_id);
            config["delegationResume"] = serde_json::to_value(&plan.expected)?;
            config["configDigest"] = json!(canonical_json_digest(&config)?);
            let run = crate::delegation_factory::append_attempt(
                tx,
                &plan.identity,
                include_str!("../resources/delegation-resume.txt").trim(),
                &config,
                &plan.runtime,
                &plan.workspace,
                next_sequence,
                plan.attempt,
                at,
            )?;
            tx.execute("INSERT INTO delegation_attempt(delegation_id,attempt_no,agent_run_id,trigger_parent_run_id,trigger_parent_epoch,trigger_tool_call_id,kind,interruption_reason,created_at)
                VALUES(?1,?2,?3,?4,?5,?6,'resume',?7,?8)",params![self.task,plan.attempt,run,inv.source.agent_run_id,inv.source.execution_epoch,inv.tool_call,inv.input.reason.as_ref().map(|r|r.as_str()),at])?;
            let report =
                crate::delegation_model_command::task_report(tx, &self.task, &self.parent)?;
            crate::collaboration::append_domain_event(
                tx,
                "delegation.resumed",
                Some(&inv.source.camp_id),
                Some(("delegation", &self.task)),
                actor,
                Some(inv.source.execution_epoch),
                &json!({"delegationId":self.task,"attemptNo":plan.attempt,"agentRunId":run}),
            )?;
            Ok(CommandHandlerResult::accepted(
                "delegation.resumed",
                report,
                None,
            ))
        })();
        match result {
            Ok(result) => {
                tx.execute_batch("RELEASE delegation_resume;")?;
                Ok(result)
            }
            Err(error) => {
                tx.execute_batch("ROLLBACK TO delegation_resume; RELEASE delegation_resume;")?;
                Err(error)
            }
        }
    }
}

pub(crate) fn expected(connection: &Connection, run: &str) -> Result<Option<ExpectedSession>> {
    let config: Option<String> = connection
        .query_row(
            "SELECT effective_config_json FROM agent_run WHERE id=?1",
            [run],
            |r| r.get(0),
        )
        .optional()?;
    let Some(config) = config else {
        return Ok(None);
    };
    let config: Value = serde_json::from_str(&config)?;
    let installed:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='delegation_attempt')",[],|r|r.get(0))?;
    let resume=installed && connection.query_row("SELECT EXISTS(SELECT 1 FROM delegation_attempt WHERE agent_run_id=?1 AND kind='resume')",[run],|r|r.get::<_,bool>(0))?;
    let meta = config.get("delegationResume");
    ensure!(
        resume == meta.is_some(),
        "delegation.resume_identity_missing"
    );
    meta.cloned()
        .map(serde_json::from_value)
        .transpose()
        .context("delegation.resume_identity_invalid")
}
pub(crate) fn validate_session(
    connection: &Connection,
    run: &str,
) -> Result<Option<ExpectedSession>> {
    let Some(expected) = expected(connection, run)? else {
        return Ok(None);
    };
    let valid:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM delegation d JOIN delegation_control control ON control.delegation_id=d.id
        JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=control.current_attempt_no
        JOIN agent_run r ON r.id=a.agent_run_id JOIN conversation c ON c.id=d.child_conversation_id
        JOIN runtime_input_delivery accepted ON accepted.id=?8
        WHERE d.id=?1 AND a.agent_run_id=?2 AND a.kind='resume' AND a.attempt_no=?3 AND control.takeover_at IS NULL
          AND c.id=r.conversation_id AND c.ended_at IS NULL AND c.native_session_id=?4 AND c.native_binding_id=?5 AND c.native_binding_generation=?6
          AND c.native_session_compatibility_key IS ?7 AND c.native_adapter_installation_id=?9
          AND c.native_binding_compatibility_digest IN (?10,r.runtime_binding_compatibility_digest)
          AND r.runtime_native_session_compatibility_key IS ?7
          AND EXISTS(SELECT 1 FROM agent_run original WHERE original.id=accepted.agent_run_id AND original.runtime_binding_compatibility_digest=?11)
          AND accepted.agent_run_id=?12 AND accepted.execution_epoch=?13 AND accepted.status='accepted'
          AND accepted.native_binding_id=?5 AND accepted.native_binding_generation=?6
          AND NOT EXISTS(SELECT 1 FROM agent_run other JOIN conversation c2 ON c2.id=other.conversation_id
            WHERE other.conversation_id<>c.id AND c2.native_session_id=c.native_session_id
              AND other.runtime_adapter_kind=r.runtime_adapter_kind AND other.runtime_auth_scope=r.runtime_auth_scope
              AND (other.status IN ('queued','running','waiting') OR other.runtime_recovery_required<>0
                OR (other.cancel_requested_at IS NOT NULL AND other.cancel_acknowledged_at IS NULL))))",
        params![expected.delegation,run,expected.attempt,expected.session,expected.binding,expected.generation,expected.session_key,
            expected.delivery,expected.installation,expected.previous_digest,expected.runtime_digest,expected.previous_run,expected.previous_epoch],|r|r.get(0))?;
    ensure!(expected.version == 1 && valid, "delegation.session_changed");
    Ok(Some(expected))
}
pub(crate) fn require_rebound_runtime(
    connection: &Connection,
    run: &str,
    runtime: &FrozenAgentRuntimeConfig,
) -> Result<()> {
    if let Some(expected) = validate_session(connection, run)? {
        ensure!(
            runtime.native_session_compatibility_key == expected.session_key,
            "delegation.session_incompatible"
        );
    }
    Ok(())
}
pub(crate) fn require_execution(
    connection: &Connection,
    execution: &crate::runtime::AgentRunExecution,
) -> Result<()> {
    if let Some(expected) = validate_session(connection, &execution.agent_run_id)? {
        ensure!(
            execution.native_session_id.as_deref() == Some(expected.session.as_str()),
            "delegation.session_changed"
        );
    }
    Ok(())
}
pub(crate) fn require_binding(
    connection: &Connection,
    run: &str,
    session: &str,
    binding: &str,
    generation: i64,
) -> Result<()> {
    if let Some(expected) = validate_session(connection, run)? {
        ensure!(
            expected.session == session
                && expected.binding == binding
                && expected.generation == generation,
            "delegation.session_changed"
        );
    }
    Ok(())
}
