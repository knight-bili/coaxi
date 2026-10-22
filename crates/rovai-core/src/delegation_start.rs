//! Complete production start planning. Identity comes from the authenticated
//! transport; only the observed filesystem stage runs outside Core's DB mutex.
use crate::{
    agent_profile::{AdapterKind, MemberRuntimeConfiguration},
    command::{CommandExecution, CommandResultStatus, canonical_json_digest},
    db::Database,
    delegation_admission::{ParentStartAdmission, WorkspaceObservation},
    delegation_contract::StartParams,
    delegation_model_command::{self as model, Invocation},
    managed_blob::{
        CandidateReservation, GC_OWNER_EXECUTION_LIFECYCLE, ManagedBlobStore, PreparedBlobCandidate,
    },
};
use anyhow::{Context, Result, ensure};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};

struct Plan {
    admission: ParentStartAdmission,
    input: StartParams,
    installation: String,
    configuration: MemberRuntimeConfiguration,
    preset: Value,
    reservation: CandidateReservation,
}
enum Preparation {
    Recorded,
    Denied(String),
    Ready(Box<Plan>),
}
pub(crate) struct PreparedStart {
    invocation: String,
    state: Preparation,
}
struct ObservedPlan {
    admission: ParentStartAdmission,
    input: StartParams,
    installation: String,
    configuration: MemberRuntimeConfiguration,
    preset: Value,
    workspace: WorkspaceObservation,
    blob: PreparedBlobCandidate,
}
enum Observation {
    Recorded,
    Denied(String),
    Ready(Box<ObservedPlan>),
}
pub(crate) struct ObservedStart {
    invocation: String,
    state: Observation,
}

fn fingerprint(inv: &Invocation<'_, StartParams>) -> Result<String> {
    canonical_json_digest(
        &json!({"operation":"delegation.start","run":inv.source.agent_run_id,
        "epoch":inv.source.execution_epoch,"thread":inv.source.camp_id,"agent":inv.source.agent_id,
        "binding":inv.binding.native_binding_id,"generation":inv.binding.native_binding_generation,
        "compatibility":inv.binding.binding_compatibility_digest,"credential":inv.binding.binding_credential,
        "call":inv.tool_call,"input":inv.input}),
    )
}
fn business_code(error: anyhow::Error) -> Result<String> {
    if error.chain().any(|cause| cause.is::<rusqlite::Error>()) {
        return Err(error);
    }
    let code = error.to_string();
    Ok(
        if code.starts_with("delegation.")
            && code
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_')
        {
            code
        } else {
            "delegation.start_unavailable".into()
        },
    )
}

pub(crate) fn prepare(
    db: &Database,
    store: &ManagedBlobStore,
    inv: &Invocation<'_, StartParams>,
) -> Result<PreparedStart> {
    let invocation = fingerprint(inv)?;
    if model::replay_start(db, inv)?.is_some() {
        return Ok(PreparedStart {
            invocation,
            state: Preparation::Recorded,
        });
    }
    crate::delegation_admission::authenticate_operation(
        db.connection(),
        inv.binding,
        inv.source,
        inv.tool_call,
        "delegation.status",
    )?;
    let planned = (|| -> Result<Plan> {
        let admission = crate::delegation_admission::authenticate_new_child(
            db.connection(),
            inv.binding,
            inv.source,
            inv.tool_call,
        )?;
        let (installation, kind): (String, AdapterKind) = if let Some(id) =
            inv.input.agent_type.as_str().strip_prefix("custom:")
        {
            let kind: String = db.connection().query_row(
                "SELECT adapter_kind FROM adapter_installation WHERE id=?1 AND installation_class='custom' AND enabled=1",
                [id], |r| r.get(0),
            ).optional()?.context("delegation.runtime_unavailable")?;
            (
                id.into(),
                kind.parse()
                    .map_err(|_| anyhow::anyhow!("delegation.runtime_unavailable"))?,
            )
        } else {
            let kind = crate::delegation_factory::builtin_adapter(&inv.input.agent_type)
                .context("delegation.runtime_unavailable")?;
            let id = db.connection().query_row(
                "SELECT id FROM adapter_installation WHERE adapter_kind=?1 AND auth_scope='default' AND installation_class='managed_default' AND enabled=1",
                [kind.as_str()], |r| r.get(0),
            ).optional()?.context("delegation.runtime_unavailable")?;
            (id, kind)
        };
        ensure!(
            crate::agent_runtime_adapter::AgentRuntimeAdapterRegistry::default()
                .current_platform_admission(kind)
                .is_some_and(|admission| admission.allows_runtime_use()),
            "delegation.platform_unavailable"
        );
        let mut configuration = crate::agent_profile::member_runtime_defaults(kind);
        if kind==AdapterKind::CustomAcp{configuration.custom_runtime_ref=Some(crate::custom_runtime::storage::installation_reference(db.connection(),&installation)?);}
        let defaults = admission
            .settings()
            .per_runtime_defaults
            .get(&inv.input.agent_type)
            .cloned()
            .unwrap_or_default();
        crate::delegation_runtime_defaults::configuration(&configuration, &defaults)?;
        let preset = serde_json::to_value(defaults)?;
        let reservation = store.reserve_candidate(
            db,
            inv.input.task.as_str().as_bytes(),
            "text/plain",
            "sensitive",
            GC_OWNER_EXECUTION_LIFECYCLE,
        )?;
        Ok(Plan {
            admission,
            input: inv.input.clone(),
            installation,
            configuration,
            preset,
            reservation,
        })
    })();
    let state = match planned {
        Ok(plan) => Preparation::Ready(Box::new(plan)),
        Err(error) => Preparation::Denied(business_code(error)?),
    };
    Ok(PreparedStart { invocation, state })
}

