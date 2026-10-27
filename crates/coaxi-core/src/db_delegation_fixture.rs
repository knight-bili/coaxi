//! Historical test setup only. No production downgrade or caller data deletion.
use super::*;

pub(super) fn downgrade(connection: &Connection) -> Result<()> {
    crate::conversation_tools::storage::downgrade_for_test(connection)?;
    crate::portable_configuration::storage::downgrade_for_test(connection)?;
    crate::native_lifecycle::storage::downgrade_for_test(connection)?;
    crate::custom_runtime::storage::downgrade_for_test(connection)?;
    crate::db::ui_feedback::downgrade_for_test(connection)?;
    crate::db::usage_activity::downgrade_for_test(connection)?;
    crate::db::runtime_interactions::downgrade_for_test(connection)?;
    if !connection.table_exists(None, "schema_migration")? {
        return Ok(());
    }
    let applied: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migration WHERE version=210)",
        [],
        |r| r.get(0),
    )?;
    if !applied {
        return Ok(());
    }
    anyhow::ensure!(
        connection.is_autocommit(),
        "Legacy fixture reset requires an idle connection"
    );
    anyhow::ensure!(
        schema_matches(connection)?,
        "Legacy fixture reset requires complete160 schema"
    );
    anyhow::ensure!(!connection.query_row("SELECT EXISTS(SELECT 1 FROM agent_run WHERE json_type(effective_config_json,'$.delegationPolicy') IS NOT NULL)",[],|r|r.get::<_,bool>(0))?,"Cannot downgrade frozen delegation Runs in a fixture");
    for (name, sql) in owned_objects() {
        if sql.starts_with("CREATE TABLE ") {
            let count: i64 =
                connection.query_row(&format!("SELECT count(*) FROM {name}"), [], |r| r.get(0))?;
            anyhow::ensure!(count == 0, "Cannot erase delegation fixture data: {name}");
        }
    }
    connection.execute_batch("PRAGMA foreign_keys=OFF;")?;
    let result = (|| -> Result<()> {
        let tx = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
        for name in MODIFIED_GUARDS {
            tx.execute_batch(&format!("DROP TRIGGER {name};"))?;
        }
        for (name, sql) in owned_objects() {
            if sql.starts_with("CREATE TRIGGER ") {
                tx.execute_batch(&format!("DROP TRIGGER {name};"))?;
            }
        }
        for (name, sql) in owned_objects() {
            if sql.starts_with("CREATE INDEX ") || sql.starts_with("CREATE UNIQUE INDEX ") {
                tx.execute_batch(&format!("DROP INDEX {name};"))?;
            }
        }
        for (name, sql) in owned_objects() {
            if sql.starts_with("CREATE TABLE ") {
                tx.execute_batch(&format!("DROP TABLE {name};"))?;
            }
        }
        tx.execute_batch("DROP TABLE delegation_settings;")?;
        for table in [
            "conversation",
            "camp_turn",
            "agent_run",
            "context_manifest",
            "agent_run_input",
        ] {
            let actual: String = tx.query_row(
                "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )?;
            let old = shape::legacy_table(table, &actual)?;
            rebuild_table_v171(
                &tx,
                table,
                &replacement_table_schema_v171(old, table),
                &[],
                &[],
            )?;
        }
        for name in MODIFIED_GUARDS {
            tx.execute_batch(&shape::source_guard(name)?)?;
        }
        tx.execute_batch("DELETE FROM schema_migration WHERE version=210;
            UPDATE coaxi_data_contract SET contract_version='v1.72',projection_schema_version=159,updated_at=datetime('now') WHERE singleton=1;")?;
        anyhow::ensure!(
            source_schema_matches(&tx)?,
            "Legacy fixture source restoration failed"
        );
        anyhow::ensure!(
            matches!(classify_database_contract(&tx)?,DatabaseContractClassification::SupportedMigrationSource(ref marker) if marker.contract_version=="v1.72" && marker.projection_schema_version==159),
            "Restored legacy fixture is not an admitted source"
        );
        validate_migration_foreign_keys(
            &tx,
            &[
                "conversation",
                "camp_turn",
                "agent_run",
                "context_manifest",
                "agent_run_input",
            ],
        )?;
        tx.commit()?;
        Ok(())
    })();
    let restore = connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(anyhow::Error::from);
    result.and(restore)
}
