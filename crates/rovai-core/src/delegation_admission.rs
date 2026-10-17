//! New-work admission after the transport authenticated its current Run lease.
//! Original-request recovery precedes new admission in the command layer.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::{
    command::canonical_json_digest,
    delegation::{FrozenRequest, ParentInvocation, Registration},
    delegation_contract::WorkingDirectory,
    delegation_factory::ChildRunPlan,
    delegation_policy::{FrozenPolicy, Role},
    delegation_settings::DelegationSettings,
    runtime::AgentRunWorkspace,
    team_tool::{AuthenticatedTeamToolRun, BuiltinToolBindingCredential, TeamToolService},
};

// Contains an ephemeral credential. Deliberately neither Debug nor Serialize.
pub struct ParentStartAdmission {
    binding: BuiltinToolBindingCredential,
    source: AuthenticatedTeamToolRun,
    tool_call: String,
    conversation: String,
    workspace: AgentRunWorkspace,
    settings: DelegationSettings,
    child_policy: FrozenPolicy,
    fingerprint: String,
}

pub fn authenticate_new_child(
    connection: &Connection,
    binding: &BuiltinToolBindingCredential,
    source: &AuthenticatedTeamToolRun,
    tool_call: &str,
) -> Result<ParentStartAdmission> {
    let actual = TeamToolService::default().authenticate_attested_binding_on_connection(
        connection,
        &binding.native_binding_id,
        &binding.binding_credential,
        tool_call,
        &source.agent_run_id,
        source.execution_epoch,
    )?;
    ensure!(&actual == source, "delegation.source_scope_mismatch");
    let (conversation,kind,invocation,policy_name,policy_version,response,destination,config,workspace,depth):
        (String,String,String,String,i64,String,Option<String>,String,String,i64)=connection.query_row(
        "SELECT c.id,c.kind,r.invocation_kind,r.operation_policy,r.operation_policy_version,
            r.response_delivery,r.destination_conversation_id,r.effective_config_json,r.workspace_json,
            COALESCE(d.depth,0)
         FROM agent_run r JOIN conversation c ON c.id=r.conversation_id JOIN camp ON camp.id=c.camp_id
         JOIN agent_profile a ON a.id=c.agent_id LEFT JOIN delegation d ON d.child_conversation_id=c.id
         WHERE r.id=?1 AND r.execution_epoch=?2 AND c.camp_id=?3 AND c.agent_id=?4
           AND c.native_binding_id=?5 AND c.native_binding_generation=?6
           AND c.native_binding_compatibility_digest=r.runtime_binding_compatibility_digest
           AND r.runtime_binding_compatibility_digest=?7 AND c.ended_at IS NULL
           AND camp.activation_state='active' AND camp.deletion_operation_id IS NULL AND a.profile_status='present'",
        params![source.agent_run_id,source.execution_epoch,source.camp_id,source.agent_id,
            binding.native_binding_id,binding.native_binding_generation,binding.binding_compatibility_digest],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?)),
    ).optional()?.context("delegation.parent_fenced")?;
    let config: Value = serde_json::from_str(&config)?;
    let mut unsigned = config.clone();
    let digest = unsigned
        .as_object_mut()
        .context("delegation.parent_config_invalid")?
        .remove("configDigest")
        .context("delegation.parent_config_digest_required")?;
    ensure!(
        digest.as_str() == Some(canonical_json_digest(&unsigned)?.as_str()),
        "delegation.parent_config_digest_mismatch"
    );
    let frozen = FrozenPolicy::from_config(&config)?
        .context("delegation.frozen_policy_required")?
        .validate_invocation(&invocation)?;
    ensure!(
        matches!(
            (frozen.role, kind.as_str()),
            (Role::PublicParent, "camp_member")
                | (Role::PrivateParent, "single_chat")
                | (Role::Child, "delegation_child")
        ),
        "delegation.parent_role_mismatch"
    );
    if frozen.role != Role::PublicParent {
        ensure!(
            response == "conversation_message"
                && destination.as_deref() == Some(conversation.as_str()),
            "delegation.private_parent_route_mismatch"
        );
    }
    if frozen.role == Role::PrivateParent {
        ensure!(
            policy_name == "single_chat_v1" && policy_version == 3,
            "delegation.private_parent_policy_mismatch"
        );
    }
    if frozen.role == Role::Child {
        ensure!(
            policy_name == "delegation_child_v1" && policy_version == 1,
            "delegation.child_parent_policy_mismatch"
        );
        let (attempt, taken_over): (String, bool) = connection.query_row(
            "SELECT a.agent_run_id,c.takeover_at IS NOT NULL FROM delegation d
             JOIN delegation_control c ON c.delegation_id=d.id
             JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=c.current_attempt_no
             WHERE d.child_conversation_id=?1",
            [&conversation],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        ensure!(
            (invocation == "delegation" && attempt == source.agent_run_id && !taken_over)
                || (invocation == "delegation_user" && taken_over),
            "delegation.parent_control_changed"
        );
    }
    let settings = crate::delegation_settings::read(connection)?;
    ensure!(
        frozen.allows("delegation.start", settings.enabled),
        "delegation.start_disabled"
    );
    let limit = frozen.depth_limit.min(settings.depth_limit);
    ensure!(
        depth
            .checked_add(1)
            .is_some_and(|next| next <= i64::from(limit)),
        "delegation.depth_limit"
    );
    let workspace: AgentRunWorkspace = serde_json::from_str(&workspace)?;
    workspace.validate()?;
    let child_policy = FrozenPolicy::new(true, limit, Role::Child)?;
    let fingerprint = canonical_json_digest(
        &json!({"run":source.agent_run_id,"epoch":source.execution_epoch,
        "binding":binding.native_binding_id,"generation":binding.native_binding_generation,"conversation":conversation,
        "toolCall":tool_call,"configDigest":config["configDigest"],"workspace":workspace,"policy":child_policy,"defaults":settings.per_runtime_defaults}),
    )?;
    Ok(ParentStartAdmission {
        binding: binding.clone(),
        source: actual,
        tool_call: tool_call.into(),
        conversation,
        workspace,
        settings,
        child_policy,
        fingerprint,
    })
}

