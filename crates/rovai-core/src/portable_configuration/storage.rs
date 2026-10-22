use super::*;
use rusqlite::Connection;

fn installed(connection:&Connection)->rusqlite::Result<Vec<(String,String)>>{
    let mut statement=connection.prepare("SELECT name,sql FROM sqlite_schema WHERE (name GLOB 'portable_*' OR name GLOB 'configuration_import_*' OR name GLOB 'configuration_file_outbox*') AND sql IS NOT NULL ORDER BY name")?;
    statement.query_map([],|row|Ok((row.get(0)?,row.get(1)?)))?.collect()
}
pub(crate) fn absent(connection:&Connection)->rusqlite::Result<bool>{Ok(installed(connection)?.is_empty())}
pub(crate) fn schema_matches(connection:&Connection)->rusqlite::Result<bool>{let reference=Connection::open_in_memory()?;reference.execute_batch(include_str!("schema.sql"))?;Ok(installed(connection)?==installed(&reference)?)}
pub fn migrate(database:&mut crate::db::Database)->Result<()>{
    let tx=database.connection_mut().transaction()?;
    ensure!(matches!(crate::db::classify_native_lifecycle_165_contract(&tx)?,crate::db::DatabaseContractClassification::Current(ref marker) if marker.projection_schema_version==165)&&absent(&tx)?,"configuration.schema_source_invalid");
    tx.execute_batch(include_str!("schema.sql"))?;
    tx.execute_batch("INSERT INTO schema_migration(version,applied_at) VALUES(216,datetime('now'));UPDATE rovai_data_contract SET projection_schema_version=166,updated_at=datetime('now') WHERE singleton=1;")?;
    ensure!(matches!(crate::db::classify_database_contract(&tx)?,crate::db::DatabaseContractClassification::Current(ref marker) if marker.projection_schema_version==166),"configuration.schema_target_invalid");tx.commit()?;Ok(())
}
