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
