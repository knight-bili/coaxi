//! Schema159/Migration209 extends only the closed occurrence-reason set.
use super::*;

const OLD: &str = "reason TEXT CHECK(reason IS NULL OR reason IN (
    'missed', 'overlap', 'interaction_required', 'timeout',
    'interrupted', 'no_result', 'execution_failed',
    'runtime_not_ready', 'dispatch_rejected'
))";
const NEW: &str = "reason TEXT CHECK(reason IS NULL OR reason IN (
    'missed', 'overlap', 'interaction_required', 'timeout',
    'interrupted', 'no_result', 'execution_failed',
    'runtime_not_ready', 'dispatch_rejected', 'user_requested_automation_stop'
))";
const OLD_VALUES: &str = "'runtime_not_ready', 'dispatch_rejected'";
const NEW_VALUES: &str =
    "'runtime_not_ready', 'dispatch_rejected', 'user_requested_automation_stop'";

fn matches_reason(connection: &Connection, expected: &str) -> rusqlite::Result<bool> {
    let sql: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type='table' AND name='automation_run'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let normalize = |sql: &str| sql.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(sql.is_some_and(|sql| normalize(&sql).contains(&normalize(expected))))
}
pub(super) fn source_schema_matches(connection: &Connection) -> rusqlite::Result<bool> {
    Ok(matches_reason(connection, OLD)?
        && !connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migration WHERE version=209)",
            [],
            |row| row.get::<_, bool>(0),
        )?)
}
pub(super) fn schema_matches(connection: &Connection) -> rusqlite::Result<bool> {
    matches_reason(connection, NEW)
}
pub(super) fn migrate(database: &mut Database) -> Result<()> {
    database
        .connection
        .execute_batch("PRAGMA foreign_keys=OFF;")?;
    let result = (|| -> Result<()> {
        let tx = database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        anyhow::ensure!(
            matches!(classify_database_contract(&tx)?, DatabaseContractClassification::SupportedMigrationSource(ref marker) if marker.projection_schema_version==158)
                && source_schema_matches(&tx)?,
            "Automation run control requires exact schema158"
        );
        rewrite_reason(&tx, false)?;
        tx.execute_batch("INSERT INTO schema_migration VALUES(209,datetime('now'));
            UPDATE rovai_data_contract SET projection_schema_version=159,updated_at=datetime('now') WHERE singleton=1;")?;
        validate_migration_foreign_keys(
            &tx,
            &[
                "automation_run",
                "automation_notification_delivery",
                "camp_turn",
            ],
        )?;
        anyhow::ensure!(
            matches!(classify_database_contract(&tx)?, DatabaseContractClassification::Current(ref marker) if marker.projection_schema_version==159),
            "Automation run control admission failed"
        );
        tx.commit()?;
        Ok(())
    })();
    let foreign_keys = database.connection.execute_batch("PRAGMA foreign_keys=ON;");
    result.and_then(|()| foreign_keys.map_err(Into::into))
}
fn rewrite_reason(tx: &Transaction<'_>, downgrade: bool) -> Result<()> {
    let sql: String = tx.query_row(
        "SELECT sql FROM sqlite_schema WHERE type='table' AND name='automation_run'",
        [],
        |row| row.get(0),
    )?;
    let (from, to) = if downgrade {
        (NEW_VALUES, OLD_VALUES)
    } else {
        (OLD_VALUES, NEW_VALUES)
    };
    anyhow::ensure!(
        sql.matches(from).count() == 1,
        "Automation reason constraint is not the expected source"
    );
    rebuild_table_v171(
        tx,
        "automation_run",
        &replacement_table_schema_v171(sql.replacen(from, to, 1), "automation_run"),
        &[],
        &[],
    )
}

#[cfg(test)]
pub(super) fn downgrade_for_test(connection: &Connection) {
    if !connection.table_exists(None, "schema_migration").unwrap()
        || !connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migration WHERE version=209)",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap()
    {
        return;
    }
    let stopped: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM automation_run WHERE reason='user_requested_automation_stop'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        stopped, 0,
        "An old-source fixture must not discard user stops"
    );
    connection
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    rewrite_reason(&tx, true).unwrap();
    tx.execute_batch("DELETE FROM schema_migration WHERE version=209; UPDATE rovai_data_contract SET projection_schema_version=158 WHERE singleton=1 AND projection_schema_version=159;").unwrap();
    tx.commit().unwrap();
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
}

#[cfg(test)]
pub(super) fn verify_fixture(database: &mut Database) {
    let rows = |connection: &Connection, table: &str| {
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .unwrap();
        statement
            .query_map([], |row| {
                (0..row.as_ref().column_count())
                    .map(|i| row.get::<_, rusqlite::types::Value>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    let tables = [
        "automation_run",
        "automation_notification_delivery",
        "event_log",
    ];
    let before = tables
        .iter()
        .map(|table| rows(database.connection(), table))
        .collect::<Vec<_>>();
    downgrade_for_test(database.connection());
    assert!(source_schema_matches(database.connection()).unwrap());
    database.connection.execute_batch("CREATE TEMP TRIGGER reject_automation_stop_migration BEFORE INSERT ON schema_migration WHEN NEW.version=209 BEGIN SELECT RAISE(ABORT,'automation stop rollback fixture');END;").unwrap();
    assert!(
        migrate(database)
            .unwrap_err()
            .to_string()
            .contains("rollback fixture")
    );
    assert!(source_schema_matches(database.connection()).unwrap());
    for (table, expected) in tables.iter().zip(&before) {
        assert_eq!(&rows(database.connection(), table), expected);
    }
    database
        .connection
        .execute_batch("DROP TRIGGER reject_automation_stop_migration;")
        .unwrap();
    migrate(database).unwrap();
    assert!(connection_has_current_data_contract(database.connection()).unwrap());
    for (table, expected) in tables.iter().zip(&before) {
        assert_eq!(&rows(database.connection(), table), expected);
    }
    database
        .connection
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .unwrap();
    let tx = database.connection.unchecked_transaction().unwrap();
    let sql: String = tx
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name='automation_run'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let widened = sql.replace(NEW_VALUES, &format!("{NEW_VALUES}, 'forged'"));
    rebuild_table_v171(
        &tx,
        "automation_run",
        &replacement_table_schema_v171(widened, "automation_run"),
        &[],
        &[],
    )
    .unwrap();
    assert!(!schema_matches(&tx).unwrap());
    assert!(matches!(
        classify_database_contract(&tx).unwrap(),
        DatabaseContractClassification::Unknown(_)
    ));
    tx.rollback().unwrap();
    database
        .connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .unwrap();
}
