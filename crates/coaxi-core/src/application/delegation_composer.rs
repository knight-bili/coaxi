//! Child input entry points keep their own command identity and trusted editor.
use super::private_composer::Reply;
use super::*;
use crate::{
    delegation_user::{self, MutateDelegationComposerCommand},
    single_chat_composer::{self, PrivateRecoveryParams},
    single_chat_composer_scope::SingleChatComposerScopeRequest,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnapshotParams {
    conversation_id: String,
    #[serde(default)]
    composer_scope: SingleChatComposerScopeRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryParams {
    conversation_id: String,
    #[serde(default)]
    composer_scope: SingleChatComposerScopeRequest,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MutationParams {
    command_id: String,
    command: MutateDelegationComposerCommand,
}

pub(crate) fn mutation_envelope(
    client: &crate::draft_client::DraftClient,
    value: Value,
) -> Result<CommandEnvelope<MutateDelegationComposerCommand>> {
    let mut params: MutationParams = serde_json::from_value(value)?;
    params.command.0.draft_client = client.clone();
    Ok(user_camp_command_envelope(
        params.command_id,
        params.command.0.camp_id.clone(),
        params.command,
    ))
}

pub(crate) fn reconcile(
    database: &Database,
    client: &crate::draft_client::DraftClient,
    value: Value,
) -> Result<Value> {
    let envelope = mutation_envelope(client, value)?;
    Ok(DomainCommandGateway
        .replay_if_recorded(database, &envelope)?
        .map(|execution| json!({"state":"recorded","result":execution.result}))
        .unwrap_or_else(|| json!({"state":"unknown"})))
}

pub(crate) fn handle(
    database: &Database,
    client: &crate::draft_client::DraftClient,
    method: &str,
    value: Value,
) -> Result<Value> {
    let (conversation, scope, cursor, limit) = match method {
        "delegations.composer.get" | "delegations.conversation.get" => {
            let p: SnapshotParams = serde_json::from_value(value)?;
            (p.conversation_id, p.composer_scope, None, None)
        }
        "delegations.composer.recovery" => {
            let p: RecoveryParams = serde_json::from_value(value)?;
            (p.conversation_id, p.composer_scope, p.cursor, p.limit)
        }
        _ => anyhow::bail!("delegation.operation_not_admitted"),
    };
    let thread: String = database
        .connection()
        .query_row(
            "SELECT camp_id FROM conversation WHERE id=?1 AND kind='delegation_child'",
            [&conversation],
            |r| r.get(0),
        )
        .optional()?
        .context("delegation.not_found")?;
    let actor = ActorRef::User {
        user_id: CURRENT_USER_ID.into(),
    };
    let draft = delegation_user::read_draft(
        database,
        &actor,
        client,
        &thread,
        &conversation,
        scope.scope(),
    )?;
    match method {
        "delegations.conversation.get" => {
            let key =
                crate::single_chat_composer_scope::PrivateComposerKey::for_scope(scope.scope())?;
            let snapshot = SingleChatService::for_client(client.clone())
                .snapshot_for_owner(
                    database,
                    &conversation,
                    &key,
                    single_chat_composer::ComposerOwner::DelegationChild,
                )?
                .context("delegation.not_found")?;
            let editing_elsewhere: bool = database.connection().query_row(
                "SELECT EXISTS(SELECT 1 FROM single_chat_pending_input_edit_session WHERE conversation_id=?1 AND (client_id<>?2 OR scope_key<>?3))",
                rusqlite::params![conversation, client.id(), key.sql_key()], |row| row.get(0),
            )?;
            let mut value = serde_json::to_value(snapshot)?;
            value["draft"] = serde_json::to_value(draft)?;
            value["pendingInputs"]["editingElsewhere"] = json!(editing_elsewhere);
            Ok(value)
        }
        "delegations.composer.get" => Ok(serde_json::to_value(draft)?),
        "delegations.composer.recovery" => Ok(serde_json::to_value(delegation_user::recovery(
            database,
            &actor,
            client,
            &PrivateRecoveryParams {
                camp_id: thread,
                conversation_id: conversation,
                cursor,
                limit,
            },
        )?)?),
        _ => anyhow::bail!("delegation.operation_not_admitted"),
    }
}

pub(super) async fn handle_mutation(
    database: &Mutex<Database>,
    capacity: &Arc<Semaphore>,
    client: &crate::draft_client::DraftClient,
    params: Value,
) -> Result<Reply> {
    let envelope = mutation_envelope(client, params)?;
    let execution = super::private_sources::command(
        database,
        capacity,
        &envelope,
        |database| single_chat_composer::mutation_source_inputs(database, &envelope),
        |database, prepared| {
            single_chat_composer::mutate_with_prepared_sources(database, &envelope, &prepared)
        },
    )
    .await?;
    let changed = (!execution.replayed && execution.result.status != CommandResultStatus::Rejected)
        .then(|| {
            (
                envelope.payload.0.camp_id.clone(),
                envelope.payload.0.conversation_id.clone(),
            )
        });
    Ok(Reply {
        result: serde_json::to_value(execution.result)?,
        changed,
    })
}
