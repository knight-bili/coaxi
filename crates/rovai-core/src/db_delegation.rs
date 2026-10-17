//! Complete delegation schema delta and source checks for pending160/210. No startup registration.
use super::*;

const IDENTITY_SQL: &str = include_str!("delegation_schema.sql");

// The old159 reference is structural input only; Core remains the business authority.
#[cfg(all(test, feature = "extended-tests"))]
#[path = "db_delegation_shape_tests.rs"]
pub(crate) mod checks;
#[path = "db_delegation_shape.rs"]
mod shape;
const MODIFIED_GUARDS: &[&str] = &[
    "context_manifest_v34_only_insert",
    "context_manifest_quote_profile_insert",
    "runtime_input_delivery_attachment_auth_insert",
    "agent_run_input_v34_only_insert",
    "single_chat_composer_draft_scope_insert",
    "single_chat_pending_input_scope_insert",
    "single_chat_retained_input_insert_guard",
    "single_chat_source_observation_owner",
    "conversation_message_single_chat_attachment_insert_guard",
    "conversation_message_single_chat_attachment_update_guard",
];
const TARGET_RUN_INPUT_GUARD: &str = r#"CREATE TRIGGER agent_run_input_v34_only_insert BEFORE INSERT ON agent_run_input
        WHEN NOT EXISTS(SELECT 1 FROM agent_run r WHERE r.id=NEW.agent_run_id AND r.invocation_kind='batch'
            AND json_extract(r.skill_selection_snapshot_json,'$.schemaVersion')=3
            AND ((NEW.context_manifest_version=34 AND json_type(r.effective_config_json,'$.delegationPolicy') IS NULL)
                OR (NEW.context_manifest_version=35 AND json_extract(r.effective_config_json,'$.delegationPolicy.role')='public_parent'
                    AND json_extract(r.effective_config_json,'$.delegationPolicy.schemaVersion')=1)))
        BEGIN SELECT RAISE(ABORT,'New public RunInput must match its frozen role and pack selection'); END;"#;

fn extend_guard(name: &str, source: String) -> Result<String> {
    let replacement = match name {
        "context_manifest_v34_only_insert" => {
            return replace_once(
                source,
                "BEFORE INSERT ON context_manifest WHEN NOT",
                "BEFORE INSERT ON context_manifest WHEN NEW.context_manifest_version<>35 AND NOT",
            );
        }
        "context_manifest_quote_profile_insert" => {
            return replace_once(
                source,
                "WHEN NOT (",
                "WHEN NEW.context_manifest_version<>35 AND NOT (",
            );
        }
        "runtime_input_delivery_attachment_auth_insert" => {
            return replace_once(source, "31, 32, 33, 34)", "31, 32, 33, 34, 35)");
        }
        "agent_run_input_v34_only_insert" => return Ok(TARGET_RUN_INPUT_GUARD.into()),
        "single_chat_composer_draft_scope_insert" => (
            "conversation.kind = 'single_chat'",
            "(conversation.kind = 'single_chat' OR (conversation.kind='delegation_child' AND EXISTS(SELECT 1 FROM delegation d JOIN delegation_control control ON control.delegation_id=d.id WHERE d.child_conversation_id=conversation.id AND control.takeover_at IS NOT NULL AND control.takeover_message_sequence IS NOT NULL)))",
        ),
        "single_chat_pending_input_scope_insert" => (
            "conversation.kind = 'single_chat'",
            "(conversation.kind = 'single_chat' OR (conversation.kind='delegation_child' AND EXISTS(SELECT 1 FROM delegation d JOIN delegation_control control ON control.delegation_id=d.id WHERE d.child_conversation_id=conversation.id AND control.takeover_at IS NOT NULL AND control.takeover_message_sequence IS NOT NULL)))",
        ),
        "single_chat_retained_input_insert_guard" => (
            "kind='single_chat'",
            "(kind='single_chat' OR (kind='delegation_child' AND EXISTS(SELECT 1 FROM delegation d JOIN delegation_control control ON control.delegation_id=d.id WHERE d.child_conversation_id=conversation.id AND control.takeover_at IS NOT NULL AND control.takeover_message_sequence IS NOT NULL)))",
        ),
        "single_chat_source_observation_owner" => (
            "kind='single_chat'",
            "(kind='single_chat' OR (kind='delegation_child' AND EXISTS(SELECT 1 FROM delegation d JOIN delegation_control control ON control.delegation_id=d.id WHERE d.child_conversation_id=conversation.id AND control.takeover_at IS NOT NULL AND control.takeover_message_sequence IS NOT NULL)))",
        ),
        "conversation_message_single_chat_attachment_insert_guard" => (
            "conversation.kind = 'single_chat'",
            "(conversation.kind = 'single_chat' OR (conversation.kind='delegation_child' AND EXISTS(SELECT 1 FROM delegation d JOIN delegation_control control ON control.delegation_id=d.id WHERE d.child_conversation_id=conversation.id AND control.takeover_at IS NOT NULL AND control.takeover_message_sequence IS NOT NULL)))",
        ),
        "conversation_message_single_chat_attachment_update_guard" => (
            "conversation.kind = 'single_chat'",
            "(conversation.kind = 'single_chat' OR (conversation.kind='delegation_child' AND EXISTS(SELECT 1 FROM delegation d JOIN delegation_control control ON control.delegation_id=d.id WHERE d.child_conversation_id=conversation.id AND control.takeover_at IS NOT NULL AND control.takeover_message_sequence IS NOT NULL)))",
        ),
        _ => anyhow::bail!("Unexpected delegation source guard"),
    };
    replace_once(source, replacement.0, replacement.1)
}

