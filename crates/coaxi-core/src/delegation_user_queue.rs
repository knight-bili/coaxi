//! Child queues share private material algorithms, with their own sealed command identities.
use crate::{
    command::{ActorRef, CommandEnvelope, CommandExecution, DomainCommand, sealed},
    db::Database,
    draft_client::DraftClient,
    local_attachment_source::LocalAttachmentSourceRef,
    private_source_observation::PreparedSources,
    single_chat::{
        self, EditSingleChatPendingInputCommand, PrivatePendingEditCommand, PrivatePublishCommand,
        PublishSingleChatPendingInput, SingleChatPendingInputsView, SingleChatService,
    },
    single_chat_composer::ComposerOwner,
    single_chat_composer_scope::{PrivateComposerKey, SingleChatComposerScope},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EditDelegationPendingInputCommand(pub EditSingleChatPendingInputCommand);
impl sealed::Sealed for EditDelegationPendingInputCommand {}
impl DomainCommand for EditDelegationPendingInputCommand {
    const TYPE: &'static str = "delegations.pending_input.edit";
    fn draft_client(&self) -> Option<&DraftClient> {
        Some(&self.0.draft_client)
    }
}
impl PrivatePendingEditCommand for EditDelegationPendingInputCommand {
    const OWNER: ComposerOwner = ComposerOwner::DelegationChild;
    fn input(&self) -> EditSingleChatPendingInputCommand {
        self.0.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PublishDelegationPendingInput(pub PublishSingleChatPendingInput);
impl sealed::Sealed for PublishDelegationPendingInput {}
impl DomainCommand for PublishDelegationPendingInput {
    const TYPE: &'static str = "delegations.pending_input.publish";
}
impl PrivatePublishCommand for PublishDelegationPendingInput {
    const OWNER: ComposerOwner = ComposerOwner::DelegationChild;
    fn input(&self) -> PublishSingleChatPendingInput {
        self.0.clone()
    }
}

pub fn edit_pending_input(
    db: &mut Database,
    command: &CommandEnvelope<EditDelegationPendingInputCommand>,
) -> Result<CommandExecution> {
    SingleChatService::default().edit_owned_pending_input(db, command)
}
pub(crate) fn edit_source_inputs(
    db: &Database,
    command: &CommandEnvelope<EditDelegationPendingInputCommand>,
) -> Result<Vec<LocalAttachmentSourceRef>> {
    SingleChatService::edit_source_inputs(db, command)
}
pub(crate) fn edit_with_prepared_sources(
    db: &mut Database,
    command: &CommandEnvelope<EditDelegationPendingInputCommand>,
    prepared: Arc<PreparedSources>,
) -> Result<CommandExecution> {
    SingleChatService::default()
        .with_prepared_sources(prepared)
        .edit_owned_pending_input(db, command)
}
pub fn publish_pending_input(
    db: &mut Database,
    command: &CommandEnvelope<PublishDelegationPendingInput>,
) -> Result<CommandExecution> {
    SingleChatService::default().publish_owned_pending_input(db, command)
}
pub(crate) fn publish_source_inputs(
    db: &Database,
    command: &CommandEnvelope<PublishDelegationPendingInput>,
) -> Result<Vec<LocalAttachmentSourceRef>> {
    SingleChatService::publish_source_inputs(db, command)
}
pub(crate) fn publish_with_prepared_sources(
    db: &mut Database,
    command: &CommandEnvelope<PublishDelegationPendingInput>,
    prepared: Arc<PreparedSources>,
) -> Result<CommandExecution> {
    SingleChatService::default()
        .with_prepared_sources(prepared)
        .publish_owned_pending_input(db, command)
}
pub fn read_pending_inputs(
    db: &Database,
    actor: &ActorRef,
    client: &DraftClient,
    thread: &str,
    conversation: &str,
    scope: Option<&SingleChatComposerScope>,
) -> Result<SingleChatPendingInputsView> {
    super::read_draft(db, actor, client, thread, conversation, scope)?;
    single_chat::read_owned_pending_inputs(
        db,
        conversation,
        client,
        &PrivateComposerKey::for_scope(scope)?,
        ComposerOwner::DelegationChild,
    )
}
pub fn ready_pending_inputs(db: &Database) -> Result<Vec<PublishDelegationPendingInput>> {
    Ok(
        single_chat::ready_owned_pending_inputs(db, ComposerOwner::DelegationChild)?
            .into_iter()
            .map(PublishDelegationPendingInput)
            .collect(),
    )
}
