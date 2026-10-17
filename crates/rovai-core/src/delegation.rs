//! Delegation identity storage. The authenticated command layer owns tool eligibility,
//! permission admission, receipts/events, cancellation and actual Runtime launch.
use crate::delegation_contract::{RuntimeTarget, WorkingDirectory};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Supplied by the Core-authenticated current invocation, not model arguments.
pub struct ParentInvocation<'a> {
    pub run_id: &'a str,
    pub epoch: i64,
    pub binding_id: &'a str,
    pub binding_generation: i64,
    pub tool_call_id: &'a str,
}

pub struct FrozenRequest<'a> {
    pub task_blob_id: &'a str,
    pub runtime: &'a RuntimeTarget,
    pub working_dir: Option<&'a WorkingDirectory>,
    pub workspace: &'a Value,
    pub runtime_preset: &'a Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub id: String,
    pub camp_id: String,
    pub owner_agent_id: String,
    pub root_conversation_id: String,
    pub parent_conversation_id: String,
    pub parent_delegation_id: Option<String>,
    pub child_conversation_id: String,
    pub depth: i64,
}

#[derive(Debug)]
pub struct Registration {
    pub identity: Identity,
    pub replayed: bool,
}

fn identity(row: &rusqlite::Row<'_>) -> rusqlite::Result<Identity> {
    Ok(Identity {
        id: row.get(0)?,
        camp_id: row.get(1)?,
        owner_agent_id: row.get(2)?,
        root_conversation_id: row.get(3)?,
        parent_conversation_id: row.get(4)?,
        parent_delegation_id: row.get(5)?,
        child_conversation_id: row.get(6)?,
        depth: row.get(7)?,
    })
}
const IDENTITY: &str = "d.id,d.camp_id,d.owner_agent_id,d.root_conversation_id,d.parent_conversation_id,d.parent_delegation_id,d.child_conversation_id,d.depth";

/// Exact direct-parent lookup. A model-facing caller must derive parent_id from its authenticated Run.
pub fn find_for_parent(
    connection: &Connection,
    id: &str,
    parent_id: &str,
) -> Result<Option<Identity>> {
    Ok(connection
        .query_row(
            &format!(
                "SELECT {IDENTITY} FROM delegation d WHERE d.id=?1 AND d.parent_conversation_id=?2"
            ),
            params![id, parent_id],
            identity,
        )
        .optional()?)
}

