//! Migration211 adds native User interaction ownership without changing old input bytes.
use super::*;

const SCHEMA: &str = include_str!("runtime_interaction/schema.sql");

fn objects() -> rusqlite::Result<Vec<(String, String)>> {
    let reference = Connection::open_in_memory()?;
    // The two lifecycle triggers attach to existing Core tables. These reference
    // placeholders are never included in the owned DDL comparison or used as data.
    reference
        .execute_batch("CREATE TABLE agent_run(id TEXT); CREATE TABLE conversation(id TEXT);")?;
    reference.execute_batch(SCHEMA)?;
    installed_objects(&reference)
}
fn installed_objects(connection: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut statement=connection.prepare("SELECT name,sql FROM sqlite_schema WHERE lower(name) GLOB 'runtime_user_interaction*' AND sql IS NOT NULL ORDER BY name")?;
    statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}
pub(crate) fn absent(connection: &Connection) -> rusqlite::Result<bool> {
    connection.query_row("SELECT NOT EXISTS(SELECT 1 FROM sqlite_schema WHERE lower(name) GLOB 'runtime_user_interaction*')",[],|r|r.get(0))
}
pub(crate) fn schema_matches(connection: &Connection) -> rusqlite::Result<bool> {
    // Exact owned DDL, including extra reserved objects. Never strip whitespace
    // inside string literals: that could turn a changed state name into a match.
    Ok(installed_objects(connection)? == objects()?)
}
pub(crate) fn migrate(database: &mut Database) -> Result<()> {
    let tx = database
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    if matches!(classify_database_contract(&tx)?,DatabaseContractClassification::Current(ref marker) | DatabaseContractClassification::SupportedMigrationSource(ref marker)
        if marker.projection_schema_version==161)
    {
        return Ok(());
    }
    anyhow::ensure!(
        matches!(classify_database_contract(&tx)?,DatabaseContractClassification::SupportedMigrationSource(ref marker)
        if marker.contract_version=="v1.73" && marker.projection_schema_version==160),
        "Native interactions require the complete v1.73/schema160 source"
    );
    tx.execute_batch(SCHEMA)?;
    tx.execute_batch("INSERT INTO schema_migration VALUES(211,datetime('now'));
        UPDATE rovai_data_contract SET projection_schema_version=161,updated_at=datetime('now') WHERE singleton=1;")?;
    anyhow::ensure!(
        matches!(classify_database_contract(&tx)?,DatabaseContractClassification::Current(ref marker) | DatabaseContractClassification::SupportedMigrationSource(ref marker)
        if marker.projection_schema_version==161),
        "Native interaction target admission failed"
    );
    tx.commit()?;
    Ok(())
}
#[cfg(test)]
pub(crate) fn downgrade_for_test(connection: &Connection) -> Result<()> {
    if absent(connection)? {
        return Ok(());
    }
    anyhow::ensure!(
        schema_matches(connection)?,
        "Only the exact interaction fixture can be downgraded"
    );
    anyhow::ensure!(
        connection.query_row("SELECT count(*) FROM runtime_user_interaction", [], |r| r
            .get::<_, i64>(
            0
        ))? == 0,
        "Cannot discard native interaction records"
    );
    let tx = connection.unchecked_transaction()?;
    tx.execute_batch("DROP TRIGGER runtime_user_interaction_run_closed;
        DROP TRIGGER runtime_user_interaction_binding_closed;
        DROP TABLE runtime_user_interaction;
        DELETE FROM schema_migration WHERE version=211;
        UPDATE rovai_data_contract SET projection_schema_version=160 WHERE singleton=1 AND contract_version='v1.73' AND projection_schema_version=161;")?;
    tx.commit()?;
    Ok(())
}
