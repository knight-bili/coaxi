use super::*;
use rusqlite::{Connection, OptionalExtension, params};

pub(crate) fn scope(value: &NativeSteerScopeV1) -> Result<()> {
    validate_shape("NativeSteerScopeV1", &serde_json::to_value(value)?)?;
    crate::camp_id::ThreadId::parse(&value.thread_id)?;
    ensure!(
        if value.scope_kind == "public" {
            value.conversation_id.is_none()
        } else {
            value.conversation_id.is_some()
        },
        "native_lifecycle.scope_invalid"
    );
    Ok(())
}
pub(crate) fn require(
    connection: &Connection,
    target: &NativeSteerTargetV1,
) -> Result<crate::agent_profile::FrozenAgentRuntimeConfig> {
    scope(&target.scope)?;
    validate_shape("NativeSteerTargetV1", &serde_json::to_value(target)?)?;
    let raw:Option<String>=connection.query_row(r#"
        SELECT r.effective_config_json FROM agent_run r
        JOIN conversation c ON c.id=r.conversation_id JOIN camp ON camp.id=c.camp_id
        JOIN camp_member m ON m.camp_id=camp.id AND m.agent_id=c.agent_id
        LEFT JOIN camp_turn t ON t.id=r.camp_turn_id
        WHERE r.id=?1 AND r.execution_epoch=?2 AND r.status='running' AND r.cancel_requested_at IS NULL
        AND camp.id=?3 AND camp.activation_state='active' AND camp.deletion_operation_id IS NULL
        AND c.agent_id=?4 AND c.ended_at IS NULL AND c.native_binding_id=?5 AND c.native_binding_generation=?6
        AND c.native_binding_compatibility_digest=?7 AND c.native_session_id IS NOT NULL
        AND m.status='active' AND m.leave_requested_at IS NULL
        AND m.version=CAST(json_extract(r.effective_config_json,'$.campMemberVersion') AS INTEGER)
        AND ((?8='public' AND c.kind='camp_member' AND ?9 IS NULL) OR(c.kind=?8 AND c.id=?9))
        AND (r.camp_turn_id IS NULL OR(t.status IN ('running','waiting') AND t.cancel_requested_at IS NULL
             AND t.execution_budget_exhausted_at IS NULL AND(t.execution_budget_deadline_at>?10
             OR(t.execution_budget_deadline_at IS NULL AND t.execution_budget_schema_version=2))))
    "#,params![target.agent_run_id,target.execution_epoch,target.scope.thread_id,target.agent_id,target.native_binding_id,target.native_binding_generation,target.binding_compatibility_digest,target.scope.scope_kind,target.scope.conversation_id,crate::execution_budget::camp_turn_execution_budget_now().to_rfc3339()],|row|row.get(0)).optional()?;
    let config: Value =
        serde_json::from_str(&raw.context("native_lifecycle.target_no_longer_active")?)?;
    let runtime: crate::agent_profile::FrozenAgentRuntimeConfig =
        serde_json::from_value(config["runtime"].clone())?;
    runtime
        .native_steer
        .as_ref()
        .context("native_lifecycle.not_frozen_for_run")?
        .validate()?;
    ensure!(
        runtime.binding_compatibility_digest == target.binding_compatibility_digest,
        "native_lifecycle.binding_changed"
    );
    Ok(runtime)
}
pub(crate) fn candidates(
    connection: &Connection,
    scope: &NativeSteerScopeV1,
) -> Result<Vec<NativeSteerTargetV1>> {
    self::scope(scope)?;
    let mut statement=connection.prepare(r#"
        SELECT r.id,r.execution_epoch,c.agent_id,c.native_binding_id,c.native_binding_generation,c.native_binding_compatibility_digest
        FROM agent_run r JOIN conversation c ON c.id=r.conversation_id JOIN camp ON camp.id=c.camp_id
        WHERE camp.id=?1 AND camp.activation_state='active' AND camp.deletion_operation_id IS NULL
        AND r.status='running' AND r.cancel_requested_at IS NULL AND c.ended_at IS NULL
        AND c.native_binding_id IS NOT NULL AND c.native_binding_compatibility_digest IS NOT NULL
        AND json_extract(r.effective_config_json,'$.runtime.nativeSteer.enabled')=1
        AND ((?2='public' AND c.kind='camp_member' AND ?3 IS NULL)OR(c.kind=?2 AND c.id=?3))
        ORDER BY r.created_at DESC,r.id LIMIT 200
    "#)?;
    let rows = statement.query_map(
        params![scope.thread_id, scope.scope_kind, scope.conversation_id],
        |row| {
            Ok(NativeSteerTargetV1 {
                scope: scope.clone(),
                agent_run_id: row.get(0)?,
                execution_epoch: row.get(1)?,
                agent_id: row.get(2)?,
                native_binding_id: row.get(3)?,
                native_binding_generation: row.get(4)?,
                binding_compatibility_digest: row.get(5)?,
                active_turn_id: String::new(),
            })
        },
    )?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}
