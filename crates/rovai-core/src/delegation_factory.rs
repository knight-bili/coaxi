//! Transactional initial child creation from a Core-selected target, never a copied parent Run.
//! The authenticated command layer owns parent eligibility, User permission grants and replay.
//! No filesystem preparation or Runtime process launch occurs in this transaction.
use anyhow::{Context, Result, ensure};
use rusqlite::{Transaction, params};
use serde_json::json;
use uuid::Uuid;

use crate::{
    agent_profile::{AdapterKind, MemberRuntimeConfiguration, ResolvedRuntimeBinding},
    command::canonical_json_digest,
    delegation::Identity,
    delegation_contract::RuntimeTarget,
    delegation_policy::{FrozenPolicy, Role},
    runtime::AgentRunWorkspace,
};

pub struct ChildRunPlan<'a> {
    pub task: &'a str,
    pub target: &'a RuntimeTarget,
    pub installation_id: &'a str,
    pub configuration: &'a MemberRuntimeConfiguration,
    pub workspace: &'a AgentRunWorkspace,
    pub policy: FrozenPolicy,
}

/// Codeg wire identity maps to the existing Rovai adapter; custom targets use an exact
/// persisted installation identity and retain its real adapter instead of a guessed provider.
pub fn builtin_adapter(target: &RuntimeTarget) -> Option<AdapterKind> {
    Some(match target.as_str() {
        "antigravity" => AdapterKind::AntigravityApp,
        "claude_code" => AdapterKind::ClaudeCodeCli,
        "cline" => AdapterKind::ClineCli,
        "code_buddy" => AdapterKind::CodebuddyCli,
        "codex" => AdapterKind::CodexCli,
        "command_code" => AdapterKind::CommandCodeCli,
        "copilot" => AdapterKind::CopilotCli,
        "cursor" => AdapterKind::CursorAgent,
        "deepseek" => AdapterKind::DeepseekHarness,
        "gemini" => AdapterKind::GeminiCli,
        "grok" => AdapterKind::GrokBuild,
        "hermes" => AdapterKind::HermesCli,
        "kimi_code" => AdapterKind::KimiCodeCli,
        "kiro" => AdapterKind::KiroCli,
        "open_claw" => AdapterKind::OpenClawCli,
        "open_code" => AdapterKind::OpencodeCli,
        "pi" => AdapterKind::Pi,
        "qoder" => AdapterKind::QoderCli,
        "qwen_code" => AdapterKind::QwenCode,
        "trae_cn" => AdapterKind::TraeCnCli,
        "zcode" => AdapterKind::ZcodeApp,
        _ => return None,
    })
}

/// The command layer supplies one validated plan. Its persisted identity and
/// the execution plan cannot disagree about the requested target or workspace.
pub fn register_initial(
    tx: &Transaction<'_>,
    parent: &crate::delegation::ParentInvocation<'_>,
    request: &crate::delegation::FrozenRequest<'_>,
    plan: &ChildRunPlan<'_>,
    at: &str,
) -> Result<crate::delegation::Registration> {
    ensure!(
        request.runtime == plan.target,
        "Initial delegation target plan mismatch"
    );
    ensure!(
        *request.workspace == serde_json::to_value(plan.workspace)?,
        "Initial delegation workspace plan mismatch"
    );
    crate::delegation::register(tx, parent, request, at, |tx, identity| {
        create_child(tx, identity, plan, request.runtime_preset, at)
    })
}

