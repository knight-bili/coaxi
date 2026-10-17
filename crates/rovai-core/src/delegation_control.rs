//! User control uses the original Run settlement and cleanup authority.
use crate::{
    command::{
        ActorRef, CommandEnvelope, CommandExecution, CommandHandlerResult, DomainCommand,
        DomainCommandGateway, sealed,
    },
    db::Database,
};
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TakeoverDelegationCommand {
    pub delegation_id: String,
    pub expected_control_version: i64,
}
impl sealed::Sealed for TakeoverDelegationCommand {}
impl DomainCommand for TakeoverDelegationCommand {
    const TYPE: &'static str = "delegations.takeover";
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelDelegationCommand {
    pub delegation_id: String,
    pub attempt_no: i64,
}
impl sealed::Sealed for CancelDelegationCommand {}
impl DomainCommand for CancelDelegationCommand {
    const TYPE: &'static str = "delegations.cancel";
}

// A later User Run is not a delegation_attempt and cannot enter its old parent's tree.
// A separately taken-over descendant cuts the entire branch.
const TREE: &str = "WITH RECURSIVE tree(id,depth) AS (
    SELECT id,0 FROM delegation WHERE id=?1
    UNION ALL SELECT child.id,tree.depth+1 FROM tree
    JOIN delegation child ON child.parent_delegation_id=tree.id
    JOIN delegation_control control ON control.delegation_id=child.id
    JOIN delegation_attempt current ON current.delegation_id=child.id AND current.attempt_no=control.current_attempt_no
    WHERE control.takeover_at IS NULL AND EXISTS(SELECT 1 FROM delegation_attempt parent
        WHERE parent.delegation_id=tree.id AND parent.agent_run_id=current.trigger_parent_run_id)
)";

