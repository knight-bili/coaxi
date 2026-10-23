use super::*;
use crate::{
    agent_profile::{AdapterKind, FrozenAgentRuntimeConfig},
    member_environment::{LaunchEnvironment, LaunchEnvironmentSource},
};
use std::{collections::BTreeMap, path::PathBuf};
#[derive(Clone)]
pub(crate) struct Plan {
    arguments: Vec<String>,
    environment: BTreeMap<String, String>,
    lock_digest: String,
}
tokio::task_local! {static PLAN:Option<Plan>;}
pub async fn with_plan<F: std::future::Future>(plan: Option<Plan>, future: F) -> F::Output {
    PLAN.scope(plan, future).await
}
pub async fn prepare(
    runtime: &FrozenAgentRuntimeConfig,
    source: Option<&LaunchEnvironmentSource>,
) -> Result<Option<Plan>> {
    prepare_original(runtime, source).await.map_err(|error| {
        if runtime.adapter_kind == AdapterKind::CustomAcp {
            super::launch_failure::public(runtime, error)
        } else {
            error
        }
    })
}
async fn prepare_original(
    runtime: &FrozenAgentRuntimeConfig,
    source: Option<&LaunchEnvironmentSource>,
) -> Result<Option<Plan>> {
    if runtime.adapter_kind != AdapterKind::CustomAcp {
        ensure!(
            runtime.custom_runtime.is_none(),
            "custom_runtime.kind_mismatch"
        );
        return Ok(None);
    }
    let custom = runtime
        .custom_runtime
        .as_ref()
        .context("custom_runtime.frozen_definition_missing")?;
    ensure!(
        custom.schema_version == 1,
        "custom_runtime.frozen_version_invalid"
    );
    let source = source.context("custom_runtime.host_launch_source_missing")?;
    let (lock, environment, payload) = {
        let database = source.database.lock().await;
        let stored = storage::frozen(
            database.connection(),
            &custom.reference,
            &runtime.installation_id,
        )?;
        ensure!(
            stored == *custom,
            "custom_runtime.frozen_definition_changed"
        );
        let (id, lock) = storage::retained_lock(
            database.connection(),
            &custom.reference,
            &runtime.installation_id,
        )?;
        ensure!(
            text(&lock, "executable")? == runtime.executable_path,
            "custom_runtime.entrypoint_changed"
        );
        let environment = storage::launch_environment(
            database.connection(),
            &custom.resolved_definition,
            &installer::environment_path(&custom.resolved_definition, installer::platform()?)?,
        )?;
        let root = PathBuf::from(
            database
                .connection()
                .path()
                .context("custom_runtime.database_path_missing")?,
        )
        .parent()
        .context("custom_runtime.database_parent_missing")?
        .to_path_buf();
        let payload = root
            .join("custom-runtimes")
            .join(&custom.reference.id)
            .join(custom.reference.definition_revision.to_string())
            .join(installer::platform()?)
            .join(id)
            .join("payload");
        (lock, environment, payload)
    };
    let expected = text(&lock, "artifactDigest")?.to_owned();
    let executable = PathBuf::from(text(&lock, "executable")?);
    let observed =
        tokio::task::spawn_blocking(move || artifacts::tree_digest(&payload, &executable))
            .await??;
    ensure!(observed == expected, "custom_runtime.artifacts_changed");
    let arguments = lock["arguments"]
        .as_array()
        .context("custom_runtime.arguments_invalid")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .context("custom_runtime.arguments_invalid")
        })
        .collect::<Result<_>>()?;
    Ok(Some(Plan {
        arguments,
        environment,
        lock_digest: digest(&lock)?,
    }))
}
pub fn merge_environment(
    plan: Option<&Plan>,
    environment: &mut Option<LaunchEnvironment>,
) -> Result<()> {
    let Some(plan) = plan else { return Ok(()) };
    let host = environment
        .as_mut()
        .context("custom_runtime.launch_environment_missing")?;
    let values = host
        .values
        .as_mut()
        .context("custom_runtime.launch_environment_missing")?;
    let member = host.overlay.clone().unwrap_or_default();
    for (name, value) in plan.environment.iter().chain(member.iter()) {
        if cfg!(windows) {
            values.retain(|key, _| !key.eq_ignore_ascii_case(name))
        }
        values.insert(name.clone(), value.clone());
    }
    Ok(())
}
pub fn configure(
    command: &mut tokio::process::Command,
    runtime: &FrozenAgentRuntimeConfig,
) -> Result<()> {
    let custom = runtime
        .custom_runtime
        .as_ref()
        .context("custom_runtime.frozen_definition_missing")?;
    PLAN.try_with(|plan| {
        let plan = plan
            .as_ref()
            .context("custom_runtime.launch_plan_missing")?;
        ensure!(
            plan.lock_digest == custom.launch_lock_digest,
            "custom_runtime.launch_lock_changed"
        );
        command.args(&plan.arguments);
        Ok(())
    })
    .context("custom_runtime.host_launch_scope_missing")?
}
pub fn redactor() -> Option<crate::runtime_custom_api::CredentialRedactor> {
    PLAN.try_with(|plan| {
        plan.as_ref().map(|plan| {
            crate::runtime_custom_api::CredentialRedactor::from_values(
                plan.environment.values().cloned().collect(),
            )
        })
    })
    .ok()
    .flatten()
}