pub(crate) fn schema_matches(connection: &Connection) -> Result<bool> {
    shape::matches(connection, true)
}
pub(crate) fn source_schema_matches(connection: &Connection) -> Result<bool> {
    shape::matches(connection, false)
}
pub(crate) fn source_matches_for_classifier(connection: &Connection) -> rusqlite::Result<bool> {
    source_schema_matches(connection)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))
}

pub(crate) fn source_objects_absent(connection: &Connection) -> rusqlite::Result<bool> {
    for (name, _) in owned_objects()
        .into_iter()
        .chain(std::iter::once(("delegation_settings", "")))
    {
        if connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name=?1 COLLATE NOCASE)",
            [name],
            |r| r.get::<_, bool>(0),
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn owned_objects() -> Vec<(&'static str, &'static str)> {
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
        return Vec::new();
    }
    offsets
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let expected = &IDENTITY_SQL[*start
                ..offsets
                    .get(index + 1)
                    .copied()
                    .unwrap_or(IDENTITY_SQL.len())];
            let tokens = expected.split_whitespace().collect::<Vec<_>>();
            (
                if tokens[1] == "UNIQUE" {
                    tokens[3]
                } else {
                    tokens[2]
                },
                expected,
            )
        })
        .collect()
}
pub(crate) fn identity_schema_matches(connection: &Connection) -> rusqlite::Result<bool> {
    let objects = owned_objects();
    if objects.is_empty() {
        return Ok(false);
    }
    for (name, expected) in objects {
        let actual: Option<String> = connection
            .query_row("SELECT sql FROM sqlite_schema WHERE name=?1", [name], |r| {
                r.get(0)
            })
            .optional()?;
        if !actual.is_some_and(|sql| shape::equivalent(&sql, expected)) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn replace_once(source: String, from: &str, to: &str) -> Result<String> {
    shape::replace(source, from, to, true)
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
    anyhow::ensure!(
        source_schema_matches(tx)?,
        "Delegation migration source delta is incomplete"
    );
    for table in [
        "conversation",
        "camp_turn",
        "agent_run",
        "context_manifest",
        "agent_run_input",
    ] {
        let original: String = tx.query_row(
            "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?1",
            [table],
            |r| r.get(0),
        )?;
        let extended = extend_table(table, original)?;
        let replacement = shape::replace(
            extended,
            &format!("CREATE TABLE {table}"),
            &format!("CREATE TABLE {table}_v171"),
            true,
        )?;
        rebuild_table_v171(tx, table, &replacement, &[], &[])?;
    }
    tx.execute_batch(crate::delegation_settings::SCHEMA)?;
    tx.execute_batch(IDENTITY_SQL)?;
    for name in MODIFIED_GUARDS {
        let extended = shape::target_guard(name)?;
        tx.execute_batch(&format!("DROP TRIGGER {name};"))?;
        tx.execute_batch(&extended)?;
    }
    anyhow::ensure!(
        schema_matches(tx)?,
        "Complete delegation schema admission failed"
    );
    validate_migration_foreign_keys(
        tx,
        &[
            "conversation",
            "camp_turn",
            "agent_run",
            "context_manifest",
            "agent_run_input",
            "delegation",
            "delegation_attempt",
            "delegation_control",
            "delegation_workspace_permission",
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
            let source = shape::replace(
                source,
                "operation_policy_version IN (1, 2)",
                "operation_policy_version IN (1, 2, 3)",
                false,
            )?;
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
        "agent_run_input" => replace_once(
            source,
            "context_manifest_version IN (26, 27, 28, 29, 30, 31, 32, 33, 34)",
            "context_manifest_version IN (26, 27, 28, 29, 30, 31, 32, 33, 34, 35)",
        ),
        "context_manifest" => {
            let mut sql = source;
            for (old, new) in [
                (
                    "context_delivery_profile_version IN (4, 5, 6, 7, 8, 9, 10)",
                    "context_delivery_profile_version IN (4, 5, 6, 7, 8, 9, 10, 11, 12, 13)",
                ),
                (
                    "formatter_version IN (20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34)",
                    "formatter_version IN (20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35)",
                ),
                (
                    "context_manifest_version IN (19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34)",
                    "context_manifest_version IN (19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35)",
                ),
                (
                    "run_facts_schema_version IN (1, 2, 3, 4, 5, 6, 7, 8, 9)",
                    "run_facts_schema_version IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10)",
                ),
                (
                    "(context_manifest_version = 34 AND",
                    "(context_manifest_version = 35 AND formatter_version = 35 AND run_facts_schema_version IN (6,9,10) AND ((camp_attachment_view_receipt_version IS NULL AND camp_attachment_view_receipt_json IS NULL AND camp_attachment_view_receipt_digest IS NULL) OR (camp_attachment_view_receipt_version = 2 AND camp_attachment_view_receipt_json IS NOT NULL AND camp_attachment_view_receipt_digest IS NOT NULL))) OR (context_manifest_version = 34 AND",
                ),
            ] {
                sql = replace_once(sql, old, new)?;
            }
            let end = shape::table_end(&sql).context("Invalid Context Manifest schema")?;
            sql.insert_str(end,", CHECK((context_manifest_version<>35 AND context_delivery_profile_version<=10) OR (context_manifest_version=35 AND ((context_delivery_profile_version=11 AND run_facts_schema_version=10) OR (context_delivery_profile_version=12 AND run_facts_schema_version=9) OR (context_delivery_profile_version=13 AND run_facts_schema_version=6))))");
            Ok(sql)
        }
        _ => anyhow::bail!("Unexpected delegation table"),
    }
}
