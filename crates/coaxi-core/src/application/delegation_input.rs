//! User child sends/queue edits derive owner and editor from the trusted Core boundary.
use super::private_composer::Reply;
use super::*;
use crate::{
    delegation_user::{self, EditDelegationPendingInputCommand, SendDelegationMessageCommand},
    single_chat::{EditSingleChatPendingInputCommand, SingleChatPendingInputEditAction},
    single_chat_composer_scope::{SingleChatComposerScope, SingleChatComposerScopeRequest},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Params<T> {
    command_id: String,
    command: T,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EditInput {
    conversation_id: String,
    pending_input_id: String,
    expected_revision: i64,
    edit_token: Option<String>,
    #[serde(default)]
    composer_scope: Option<SingleChatComposerScope>,
    action: SingleChatPendingInputEditAction,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadInput {
    conversation_id: String,
    #[serde(default)]
    composer_scope: SingleChatComposerScopeRequest,
}

fn thread(database: &Database, conversation: &str) -> Result<String> {
    database
        .connection()
        .query_row(
            "SELECT camp_id FROM conversation WHERE id=?1 AND kind='delegation_child'",
            [conversation],
            |r| r.get(0),
        )
        .optional()?
        .context("delegation.not_found")
}
pub(crate) fn send_envelope(
    database: &Database,
    client: &crate::draft_client::DraftClient,
    value: Value,
) -> Result<CommandEnvelope<SendDelegationMessageCommand>> {
    let mut p: Params<SendDelegationMessageCommand> = serde_json::from_value(value)?;
    p.command.camp_id = thread(database, &p.command.conversation_id)?;
    p.command.draft_client = client.clone();
    Ok(user_camp_command_envelope(
        p.command_id,
        p.command.camp_id.clone(),
        p.command,
    ))
}
pub(crate) fn edit_envelope(
    database: &Database,
    client: &crate::draft_client::DraftClient,
    value: Value,
) -> Result<CommandEnvelope<EditDelegationPendingInputCommand>> {
    let p: Params<EditInput> = serde_json::from_value(value)?;
    let thread = thread(database, &p.command.conversation_id)?;
    let command = EditDelegationPendingInputCommand(EditSingleChatPendingInputCommand {
        draft_client: client.clone(),
        camp_id: thread.clone(),
        conversation_id: p.command.conversation_id,
        pending_input_id: p.command.pending_input_id,
        expected_revision: p.command.expected_revision,
        edit_token: p.command.edit_token,
        composer_scope: p.command.composer_scope,
        action: p.command.action,
    });
    Ok(user_camp_command_envelope(p.command_id, thread, command))
}
pub(crate) fn read(
    database: &Database,
    client: &crate::draft_client::DraftClient,
    value: Value,
) -> Result<Value> {
    let p: ReadInput = serde_json::from_value(value)?;
    let thread = thread(database, &p.conversation_id)?;
    Ok(serde_json::to_value(delegation_user::read_pending_inputs(
        database,
        &ActorRef::User {
            user_id: CURRENT_USER_ID.into(),
        },
        client,
        &thread,
        &p.conversation_id,
        p.composer_scope.scope(),
    )?)?)
}
pub(crate) fn reconcile(
    database: &Database,
    client: &crate::draft_client::DraftClient,
    method: &str,
    value: Value,
) -> Result<Value> {
    let recorded = match method {
        "delegations.message.send" => DomainCommandGateway
            .replay_if_recorded(database, &send_envelope(database, client, value)?)?,
        "delegations.pendingInputs.edit" => DomainCommandGateway
            .replay_if_recorded(database, &edit_envelope(database, client, value)?)?,
        _ => anyhow::bail!("delegation.operation_not_admitted"),
    };
    Ok(recorded
        .map(|e| json!({"state":"recorded","result":e.result}))
        .unwrap_or_else(|| json!({"state":"unknown"})))
}
pub(super) async fn handle_mutation(
    database: &Mutex<Database>,
    capacity: &Arc<Semaphore>,
    client: &crate::draft_client::DraftClient,
    method: &str,
    value: Value,
) -> Result<Reply> {
    let (execution, thread, conversation) = match method {
        "delegations.message.send" => {
            let envelope = {
                let db = database.lock().await;
                send_envelope(&db, client, value)?
            };
            let execution = super::private_sources::command(
                database,
                capacity,
                &envelope,
                |db| delegation_user::send_source_inputs(db, &envelope),
                |db, prepared| delegation_user::send_with_prepared_sources(db, &envelope, prepared),
            )
            .await?;
            (
                execution,
                envelope.payload.camp_id,
                envelope.payload.conversation_id,
            )
        }
        "delegations.pendingInputs.edit" => {
            let envelope = {
                let db = database.lock().await;
                edit_envelope(&db, client, value)?
            };
            let execution = super::private_sources::command(
                database,
                capacity,
                &envelope,
                |db| delegation_user::edit_source_inputs(db, &envelope),
                |db, prepared| delegation_user::edit_with_prepared_sources(db, &envelope, prepared),
            )
            .await?;
            (
                execution,
                envelope.payload.0.camp_id,
                envelope.payload.0.conversation_id,
            )
        }
        _ => anyhow::bail!("delegation.operation_not_admitted"),
    };
    let changed = (!execution.replayed && execution.result.status != CommandResultStatus::Rejected)
        .then_some((thread, conversation));
    Ok(Reply {
        result: serde_json::to_value(execution.result)?,
        changed,
    })
}
