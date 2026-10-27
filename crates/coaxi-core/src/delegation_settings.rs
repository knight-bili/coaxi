//! User-only settings storage primitive. The command gateway owns authorization, receipt and event.
//! Host methods refuse an old database; live installation awaits the complete delegation migration.
use crate::delegation_contract::{RuntimeTarget, present};
use anyhow::{Context, Result};
use rusqlite::{Connection, Transaction, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA: &str = include_str!("delegation_settings_schema.sql");

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub config_values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DelegationSettings {
    pub version: i64,
    pub enabled: bool,
    pub depth_limit: u8,
    pub per_runtime_defaults: BTreeMap<RuntimeTarget, RuntimeDefaults>,
    pub completed_cache_max_mb: i64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub enabled: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub depth_limit: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub per_runtime_defaults: Option<BTreeMap<RuntimeTarget, RuntimeDefaults>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub completed_cache_max_mb: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateSettingsParams {
    pub expected_version: i64,
    pub patch: SettingsPatch,
}

impl crate::command::sealed::Sealed for UpdateSettingsParams {}
impl crate::command::DomainCommand for UpdateSettingsParams {
    const TYPE: &'static str = "delegations.settings.update";
    const EXACT_REPLAY: bool = true;
}

pub(crate) fn installed(connection: &Connection) -> Result<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='delegation_settings')",
        [], |row| row.get(0),
    )?)
}

pub(crate) fn read_user(
    db: &crate::db::Database,
    actor: &crate::command::ActorRef,
) -> Result<DelegationSettings> {
    anyhow::ensure!(
        matches!(actor, crate::command::ActorRef::User { .. }),
        "delegation.user_required"
    );
    anyhow::ensure!(installed(db.connection())?, "delegation.unavailable");
    read(db.connection())
}

pub(crate) fn update_user(
    db: &mut crate::db::Database,
    envelope: &crate::command::CommandEnvelope<UpdateSettingsParams>,
) -> Result<crate::command::CommandExecution> {
    use crate::command::{ActorRef, CommandHandlerResult, DomainCommandGateway};
    DomainCommandGateway.execute(db, envelope, |tx| {
        let reject = |code| Ok(CommandHandlerResult::rejected(code, serde_json::json!({})));
        if !matches!(envelope.actor, ActorRef::User { .. }) {
            return reject("delegation.user_required");
        }
        if envelope.camp_id.is_some() {
            return reject("delegation.settings_scope_mismatch");
        }
        if !installed(tx)? {
            return reject("delegation.unavailable");
        }
        let current = read(tx)?;
        if current.version != envelope.payload.expected_version {
            return reject("delegation.settings_changed");
        }
        let patch = &envelope.payload.patch;
        if current.version == i64::MAX
            || patch.depth_limit.is_some_and(|v| !(1..=8).contains(&v))
            || patch.completed_cache_max_mb.is_some_and(|v| v < 0)
            || patch.per_runtime_defaults.as_ref().is_some_and(|defaults| {
                defaults
                    .values()
                    .any(|preset| preset.config_values.keys().any(String::is_empty))
            })
        {
            return reject("delegation.settings_invalid");
        }
        // Storage faults propagate and roll back; they are not durable business refusals.
        let next = update(tx, &envelope.payload)?;
        crate::collaboration::append_domain_event(
            tx,
            "delegation.settings_changed",
            None,
            Some(("delegation_settings", "1")),
            &envelope.actor,
            None,
            &serde_json::json!({"version": next.version}),
        )?;
        Ok(CommandHandlerResult::applied(
            "delegation.settings_updated",
            serde_json::to_value(next)?,
            None,
        ))
    })
}

pub fn read(connection: &Connection) -> Result<DelegationSettings> {
    let (version, enabled, depth_limit, defaults, cache) = connection.query_row(
        "SELECT version,enabled,depth_limit,per_runtime_defaults,completed_cache_max_mb
         FROM delegation_settings WHERE singleton=1",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get::<_, String>(3)?,
                row.get(4)?,
            ))
        },
    )?;
    let settings = DelegationSettings {
        version,
        enabled,
        depth_limit,
        per_runtime_defaults: serde_json::from_str(&defaults)
            .context("Invalid delegation defaults")?,
        completed_cache_max_mb: cache,
    };
    validate(&settings)?;
    Ok(settings)
}

fn validate(settings: &DelegationSettings) -> Result<()> {
    anyhow::ensure!(settings.version >= 1, "Invalid delegation settings version");
    anyhow::ensure!(
        (1..=8).contains(&settings.depth_limit),
        "Delegation depth must be 1..8"
    );
    anyhow::ensure!(
        settings.completed_cache_max_mb >= 0,
        "Delegation cache budget must not be negative"
    );
    for defaults in settings.per_runtime_defaults.values() {
        anyhow::ensure!(
            defaults.config_values.keys().all(|key| !key.is_empty()),
            "Empty delegation config key"
        );
    }
    Ok(())
}

