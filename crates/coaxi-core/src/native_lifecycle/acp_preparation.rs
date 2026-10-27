//! ACP 私有子模块：创建准备没有 AgentRun、模型 prompt 或 Built-in/MCP 授权。
use super::*;
use crate::native_lifecycle::{self as native, acp_policy};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NativePreparationProof {
    pub reported_session_id: String,
    pub host_instance_id: String,
    pub initialize_digest: String,
    pub native_request_digest: String,
    pub native_receipt_digest: String,
}
pub(crate) struct NativePreparationFailure {
    pub outcome_unknown: bool,
    pub code: &'static str,
}

async fn host(
    cwd: &Path,
    runtime: &FrozenAgentRuntimeConfig,
    origin: &native::NativeSessionOriginV1,
    private: &Path,
) -> Result<Arc<AcpHost>> {
    anyhow::ensure!(
        runtime.adapter_kind == AdapterKind::CustomAcp,
        "native_lifecycle.explicit_acp_program_required"
    );
    let source = coaxi_core::member_environment::launch_source();
    let plan = crate::custom_runtime::launch::prepare(runtime, source.as_ref()).await?;
    let mut runtime = runtime.clone();
    runtime.environment =
        coaxi_core::member_environment::resolve_for_launch(source, runtime.environment.clone())
            .await?;
    crate::custom_runtime::launch::merge_environment(plan.as_ref(), &mut runtime.environment)?;
    if runtime
        .environment
        .as_ref()
        .is_some_and(|value| value.has_overlay())
    {
        runtime.custom_api = None;
    }
    // 来源种类只决定如何核对最终子进程目录，随后仍必须核对真实 initialize 的程序身份。
    let preparation_kind = match origin.source.provider.as_str() {
        "claude" => Some(AdapterKind::ClaudeCodeCli),
        "codex" => Some(AdapterKind::CodexCli),
        "deepseek" => Some(AdapterKind::DeepseekHarness),
        _ => None,
    };
    let workspace = AgentRunWorkspace::runtime_managed_path(cwd.to_string_lossy().into_owned());
    let (incoming, mut receiver) = mpsc::unbounded_channel();
    let host = crate::custom_runtime::launch::with_plan(
        plan,
        coaxi_core::runtime_discovery::with_launch_environment(
            runtime.environment.clone(),
            AcpHost::spawn_in_environment(
                cwd,
                &workspace,
                PermissionSemantics::RuntimeManagedV2,
                &runtime,
                incoming,
                None,
                CompactionDetectorPolicy::Disabled,
                false,
                &BTreeMap::new(),
                private,
                None,
                preparation_kind,
            ),
        ),
    )
    .await?;
    // 未绑定 Session 的请求由已有 Host 路由拒绝；这里只排空诊断，不将历史重放成新模型输出。
    tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    Ok(host)
}
async fn qualify(
    host: &AcpHost,
    origin: &native::NativeSessionOriginV1,
    location: &native::source::Location,
) -> Result<(Value, String)> {
    let initialize = host
        .initialize_result
        .read()
        .await
        .clone()
        .context("native_lifecycle.initialize_missing")?;
    anyhow::ensure!(
        acp_policy::provider_matches(&origin.source.provider, &initialize),
        "native_lifecycle.provider_program_mismatch"
    );
    let captured = host
        .native_preparation_source
        .as_ref()
        .or(host.native_source.as_ref())
        .context("native_lifecycle.program_source_unproven")?;
    anyhow::ensure!(
        captured.contains_selected_root(&location.root)?,
        "native_lifecycle.program_source_mismatch"
    );
    match origin.mode {
        native::NativeContinuationMode::Continue => anyhow::ensure!(
            initialize
                .pointer("/agentCapabilities/loadSession")
                .and_then(Value::as_bool)
                == Some(true),
            "native_lifecycle.strict_load_unavailable"
        ),
        _ => {
            anyhow::ensure!(
                initialize
                    .pointer("/agentCapabilities/sessionCapabilities/fork")
                    .is_some_and(Value::is_object),
                "native_lifecycle.fork_unavailable"
            );
            if origin.point.is_some() {
                anyhow::ensure!(
                    acp_policy::exact_fork_available(&origin.source.provider, &initialize),
                    "native_lifecycle.exact_fork_unavailable"
                );
            }
        }
    }
    let digest = native::digest(&initialize)?;
    Ok((initialize, digest))
}

/// 用户预览时只握手核对实际广告，不发送 load、fork 或模型输入。
pub(crate) async fn inspect_native_preparation(
    cwd: &Path,
    runtime: &FrozenAgentRuntimeConfig,
    origin: &native::NativeSessionOriginV1,
    location: &native::source::Location,
    private: &Path,
) -> Result<String> {
    let host = host(cwd, runtime, origin, private).await?;
    let result = qualify(&host, origin, location)
        .await
        .map(|(_, digest)| digest);
    host.shutdown_and_reap().await;
    result
}

pub(crate) async fn prepare_native_session(
    cwd: &Path,
    runtime: &FrozenAgentRuntimeConfig,
    origin: &native::NativeSessionOriginV1,
    location: &native::source::Location,
    expected_initialize: &str,
    private: &Path,
) -> std::result::Result<NativePreparationProof, NativePreparationFailure> {
    let host = host(cwd, runtime, origin, private)
        .await
        .map_err(|_| NativePreparationFailure {
            outcome_unknown: false,
            code: "native_lifecycle.program_start_failed",
        })?;
    let mut external_requested = false;
    let result = async {
        let (_, initialize_digest) = qualify(&host, origin, location).await?;
        anyhow::ensure!(
            initialize_digest == expected_initialize,
            "native_lifecycle.program_capability_changed"
        );
        anyhow::ensure!(
            native::source::revision(location)? == origin.source.source_revision,
            "native_lifecycle.source_changed"
        );
        let (method, params) = match origin.mode {
            native::NativeContinuationMode::Continue => (
                "session/load",
                build_acp_resume_session_params(
                    AdapterKind::CustomAcp,
                    &origin.source.native_session_id,
                    &acp_protocol_path(cwd),
                    &[],
                    &[],
                ),
            ),
            _ => (
                "session/fork",
                native::fork::request(
                    &origin.source.native_session_id,
                    &acp_protocol_path(cwd),
                    origin.point.as_ref(),
                )?,
            ),
        };
        let native_request_digest = native::digest(&json!({"method":method,"params":params}))?;
        // rpc 写入失败亦可能部分写出；从此点起失败只允许查询原请求，不能重建。
        external_requested = true;
        let reply = host
            .rpc_with_timeout(method, params, ACP_HISTORY_RESTORE_TIMEOUT)
            .await?;
        let reported = reply["sessionId"]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 4096 && !id.contains('\0'))
            .context("native_lifecycle.reported_session_missing")?;
        if origin.mode == native::NativeContinuationMode::Continue {
            anyhow::ensure!(
                reported == origin.source.native_session_id,
                "native_lifecycle.load_session_changed"
            );
        } else {
            anyhow::ensure!(
                reported != origin.source.native_session_id,
                "native_lifecycle.fork_returned_original_session"
            );
        }
        anyhow::ensure!(
            !host.protocol_violated.load(Ordering::Acquire),
            "native_lifecycle.preparation_protocol_violated"
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
