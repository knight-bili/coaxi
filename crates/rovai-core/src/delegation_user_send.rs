//! Child User admission uses its own frozen target; it never resolves the owner's current default.
use crate::{
    agent_profile::{FrozenAgentRuntimeConfig, RuntimeConfigurationBlocker},
    command::{CommandEnvelope, CommandExecution, DomainCommand, canonical_json_digest, sealed},
    db::Database,
    draft_client::DraftClient,
    single_chat::{PrivateSendCommand, SendSingleChatMessageCommand, SingleChatService},
    single_chat_composer::ComposerOwner,
    single_chat_composer_scope::SingleChatComposerScope,
};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendDelegationMessageCommand {
    #[serde(
        default,
        skip_deserializing,
        skip_serializing_if = "DraftClient::is_desktop"
    )]
    pub draft_client: DraftClient,
    // Derived from the exact child by the Core entry point, never accepted from the User payload.
    #[serde(skip_deserializing, rename = "threadId")]
    pub camp_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer_scope: Option<SingleChatComposerScope>,
    pub expected_revision: i64,
    pub body: String,
}
impl sealed::Sealed for SendDelegationMessageCommand {}
impl DomainCommand for SendDelegationMessageCommand {
    const TYPE: &'static str = "delegations.message.send";
    fn draft_client(&self) -> Option<&DraftClient> {
        Some(&self.draft_client)
    }
}
impl PrivateSendCommand for SendDelegationMessageCommand {
    const OWNER: ComposerOwner = ComposerOwner::DelegationChild;
    fn input(&self) -> SendSingleChatMessageCommand {
        SendSingleChatMessageCommand {
            draft_client: self.draft_client.clone(),
            camp_id: self.camp_id.clone(),
            conversation_id: self.conversation_id.clone(),
            composer_scope: self.composer_scope.clone(),
            draft_revision: self.expected_revision,
            body: self.body.clone(),
        }
    }
}
pub fn send(
    database: &mut Database,
    envelope: &CommandEnvelope<SendDelegationMessageCommand>,
) -> Result<CommandExecution> {
    SingleChatService::default().send_owned(database, envelope)
}
pub fn send_source_inputs(
    database: &Database,
    envelope: &CommandEnvelope<SendDelegationMessageCommand>,
) -> Result<Vec<crate::local_attachment_source::LocalAttachmentSourceRef>> {
    SingleChatService::send_source_inputs(database, envelope)
}
pub(crate) fn send_with_prepared_sources(
    database: &mut Database,
    envelope: &CommandEnvelope<SendDelegationMessageCommand>,
    prepared: std::sync::Arc<crate::private_source_observation::PreparedSources>,
) -> Result<CommandExecution> {
    SingleChatService::default()
        .with_prepared_sources(prepared)
        .send_owned(database, envelope)
}

pub(crate) fn original_cleanup_blocker(
    connection: &Connection,
    conversation: &str,
) -> Result<Option<&'static str>> {
    // Only the original attempt tree participates. A separately taken-over descendant or a new
    // child created by a later User Run is outside the old parent's cancellation authority.
    let pending:bool=connection.query_row("WITH RECURSIVE tree(id) AS (
        SELECT d.id FROM delegation d WHERE d.child_conversation_id=?1
        UNION ALL SELECT child.id FROM delegation child JOIN tree ON child.parent_delegation_id=tree.id
        JOIN delegation_control control ON control.delegation_id=child.id
        WHERE control.takeover_at IS NULL AND EXISTS(SELECT 1 FROM delegation_attempt origin
            WHERE origin.delegation_id=tree.id AND origin.agent_run_id=child.origin_parent_run_id)
    ) SELECT EXISTS(SELECT 1 FROM tree JOIN delegation_attempt attempt ON attempt.delegation_id=tree.id
        JOIN agent_run run ON run.id=attempt.agent_run_id
        WHERE run.status IN ('queued','running','waiting') OR run.runtime_recovery_required<>0
          OR (run.cancel_requested_at IS NOT NULL AND run.cancel_acknowledged_at IS NULL)
          OR EXISTS(SELECT 1 FROM runtime_input_delivery input WHERE input.agent_run_id=run.id
              AND input.status IN ('prepared','delivery_unknown')))",[conversation],|r|r.get(0))?;
    if pending {
        return Ok(Some("delegation.cleanup_pending"));
    }
    Ok(None)
}

