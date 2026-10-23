//! Migration212 只追加按运行归属的用量投影，不修改已有模型输入和原用量事实。
use super::*;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE runtime_usage_activity (
    collection_epoch TEXT NOT NULL,
    agent_run_id TEXT NOT NULL,
    bucket_start_at TEXT NOT NULL,
    runtime_kind TEXT NOT NULL,
    provider_key TEXT NOT NULL,
    model_key TEXT NOT NULL,
    project_root TEXT,
    partial_observations INTEGER NOT NULL CHECK(partial_observations>=0),
    prompt_input_total_tokens INTEGER CHECK(prompt_input_total_tokens IS NULL OR prompt_input_total_tokens>=0),
    uncached_input_tokens INTEGER CHECK(uncached_input_tokens IS NULL OR uncached_input_tokens>=0),
    cache_read_tokens INTEGER CHECK(cache_read_tokens IS NULL OR cache_read_tokens>=0),
    cache_write_tokens INTEGER CHECK(cache_write_tokens IS NULL OR cache_write_tokens>=0),
    output_tokens INTEGER CHECK(output_tokens IS NULL OR output_tokens>=0),
    PRIMARY KEY(collection_epoch,agent_run_id,bucket_start_at,runtime_kind,provider_key,model_key)
);
CREATE INDEX runtime_usage_activity_range ON runtime_usage_activity(collection_epoch,bucket_start_at);
CREATE INDEX runtime_usage_activity_project ON runtime_usage_activity(collection_epoch,project_root,bucket_start_at);
"#;
fn objects(connection: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut query=connection.prepare("SELECT name,sql FROM sqlite_schema WHERE name GLOB 'runtime_usage_activity*' AND sql IS NOT NULL ORDER BY name")?;
    query
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect()
}
pub(super) fn absent(connection: &Connection) -> rusqlite::Result<bool> {
    Ok(objects(connection)?.is_empty())
}
pub(super) fn schema_matches(connection: &Connection) -> rusqlite::Result<bool> {
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(SCHEMA)?;
    Ok(objects(connection)? == objects(&reference)?)
}
pub(super) fn migrate(database: &mut Database) -> Result<()> {
    let tx = database
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    anyhow::ensure!(
        matches!(classify_database_contract(&tx)?,DatabaseContractClassification::SupportedMigrationSource(ref marker) if marker.projection_schema_version==161)
            && absent(&tx)?,
        "Usage activity requires complete schema161"
    );
    tx.execute_batch(SCHEMA)?;
    tx.execute_batch("INSERT INTO schema_migration VALUES(212,datetime('now'));UPDATE rovai_data_contract SET projection_schema_version=162,updated_at=datetime('now') WHERE singleton=1;")?;
    anyhow::ensure!(
        matches!(classify_database_contract(&tx)?,DatabaseContractClassification::Current(ref marker) | DatabaseContractClassification::SupportedMigrationSource(ref marker) if marker.projection_schema_version==162),
        "Usage activity target admission failed"
    );
    tx.commit()?;
    Ok(())
}
#[cfg(test)]
pub(super) fn downgrade_for_test(connection: &Connection) -> Result<()> {
    let objects = objects(connection)?;
    if !objects.is_empty() {
        anyhow::ensure!(
            schema_matches(connection)?,
            "Usage activity fixture reset requires the exact owned schema"
        );
    }
    crate::db::downgrade_owned_migration_for_test(connection, 212, 162, 161, &objects, &[], |_| {
        Ok(())
    })
}