impl ParentStartAdmission {
    pub fn conversation_id(&self) -> &str {
        &self.conversation
    }
    pub fn settings(&self) -> &DelegationSettings {
        &self.settings
    }
    pub fn child_policy(&self) -> FrozenPolicy {
        self.child_policy
    }
    pub fn revalidate(&self, connection: &Connection) -> Result<()> {
        let current =
            authenticate_new_child(connection, &self.binding, &self.source, &self.tool_call)?;
        ensure!(
            current.fingerprint == self.fingerprint,
            "delegation.parent_snapshot_changed"
        );
        Ok(())
    }
    /// Filesystem observation belongs outside the global database lock. Keep the
    /// original absolute request and freeze the resolved actual directory.
    pub fn observe_workspace(
        &self,
        requested: Option<&WorkingDirectory>,
    ) -> Result<WorkspaceObservation> {
        let root = std::fs::canonicalize(&self.workspace.execution_root)
            .context("delegation.parent_directory_unavailable")?;
        ensure!(root.is_dir(), "delegation.parent_directory_required");
        let original = requested.map(|p| p.as_path().to_path_buf());
        let target = std::fs::canonicalize(original.as_deref().unwrap_or(&root))
            .context("delegation.directory_unavailable")?;
        ensure!(target.is_dir(), "delegation.directory_required");
        let within_parent = target.starts_with(&root);
        let workspace = AgentRunWorkspace {
            execution_root: target
                .to_str()
                .context("delegation.directory_not_utf8")?
                .into(),
            access: self.workspace.access.clone(),
            isolation: self.workspace.isolation.clone(),
        };
        Ok(WorkspaceObservation {
            parent_fingerprint: self.fingerprint.clone(),
            requested: original,
            workspace,
            within_parent,
        })
    }
    /// New work only. The command layer recovers an original recorded request
    /// before selecting a new plan. External directories require a separate,
    /// exact User permission fact; observation alone never supplies that fact.
    pub fn register_initial_within_parent(
        &self,
        tx: &Transaction<'_>,
        request: &FrozenRequest<'_>,
        plan: &ChildRunPlan<'_>,
        observation: &WorkspaceObservation,
        at: &str,
    ) -> Result<Registration> {
        self.validate_plan(tx, request, plan, observation)?;
        ensure!(
            plan.workspace == observation.authorized_workspace()?,
            "delegation.workspace_plan_changed"
        );
        crate::delegation_factory::register_initial(
            tx,
            &ParentInvocation {
                run_id: &self.source.agent_run_id,
                epoch: self.source.execution_epoch,
                binding_id: &self.binding.native_binding_id,
                binding_generation: self.binding.native_binding_generation,
                tool_call_id: &self.tool_call,
            },
            request,
            plan,
            at,
        )
    }

