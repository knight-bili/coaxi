//! Assertions in the existing identity migration owner; no second Core database.
use super::*;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn ddl(connection: &Connection, kind: &str, name: &str) -> String {
    connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type=?1 AND name=?2",
            params![kind, name],
            |r| r.get(0),
        )
        .unwrap()
}
fn inventory(connection: &Connection) -> Vec<(String, String, Option<String>)> {
    connection
        .prepare("SELECT type,name,sql FROM sqlite_schema ORDER BY type,name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}
fn rebuild(tx: &Transaction<'_>, table: &str, from: &str, to: &str) {
    let source = ddl(tx, "table", table);
    let changed = shape::replace(source, from, to, true).unwrap();
    rebuild_table_v171(
        tx,
        table,
        &replacement_table_schema_v171(changed, table),
        &[],
        &[],
    )
    .unwrap();
}
fn fake_guard(tx: &Transaction<'_>, name: &str) {
    let table: String = tx
        .query_row(
            "SELECT tbl_name FROM sqlite_schema WHERE type='trigger' AND name=?1",
            [name],
            |r| r.get(0),
        )
        .unwrap();
    tx.execute_batch(&format!(
        "DROP TRIGGER {name}; CREATE TRIGGER {name} BEFORE INSERT ON {table} BEGIN SELECT 1; END;"
    ))
    .unwrap();
}
fn unknown(connection: &Connection) {
    assert!(matches!(
        classify_database_contract(connection).unwrap(),
        DatabaseContractClassification::Unknown(_)
    ));
}

pub(crate) fn source(connection: &Connection) {
    let before = inventory(connection);
    let writes = connection.total_changes();
    assert!(source_schema_matches(connection).unwrap());
    assert!(!schema_matches(connection).unwrap());
    assert_eq!(
        connection.total_changes(),
        writes,
        "classification is read-only"
    );
    connection
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    tx.execute_batch(crate::delegation_settings::SCHEMA)
        .unwrap();
    assert!(
        !source_schema_matches(&tx).unwrap(),
        "settings-only is a partial target"
    );
    unknown(&tx);
    assert!(install_identity_schema(&tx).is_err());
    tx.rollback().unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    tx.execute_batch("CREATE TABLE DELEGATION(collision INTEGER);")
        .unwrap();
    assert!(!source_schema_matches(&tx).unwrap());
    unknown(&tx);
    assert!(install_identity_schema(&tx).is_err());
    tx.rollback().unwrap();
    for name in MODIFIED_GUARDS {
        let tx = connection.unchecked_transaction().unwrap();
        fake_guard(&tx, name);
        assert!(!source_schema_matches(&tx).unwrap(), "source forged {name}");
        unknown(&tx);
        assert!(
            install_identity_schema(&tx).is_err(),
            "reject before rebuilding {name}"
        );
        tx.rollback().unwrap();
    }
    for (table, from, to) in [
        (
            "conversation",
            "kind IN ('camp_member','single_chat')",
            "kind IN ('camp_member','single_chat','intruder')",
        ),
        (
            "camp_turn",
            "kind IN ('camp','single_chat')",
            "kind IN ('camp','single_chat','intruder')",
        ),
        (
            "agent_run",
            "operation_policy IN ('camp_member_v1','single_chat_v1')",
            "operation_policy IN ('camp_member_v1','single_chat_v1','intruder')",
        ),
        (
            "context_manifest",
            "context_delivery_profile_version IN (4,5,6,7,8,9,10)",
            "context_delivery_profile_version IN (4,5,6,7,8,9,10,99)",
        ),
        (
            "agent_run_input",
            "context_manifest_version IN (26,27,28,29,30,31,32,33,34)",
            "context_manifest_version IN (26,27,28,29,30,31,32,33,34,99)",
        ),
    ] {
        let tx = connection.unchecked_transaction().unwrap();
        rebuild(&tx, table, from, to);
        assert!(
            !source_schema_matches(&tx).unwrap(),
            "source widened {table}"
        );
        unknown(&tx);
        assert!(install_identity_schema(&tx).is_err());
        tx.rollback().unwrap();
    }
    // Deny a late CREATE after all five rebuilds and settings, not preflight.
    let denied = Arc::new(AtomicBool::new(false));
    let hit = Arc::clone(&denied);
    connection
        .authorizer(Some(move |ctx: AuthContext<'_>| match ctx.action {
            AuthAction::CreateTrigger {
                trigger_name: "delegation_owner_insert",
                ..
            } => {
                hit.store(true, Ordering::SeqCst);
                Authorization::Deny
            }
            _ => Authorization::Allow,
        }))
        .unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    assert!(install_identity_schema(&tx).is_err());
    assert!(
        denied.load(Ordering::SeqCst),
        "fault reached the migration body"
    );
    assert!(crate::delegation_settings::installed(&tx).unwrap());
    tx.rollback().unwrap();
    connection
        .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
        .unwrap();
    assert_eq!(
        inventory(connection),
        before,
        "failed migration rolls back every schema object"
    );
    assert!(source_schema_matches(connection).unwrap());
    // Compatible whitespace, quoted identifiers, comments and an unrelated
    // optional column remain valid and survive the actual transformation.
    let tx = connection.unchecked_transaction().unwrap();
    rebuild(
        &tx,
        "conversation",
        "kind IN ('camp_member','single_chat')",
        "\"kind\" in ( /* compatible */ 'camp_member', 'single_chat' )",
    );
    tx.execute_batch("ALTER TABLE agent_run ADD COLUMN preserved_usage_extra TEXT;")
        .unwrap();
    assert!(source_schema_matches(&tx).unwrap());
    install_identity_schema(&tx).unwrap();
    assert!(schema_matches(&tx).unwrap());
    assert!(
        table_columns(&tx, "agent_run")
            .unwrap()
            .iter()
            .any(|c| c == "preserved_usage_extra")
    );
    tx.rollback().unwrap();
    assert_eq!(inventory(connection), before);
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
}

pub(crate) fn target(connection: &Connection) {
    let before = inventory(connection);
    let writes = connection.total_changes();
    assert!(schema_matches(connection).unwrap());
    assert!(!source_schema_matches(connection).unwrap());
    unknown(connection); // No premature current160/210 registration.
    assert_eq!(connection.total_changes(), writes);
    for name in MODIFIED_GUARDS {
        let tx = connection.unchecked_transaction().unwrap();
        fake_guard(&tx, name);
        assert!(
            identity_schema_matches(&tx).unwrap(),
            "owned objects alone miss {name}"
        );
        assert!(!schema_matches(&tx).unwrap(), "target forged {name}");
        tx.rollback().unwrap();
    }
    connection
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .unwrap();
    for (table, from, to) in [
        (
            "conversation",
            "kind IN ('camp_member','single_chat','delegation_child')",
            "kind IN ('camp_member','single_chat','delegation_child','intruder')",
        ),
        (
            "camp_turn",
            "kind IN ('camp','single_chat','delegation')",
            "kind IN ('camp','single_chat','delegation','intruder')",
        ),
        (
            "agent_run",
            "operation_policy IN ('camp_member_v1','single_chat_v1','delegation_child_v1')",
            "operation_policy IN ('camp_member_v1','single_chat_v1','delegation_child_v1','intruder')",
        ),
        (
            "context_manifest",
            "context_delivery_profile_version IN (4,5,6,7,8,9,10,11,12,13)",
            "context_delivery_profile_version IN (4,5,6,7,8,9,10,11,12,13,99)",
        ),
        (
            "agent_run_input",
            "context_manifest_version IN (26,27,28,29,30,31,32,33,34,35)",
            "context_manifest_version IN (26,27,28,29,30,31,32,33,34)",
        ),
        ("conversation", "kind TEXT NOT NULL", "kind TEXT"),
        (
            "conversation",
            "kind TEXT NOT NULL",
            "kind TEXT COLLATE NOCASE NOT NULL",
        ),
        (
            "conversation",
            "CHECK (kind IN ('camp_member','single_chat','delegation_child'))",
            "CHECK (kind IN ('camp_member','single_chat','delegation_child') OR 1=1)",
        ),
        (
            "delegation_settings",
            "depth_limit BETWEEN 1 AND 8",
            "depth_limit BETWEEN 1 AND 9",
        ),
    ] {
        let tx = connection.unchecked_transaction().unwrap();
        rebuild(&tx, table, from, to);
        assert!(
            !schema_matches(&tx).unwrap(),
            "target changed {table}: {from}"
        );
        tx.rollback().unwrap();
    }
    assert_eq!(inventory(connection), before);
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    assert!(schema_matches(connection).unwrap());
}
