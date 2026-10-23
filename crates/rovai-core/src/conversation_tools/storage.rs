use super::*;
use rusqlite::Connection;

fn installed(connection:&Connection)->rusqlite::Result<Vec<(String,String)>>{
    let mut query=connection.prepare("SELECT name,sql FROM sqlite_schema WHERE lower(name) GLOB 'conversation_tool_*' AND sql IS NOT NULL ORDER BY name")?;
    query.query_map([],|row|Ok((row.get(0)?,row.get(1)?)))?.collect()
}
pub(crate) fn absent(connection:&Connection)->rusqlite::Result<bool>{Ok(installed(connection)?.is_empty())}
pub(crate) fn schema_matches(connection:&Connection)->rusqlite::Result<bool>{
    let reference=Connection::open_in_memory()?;reference.execute_batch(include_str!("schema.sql"))?;
    Ok(installed(connection)?==installed(&reference)?)
}
#[cfg(test)]
pub(crate) fn downgrade_for_test(connection:&Connection)->anyhow::Result<()>{
    let objects=installed(connection)?;
    if !objects.is_empty(){anyhow::ensure!(schema_matches(connection)?,"Conversation Tools fixture reset requires the exact owned schema");}
    crate::db::downgrade_owned_migration_for_test(connection,217,167,166,&objects,&[],|_|Ok(()))
}
pub fn migrate(database:&mut crate::db::Database)->Result<()>{
    let tx=database.connection_mut().transaction()?;
    ensure!(matches!(crate::db::classify_portable_configuration_166_contract(&tx)?,crate::db::DatabaseContractClassification::Current(ref marker) if marker.projection_schema_version==166)&&absent(&tx)?,"conversation_tools.schema_source_invalid");
    tx.execute_batch(include_str!("schema.sql"))?;
    tx.execute_batch("INSERT INTO schema_migration(version,applied_at) VALUES(217,datetime('now'));UPDATE rovai_data_contract SET projection_schema_version=167,updated_at=datetime('now') WHERE singleton=1;")?;
    ensure!(matches!(crate::db::classify_database_contract(&tx)?,crate::db::DatabaseContractClassification::Current(ref marker) if marker.projection_schema_version==167),"conversation_tools.schema_target_invalid");
    tx.commit()?;Ok(())
}
