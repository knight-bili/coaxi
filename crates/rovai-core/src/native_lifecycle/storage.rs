use super::*;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

pub fn migrate(database: &mut crate::db::Database) -> Result<()> {
    let tx = database.connection_mut().transaction()?;
    ensure!(
        matches!(crate::db::classify_native_lifecycle_165_contract(&tx)?,crate::db::DatabaseContractClassification::SupportedMigrationSource(ref marker) if marker.projection_schema_version==164)
            && absent(&tx)?,
        "native_lifecycle.schema_source_invalid"
    );
    tx.execute_batch(include_str!("schema.sql"))?;
    tx.execute_batch("INSERT INTO schema_migration(version,applied_at) VALUES(215,datetime('now'));UPDATE rovai_data_contract SET projection_schema_version=165,updated_at=datetime('now') WHERE singleton=1;")?;
    ensure!(
        matches!(crate::db::classify_native_lifecycle_165_contract(&tx)?,crate::db::DatabaseContractClassification::Current(ref marker) if marker.projection_schema_version==165),
        "native_lifecycle.schema_target_invalid"
    );
    tx.commit()?;
    Ok(())
}
fn installed(connection: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut statement=connection.prepare("SELECT name,sql FROM sqlite_schema WHERE lower(name) GLOB 'native_lifecycle_*' AND sql IS NOT NULL ORDER BY name")?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect()
}
pub(crate) fn absent(connection: &Connection) -> rusqlite::Result<bool> {
    Ok(installed(connection)?.is_empty())
}
pub(crate) fn schema_matches(connection: &Connection) -> rusqlite::Result<bool> {
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("schema.sql"))?;
    Ok(installed(connection)? == installed(&reference)?)
}
#[cfg(test)]
pub(crate) fn downgrade_for_test(connection: &Connection) -> anyhow::Result<()> {
    let objects = installed(connection)?;
    if !objects.is_empty() {
        anyhow::ensure!(
            schema_matches(connection)?,
            "Native Lifecycle fixture reset requires the exact owned schema"
        );
    }
    crate::db::downgrade_owned_migration_for_test(connection, 215, 165, 164, &objects, &[], |_| {
        Ok(())
    })
}

