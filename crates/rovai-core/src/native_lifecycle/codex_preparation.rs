//! Codex 原生继续只复用已存在的 app-server 握手和 thread/resume；准备阶段没有模型轮次。
use super::*;
use crate::{
    acp::{NativePreparationFailure, NativePreparationProof},
    native_lifecycle as native,
};

async fn host(cwd: &Path, runtime: &FrozenAgentRuntimeConfig) -> Result<Arc<CodexHost>> {
    anyhow::ensure!(
        runtime.adapter_kind == AdapterKind::CodexCli,
        "native_lifecycle.codex_program_required"
    );
    let source = rovai_core::member_environment::launch_source();
    let plan = crate::portable_configuration::launch::prepare(runtime, source.as_ref()).await?;
    let environment = if let Some(plan) = &plan {
        Some(plan.environment.clone())
    } else {
        rovai_core::member_environment::resolve_for_launch(source, runtime.environment.clone())
            .await?
    };
    let api = if environment
        .as_ref()
        .is_some_and(|value| value.has_overlay())
    {
        None
    } else {
        runtime.custom_api.clone()
    };
    let (incoming, mut receiver) = mpsc::unbounded_channel();
    let host = crate::portable_configuration::launch::with_plan(
        plan,
        CodexHost::spawn_uninitialized(
            Path::new(&runtime.executable_path),
            cwd,
            incoming,
            None,
            api,
            environment,
        ),
    )
    .await?;
    tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    Ok(host)
}
async fn qualify(
    host: &CodexHost,
    runtime: &FrozenAgentRuntimeConfig,
    origin: &native::NativeSessionOriginV1,
    location: &native::source::Location,
) -> Result<String> {
    anyhow::ensure!(
        origin.source.provider == "codex"
            && origin.mode == native::NativeContinuationMode::Continue
            && origin.point.is_none(),
        "native_lifecycle.codex_continue_required"
    );
    anyhow::ensure!(
        host.native_source
            .as_ref()
            .context("native_lifecycle.program_source_unproven")?
            .contains_selected_root(&location.root)?,
        "native_lifecycle.program_source_mismatch"
    );
    host.initialize().await?;
    let initialize = host
        .initialized
        .get()
        .context("native_lifecycle.initialize_missing")?;
    // 只保存握手摘要；安装身份来自已经冻结的队员配置，不把历史或凭据投影给模型。
    native::digest(
        &json!({"initialize":initialize,"installationId":runtime.installation_id,"installationGeneration":runtime.installation_generation,"executableFingerprint":runtime.executable_fingerprint,"protocol":"codex-app-server-thread-resume"}),
    )
}
pub(crate) async fn inspect_native_preparation(
    cwd: &Path,
    runtime: &FrozenAgentRuntimeConfig,
    origin: &native::NativeSessionOriginV1,
    location: &native::source::Location,
) -> Result<String> {
    anyhow::ensure!(
        origin.source.provider == "codex"
            && origin.mode == native::NativeContinuationMode::Continue,
        "native_lifecycle.codex_continue_required"
    );
    let host = host(cwd, runtime).await?;
    let result = timeout(
        Duration::from_secs(30),
        qualify(&host, runtime, origin, location),
    )
    .await
    .map_err(|_| anyhow::anyhow!("native_lifecycle.initialize_timeout"))
    .and_then(|value| value);
    host.shutdown_and_reap().await;
    result
}
pub(crate) async fn prepare_native_session(
    cwd: &Path,
    runtime: &FrozenAgentRuntimeConfig,
    origin: &native::NativeSessionOriginV1,
    location: &native::source::Location,
    expected_initialize: &str,
) -> std::result::Result<NativePreparationProof, NativePreparationFailure> {
    let host = host(cwd, runtime)
        .await
        .map_err(|_| NativePreparationFailure {
            outcome_unknown: false,
            code: "native_lifecycle.program_start_failed",
        })?;
    let mut external_requested = false;
    let result = async {
        let initialize_digest = timeout(
            Duration::from_secs(30),
            qualify(&host, runtime, origin, location),
        )
        .await
        .map_err(|_| anyhow::anyhow!("native_lifecycle.initialize_timeout"))??;
        anyhow::ensure!(
            initialize_digest == expected_initialize,
            "native_lifecycle.program_capability_changed"
        );
        anyhow::ensure!(
            native::source::revision(location)? == origin.source.source_revision,
            "native_lifecycle.source_changed"
        );
        let (method, params) = thread_start_or_resume_request(
            cwd,
            Some(&origin.source.native_session_id),
            CodexThreadStartOptions {
                developer_instructions: None,
                sandbox: "read-only",
                approval_policy: "never",
                model: None,
                config: None,
                runtime_workspace_roots: None,
                ephemeral: false,
            },
        )?;
        let native_request_digest = native::digest(&json!({"method":method,"params":params}))?;
        external_requested = true;
        let reply = timeout(Duration::from_secs(90), host.rpc(method, params))
            .await
            .map_err(|_| anyhow::anyhow!("native_lifecycle.native_reply_timeout"))??;
        let reported = reply
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .context("native_lifecycle.reported_session_missing")?;
        anyhow::ensure!(
            reported == origin.source.native_session_id,
            "native_lifecycle.load_session_changed"
        );
        Ok::<_, anyhow::Error>(NativePreparationProof {
            reported_session_id: reported.into(),
            host_instance_id: host.host_instance_id.clone(),
            initialize_digest,
            native_request_digest,
            native_receipt_digest: native::digest(&reply)?,
        })
    }
    .await;
    host.shutdown_and_reap().await;
    result.map_err(|_| NativePreparationFailure {
        outcome_unknown: external_requested,
        code: if external_requested {
            "native_lifecycle.creation_result_unknown"
        } else {
            "native_lifecycle.preparation_changed"
        },
    })
}