/// Called as the factory of delegation::register; propagate every error so its
/// outer command transaction rolls back the complete identity/message/Run graph.
pub(crate) fn create_child(
    tx: &Transaction<'_>,
    identity: &Identity,
    plan: &ChildRunPlan<'_>,
    preset: &serde_json::Value,
    at: &str,
) -> Result<String> {
    ensure!(!plan.task.is_empty(), "Missing complete delegation task");
    ensure!(
        plan.policy.role == Role::Child,
        "Initial child requires child policy"
    );
    plan.policy.validate_invocation("delegation")?;
    ensure!(
        plan.policy.enabled,
        "Initial child cannot freeze disabled delegation"
    );
    ensure!(
        identity.depth <= i64::from(plan.policy.depth_limit),
        "Frozen delegation depth exceeded"
    );
    plan.workspace.validate()?;
    if let Some(expected) = builtin_adapter(plan.target) {
        ensure!(
            plan.configuration.adapter_kind == expected,
            "Delegation target adapter mismatch"
        );
    } else {
        ensure!(
            plan.target.as_str().strip_prefix("custom:") == Some(plan.installation_id),
            "Custom delegation installation mismatch"
        );
        let custom: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM adapter_installation WHERE id=?1 AND installation_class='custom')",
            [plan.installation_id], |r|r.get(0),
        )?;
        ensure!(
            custom,
            "Custom delegation requires its registered custom installation"
        );
    }
    let admission = crate::agent_runtime_adapter::AgentRuntimeAdapterRegistry::default()
        .current_platform_admission(plan.configuration.adapter_kind)
        .context("Unknown Runtime host platform")?;
    ensure!(
        admission.allows_runtime_use(),
        "Delegation Runtime is not admitted on this platform"
    );
    let defaults =
        serde_json::from_value(preset.clone()).context("Invalid delegation Runtime defaults")?;
    let selected =
        crate::delegation_runtime_defaults::configuration(plan.configuration, &defaults)?;
    let base = crate::agent_profile::resolve_frozen_runtime_binding(
        tx,
        &ResolvedRuntimeBinding {
            custom_runtime_ref: selected.custom_runtime_ref.clone(),
            adapter_kind: plan.configuration.adapter_kind,
            installation_id: plan.installation_id.into(),
            model: selected.model,
            permissions: selected.permissions,
        },
    )?
    .map_err(|blocker| anyhow::anyhow!("{}", blocker.code))?;
    // Resolve afresh through the actual Registry. A previously child-tagged parent
    // binding, Session, evidence, lease or recovery state never enters this Run.
    let runtime = plan.policy.freeze_resolved_runtime(&base)?;
    tx.execute("INSERT INTO conversation(id,camp_id,agent_id,kind,created_at,updated_at,last_message_sequence)
        VALUES(?1,?2,?3,'delegation_child',?4,?4,1)",params![identity.child_conversation_id,identity.camp_id,identity.owner_agent_id,at])?;
    let mut config = crate::collaboration::build_effective_config(
        tx,
        &identity.child_conversation_id,
        &identity.owner_agent_id,
        &runtime,
    )?;
    config
        .as_object_mut()
        .context("Invalid child effective config")?
        .remove("configDigest");
    config["delegationPolicy"] = serde_json::to_value(plan.policy)?;
    config["configDigest"] = json!(canonical_json_digest(&config)?);
    append_attempt(
        tx,
        identity,
        plan.task,
        &config,
        &runtime,
        plan.workspace,
        1,
        1,
        at,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn append_attempt(
    tx: &Transaction<'_>,
    identity: &Identity,
    message_body: &str,
    config: &serde_json::Value,
    runtime: &crate::agent_profile::FrozenAgentRuntimeConfig,
    workspace: &AgentRunWorkspace,
    sequence: i64,
    attempt: i64,
    at: &str,
) -> Result<String> {
    let now = chrono::DateTime::parse_from_rfc3339(at)?.with_timezone(&chrono::Utc);
    let budget = crate::execution_budget::freeze_camp_turn_execution_budget(None, now, 1)?;
    let message = Uuid::new_v4().to_string();
    let turn = Uuid::new_v4().to_string();
    let run = Uuid::new_v4().to_string();
    tx.execute("INSERT INTO conversation_message(id,conversation_id,sequence,author_type,author_id,body,created_at)
        VALUES(?1,?2,?5,'system','delegation-parent',?3,?4)",params![message,identity.child_conversation_id,message_body,at,sequence])?;
    tx.execute("INSERT INTO camp_turn(id,camp_id,trigger_type,trigger_id,status,kind,created_at,updated_at,
        execution_budget_schema_version,execution_budget_accepted_at,execution_budget_deadline_at,
        execution_budget_elapsed_seconds,execution_budget_max_agent_run_responsibilities,
        execution_budget_max_accepted_a2a,execution_budget_root_agent_run_responsibilities)
        VALUES(?1,?2,'conversation_message',?3,'running','delegation',?4,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![turn,identity.camp_id,message,at,budget.schema_version,budget.accepted_at,budget.deadline_at,
            budget.elapsed_seconds,budget.max_agent_run_responsibilities,budget.max_accepted_a2a,budget.root_agent_run_responsibilities])?;
    tx.execute("INSERT INTO agent_run(
        id,camp_turn_id,conversation_id,trigger_conversation_message_id,input_ready_at,
        initial_camp_context_through_sequence,initial_conversation_context_through_sequence,
        responsibility_key,responsibility_generation,start_reason,purpose,completion_role,
        effective_config_json,workspace_json,permission_semantics,
        runtime_adapter_kind,runtime_installation_id,runtime_executable_path,runtime_auth_scope,
        runtime_reported_version,runtime_executable_fingerprint,runtime_initial_reported_version,
        runtime_initial_executable_fingerprint,runtime_capabilities_json,runtime_model_selection_json,
        runtime_permission_config_json,runtime_binding_compatibility_digest,runtime_host_config_digest,
        runtime_protocol_version,runtime_installation_generation,runtime_search_environment_generation,
        runtime_native_session_compatibility_key,invocation_kind,response_delivery,operation_policy,
        operation_policy_version,destination_conversation_id,status,idempotency_key,version,created_at,updated_at)
        VALUES(?1,?2,?3,?4,?5,0,?24,?6,0,?25,?26,'required',
        ?7,?8,'runtime_managed_v2',?9,?10,?11,?12,?13,?14,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,
        'delegation','conversation_message','delegation_child_v1',1,?3,'queued',?6,1,?5,?5)",
        params![run,turn,identity.child_conversation_id,message,at,if attempt==1 {format!("delegation/{}/initial",identity.id)} else {format!("delegation/{}/resume/{attempt}",identity.id)},
            serde_json::to_string(config)?,serde_json::to_string(workspace)?,runtime.adapter_kind.as_str(),
            runtime.installation_id,runtime.executable_path,runtime.auth_scope,runtime.reported_version,
            runtime.executable_fingerprint,serde_json::to_string(&runtime.capabilities)?,serde_json::to_string(&runtime.model)?,
            serde_json::to_string(&runtime.permissions)?,runtime.binding_compatibility_digest,runtime.host_config_digest,
            runtime.protocol_version,runtime.installation_generation,runtime.search_environment_generation,runtime.native_session_compatibility_key,
            sequence,if attempt==1 {"initial"} else {"retry"},if attempt==1 {"Execute the complete delegated task"} else {"Continue the original delegated task"}])?;
    Ok(run)
}