    /// Persist the queued child and its mandatory Core permission request in one
    /// transaction. This does not authorize workspace preparation or spawn.
    pub fn register_initial_awaiting_workspace_permission(
        &self,
        tx: &Transaction<'_>,
        request: &FrozenRequest<'_>,
        plan: &ChildRunPlan<'_>,
        observation: &WorkspaceObservation,
        at: &str,
    ) -> Result<Registration> {
        self.validate_plan(tx, request, plan, observation)?;
        ensure!(
            observation.requires_user_permission(),
            "delegation.external_directory_required"
        );
        let registered = crate::delegation_factory::register_initial(
            tx,
            &ParentInvocation {
                run_id: &self.source.agent_run_id,
                epoch: self.source.execution_epoch,
                binding_id: &self.binding.native_binding_id,
                binding_generation: self.binding.native_binding_generation,
                tool_call_id: &self.tool_call,
            },
            request,
            plan,
            at,
        )?;
        if !registered.replayed {
            crate::delegation_workspace_permission::prepare(tx, &registered.identity.id, at)?;
        } else {
            ensure!(
                crate::delegation_workspace_permission::exists(tx, &registered.identity.id)?,
                "delegation.workspace_permission_missing"
            );
        }
        Ok(registered)
    }

    fn validate_plan(
        &self,
        tx: &Transaction<'_>,
        request: &FrozenRequest<'_>,
        plan: &ChildRunPlan<'_>,
        observation: &WorkspaceObservation,
    ) -> Result<()> {
        self.revalidate(tx)?;
        let defaults = self
            .settings
            .per_runtime_defaults
            .get(request.runtime)
            .cloned()
            .unwrap_or_default();
        ensure!(
            *request.runtime_preset == serde_json::to_value(defaults)?,
            "delegation.runtime_defaults_plan_changed"
        );
        ensure!(
            observation.parent_fingerprint == self.fingerprint,
            "delegation.workspace_owner_mismatch"
        );
        ensure!(
            request.working_dir.map(|p| p.as_path()) == observation.requested.as_deref(),
            "delegation.directory_request_changed"
        );
        ensure!(
            plan.workspace == observation.candidate(),
            "delegation.workspace_plan_changed"
        );
        ensure!(
            plan.policy == self.child_policy,
            "delegation.child_policy_changed"
        );
        Ok(())
    }
}

pub struct WorkspaceObservation {
    parent_fingerprint: String,
    requested: Option<PathBuf>,
    workspace: AgentRunWorkspace,
    within_parent: bool,
}
impl WorkspaceObservation {
    pub fn requires_user_permission(&self) -> bool {
        !self.within_parent
    }
    pub fn requested_path(&self) -> Option<&Path> {
        self.requested.as_deref()
    }
    pub fn candidate(&self) -> &AgentRunWorkspace {
        &self.workspace
    }
    pub fn authorized_workspace(&self) -> Result<&AgentRunWorkspace> {
        ensure!(
            self.within_parent,
            "delegation.workspace_permission_required"
        );
        Ok(&self.workspace)
    }
}
