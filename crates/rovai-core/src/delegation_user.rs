//! User-only child materials. Takeover/cleanup and execution admission remain separate commands.
use anyhow::{Result, ensure};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::{
    command::{ActorRef, CommandEnvelope, CommandExecution, DomainCommand, sealed},
    db::Database,
    draft_client::DraftClient,
    single_chat_composer::{
        self, ComposerOwner, MutatePrivateComposerCommand, PrivateComposerCommand,
        PrivateRecoveryParams, PrivateRecoveryResult, ScopedPrivateDraftView,
    },
    single_chat_composer_scope::SingleChatComposerScope,
};

#[path = "delegation_user_send.rs"]
mod send;
pub use send::{SendDelegationMessageCommand, send, send_source_inputs};
pub(crate) use send::{
    followup_config, followup_runtime, send_blocker, send_with_prepared_sources,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MutateDelegationComposerCommand(pub MutatePrivateComposerCommand);

impl sealed::Sealed for MutateDelegationComposerCommand {}
impl DomainCommand for MutateDelegationComposerCommand {
    const TYPE: &'static str = "delegations.composer.mutate";
    fn draft_client(&self) -> Option<&DraftClient> {
        Some(&self.0.draft_client)
    }
}
impl PrivateComposerCommand for MutateDelegationComposerCommand {
    const OWNER: ComposerOwner = ComposerOwner::DelegationChild;
    fn materials(&self) -> &MutatePrivateComposerCommand {
        &self.0
    }
}

pub(crate) fn material_owner_exists(
    connection: &Connection,
    thread: &str,
    conversation: &str,
    active: bool,
) -> Result<bool> {
    let installed: bool = connection.query_row(
        "SELECT count(*)=2 FROM sqlite_schema WHERE type='table' AND name IN ('delegation','delegation_control')",
        [], |r| r.get(0),
    )?;
    if !installed {
        return Ok(false);
    }
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation c JOIN camp ON camp.id=c.camp_id
         JOIN delegation d ON d.child_conversation_id=c.id
         JOIN delegation_control control ON control.delegation_id=d.id
         WHERE c.id=?1 AND c.camp_id=?2 AND c.kind='delegation_child'
           AND d.camp_id=c.camp_id AND d.owner_agent_id=c.agent_id
           AND control.takeover_at IS NOT NULL AND control.takeover_command_id IS NOT NULL
           AND control.takeover_message_sequence IS NOT NULL AND camp.deletion_operation_id IS NULL
           AND (?3=0 OR (c.ended_at IS NULL AND camp.activation_state='active'
             AND EXISTS(SELECT 1 FROM camp_member m JOIN agent_profile a ON a.id=m.agent_id
               WHERE m.camp_id=c.camp_id AND m.agent_id=c.agent_id AND m.status='active' AND a.profile_status<>'removed'))))",
        params![conversation,thread,active], |r| r.get(0),
    )?)
}

// Published User-message sources can be inspected before takeover; draft and retained input cannot.
pub(crate) fn conversation_owner_exists(
    connection: &Connection,
    thread: &str,
    conversation: &str,
) -> Result<bool> {
    let installed: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='delegation')",
        [],
        |r| r.get(0),
    )?;
    if !installed {
        return Ok(false);
    }
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation c JOIN camp ON camp.id=c.camp_id
        JOIN delegation d ON d.child_conversation_id=c.id WHERE c.id=?1 AND c.camp_id=?2
        AND c.kind='delegation_child' AND d.camp_id=c.camp_id AND d.owner_agent_id=c.agent_id
        AND camp.deletion_operation_id IS NULL)",
        params![conversation, thread],
        |r| r.get(0),
    )?)
}

pub fn read_draft(
    database: &Database,
    actor: &ActorRef,
    client: &DraftClient,
    thread: &str,
    conversation: &str,
    scope: Option<&SingleChatComposerScope>,
) -> Result<ScopedPrivateDraftView> {
    ensure!(
        matches!(actor, ActorRef::User { .. }),
        "delegation.user_required"
    );
    single_chat_composer::read_owned_draft(
        database,
        client,
        thread,
        conversation,
        scope,
        ComposerOwner::DelegationChild,
    )
}

pub fn mutate_draft(
    database: &mut Database,
    envelope: &CommandEnvelope<MutateDelegationComposerCommand>,
) -> Result<CommandExecution> {
    single_chat_composer::mutate_owned(database, envelope)
}

pub fn recovery(
    database: &Database,
    actor: &ActorRef,
    client: &DraftClient,
    input: &PrivateRecoveryParams,
) -> Result<PrivateRecoveryResult> {
    ensure!(
        matches!(actor, ActorRef::User { .. }),
        "delegation.user_required"
    );
    single_chat_composer::recover_owned(database, client, input, ComposerOwner::DelegationChild)
}
