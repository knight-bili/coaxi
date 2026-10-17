//! Frozen Core capability, not a model-provided preference. Global settings can revoke new work,
//! but cannot grant operations to a Run which was admitted without this snapshot.
use crate::{
    agent_profile::FrozenAgentRuntimeConfig, command::canonical_json_digest, context_delivery::*,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    PublicParent,
    PrivateParent,
    Child,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenPolicy {
    schema_version: u8,
    pub enabled: bool,
    pub depth_limit: u8,
    pub role: Role,
}
impl FrozenPolicy {
    pub fn new(enabled: bool, depth_limit: u8, role: Role) -> Result<Self> {
        anyhow::ensure!(
            (1..=8).contains(&depth_limit),
            "Invalid frozen delegation depth"
        );
        Ok(Self {
            schema_version: 1,
            enabled,
            depth_limit,
            role,
        })
    }
    pub fn from_config(config: &Value) -> Result<Option<Self>> {
        let Some(value) = config.get("delegationPolicy") else {
            return Ok(None);
        };
        let frozen: Self =
            serde_json::from_value(value.clone()).context("Invalid frozen delegation policy")?;
        anyhow::ensure!(
            frozen.schema_version == 1 && (1..=8).contains(&frozen.depth_limit),
            "Unsupported frozen delegation policy"
        );
        Ok(Some(frozen))
    }
    pub fn validate_invocation(self, invocation: &str) -> Result<Self> {
        let valid = match self.role {
            Role::PublicParent => {
                matches!(invocation, "batch" | "direct" | "a2a" | "gather_completion")
            }
            Role::PrivateParent => invocation == "single_chat",
            Role::Child => matches!(invocation, "delegation" | "delegation_user"),
        };
        anyhow::ensure!(valid, "Delegation role does not match the frozen Run");
        Ok(self)
    }
    pub fn allows(self, operation: &str, current_enabled: bool) -> bool {
        match operation {
            "delegation.start" | "delegation.resume" => self.enabled && current_enabled,
            "delegation.status" | "delegation.cancel" | "delegation.read"
            | "delegation.history" => true,
            _ => false,
        }
    }
    pub fn guidance(self) -> Result<Value> {
        anyhow::ensure!(
            self.role != Role::Child,
            "Child uses DELEGATION_CONTEXT, not parent guidance"
        );
        Ok(
            json!({"schemaVersion":1,"enabled":self.enabled,"depthLimit":self.depth_limit,"role":"parent","resultReadCommand":"rovai delegation read"}),
        )
    }
    pub fn profile(self) -> ContextDeliveryProfile {
        match self.role {
            Role::PublicParent => DELEGATION_PUBLIC_PROFILE_V12,
            Role::PrivateParent => DELEGATION_PRIVATE_PROFILE_V13,
            Role::Child => DELEGATION_CHILD_PROFILE_V11,
        }
    }
    /// Only a newly resolved registry Runtime is passed here, never a previously tagged snapshot.
    pub(crate) fn freeze_resolved_runtime(
        self,
        runtime: &FrozenAgentRuntimeConfig,
    ) -> Result<FrozenAgentRuntimeConfig> {
        let mut frozen = runtime.clone();
        let context = json!({"nativeSessionBootstrap":crate::context_contract::NATIVE_SESSION_BOOTSTRAP_CONTRACT_VERSION,"bootstrapFormatterVersion":crate::context_contract::BOOTSTRAP_FORMATTER_VERSION,"sessionCharterRevision":crate::context_contract::DELEGATION_BINDING_REVISION,"agentRunContextFormatterVersion":crate::context_contract::DELEGATION_CONTEXT_VERSION,"contextManifestVersion":crate::context_contract::DELEGATION_CONTEXT_VERSION,"delegationRole":self.role});
        frozen.binding_compatibility_digest = canonical_json_digest(
            &json!({"runtimeBinding":runtime.binding_compatibility_digest,"context":context}),
        )?;
        frozen.native_session_compatibility_key = runtime
            .native_session_compatibility_key
            .as_ref()
            .map(|key| canonical_json_digest(&json!({"runtimeSession":key,"context":context})))
            .transpose()?;
        frozen.refresh_config_digest()?;
        Ok(frozen)
    }
}

/// Core startup must have admitted the complete 160/210 contract before producers can opt in.
/// The unregistered identity prototype, or a settings table on its own, never activates the feature.
pub(crate) fn freeze_for_conversation(
    connection: &Connection,
    conversation_id: &str,
) -> Result<Option<FrozenPolicy>> {
    let ready:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM rovai_data_contract WHERE singleton=1 AND projection_schema_version>=160) AND EXISTS(SELECT 1 FROM schema_migration WHERE version=210)",[],|r|r.get(0))?;
    if !ready {
        return Ok(None);
    }
    let kind: String = connection.query_row(
        "SELECT kind FROM conversation WHERE id=?1 AND ended_at IS NULL",
        [conversation_id],
        |r| r.get(0),
    )?;
    let settings = crate::delegation_settings::read(connection)?;
    let role = match kind.as_str() {
        "camp_member" => Role::PublicParent,
        "single_chat" => Role::PrivateParent,
        "delegation_child" => Role::Child,
        _ => anyhow::bail!("Unsupported delegation Conversation"),
    };
    let has_children: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM delegation WHERE parent_conversation_id=?1)",
        [conversation_id],
        |r| r.get(0),
    )?;
    if !settings.enabled && !has_children && role != Role::Child {
        return Ok(None);
    }
    FrozenPolicy::new(settings.enabled, settings.depth_limit, role).map(Some)
}

