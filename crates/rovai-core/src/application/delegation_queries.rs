//! Host-verified User entry points; business authority stays in Core.
use super::*;
use crate::{delegation_model_query::user, delegation_settings};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SettingsUpdate {
    command_id: String,
    command: delegation_settings::UpdateSettingsParams,
}
fn actor() -> ActorRef {
    ActorRef::User {
        user_id: CURRENT_USER_ID.into(),
    }
}
pub(crate) fn read(db: &Database, method: &str, value: Value) -> Result<Value> {
    let actor = actor();
    match method {
        "delegations.list" => user::list(db, &actor, &serde_json::from_value(value)?),
        "delegations.get" => user::get(db, &actor, &serde_json::from_value(value)?),
        "delegations.history" => user::history(db, &actor, &serde_json::from_value(value)?),
        "delegations.read" => user::read(db, &actor, &serde_json::from_value(value)?),
        "delegations.settings.get" => {
            let _: Empty = serde_json::from_value(value)?;
            Ok(serde_json::to_value(delegation_settings::read_user(
                db, &actor,
            )?)?)
        }
        _ => anyhow::bail!("delegation.operation_not_admitted"),
    }
}
fn settings_envelope(
    value: Value,
) -> Result<CommandEnvelope<delegation_settings::UpdateSettingsParams>> {
    let p: SettingsUpdate = serde_json::from_value(value)?;
    Ok(user_command_envelope(p.command_id, p.command))
}
pub(crate) fn reconcile(db: &Database, value: Value) -> Result<Value> {
    Ok(DomainCommandGateway
        .replay_if_recorded(db, &settings_envelope(value)?)?
        .map(|r| json!({"state":"recorded","result":r.result}))
        .unwrap_or_else(|| json!({"state":"unknown"})))
}
pub(crate) fn update_settings(db: &mut Database, value: Value) -> Result<CommandExecution> {
    delegation_settings::update_user(db, &settings_envelope(value)?)
}
