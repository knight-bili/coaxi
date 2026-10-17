use super::private_composer::Reply;
use super::*;
use crate::delegation_control::{
    self as control, CancelDelegationCommand, TakeoverDelegationCommand,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Params<T> {
    command_id: String,
    command: T,
}
pub(crate) fn takeover_envelope(
    db: &Database,
    value: Value,
) -> Result<CommandEnvelope<TakeoverDelegationCommand>> {
    let p: Params<TakeoverDelegationCommand> = serde_json::from_value(value)?;
    let target = control::target(db.connection(), &p.command.delegation_id)?
        .context("delegation.not_found")?;
    Ok(user_camp_command_envelope(
        p.command_id,
        target.thread,
        p.command,
    ))
}
pub(crate) fn cancel_envelope(
    db: &Database,
    value: Value,
) -> Result<CommandEnvelope<CancelDelegationCommand>> {
    let p: Params<CancelDelegationCommand> = serde_json::from_value(value)?;
    let target = control::target(db.connection(), &p.command.delegation_id)?
        .context("delegation.not_found")?;
    Ok(user_camp_command_envelope(
        p.command_id,
        target.thread,
        p.command,
    ))
}
pub(crate) fn reconcile(db: &Database, method: &str, value: Value) -> Result<Value> {
    let recorded = match method {
        "delegations.takeover" => {
            DomainCommandGateway.replay_if_recorded(db, &takeover_envelope(db, value)?)?
        }
        "delegations.cancel" => {
            DomainCommandGateway.replay_if_recorded(db, &cancel_envelope(db, value)?)?
        }
        _ => anyhow::bail!("delegation.operation_not_admitted"),
    };
    Ok(recorded
        .map(|r| json!({"state":"recorded","result":r.result}))
        .unwrap_or_else(|| json!({"state":"unknown"})))
}
pub(crate) fn handle(db: &mut Database, method: &str, value: Value) -> Result<Reply> {
    let (execution, thread) = match method {
        "delegations.takeover" => {
            let e = takeover_envelope(db, value)?;
            (control::takeover(db, &e)?, e.camp_id.unwrap())
        }
        "delegations.cancel" => {
            let e = cancel_envelope(db, value)?;
            (control::cancel(db, &e)?, e.camp_id.unwrap())
        }
        _ => anyhow::bail!("delegation.operation_not_admitted"),
    };
    let changed = (!execution.replayed
        && matches!(
            execution.result.code.as_str(),
            "delegation.taken_over" | "delegation.cancelled"
        ))
    .then(|| {
        (
            thread,
            execution.result.payload["conversationId"]
                .as_str()
                .unwrap()
                .into(),
        )
    });
    Ok(Reply {
        result: serde_json::to_value(execution.result)?,
        changed,
    })
}
