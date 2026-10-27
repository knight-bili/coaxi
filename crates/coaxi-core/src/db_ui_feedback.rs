//! 已确认反馈 r1 的独立存储；旧消息和冻结模型输入不回填。
use super::*;
pub(crate) const SCHEMA: &str = include_str!("ui_feedback/schema.sql");
fn installed(connection: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut query=connection.prepare("SELECT name,sql FROM sqlite_schema WHERE lower(name) GLOB 'ui_feedback_*' AND sql IS NOT NULL ORDER BY name")?;
    query
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect()
}
pub(super) fn absent(connection: &Connection) -> rusqlite::Result<bool> {
    Ok(installed(connection)?.is_empty())
}
pub(super) fn schema_matches(connection: &Connection) -> rusqlite::Result<bool> {
    let reference = Connection::open_in_memory()?;
    reference.execute_batch("CREATE TABLE agent_run(id TEXT,status TEXT,execution_epoch INTEGER,cancel_requested_at TEXT);CREATE TABLE single_chat_pending_input(id TEXT,published_conversation_message_id TEXT);")?;
    reference.execute_batch(SCHEMA)?;
    Ok(installed(connection)? == installed(&reference)?)
}
pub(super) fn migrate(database: &mut Database) -> Result<()> {
    let tx = database
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    anyhow::ensure!(
        matches!(classify_database_contract(&tx)?,DatabaseContractClassification::SupportedMigrationSource(ref marker) if marker.projection_schema_version==162)
            && absent(&tx)?,
        "Feedback requires complete schema162"
    );
    tx.execute_batch(SCHEMA)?;
    tx.execute_batch("INSERT INTO schema_migration VALUES(213,datetime('now'));UPDATE coaxi_data_contract SET projection_schema_version=163,updated_at=datetime('now') WHERE singleton=1;")?;
    anyhow::ensure!(
        matches!(classify_feedback_163_contract(&tx)?,DatabaseContractClassification::Current(ref marker) if marker.projection_schema_version==163),
        "Feedback target admission failed"
    );
    tx.commit()?;
    Ok(())
}
#[cfg(test)]
pub(super) fn downgrade_for_test(connection: &Connection) -> Result<()> {
    let objects = installed(connection)?;
    if !objects.is_empty() {
        anyhow::ensure!(
            schema_matches(connection)?,
            "Feedback fixture reset requires the exact owned schema"
        );
        let policy: Option<(i64, i64, String)> = connection
            .query_row(
                "SELECT singleton,revision,value_json FROM ui_feedback_policy",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        anyhow::ensure!(
            policy
                == Some((
                    1,
                    0,
                    r#"{"schemaVersion":1,"revision":0,"members":[]}"#.into()
                )),
            "Cannot discard edited feedback policy in a fixture"
        );
        let other_rows:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM ui_feedback_preview) OR EXISTS(SELECT 1 FROM ui_feedback_entry) OR EXISTS(SELECT 1 FROM ui_feedback_command) OR EXISTS(SELECT 1 FROM ui_feedback_attempt) OR EXISTS(SELECT 1 FROM ui_feedback_continuation)",[],|row|row.get(0))?;
        anyhow::ensure!(!other_rows, "Cannot discard feedback fixture data");
    }
    crate::db::downgrade_owned_migration_for_test(
        connection,
        213,
        163,
        162,
        &objects,
        &["ui_feedback_policy"],
        |_| Ok(()),
    )
}