/// Caller commits this update with the same User command receipt and settings-changed event.
/// A toggle carries only enabled; it never writes a stale copy of per-runtime defaults.
pub fn update(tx: &Transaction<'_>, input: &UpdateSettingsParams) -> Result<DelegationSettings> {
    let mut next = read(tx)?;
    anyhow::ensure!(
        next.version == input.expected_version,
        "Delegation settings revision conflict"
    );
    next.version = next
        .version
        .checked_add(1)
        .context("Delegation settings version exhausted")?;
    if let Some(enabled) = input.patch.enabled {
        next.enabled = enabled;
    }
    if let Some(depth) = input.patch.depth_limit {
        next.depth_limit = depth;
    }
    if let Some(defaults) = &input.patch.per_runtime_defaults {
        next.per_runtime_defaults = defaults.clone();
    }
    if let Some(cache) = input.patch.completed_cache_max_mb {
        next.completed_cache_max_mb = cache;
    }
    validate(&next)?;
    let changed = tx.execute(
        "UPDATE delegation_settings SET version=?1,enabled=?2,depth_limit=?3,
         per_runtime_defaults=?4,completed_cache_max_mb=?5,updated_at=datetime('now')
         WHERE singleton=1 AND version=?6",
        params![
            next.version,
            next.enabled,
            next.depth_limit,
            serde_json::to_string(&next.per_runtime_defaults)?,
            next.completed_cache_max_mb,
            input.expected_version
        ],
    )?;
    anyhow::ensure!(changed == 1, "Delegation settings revision conflict");
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn settings_patch_is_atomic_durable_and_preserves_other_saved_fields() {
        // SQLite is the lowest owner for CAS, rollback and reopen; no Runtime or full Core fixture.
        let path = std::env::temp_dir().join(format!(
            "delegation-settings-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch(SCHEMA).unwrap();
        let initial = read(&connection).unwrap();
        assert!(!initial.enabled);
        assert_eq!(
            (
                initial.version,
                initial.depth_limit,
                initial.completed_cache_max_mb
            ),
            (1, 1, 512)
        );
        assert!(initial.per_runtime_defaults.is_empty());
        // Coaxi omits an empty config_values map and accepts null/absent mode_id.
        // Keep that nested preset encoding; only the outer patch treats null as invalid.
        for (wire, canonical) in [
            (
                json!({"mode_id":"fixture-mode"}),
                json!({"mode_id":"fixture-mode"}),
            ),
            (json!({"mode_id":null}), json!({})),
            (json!({}), json!({})),
        ] {
            let defaults: RuntimeDefaults = serde_json::from_value(wire).unwrap();
            assert_eq!(serde_json::to_value(defaults).unwrap(), canonical);
        }
        let input: UpdateSettingsParams = serde_json::from_value(json!({"expectedVersion":1,"patch":{"depthLimit":8,"completedCacheMaxMb":0,"perRuntimeDefaults":{"codex":{"mode_id":"read-only","config_values":{"model":"fixture-model"}}}}})).unwrap();
        let tx = connection.transaction().unwrap();
        let configured = update(&tx, &input).unwrap();
        tx.commit().unwrap();
        let toggle =
            serde_json::from_value(json!({"expectedVersion":2,"patch":{"enabled":true}})).unwrap();
        let tx = connection.transaction().unwrap();
        let enabled = update(&tx, &toggle).unwrap();
        tx.commit().unwrap();
        assert!(enabled.enabled);
        assert_eq!(
            enabled.per_runtime_defaults,
            configured.per_runtime_defaults
        );
        // A stale full form cannot overwrite the separately saved switch or presets.
        let tx = connection.transaction().unwrap();
        assert!(
            update(&tx, &input)
                .unwrap_err()
                .to_string()
                .contains("revision conflict")
        );
        tx.rollback().unwrap();
        for patch in [
            json!({"depthLimit":0}),
            json!({"depthLimit":9}),
            json!({"completedCacheMaxMb":-1}),
            json!({"perRuntimeDefaults":{"codex":{"config_values":{"":"value"}}}}),
        ] {
            let tx = connection.transaction().unwrap();
            let invalid =
                serde_json::from_value(json!({"expectedVersion":3,"patch":patch})).unwrap();
            assert!(update(&tx, &invalid).is_err());
            tx.rollback().unwrap();
        }
        for patch in [
            json!({"enabled":null}),
            json!({"perRuntimeDefaults":{"unknown":{"config_values":{}}}}),
            json!({"depthLimit":true}),
            json!({"ownerAgentId":"forged"}),
        ] {
            assert!(serde_json::from_value::<SettingsPatch>(patch).is_err());
        }
        // The outer command's receipt/event failure must roll back the settings, too.
        let tx = connection.transaction().unwrap();
        let off =
            serde_json::from_value(json!({"expectedVersion":3,"patch":{"enabled":false}})).unwrap();
        update(&tx, &off).unwrap();
        tx.rollback().unwrap();
        assert_eq!(read(&connection).unwrap(), enabled);
        for sql in [
            "UPDATE delegation_settings SET depth_limit=0",
            "UPDATE delegation_settings SET depth_limit=1.5",
            "UPDATE delegation_settings SET version=1.5",
            "UPDATE delegation_settings SET enabled=2",
            "UPDATE delegation_settings SET per_runtime_defaults='[]'",
            "UPDATE delegation_settings SET completed_cache_max_mb=-1",
            "UPDATE delegation_settings SET completed_cache_max_mb=1.5",
        ] {
            assert!(connection.execute(sql, []).is_err(), "{sql}");
        }
        drop(connection);
        let reopened = Connection::open(&path).unwrap();
        assert_eq!(read(&reopened).unwrap(), enabled);
        let columns: Value = serde_json::to_value(read(&reopened).unwrap()).unwrap();
        assert_eq!(columns["version"], 3);
        assert_eq!(columns["completedCacheMaxMb"], 0);
        let changes = reopened.total_changes();
        read(&reopened).unwrap();
        assert_eq!(
            reopened.total_changes(),
            changes,
            "settings reads must not write"
        );
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
}