pub(crate) fn child_identity(
    connection: &Connection,
    conversation_id: &str,
    owner: &str,
    camp_id: &str,
) -> Result<(String, String)> {
    connection.query_row("SELECT d.id,d.requested_runtime FROM delegation d JOIN conversation c ON c.id=d.child_conversation_id JOIN camp_member m ON m.camp_id=c.camp_id AND m.agent_id=c.agent_id JOIN agent_profile a ON a.id=c.agent_id WHERE c.id=?1 AND c.kind='delegation_child' AND c.camp_id=?2 AND c.agent_id=?3 AND d.camp_id=c.camp_id AND d.owner_agent_id=c.agent_id AND m.status='active' AND a.profile_status<>'removed'",params![conversation_id,camp_id,owner],|r|Ok((r.get(0)?,r.get(1)?))).optional()?.context("Missing owned delegation child identity")
}

pub(crate) fn child_display_name(
    connection: &Connection,
    camp: &str,
    conversation: &str,
    owner: &str,
) -> Result<String> {
    let (id, target, config): (String, String, String) = connection
        .query_row(
            "SELECT d.id,d.requested_runtime,r.effective_config_json
         FROM delegation d JOIN conversation c ON c.id=d.child_conversation_id
         JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=1
         JOIN agent_run r ON r.id=a.agent_run_id
         WHERE c.id=?1 AND c.camp_id=?2 AND c.agent_id=?3 AND c.kind='delegation_child'
           AND d.camp_id=c.camp_id AND d.owner_agent_id=c.agent_id AND r.conversation_id=c.id",
            params![conversation, camp, owner],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .context("Missing owned child quote author")?;
    let config: Value = serde_json::from_str(&config)?;
    let policy =
        FrozenPolicy::from_config(&config)?.context("Missing child quote author policy")?;
    anyhow::ensure!(
        policy.role == Role::Child,
        "Invalid child quote author policy"
    );
    let runtime: FrozenAgentRuntimeConfig = serde_json::from_value(config["runtime"].clone())?;
    let label = if target.starts_with("custom:") {
        format!("Custom ACP {}", target.trim_start_matches("custom:"))
    } else {
        runtime.adapter_kind.display_name().to_string()
    };
    Ok(format!(
        "{} {}",
        label,
        id.chars().take(8).collect::<String>()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_capability_cannot_be_granted_by_later_settings_or_private_shape() {
        assert_eq!(
            FrozenPolicy::from_config(&json!({"runtime":"legacy"})).unwrap(),
            None
        );
        for invalid in [
            json!(null),
            json!({"schemaVersion":1,"enabled":true,"depthLimit":9,"role":"child"}),
            json!({"schemaVersion":2,"enabled":true,"depthLimit":1,"role":"public_parent"}),
            json!({"schemaVersion":1,"enabled":true,"depthLimit":1,"role":"private_parent","ownerAgentId":"forged"}),
        ] {
            assert!(FrozenPolicy::from_config(&json!({"delegationPolicy":invalid})).is_err());
        }
        for role in [Role::PublicParent, Role::PrivateParent, Role::Child] {
            for frozen_enabled in [false, true] {
                let frozen = FrozenPolicy::new(frozen_enabled, 8, role).unwrap();
                let config = json!({"delegationPolicy":frozen});
                for current_enabled in [false, true] {
                    assert_eq!(FrozenPolicy::from_config(&config).unwrap(), Some(frozen));
                    for op in ["delegation.start", "delegation.resume"] {
                        assert_eq!(
                            frozen.allows(op, current_enabled),
                            frozen_enabled && current_enabled
                        );
                    }
                    for op in [
                        "delegation.status",
                        "delegation.cancel",
                        "delegation.read",
                        "delegation.history",
                    ] {
                        assert!(frozen.allows(op, current_enabled));
                    }
                    assert!(!frozen.allows("task.create", current_enabled));
                }
                frozen.profile().validate().unwrap();
                if role == Role::Child {
                    assert!(frozen.guidance().is_err());
                } else {
                    assert_eq!(
                        frozen.guidance().unwrap(),
                        json!({"schemaVersion":1,"enabled":frozen_enabled,"depthLimit":8,"role":"parent","resultReadCommand":"rovai delegation read"})
                    );
                }
            }
        }
        assert!(
            FrozenPolicy::new(true, 1, Role::Child)
                .unwrap()
                .validate_invocation("single_chat")
                .is_err()
        );
        assert!(
            FrozenPolicy::new(true, 1, Role::PrivateParent)
                .unwrap()
                .validate_invocation("batch")
                .is_err()
        );
        // Minimal SQL owner for the feature gate, not a Core migration/authentication fixture.
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE rovai_data_contract(singleton INTEGER,projection_schema_version INTEGER); INSERT INTO rovai_data_contract VALUES(1,159); CREATE TABLE schema_migration(version INTEGER); CREATE TABLE conversation(id TEXT,kind TEXT,ended_at TEXT); INSERT INTO conversation VALUES('parent','camp_member',NULL),('private','single_chat',NULL),('child','delegation_child',NULL); CREATE TABLE delegation(parent_conversation_id TEXT);").unwrap();
        connection
            .execute_batch(crate::delegation_settings::SCHEMA)
            .unwrap();
        connection
            .execute("UPDATE delegation_settings SET enabled=1", [])
            .unwrap();
        assert_eq!(
            freeze_for_conversation(&connection, "parent").unwrap(),
            None
        );
        connection
            .execute(
                "UPDATE rovai_data_contract SET projection_schema_version=160",
                [],
            )
            .unwrap();
        assert_eq!(
            freeze_for_conversation(&connection, "parent").unwrap(),
            None
        );
        connection
            .execute("INSERT INTO schema_migration VALUES(210)", [])
            .unwrap();
        let frozen = freeze_for_conversation(&connection, "parent")
            .unwrap()
            .unwrap();
        assert_eq!(frozen.role, Role::PublicParent);
        let config = json!({"delegationPolicy":frozen});
        assert_eq!(
            freeze_for_conversation(&connection, "private")
                .unwrap()
                .unwrap()
                .role,
            Role::PrivateParent
        );
        assert_eq!(
            freeze_for_conversation(&connection, "child")
                .unwrap()
                .unwrap()
                .role,
            Role::Child
        );
        connection
            .execute("UPDATE delegation_settings SET enabled=0,depth_limit=8", [])
            .unwrap();
        assert_eq!(
            freeze_for_conversation(&connection, "parent").unwrap(),
            None
        );
        assert_eq!(FrozenPolicy::from_config(&config).unwrap(), Some(frozen));
        connection
            .execute("INSERT INTO delegation VALUES('parent')", [])
            .unwrap();
        let history_only = freeze_for_conversation(&connection, "parent")
            .unwrap()
            .unwrap();
        assert!(!history_only.enabled && history_only.depth_limit == 8);
        assert!(!history_only.allows("delegation.start", true));
        assert!(history_only.allows("delegation.history", false));
    }
}
