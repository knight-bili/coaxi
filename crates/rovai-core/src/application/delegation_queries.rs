//! Host-verified User entry points; business authority stays in Core.
use super::*;
use crate::delegation_resume::user as resume;
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResumeParams {
    command_id: String,
    command: crate::delegation_user::ResumeDelegationCommand,
}
pub(crate) fn resume_envelope(
    value: Value,
) -> Result<CommandEnvelope<crate::delegation_user::ResumeDelegationCommand>> {
    let p: ResumeParams = serde_json::from_value(value)?;
    // A global User receipt remains recoverable after its target Thread is gone.
    // The handler derives and checks the actual Thread before admitting work.
    Ok(user_command_envelope(p.command_id, p.command))
}
pub(crate) fn reconcile_resume(db: &Database, value: Value) -> Result<Value> {
    Ok(DomainCommandGateway
        .replay_if_recorded(db, &resume_envelope(value)?)?
        .map(|r| json!({"state":"recorded","result":r.result}))
        .unwrap_or_else(|| json!({"state":"unknown"})))
}
pub(crate) async fn resume<B: std::borrow::BorrowMut<Database>>(
    database: &tokio::sync::Mutex<B>,
    capacity: &Arc<tokio::sync::Semaphore>,
    value: Value,
) -> Result<CommandExecution> {
    let envelope = resume_envelope(value)?;
    let prepared = {
        let mut db = database.lock().await;
        let db = db.borrow_mut();
        if let Some(recorded) = DomainCommandGateway.replay_if_recorded(db, &envelope)? {
            return Ok(recorded);
        }
        resume::prepare(db, &envelope)?
    };
    let permit = capacity
        .clone()
        .acquire_owned()
        .await
        .context("delegation.directory_unavailable")?;
    let (prepared, observation) = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let observation = prepared.observe_directory();
        (prepared, observation)
    })
    .await
    .context("delegation.directory_unavailable")?;
    let mut db = database.lock().await;
    resume::commit(db.borrow_mut(), &envelope, &prepared, &observation)
}
