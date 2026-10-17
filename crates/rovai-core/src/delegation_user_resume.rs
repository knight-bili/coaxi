//! Explicit User continuation, without borrowing a live parent model credential.
use super::*;
use crate::command::{
    CommandEnvelope, CommandExecution, DomainCommand, DomainCommandGateway, sealed,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResumeDelegationCommand {
    pub delegation_id: crate::delegation_contract::NonEmpty,
    pub expected_attempt_no: i64,
    #[serde(
        default,
        deserialize_with = "crate::delegation_contract::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub reason: Option<crate::delegation_contract::InterruptionReason>,
}
impl sealed::Sealed for ResumeDelegationCommand {}
impl DomainCommand for ResumeDelegationCommand {
    const TYPE: &'static str = "delegations.resume";
    const EXACT_REPLAY: bool = true;
}
#[derive(Serialize)]
struct ParentBasis {
    run: String,
    epoch: i64,
    config_digest: String,
}
pub(crate) struct PreparedUserResume {
    inner: PreparedResume,
    basis: Option<ParentBasis>,
}

fn digest(command: &CommandEnvelope<ResumeDelegationCommand>) -> Result<String> {
    canonical_json_digest(&serde_json::to_value(command)?)
}
fn load_user_plan(
    conn: &Connection,
    command: &CommandEnvelope<ResumeDelegationCommand>,
) -> Result<(Plan, ParentBasis)> {
    ensure!(
        matches!(command.actor, ActorRef::User { .. }),
        "delegation.user_required"
    );
    ensure!(command.camp_id.is_none(), "delegation.user_scope_mismatch");
    ensure!(conn.query_row("SELECT count(*)=4 FROM sqlite_schema WHERE type='table' AND name IN ('delegation','delegation_attempt','delegation_control','delegation_settings')",[],|r|r.get::<_,bool>(0))?,"delegation.unavailable");
    let task = command.payload.delegation_id.as_str();
    let (parent,attempt,run,epoch,config,kind,invocation,policy_name,policy_version,response,destination):
        (String,i64,String,i64,String,String,String,String,i64,String,Option<String>)=conn.query_row(
        "SELECT d.parent_conversation_id,control.current_attempt_no,parent_run.id,parent_run.execution_epoch,
            parent_run.effective_config_json,parent.kind,parent_run.invocation_kind,parent_run.operation_policy,
            parent_run.operation_policy_version,parent_run.response_delivery,parent_run.destination_conversation_id
         FROM delegation d JOIN delegation_control control ON control.delegation_id=d.id
         JOIN delegation_attempt previous ON previous.delegation_id=d.id AND previous.attempt_no=control.current_attempt_no
         JOIN conversation parent ON parent.id=d.parent_conversation_id JOIN camp ON camp.id=d.camp_id
         JOIN camp_member member ON member.camp_id=d.camp_id AND member.agent_id=d.owner_agent_id
         JOIN agent_profile profile ON profile.id=d.owner_agent_id
         JOIN agent_run parent_run ON parent_run.id=previous.trigger_parent_run_id
         WHERE d.id=?1 AND parent_run.conversation_id=parent.id AND parent_run.execution_epoch=previous.trigger_parent_epoch
           AND parent.camp_id=d.camp_id AND parent.agent_id=d.owner_agent_id AND parent.ended_at IS NULL
           AND camp.activation_state='active' AND camp.deletion_operation_id IS NULL
           AND member.status='active' AND member.leave_requested_at IS NULL AND profile.profile_status='present'",
        [task],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?)),
    ).optional()?.context("delegation.parent_unavailable")?;
    ensure!(
        attempt == command.payload.expected_attempt_no && attempt >= 1,
        "delegation.attempt_changed"
    );
    let config: Value = serde_json::from_str(&config)?;
    let mut unsigned = config.clone();
    let saved = unsigned
        .as_object_mut()
        .context("delegation.parent_config_invalid")?
        .remove("configDigest")
        .context("delegation.parent_config_invalid")?;
    ensure!(
        saved.as_str() == Some(canonical_json_digest(&unsigned)?.as_str()),
        "delegation.parent_config_invalid"
    );
    let policy = crate::delegation_policy::FrozenPolicy::from_config(&config)?
        .context("delegation.frozen_policy_required")?
        .validate_invocation(&invocation)?;
    use crate::delegation_policy::Role;
    ensure!(
        matches!(
            (policy.role, kind.as_str()),
            (Role::PublicParent, "camp_member")
                | (Role::PrivateParent, "single_chat")
                | (Role::Child, "delegation_child")
        ),
        "delegation.parent_role_mismatch"
    );
    if policy.role != Role::PublicParent {
        ensure!(
            response == "conversation_message" && destination.as_deref() == Some(parent.as_str()),
            "delegation.private_parent_route_mismatch"
        );
        ensure!(
            (policy.role == Role::PrivateParent
                && policy_name == "single_chat_v1"
                && policy_version == 3)
                || (policy.role == Role::Child
                    && policy_name == "delegation_child_v1"
                    && policy_version == 1),
            "delegation.parent_policy_mismatch"
        );
    }
    let identity =
        crate::delegation::find_for_parent(conn, task, &parent)?.context("delegation.not_found")?;
    let settings = crate::delegation_settings::read(conn)?;
    let mut plan = load_plan(conn, identity, &policy, &settings)?;
    let basis = ParentBasis {
        run,
        epoch,
        config_digest: saved.as_str().unwrap().into(),
    };
    plan.fingerprint = canonical_json_digest(&json!({"child":plan.fingerprint,"parent":basis}))?;
    Ok((plan, basis))
}
pub(crate) fn prepare(
    db: &Database,
    command: &CommandEnvelope<ResumeDelegationCommand>,
) -> Result<PreparedUserResume> {
    prepare_on(db.connection(), command)
}
fn prepare_on(
    conn: &Connection,
    command: &CommandEnvelope<ResumeDelegationCommand>,
) -> Result<PreparedUserResume> {
    let (state, basis, parent) = match load_user_plan(conn, command) {
        Ok((plan, basis)) => {
            let parent = plan.identity.parent_conversation_id.clone();
            (Preparation::Ready(Box::new(plan)), Some(basis), parent)
        }
        Err(error) => {
            if error.chain().any(|cause| cause.is::<rusqlite::Error>()) {
                return Err(error);
            }
            (Preparation::Denied(safe_code(&error)), None, String::new())
        }
    };
    Ok(PreparedUserResume {
        inner: PreparedResume {
            invocation: digest(command)?,
            parent,
            task: command.payload.delegation_id.as_str().into(),
            state,
        },
        basis,
    })
}
impl PreparedUserResume {
    pub(crate) fn observe_directory(&self) -> DirectoryObservation {
        self.inner.observe_directory()
    }
}
pub(crate) fn commit(
    db: &mut Database,
    command: &CommandEnvelope<ResumeDelegationCommand>,
    prepared: &PreparedUserResume,
    observation: &DirectoryObservation,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(db,command,|tx| {
        ensure!(prepared.inner.invocation==digest(command)? && observation.invocation==prepared.inner.invocation,"delegation.preparation_scope_mismatch");
        let current=prepare_on(tx,command)?;
        if let Preparation::Denied(code)=&prepared.inner.state { return Ok(reject(&prepared.inner.task,code)); }
        let Preparation::Ready(original)=&prepared.inner.state else { anyhow::bail!("delegation.not_resumable"); };
        let Preparation::Ready(plan)=&current.inner.state else {
            let code=match &current.inner.state { Preparation::Denied(code)=>code.as_str(),_=>"delegation.not_resumable" };
            return Ok(reject(&prepared.inner.task,code));
        };
        ensure!(observation.plan.as_deref()==Some(original.fingerprint.as_str()),"delegation.preparation_scope_mismatch");
        if plan.fingerprint!=original.fingerprint { return Ok(reject(&prepared.inner.task,"delegation.resume_state_changed")); }
        if let Some(code)=observation.error { return Ok(reject(&prepared.inner.task,code)); }
        let basis=current.basis.as_ref().context("delegation.parent_unavailable")?;
        let call=format!("user-command:{}",command.command_id);
        let run=append_resume_attempt(tx,plan,&ResumeTrigger { parent_run:&basis.run,parent_epoch:basis.epoch,call:&call,
            reason:command.payload.reason.as_ref().map(|r|r.as_str()),actor:&command.actor,user_command:Some(&command.command_id) },&chrono::Utc::now().to_rfc3339())?;
        Ok(CommandHandlerResult::accepted("delegation.resumed",json!({"delegationId":plan.identity.id,"threadId":plan.identity.camp_id,
            "conversationId":plan.identity.child_conversation_id,"attemptNo":plan.attempt,"controlVersion":plan.control_version+1,"agentRunId":run,"status":"queued"}),None))
    })
}