impl PreparedStart {
    /// No DB handle is accepted. The caller moves this stage to a worker and
    /// reauthenticates the exact transport lease before calling commit.
    pub(crate) fn observe(self) -> Result<ObservedStart> {
        let state = match self.state {
            Preparation::Recorded => Observation::Recorded,
            Preparation::Denied(code) => Observation::Denied(code),
            Preparation::Ready(plan) => {
                let Plan {
                    admission,
                    input,
                    installation,
                    configuration,
                    preset,
                    reservation,
                } = *plan;
                match admission.observe_workspace(input.working_dir.as_ref()) {
                    Ok(workspace) => {
                        let blob = reservation.write(input.task.as_str().as_bytes())?;
                        Observation::Ready(Box::new(ObservedPlan {
                            admission,
                            input,
                            installation,
                            configuration,
                            preset,
                            workspace,
                            blob,
                        }))
                    }
                    Err(error) => Observation::Denied(business_code(error)?),
                }
            }
        };
        Ok(ObservedStart {
            invocation: self.invocation,
            state,
        })
    }
}
impl ObservedStart {
    pub(crate) fn commit(
        &self,
        db: &mut Database,
        inv: &Invocation<'_, StartParams>,
    ) -> Result<CommandExecution> {
        self.commit_observed(db, inv, None)
    }
    pub(crate) fn commit_observed(
        &self,
        db: &mut Database,
        inv: &Invocation<'_, StartParams>,
        observation: Option<&str>,
    ) -> Result<CommandExecution> {
        ensure!(
            self.invocation == fingerprint(inv)?,
            "delegation.preparation_scope_mismatch"
        );
        if let Some(recorded) = model::replay_start(db, inv)? {
            return Ok(recorded);
        }
        match &self.state {
            Observation::Recorded => anyhow::bail!("delegation.receipt_missing"),
            Observation::Denied(code) => model::reject_start(db, inv, code, observation),
            Observation::Ready(plan) => {
                let workspace = serde_json::to_value(plan.workspace.candidate())?;
                let request = crate::delegation::FrozenRequest {
                    task_blob_id: plan.blob.id(),
                    runtime: &plan.input.agent_type,
                    working_dir: plan.input.working_dir.as_ref(),
                    workspace: &workspace,
                    runtime_preset: &plan.preset,
                };
                let child = crate::delegation_factory::ChildRunPlan {
                    task: plan.input.task.as_str(),
                    target: &plan.input.agent_type,
                    installation_id: &plan.installation,
                    configuration: &plan.configuration,
                    workspace: plan.workspace.candidate(),
                    policy: plan.admission.child_policy(),
                };
                model::start_with_candidate(
                    db,
                    inv,
                    model::PreparedStart {
                        admission: &plan.admission,
                        observation: &plan.workspace,
                        request: &request,
                        plan: &child,
                    },
                    &plan.blob,
                    observation,
                )
            }
        }
    }
    /// The caller has released the DB mutex. Cleanup failure must not replace
    /// the already committed result; its durable intent remains recoverable.
    pub(crate) fn finish_after(&self, execution: &CommandExecution) -> Result<()> {
        if execution.result.status == CommandResultStatus::Accepted {
            if let Observation::Ready(plan) = &self.state {
                plan.blob.finish()?;
            }
        }
        Ok(())
    }
}
