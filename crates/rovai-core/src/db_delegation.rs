//! The identity portion of the pending 160/210 migration. No startup registration until formats land.
use super::*;

const IDENTITY_SQL: &str = include_str!("delegation_schema.sql");

// New owned objects only; complete 210 admission must also check settings, extended legacy tables and formats.
pub(crate) fn identity_schema_matches(connection: &Connection) -> rusqlite::Result<bool> {
    let normalize = |s: &str| {
        s.trim()
            .trim_end_matches(';')
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut offsets = Vec::new();
    let mut offset = 0;
    // Every top-level CREATE in this migration-owned source starts at column zero.
    // Never split at the semicolons inside trigger bodies.
    for line in IDENTITY_SQL.split_inclusive('\n') {
        if line.starts_with("CREATE ") {
            offsets.push(offset);
        }
        offset += line.len();
    }
    if offsets.is_empty() {
        return Ok(false);
    }
    for (index, start) in offsets.iter().enumerate() {
        let expected = &IDENTITY_SQL[*start
            ..offsets
                .get(index + 1)
                .copied()
                .unwrap_or(IDENTITY_SQL.len())];
        let tokens = expected.split_whitespace().collect::<Vec<_>>();
        let name = if tokens[1] == "UNIQUE" {
            tokens[3]
        } else {
            tokens[2]
        };
        let actual: Option<String> = connection
            .query_row("SELECT sql FROM sqlite_schema WHERE name=?1", [name], |r| {
                r.get(0)
            })
            .optional()?;
        if !actual.is_some_and(|s| normalize(&s) == normalize(expected)) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn replace_once(source: String, from: &str, to: &str) -> Result<String> {
    anyhow::ensure!(
        source.matches(from).count() == 1,
        "Unexpected delegation migration source: {from}"
    );
    Ok(source.replacen(from, to, 1))
}

pub(crate) fn install_identity_schema(tx: &Transaction<'_>) -> Result<()> {
    anyhow::ensure!(
        !tx.query_row("PRAGMA foreign_keys", [], |r| r.get::<_, bool>(0))?,
        "Delegation table rebuild requires the migration's FK-off transaction"
    );
    anyhow::ensure!(
        matches!(classify_database_contract(tx)?,DatabaseContractClassification::Current(ref marker) | DatabaseContractClassification::SupportedMigrationSource(ref marker) if marker.contract_version=="v1.72" && marker.projection_schema_version==159),
        "Delegation identity requires exact schema159 source"
    );
    for table in ["conversation", "camp_turn", "agent_run"] {
        let original: String = tx.query_row(
            "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?1",
            [table],
            |r| r.get(0),
        )?;
        let extended = extend_table(table, original)?;
        rebuild_table_v171(
            tx,
            table,
            &replacement_table_schema_v171(extended, table),
            &[],
            &[],
        )?;
    }
    tx.execute_batch(crate::delegation_settings::SCHEMA)?;
    tx.execute_batch(IDENTITY_SQL)?;
    anyhow::ensure!(
        identity_schema_matches(tx)?,
        "Delegation identity schema admission failed"
    );
    validate_migration_foreign_keys(
        tx,
        &[
            "conversation",
            "camp_turn",
            "agent_run",
            "delegation",
            "delegation_attempt",
            "delegation_control",
        ],
    )?;
    Ok(())
}

fn extend_table(table: &str, source: String) -> Result<String> {
    match table {
        "conversation" => {
            let sql = replace_once(
                source,
                "kind IN ('camp_member', 'single_chat')",
                "kind IN ('camp_member', 'single_chat', 'delegation_child')",
            )?;
            replace_once(
                sql,
                "(kind = 'single_chat'",
                "(kind IN ('single_chat', 'delegation_child')",
            )
        }
        "camp_turn" => {
            let sql = replace_once(
                source,
                "kind IN ('camp', 'single_chat')",
                "kind IN ('camp', 'single_chat', 'delegation')",
            )?;
            replace_once(
                sql,
                "(kind = 'single_chat' AND trigger_type = 'conversation_message')",
                "(kind IN ('single_chat', 'delegation') AND trigger_type = 'conversation_message')",
            )
        }
        "agent_run" => {
            let sql = replace_once(
                source,
                "invocation_kind IN ('direct', 'a2a', 'gather_completion', 'single_chat', 'batch')",
                "invocation_kind IN ('direct', 'a2a', 'gather_completion', 'single_chat', 'batch', 'delegation', 'delegation_user')",
            )?;
            let sql = replace_once(
                sql,
                "operation_policy IN ('camp_member_v1', 'single_chat_v1')",
                "operation_policy IN ('camp_member_v1', 'single_chat_v1', 'delegation_child_v1')",
            )?;
            replace_once(
                sql,
                "(invocation_kind <> 'single_chat'",
                r#"(invocation_kind IN ('delegation', 'delegation_user')
                        AND response_delivery='conversation_message'
                        AND operation_policy='delegation_child_v1'
                        AND operation_policy_version=1 AND task_id IS NULL
                        AND destination_conversation_id IS NOT NULL AND destination_conversation_id=conversation_id)
                    OR
                    (invocation_kind NOT IN ('single_chat', 'delegation', 'delegation_user')"#,
            )
        }
        _ => anyhow::bail!("Unexpected delegation table"),
    }
}
