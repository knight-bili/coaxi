use super::*;
use rusqlite::{Connection, OptionalExtension, params};

pub(crate) fn kind(value: &ScopeKindV1) -> &'static str {
    match value {
        ScopeKindV1::Public => "public",
        ScopeKindV1::SingleChat => "single_chat",
        ScopeKindV1::DelegationChild => "delegation_child",
    }
}
pub(crate) fn require_scope(connection: &Connection, scope: &ScopeV1) -> Result<()> {
    scope.validate()?;
    crate::camp_id::ThreadId::parse(&scope.thread_id)?;
    ensure!(
        matches!(&scope.scope_kind, ScopeKindV1::Public) == scope.conversation_id.is_none(),
        "conversation_tools.scope_invalid"
    );
    let allowed:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM camp WHERE id=?1 AND activation_state='active' AND deletion_operation_id IS NULL AND (?2='public' OR EXISTS(SELECT 1 FROM conversation c WHERE c.camp_id=camp.id AND c.id=?3 AND c.kind=?2 AND c.ended_at IS NULL)))",params![scope.thread_id,kind(&scope.scope_kind),scope.conversation_id],|row|row.get(0))?;
    ensure!(allowed, "conversation_tools.scope_changed");
    Ok(())
}
pub(crate) fn require_retained_scope(connection: &Connection, scope: &ScopeV1) -> Result<()> {
    scope.validate()?;
    crate::camp_id::ThreadId::parse(&scope.thread_id)?;
    ensure!(
        matches!(&scope.scope_kind, ScopeKindV1::Public) == scope.conversation_id.is_none(),
        "conversation_tools.scope_invalid"
    );
    let present:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM camp WHERE id=?1 AND deletion_operation_id IS NULL AND (?2='public' OR EXISTS(SELECT 1 FROM conversation c WHERE c.camp_id=camp.id AND c.id=?3 AND c.kind=?2)))",params![scope.thread_id,kind(&scope.scope_kind),scope.conversation_id],|row|row.get(0))?;
    ensure!(present, "conversation_tools.scope_changed");
    Ok(())
}
/// 仅由 Core 核对已冻结的真实 Run 后创建，不能由模型 JSON 或客户端反序列化。
#[derive(Clone)]
pub(crate) struct RunLease {
    pub run_id: String,
    pub epoch: i64,
    pub frozen: FrozenConversationToolsV1,
}
/// 仅供已认证User核对原记录；结束运行后仍可读取回执，不能据此调用模型工具。
pub(crate) fn retained_run(connection: &Connection, id: &str, epoch: i64) -> Result<RunLease> {
    let raw:String=connection.query_row("SELECT owned.frozen_json FROM conversation_tool_run owned JOIN agent_run r ON r.id=owned.agent_run_id JOIN conversation c ON c.id=r.conversation_id JOIN camp ON camp.id=c.camp_id WHERE owned.agent_run_id=?1 AND owned.execution_epoch=?2 AND owned.thread_id=camp.id AND owned.agent_id=c.agent_id AND camp.deletion_operation_id IS NULL",params![id,epoch],|row|row.get(0)).context("conversation_tools.original_run_unavailable")?;
    let frozen: FrozenConversationToolsV1 = serde_json::from_str(&raw)?;
    frozen.validate()?;
    Ok(RunLease {
        run_id: id.into(),
        epoch,
        frozen,
    })
}
pub(crate) fn request_owner(connection: &Connection, id: &str) -> Result<Option<RunLease>> {
    uuid::Uuid::parse_str(id)?;
    let owner: Option<(String, i64)> = connection
        .query_row(
            "SELECT agent_run_id,execution_epoch FROM conversation_tool_request WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    owner
        .map(|(run, epoch)| retained_run(connection, &run, epoch))
        .transpose()
}
pub(crate) fn require_run(connection: &Connection, id: &str, epoch: i64) -> Result<RunLease> {
    uuid::Uuid::parse_str(id)?;
    ensure!(
        (0..=9_007_199_254_740_991).contains(&epoch),
        "conversation_tools.epoch_invalid"
    );
    let row:Option<(String,String)>=connection.query_row(r#"
        SELECT r.effective_config_json,owned.frozen_json FROM agent_run r
        JOIN conversation c ON c.id=r.conversation_id JOIN camp ON camp.id=c.camp_id
        JOIN camp_member m ON m.camp_id=camp.id AND m.agent_id=c.agent_id
        JOIN conversation_tool_run owned ON owned.agent_run_id=r.id AND owned.execution_epoch=r.execution_epoch
        LEFT JOIN camp_turn t ON t.id=r.camp_turn_id
        WHERE r.id=?1 AND r.execution_epoch=?2 AND r.status='running' AND r.cancel_requested_at IS NULL
        AND c.ended_at IS NULL AND camp.activation_state='active' AND camp.deletion_operation_id IS NULL
        AND m.status='active' AND m.leave_requested_at IS NULL
        AND m.version=CAST(json_extract(r.effective_config_json,'$.campMemberVersion') AS INTEGER)
        AND owned.agent_id=c.agent_id AND owned.thread_id=camp.id AND owned.revoked_at IS NULL
        AND ((owned.scope_kind='public' AND c.kind='camp_member' AND owned.conversation_id IS NULL)
             OR(owned.scope_kind=c.kind AND owned.conversation_id=c.id))
        AND (r.camp_turn_id IS NULL OR(t.status IN ('running','waiting') AND t.cancel_requested_at IS NULL
            AND t.execution_budget_exhausted_at IS NULL AND(t.execution_budget_deadline_at>?3
                OR(t.execution_budget_deadline_at IS NULL AND t.execution_budget_schema_version=2))))
    "#,params![id,epoch,crate::execution_budget::camp_turn_execution_budget_now().to_rfc3339()],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
    let (config, owned) = row.context("conversation_tools.run_unavailable")?;
    let config: Value = serde_json::from_str(&config)?;
    let frozen: FrozenConversationToolsV1 = serde_json::from_str(&owned)?;
    frozen.validate()?;
    ensure!(
        config.pointer("/runtime/conversationTools") == Some(&serde_json::to_value(&frozen)?),
        "conversation_tools.frozen_run_changed"
    );
    require_scope(connection, &frozen.scope)?;
    Ok(RunLease {
        run_id: id.into(),
        epoch,
        frozen,
    })
}
pub(crate) fn register_run(
    connection: &Connection,
    id: &str,
    epoch: i64,
    frozen: &FrozenConversationToolsV1,
) -> Result<()> {
    frozen.validate()?;
    let raw = serde_json::to_string(frozen)?;
    let stored:Option<String>=connection.query_row("SELECT frozen_json FROM conversation_tool_run WHERE agent_run_id=?1 AND execution_epoch=?2",params![id,epoch],|row|row.get(0)).optional()?;
    if let Some(stored) = stored {
        ensure!(
            serde_json::from_str::<FrozenConversationToolsV1>(&stored)? == *frozen,
            "conversation_tools.frozen_run_changed"
        );
        return Ok(());
    }
    connection.execute("INSERT INTO conversation_tool_run(agent_run_id,execution_epoch,agent_id,thread_id,conversation_id,scope_kind,policy_revision,policy_digest,catalog_digest,frozen_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![id,epoch,frozen.agent_id,frozen.scope.thread_id,frozen.scope.conversation_id,kind(&frozen.scope.scope_kind),frozen.policy_revision,frozen.policy_digest,frozen.catalog_digest,raw,chrono::Utc::now().to_rfc3339()])?;
    Ok(())
}