pub(crate) struct Target {
    pub thread: String,
    pub conversation: String,
    run: String,
    status: String,
    active: bool,
}
pub(crate) fn target(connection: &Connection, id: &str) -> Result<Option<Target>> {
    let installed: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='delegation')",
        [],
        |r| r.get(0),
    )?;
    if !installed {
        return Ok(None);
    }
    Ok(connection.query_row("SELECT d.camp_id,d.child_conversation_id,run.id,run.status,
        c.ended_at IS NULL AND camp.activation_state='active' AND camp.deletion_operation_id IS NULL
          AND EXISTS(SELECT 1 FROM camp_member m JOIN agent_profile a ON a.id=m.agent_id
            WHERE m.camp_id=d.camp_id AND m.agent_id=d.owner_agent_id AND m.status='active' AND a.profile_status<>'removed')
        FROM delegation d JOIN conversation c ON c.id=d.child_conversation_id
        JOIN camp ON camp.id=d.camp_id JOIN delegation_control control ON control.delegation_id=d.id
        JOIN delegation_attempt attempt ON attempt.delegation_id=d.id AND attempt.attempt_no=control.current_attempt_no
        JOIN agent_run run ON run.id=attempt.agent_run_id WHERE d.id=?1",[id],|r|Ok(Target{
            thread:r.get(0)?,conversation:r.get(1)?,run:r.get(2)?,status:r.get(3)?,active:r.get(4)?})).optional()?)
}
pub(crate) fn cleanup_pending(connection: &Connection, id: &str) -> Result<bool> {
    Ok(connection.query_row(&format!("{TREE} SELECT EXISTS(SELECT 1 FROM tree
        JOIN delegation_attempt attempt ON attempt.delegation_id=tree.id JOIN agent_run run ON run.id=attempt.agent_run_id
        WHERE run.status IN ('queued','running','waiting') OR run.runtime_recovery_required<>0
          OR (run.cancel_requested_at IS NOT NULL AND run.cancel_acknowledged_at IS NULL)
          OR EXISTS(SELECT 1 FROM runtime_input_delivery input WHERE input.agent_run_id=run.id
            AND input.status IN ('prepared','delivery_unknown')))") ,[id],|r|r.get(0))?)
}
fn cancel_roots(
    tx: &Transaction<'_>,
    roots: Vec<String>,
    actor: &ActorRef,
    reason: &str,
    at: &str,
) -> Result<Vec<String>> {
    let mut runs = Vec::new();
    for root in roots {
        runs.extend(cancel_tree(tx, &root, actor, reason, at)?);
    }
    runs.sort();
    runs.dedup();
    Ok(runs)
}
fn installed(connection: &Connection) -> Result<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='delegation')",
        [],
        |r| r.get(0),
    )?)
}
pub(crate) fn cancel_for_parent_run(
    tx: &Transaction<'_>,
    run: &str,
    epoch: i64,
    actor: &ActorRef,
    reason: &str,
    at: &str,
) -> Result<Vec<String>> {
    if !installed(tx)? {
        return Ok(Vec::new());
    }
    let roots=tx.prepare("SELECT d.id FROM delegation d JOIN delegation_control c ON c.delegation_id=d.id
        JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=c.current_attempt_no
        WHERE a.trigger_parent_run_id=?1 AND a.trigger_parent_epoch=?2 AND c.takeover_at IS NULL ORDER BY d.id")?
        .query_map(rusqlite::params![run,epoch],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    cancel_roots(tx, roots, actor, reason, at)
}
pub(crate) fn cancel_for_conversation(
    tx: &Transaction<'_>,
    conversation: &str,
    actor: &ActorRef,
    reason: &str,
    at: &str,
) -> Result<Vec<String>> {
    if !installed(tx)? {
        return Ok(Vec::new());
    }
    let roots = tx
        .prepare(
            "SELECT d.id FROM delegation d JOIN delegation_control c ON c.delegation_id=d.id
        WHERE d.parent_conversation_id=?1 AND c.takeover_at IS NULL ORDER BY d.id",
        )?
        .query_map([conversation], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    cancel_roots(tx, roots, actor, reason, at)
}
pub(crate) fn cancel_for_member(
    tx: &Transaction<'_>,
    thread: &str,
    member: &str,
    actor: &ActorRef,
    at: &str,
) -> Result<Vec<String>> {
    if !installed(tx)? {
        return Ok(Vec::new());
    }
    let conversations=tx.prepare("SELECT DISTINCT d.parent_conversation_id FROM delegation d
        JOIN conversation c ON c.id=d.parent_conversation_id WHERE c.camp_id=?1 AND c.agent_id=?2 ORDER BY c.id")?
        .query_map(rusqlite::params![thread,member],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut runs = Vec::new();
    for conversation in conversations {
        runs.extend(cancel_for_conversation(
            tx,
            &conversation,
            actor,
            "camp_membership_ended",
            at,
        )?);
    }
    runs.sort();
    runs.dedup();
    Ok(runs)
}
fn cancel_tree(
    tx: &Transaction<'_>,
    id: &str,
    actor: &ActorRef,
    reason: &str,
    at: &str,
) -> Result<Vec<String>> {
    let runs = tx.prepare(&format!("{TREE} SELECT run.id FROM tree JOIN delegation_control control ON control.delegation_id=tree.id
        JOIN delegation_attempt attempt ON attempt.delegation_id=tree.id AND attempt.attempt_no=control.current_attempt_no
        JOIN agent_run run ON run.id=attempt.agent_run_id WHERE run.status IN ('queued','running','waiting')
        ORDER BY tree.depth DESC,run.id"))?.query_map([id],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    for run in &runs {
        let settled = crate::runtime::settle_abortive_agent_run_in_tx(tx, run, reason, actor, at)?;
        if !settled.camp_turn_id.is_empty() {
            crate::runtime::recompute_camp_turn(
                tx,
                &settled.camp_id,
                &settled.camp_turn_id,
                actor,
                Some(settled.execution_epoch),
                at,
            )?;
        }
    }
    Ok(runs)
}
fn rejected(code: &'static str) -> CommandHandlerResult {
    CommandHandlerResult::rejected(code, json!({}))
}
pub fn takeover(
    db: &mut Database,
    command: &CommandEnvelope<TakeoverDelegationCommand>,
) -> Result<CommandExecution> {
    let mut stopped = Vec::new();
    let result=DomainCommandGateway.execute(db,command,|tx| {
        if !matches!(command.actor,ActorRef::User {..}) { return Ok(rejected("delegation.user_required")); }
        let Some(target)=target(tx,&command.payload.delegation_id)? else { return Ok(rejected("delegation.not_found")); };
        if command.camp_id.as_deref()!=Some(target.thread.as_str()) { return Ok(rejected("delegation.thread_mismatch")); }
        let before=crate::delegation::control(tx,&command.payload.delegation_id)?;
        if before.version!=command.payload.expected_control_version { return Ok(rejected("delegation.control_changed")); }
        if before.takeover_at.is_some() { return Ok(CommandHandlerResult::applied("delegation.already_taken_over",json!({"delegationId":command.payload.delegation_id,"controlVersion":before.version}),None)); }
        if !target.active { return Ok(rejected("delegation.not_active")); }
        let now=chrono::Utc::now().to_rfc3339();
        stopped=cancel_tree(tx,&command.payload.delegation_id,&command.actor,"user_takeover",&now)?;
        let control=crate::delegation::record_takeover_boundary(tx,&command.payload.delegation_id,before.version,&command.command_id,&now)?;
        let status:String=tx.query_row("SELECT status FROM agent_run WHERE id=?1",[&target.run],|r|r.get(0))?;
        let pending=cleanup_pending(tx,&command.payload.delegation_id)?;
        let payload=json!({"delegationId":command.payload.delegation_id,"conversationId":target.conversation,
            "controlVersion":control.version,"takeoverMessageSequence":control.takeover_message_sequence,
            "status":status,"cleanupPending":pending,"cancelledAgentRunIds":stopped});
        crate::collaboration::append_domain_event(tx,"delegation.taken_over",Some(&target.thread),Some(("delegation",&command.payload.delegation_id)),&command.actor,None,&payload)?;
        Ok(CommandHandlerResult::applied("delegation.taken_over",payload,None))
    })?;
    crate::runtime::pump_targets_after_runs_terminal(db, &stopped)?;
    Ok(result)
}
pub fn cancel(
    db: &mut Database,
    command: &CommandEnvelope<CancelDelegationCommand>,
) -> Result<CommandExecution> {
    let mut stopped = Vec::new();
    let result=DomainCommandGateway.execute(db,command,|tx| {
        if !matches!(command.actor,ActorRef::User {..}) { return Ok(rejected("delegation.user_required")); }
        let Some(target)=target(tx,&command.payload.delegation_id)? else { return Ok(rejected("delegation.not_found")); };
        if command.camp_id.as_deref()!=Some(target.thread.as_str()) { return Ok(rejected("delegation.thread_mismatch")); }
        let control=crate::delegation::control(tx,&command.payload.delegation_id)?;
        if control.current_attempt_no!=command.payload.attempt_no { return Ok(rejected("delegation.attempt_changed")); }
        // A terminal root is a no-op even with background descendants. Conversation closure owns those.
        if control.takeover_at.is_some() || !matches!(target.status.as_str(),"queued"|"running"|"waiting") {
            return Ok(CommandHandlerResult::applied("delegation.already_terminal",json!({"delegationId":command.payload.delegation_id,"status":target.status}),None));
        }
        stopped=cancel_tree(tx,&command.payload.delegation_id,&command.actor,"user_requested_agent_run_stop",&chrono::Utc::now().to_rfc3339())?;
        let payload=json!({"delegationId":command.payload.delegation_id,"conversationId":target.conversation,
            "status":"cancelled","cleanupPending":cleanup_pending(tx,&command.payload.delegation_id)?,"cancelledAgentRunIds":stopped});
        crate::collaboration::append_domain_event(tx,"delegation.cancelled",Some(&target.thread),Some(("delegation",&command.payload.delegation_id)),&command.actor,None,&payload)?;
        Ok(CommandHandlerResult::applied("delegation.cancelled",payload,None))
    })?;
    crate::runtime::pump_targets_after_runs_terminal(db, &stopped)?;
    Ok(result)
}