pub(crate) fn user_execution_pending(connection: &Connection, conversation: &str) -> Result<bool> {
    let busy:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM agent_run WHERE conversation_id=?1
        AND invocation_kind='delegation_user' AND (status IN ('queued','running','waiting') OR runtime_recovery_required<>0
            OR (cancel_requested_at IS NOT NULL AND cancel_acknowledged_at IS NULL) OR EXISTS(SELECT 1 FROM runtime_input_delivery input WHERE input.agent_run_id=agent_run.id AND input.status IN ('prepared','delivery_unknown'))))",[conversation],|r|r.get(0))?;
    Ok(busy)
}

fn original_config(connection: &Connection, conversation: &str) -> Result<Value> {
    let (config,adapter,installation,digest):(String,String,String,String)=connection.query_row(
        "SELECT run.effective_config_json,run.runtime_adapter_kind,run.runtime_installation_id,run.runtime_binding_compatibility_digest
        FROM delegation d JOIN delegation_attempt attempt ON attempt.delegation_id=d.id AND attempt.attempt_no=1
        JOIN agent_run run ON run.id=attempt.agent_run_id JOIN conversation c ON c.id=d.child_conversation_id
        WHERE c.id=?1 AND c.kind='delegation_child' AND run.conversation_id=c.id
          AND c.camp_id=d.camp_id AND c.agent_id=d.owner_agent_id AND run.invocation_kind='delegation'",[conversation],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    let config: Value = serde_json::from_str(&config)?;
    let policy = crate::delegation_policy::FrozenPolicy::from_config(&config)?
        .context("Missing child frozen policy")?;
    ensure!(
        policy.role == crate::delegation_policy::Role::Child,
        "Invalid child frozen role"
    );
    policy.validate_invocation("delegation_user")?;
    let runtime: FrozenAgentRuntimeConfig = serde_json::from_value(config["runtime"].clone())?;
    ensure!(
        runtime.adapter_kind.as_str() == adapter
            && runtime.installation_id == installation
            && runtime.binding_compatibility_digest == digest,
        "Child frozen Runtime identity mismatch"
    );
    Ok(config)
}

pub(crate) fn followup_runtime(
    connection: &Connection,
    conversation: &str,
) -> Result<std::result::Result<FrozenAgentRuntimeConfig, RuntimeConfigurationBlocker>> {
    let config = original_config(connection, conversation)?;
    let frozen = serde_json::from_value(config["runtime"].clone())?;
    crate::agent_profile::resolve_rebound_runtime_on_connection(connection, &frozen)
}

pub(crate) fn followup_config(
    tx: &Transaction<'_>,
    conversation: &str,
    version: i64,
    runtime: &FrozenAgentRuntimeConfig,
) -> Result<(Value, FrozenAgentRuntimeConfig)> {
    let mut config = original_config(tx, conversation)?;
    let policy = crate::delegation_policy::FrozenPolicy::from_config(&config)?
        .context("Missing child policy")?;
    let runtime = policy.freeze_resolved_runtime(runtime)?;
    config["conversationVersion"] = json!(version);
    config["runtime"] = serde_json::to_value(&runtime)?;
    config["runtimeAdapter"] = json!(runtime.adapter_kind);
    config["provider"] = json!(runtime.adapter_kind);
    config["model"] = json!(runtime.model.model_id);
    config
        .as_object_mut()
        .context("Invalid child effective config")?
        .remove("configDigest");
    config["configDigest"] = json!(canonical_json_digest(&config)?);
    Ok((config, runtime))
}