/// The factory writes only the child's Conversation/message/Turn/queued Run in this transaction.
/// A replay returns before invoking it, and no Runtime/file launch belongs inside this function.
pub fn register<F>(
    tx: &Transaction<'_>,
    parent: &ParentInvocation<'_>,
    request: &FrozenRequest<'_>,
    created_at: &str,
    create_child: F,
) -> Result<Registration>
where
    F: FnOnce(&Transaction<'_>, &Identity) -> Result<String>,
{
    anyhow::ensure!(
        !parent.tool_call_id.is_empty(),
        "Missing delegation tool identity"
    );
    let task_sha: String = tx
        .query_row(
            "SELECT sha256 FROM managed_blob WHERE id=?1 AND byte_size>0",
            [request.task_blob_id],
            |r| r.get(0),
        )
        .context("Missing complete delegation task Blob")?;
    let working_dir = request
        .working_dir
        .map(|p| p.as_path().to_string_lossy().into_owned());
    let existing=tx.query_row(&format!("SELECT {IDENTITY},b.sha256,d.requested_runtime,d.requested_working_dir FROM delegation d JOIN managed_blob b ON b.id=d.task_blob_id WHERE d.origin_binding_id=?1 AND d.origin_binding_generation=?2 AND d.origin_parent_run_id=?3 AND d.origin_parent_epoch=?4 AND d.origin_tool_call_id=?5"),params![parent.binding_id,parent.binding_generation,parent.run_id,parent.epoch,parent.tool_call_id],|r|Ok((identity(r)?,r.get::<_,String>(8)?,r.get::<_,String>(9)?,r.get::<_,Option<String>>(10)?))).optional()?;
    if let Some((identity, old_task, old_runtime, old_dir)) = existing {
        anyhow::ensure!(
            old_task == task_sha
                && old_runtime == request.runtime.as_str()
                && old_dir == working_dir,
            "Delegation origin reused with different input"
        );
        return Ok(Registration {
            identity,
            replayed: true,
        });
    }
    let settings = crate::delegation_settings::read(tx)?;
    anyhow::ensure!(settings.enabled, "Delegation is disabled");
    anyhow::ensure!(
        request.workspace.is_object() && request.runtime_preset.is_object(),
        "Invalid frozen delegation workspace or preset"
    );
    let (conversation,camp,owner,kind,ancestor,root,depth)=tx.query_row(
        "SELECT c.id,c.camp_id,c.agent_id,c.kind,d.id,d.root_conversation_id,d.depth
         FROM agent_run r JOIN conversation c ON c.id=r.conversation_id
         JOIN camp_member m ON m.camp_id=c.camp_id AND m.agent_id=c.agent_id
         LEFT JOIN delegation d ON d.child_conversation_id=c.id
         LEFT JOIN camp_turn turn ON turn.id=r.camp_turn_id
         WHERE r.id=?1 AND r.execution_epoch=?2 AND c.native_binding_id=?3 AND c.native_binding_generation=?4
           AND r.status IN ('running','waiting') AND r.cancel_requested_at IS NULL
           AND c.ended_at IS NULL AND m.status='active' AND COALESCE(r.camp_id,turn.camp_id)=c.camp_id",
        params![parent.run_id,parent.epoch,parent.binding_id,parent.binding_generation],
        |r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,Option<i64>>(6)?)),
    ).context("Delegation parent invocation is stale or unavailable")?;
    let (root, depth) = match kind.as_str() {
        "camp_member" | "single_chat" => (conversation.clone(), 1),
        "delegation_child" => (
            root.context("Missing delegation root")?,
            depth
                .context("Missing delegation depth")?
                .checked_add(1)
                .context("Invalid delegation depth")?,
        ),
        _ => anyhow::bail!("Unsupported delegation parent kind"),
    };
    anyhow::ensure!(
        depth <= i64::from(settings.depth_limit) && depth <= 8,
        "Delegation depth limit exceeded"
    );
    let identity = Identity {
        id: Uuid::new_v4().to_string(),
        camp_id: camp,
        owner_agent_id: owner,
        root_conversation_id: root,
        parent_conversation_id: conversation,
        parent_delegation_id: ancestor,
        child_conversation_id: Uuid::new_v4().to_string(),
        depth,
    };
    let run_id = create_child(tx, &identity)?;
    let task_body:String=tx.query_row("SELECT m.body FROM agent_run r JOIN conversation_message m ON m.id=r.trigger_conversation_message_id WHERE r.id=?1 AND m.conversation_id=?2",params![run_id,identity.child_conversation_id],|r|r.get(0))?;
    anyhow::ensure!(
        format!("{:x}", Sha256::digest(task_body.as_bytes())) == task_sha,
        "Delegation child input differs from the original task"
    );
    tx.execute("INSERT INTO delegation(id,camp_id,owner_agent_id,root_conversation_id,parent_conversation_id,parent_delegation_id,child_conversation_id,origin_parent_run_id,origin_parent_epoch,origin_binding_id,origin_binding_generation,origin_tool_call_id,depth,task_blob_id,requested_runtime,requested_working_dir,resolved_workspace_json,runtime_preset_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",params![identity.id,identity.camp_id,identity.owner_agent_id,identity.root_conversation_id,identity.parent_conversation_id,identity.parent_delegation_id,identity.child_conversation_id,parent.run_id,parent.epoch,parent.binding_id,parent.binding_generation,parent.tool_call_id,depth,request.task_blob_id,request.runtime.as_str(),working_dir,serde_json::to_string(request.workspace)?,serde_json::to_string(request.runtime_preset)?,created_at])?;
    tx.execute("INSERT INTO delegation_attempt(delegation_id,attempt_no,agent_run_id,trigger_parent_run_id,trigger_parent_epoch,trigger_tool_call_id,kind,created_at) VALUES(?1,1,?2,?3,?4,?5,'initial',?6)",params![identity.id,run_id,parent.run_id,parent.epoch,parent.tool_call_id,created_at])?;
    tx.execute("INSERT INTO delegation_control(delegation_id,current_attempt_no,control_version) VALUES(?1,1,1)",[&identity.id])?;
    Ok(Registration {
        identity,
        replayed: false,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct Control {
    pub current_attempt_no: i64,
    pub version: i64,
    pub takeover_at: Option<String>,
    pub takeover_command_id: Option<String>,
    pub takeover_message_sequence: Option<i64>,
}
pub fn control(connection: &Connection, id: &str) -> Result<Control> {
    Ok(connection.query_row("SELECT current_attempt_no,control_version,takeover_at,takeover_command_id,takeover_message_sequence FROM delegation_control WHERE delegation_id=?1",[id],|r|Ok(Control{current_attempt_no:r.get(0)?,version:r.get(1)?,takeover_at:r.get(2)?,takeover_command_id:r.get(3)?,takeover_message_sequence:r.get(4)?}))?)
}

/// Persists the User takeover boundary after the same transaction has closed the old Run's output.
/// Cancellation, command authentication, process cleanup and admission of later User input are separate Core work.
pub fn record_takeover_boundary(
    tx: &Transaction<'_>,
    id: &str,
    expected_version: i64,
    command_id: &str,
    at: &str,
) -> Result<Control> {
    let before = control(tx, id)?;
    if before.takeover_command_id.as_deref() == Some(command_id)
        && expected_version.checked_add(1) == Some(before.version)
    {
        return Ok(before);
    }
    anyhow::ensure!(
        !command_id.is_empty()
            && before.version == expected_version
            && before.takeover_at.is_none(),
        "Delegation control revision conflict"
    );
    let next = expected_version
        .checked_add(1)
        .context("Delegation control version exhausted")?;
    let changed=tx.execute("UPDATE delegation_control SET control_version=?1,takeover_at=?2,takeover_command_id=?3,takeover_message_sequence=(SELECT c.last_message_sequence FROM delegation d JOIN conversation c ON c.id=d.child_conversation_id WHERE d.id=?4) WHERE delegation_id=?4 AND control_version=?5 AND takeover_at IS NULL",params![next,at,command_id,id,expected_version])?;
    anyhow::ensure!(changed == 1, "Delegation control revision conflict");
    control(tx, id)
}

#[cfg(all(test, feature = "extended-tests"))]
#[path = "delegation_tests.rs"]
mod tests;