pub fn policy(connection: &Connection) -> Result<Value> {
    let raw: Option<String> = connection
        .query_row(
            "SELECT value_json FROM native_lifecycle_steer_policy WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let value = match raw {
        Some(raw) => serde_json::from_str(&raw)?,
        None => json!({"schemaVersion":1,"revision":0,"members":[]}),
    };
    validate_shape("NativeSteerPolicyV1", &value)?;
    Ok(value)
}
/// 由原命令事务调用；仅新执行读取该意图，旧执行持有自己的冻结值。
pub(crate) fn set_policy(tx: &Transaction<'_>, expected: i64, members: &Value) -> Result<Value> {
    let current = policy(tx)?;
    ensure!(
        current["revision"].as_i64() == Some(expected),
        "native_lifecycle.policy_changed"
    );
    let next = expected
        .checked_add(1)
        .context("native_lifecycle.policy_revision_exhausted")?;
    let value = json!({"schemaVersion":1,"revision":next,"members":members});
    validate_shape("NativeSteerPolicyV1", &value)?;
    let mut seen = std::collections::HashSet::new();
    for member in value["members"]
        .as_array()
        .context("native_lifecycle.members_invalid")?
    {
        let id = member["agentId"]
            .as_str()
            .context("native_lifecycle.member_invalid")?;
        ensure!(seen.insert(id), "native_lifecycle.duplicate_member");
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_profile WHERE id=?1 AND profile_status<>'removed')",
            [id],
            |row| row.get(0),
        )?;
        ensure!(exists, "native_lifecycle.member_missing");
    }
    tx.execute("INSERT INTO native_lifecycle_steer_policy(singleton,revision,value_json) VALUES(1,?1,?2) ON CONFLICT(singleton) DO UPDATE SET revision=excluded.revision,value_json=excluded.value_json",params![next,serde_json::to_string(&value)?])?;
    Ok(value)
}
pub(crate) fn freeze_policy(
    connection: &Connection,
    member: &str,
) -> Result<Option<FrozenNativeSteerV1>> {
    if absent(connection)? {
        return Ok(None);
    }
    let value = policy(connection)?;
    let enabled = value["members"].as_array().is_some_and(|members| {
        members
            .iter()
            .any(|entry| entry["agentId"] == member && entry["enabled"] == true)
    });
    Ok(enabled.then(|| FrozenNativeSteerV1 {
        schema_version: 1,
        policy_revision: value["revision"].as_i64().unwrap_or(0),
        enabled: true,
    }))
}
pub(crate) fn put_preview(
    tx: &Transaction<'_>,
    kind: &str,
    owner: &str,
    client: &str,
    mut value: Value,
) -> Result<Value> {
    let now = Utc::now();
    let expires = now + chrono::Duration::minutes(10);
    value["previewId"] = json!(uuid::Uuid::new_v4().to_string());
    value["previewDigest"] = json!("");
    value["expiresAt"] = json!(expires.to_rfc3339());
    value["previewDigest"] = json!(digest(&value)?);
    validate_shape(
        match kind {
            "continuation" => "NativeContinuationPreviewV1",
            "steer" => "NativeSteerPreviewV1",
            _ => anyhow::bail!("native_lifecycle.preview_kind_invalid"),
        },
        &value,
    )?;
    tx.execute("INSERT INTO native_lifecycle_preview(id,kind,owner_thread_id,client_id,digest,value_json,expires_at_ms,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![value["previewId"].as_str(),kind,owner,client,value["previewDigest"].as_str(),serde_json::to_string(&value)?,expires.timestamp_millis(),now.to_rfc3339()])?;
    Ok(value)
}
pub(crate) fn preview(
    connection: &Connection,
    owner: &str,
    id: &str,
    expected: &str,
    kind: &str,
) -> Result<Value> {
    uuid::Uuid::parse_str(id)?;
    let(raw,expires):(String,i64)=connection.query_row("SELECT value_json,expires_at_ms FROM native_lifecycle_preview WHERE id=?1 AND owner_thread_id=?2 AND digest=?3 AND kind=?4",params![id,owner,expected,kind],|row|Ok((row.get(0)?,row.get(1)?))).context("native_lifecycle.preview_missing_or_changed")?;
    ensure!(
        Utc::now().timestamp_millis() < expires,
        "native_lifecycle.preview_expired"
    );
    let value: Value = serde_json::from_str(&raw)?;
    let mut basis = value.clone();
    basis["previewDigest"] = json!("");
    ensure!(
        digest(&basis)? == expected,
        "native_lifecycle.preview_changed"
    );
    Ok(value)
}
/// 调用方先核对当前用户/作用域；相同编号永远不能变为另一份输入。
pub(crate) fn prior_intent(
    connection: &Connection,
    command: &str,
    owner: &str,
    kind: &str,
    input: &str,
) -> Result<Option<Value>> {
    let row:Option<(String,String,String,String)>=connection.query_row("SELECT owner_thread_id,kind,input_digest,value_json FROM native_lifecycle_intent WHERE command_id=?1",[command],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    row.map(|(stored_owner, stored_kind, stored_input, raw)| {
        ensure!(
            stored_owner == owner && stored_kind == kind && stored_input == input,
            "native_lifecycle.command_payload_changed"
        );
        Ok(serde_json::from_str(&raw)?)
    })
    .transpose()
}
pub(crate) fn begin_intent(
    tx: &Transaction<'_>,
    command: &str,
    owner: &str,
    client: &str,
    core_instance: &str,
    kind: &str,
    preview_id: &str,
    input: &str,
    record: &Value,
) -> Result<()> {
    uuid::Uuid::parse_str(command)?;
    let now = Utc::now().to_rfc3339();
    tx.execute("INSERT INTO native_lifecycle_intent(command_id,kind,owner_thread_id,client_id,core_instance_id,preview_id,input_digest,state,value_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,'prepared',?8,?9,?9)",params![command,kind,owner,client,core_instance,preview_id,input,serde_json::to_string(record)?,now])?;
    Ok(())
}
pub(crate) fn reserve_images(
    tx: &Transaction<'_>,
    command: &str,
    target: &NativeSteerTargetV1,
    input: &NativeSteerInputV1,
) -> Result<()> {
    validate_shape("NativeSteerTargetV1", &serde_json::to_value(target)?)?;
    let bytes = input.image_bytes()?;
    let turn = digest(&serde_json::to_value(target)?)?;
    let previous_raw:i64=tx.query_row("SELECT COALESCE(SUM(byte_size),0) FROM native_lifecycle_image_reservation WHERE turn_digest=?1 AND state<>'not_consumed'",[&turn],|row|row.get(0))?;
    let previous =
        u64::try_from(previous_raw).context("native_lifecycle.image_reservation_invalid")?;
    ensure!(
        previous
            .checked_add(bytes)
            .is_some_and(|total| total <= 32 * 1024 * 1024),
        "native_lifecycle.image_budget_exceeded"
    );
    let byte_size = i64::try_from(bytes).context("native_lifecycle.image_budget_exceeded")?;
    tx.execute("INSERT INTO native_lifecycle_image_reservation(command_id,turn_digest,byte_size,state) VALUES(?1,?2,?3,'reserved')",params![command,turn,byte_size])?;
    Ok(())
}
