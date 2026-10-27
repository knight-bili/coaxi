//! User transport only. Original native request and ownership never come from the UI.
use super::*;
use crate::runtime_interaction::storage::{self, ResolveInteractionCommand};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GetParams {
    interaction_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResolveParams {
    command_id: String,
    command: ResolveInteractionCommand,
}

pub(crate) fn envelope(value: Value) -> Result<CommandEnvelope<ResolveInteractionCommand>> {
    let params: ResolveParams = serde_json::from_value(value)?;
    // Scope comes from the durable request in the handler. A receipt does not
    // depend on that target still existing when the User reconnects.
    Ok(user_command_envelope(params.command_id, params.command))
}

pub(crate) fn read(database: &Database, method: &str, value: Value) -> Result<Value> {
    let actor = ActorRef::User {
        user_id: CURRENT_USER_ID.into(),
    };
    match method {
        "runtime.interactions.list" => Ok(serde_json::to_value(storage::list(
            database,
            &actor,
            &serde_json::from_value(value)?,
        )?)?),
        "runtime.interactions.get" => {
            let params: GetParams = serde_json::from_value(value)?;
            let Some((camp, _)) = storage::owner(database.connection(), &params.interaction_id)?
            else {
                return Ok(Value::Null);
            };
            Ok(serde_json::to_value(storage::read(
                database,
                &actor,
                &camp,
                &params.interaction_id,
            )?)?)
        }
        _ => anyhow::bail!("interaction.operation_not_admitted"),
    }
}

pub(crate) fn resolve(
    database: &mut Database,
    value: Value,
    generation: &str,
) -> Result<CommandExecution> {
    storage::resolve(database, &envelope(value)?, generation)
}

pub(crate) fn reconcile(database: &Database, value: Value) -> Result<Value> {
    Ok(DomainCommandGateway
        .replay_if_recorded(database, &envelope(value)?)?
        .map(|execution| json!({"state":"recorded","result":execution.result}))
        .unwrap_or_else(|| json!({"state":"unknown"})))
}

/// Both native callbacks and User decisions invalidate the same authoritative
/// child views. Notifications contain only stable IDs and versions.
pub(super) async fn changed(core: &Core, id: &str) {
    let metadata = {
        let db = core.database.lock().await;
        db.connection()
            .query_row(
                "SELECT i.camp_id,i.conversation_id,i.version,d.id FROM runtime_user_interaction i
             LEFT JOIN delegation d ON d.child_conversation_id=i.conversation_id WHERE i.id=?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()
    };
    if let Ok(Some((thread, conversation, version, delegation))) = metadata {
        emit(
            &core.output,
            "runtime.interaction.changed",
            json!({
                "threadId":thread,"conversationId":conversation,"interactionId":id,"version":version
            }),
        );
        if let Some(delegation) = delegation {
            emit(
                &core.output,
                "delegation.changed",
                json!({
                    "threadId":thread,"conversationId":conversation,"delegationId":delegation,
                    "interactionId":id,"interactionVersion":version
                }),
            );
        }
    }
}
