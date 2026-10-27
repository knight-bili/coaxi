//! 资料选择器只拿Core签发的编号、摘要和可见边界，不接触加密定位或原日志路径。
use super::*;
use crate::{
    command::{
        ActorRef, CommandEnvelope, CommandExecution, CommandHandlerResult, DomainCommand,
        DomainCommandGateway, sealed,
    },
    draft_client::DraftClient,
};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

fn descriptor(source: &sources::Source, snapshot: &source_projection::Snapshot) -> Result<Value> {
    let value = json!({"sourceSessionId":source.id,"sourceDigest":snapshot.source_digest,"throughSequence":snapshot.through_sequence,"sourceKind":source.source_kind,"title":snapshot.metadata["title"].as_str().unwrap_or("未命名会话").chars().take(120).collect::<String>()});
    ensure!(
        serde_json::to_vec(&value)?.len() <= 4096,
        "conversation_tools.source_descriptor_too_large"
    );
    Ok(value)
}
pub fn live_descriptor(connection: &Connection, conversation: &str) -> Result<Value> {
    let source = sources::live_record(connection, conversation)?;
    let snapshot = source_projection::live(connection, &source, None)?;
    descriptor(&source, &snapshot)
}
/// Read Side不登记来源、不自动生成授权；只有明确的预览或准备操作写入来源记录。
pub fn list(
    connection: &Connection,
    scope: &ScopeV1,
    after: Option<&str>,
    limit: u32,
) -> Result<Value> {
    scope::require_scope(connection, scope)?;
    ensure!((1..=50).contains(&limit), "conversation_tools.source_limit");
    if let Some(after) = after {
        uuid::Uuid::parse_str(after)?;
    }
    let mut query=connection.prepare(r#"
        SELECT id,descriptor FROM(
            SELECT c.id AS id,NULL AS descriptor FROM conversation c JOIN camp ON camp.id=c.camp_id
            WHERE camp.deletion_operation_id IS NULL AND(c.kind IN ('single_chat','delegation_child') AND ?1<>'public'
                OR c.kind='camp_member' AND c.id=(SELECT MIN(other.id) FROM conversation other WHERE other.camp_id=c.camp_id AND other.kind='camp_member'))
            UNION ALL
            SELECT s.id,s.descriptor_json FROM conversation_tool_source s JOIN camp ON camp.id=s.owner_thread_id
            WHERE s.source_kind='imported' AND s.descriptor_json IS NOT NULL AND ?1<>'public' AND camp.deletion_operation_id IS NULL
        ) WHERE(?2 IS NULL OR id>?2) ORDER BY id LIMIT ?3
    "#)?;
    let mut rows = query
        .query_map(
            params![scope::kind(&scope.scope_kind), after, limit + 1],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next = if more {
        rows.last().map(|row| row.0.clone())
    } else {
        None
    };
    let items = rows
        .into_iter()
        .map(|(id, cached)| match cached {
            Some(raw) => serde_json::from_str::<Value>(&raw).map_err(Into::into),
            None => live_descriptor(connection, &id),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({"items":items,"next":next}))
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareImport {
    #[serde(default, skip_deserializing)]
    pub client: DraftClient,
    pub thread_id: String,
    pub import_id: String,
    pub expected_revision: String,
}
impl sealed::Sealed for PrepareImport {}
impl DomainCommand for PrepareImport {
    const TYPE: &'static str = "conversationTools.source.prepareImported";
    const EXACT_REPLAY: bool = true;
    fn draft_client(&self) -> Option<&DraftClient> {
        Some(&self.client)
    }
}
pub fn prepare_import(
    database: &mut crate::db::Database,
    root: &Path,
    envelope: &CommandEnvelope<PrepareImport>,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(database, envelope, |tx| {
        ensure!(
            matches!(&envelope.actor, ActorRef::User { .. }) && envelope.camp_id.is_none(),
            "conversation_tools.user_required"
        );
        let p = &envelope.payload;
        scope::require_retained_scope(
            tx,
            &ScopeV1 {
                thread_id: p.thread_id.clone(),
                conversation_id: None,
                scope_kind: ScopeKindV1::Public,
            },
        )?;
        let source = sources::register_import(tx, root, &p.thread_id, &p.import_id)?;
        let sources::Locator::Imported {
            manifest_digest, ..
        } = &source.locator
        else {
            anyhow::bail!("conversation_tools.import_required")
        };
        ensure!(
            manifest_digest == &p.expected_revision,
            "conversation_tools.source_changed"
        );
        let snapshot = source_projection::imported(root, &source, None)?;
        let value = descriptor(&source, &snapshot)?;
        tx.execute(
            "UPDATE conversation_tool_source SET descriptor_json=?2 WHERE id=?1",
            params![source.id, serde_json::to_string(&value)?],
        )?;
        crate::collaboration::append_domain_event(
            tx,
            "conversationTools.sourcePrepared",
            Some(&p.thread_id),
            Some(("conversation_tool_source", &source.id)),
            &envelope.actor,
            None,
            &json!({"sourceSessionId":source.id,"sourceDigest":snapshot.source_digest}),
        )?;
        Ok(CommandHandlerResult::applied(
            "conversation_tools.source_prepared",
            value,
            None,
        ))
    })
}
pub fn imported_descriptor(
    connection: &Connection,
    owner: &str,
    id: &str,
    revision: &str,
) -> Result<Option<Value>> {
    let raw:Option<String>=connection.query_row("SELECT descriptor_json FROM conversation_tool_source WHERE owner_thread_id=?1 AND import_id=?2 AND import_revision=?3 AND descriptor_json IS NOT NULL",params![owner,id,revision],|row|row.get(0)).optional()?;
    raw.map(|raw| serde_json::from_str::<Value>(&raw).map_err(Into::into))
        .transpose()
}
