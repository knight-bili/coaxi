//! 连接恢复失败使用既有公开 RuntimeFailure，原始秘密与主机目录不投影到界面。
use crate::agent_profile::FrozenAgentRuntimeConfig;
use crate::runtime_failure::{
    RuntimeFailureError, RuntimeFailureOrigin, RuntimeFailurePhase, RuntimeFailureView,
};

pub(super) fn public(runtime: &FrozenAgentRuntimeConfig, error: anyhow::Error) -> anyhow::Error {
    let known = |code: &str| error.chain().any(|cause| cause.to_string() == code);
    let (code, summary, detail) = if known("configuration.connection_requires_original_platform") {
        (
            "configuration_original_platform_required",
            "这次任务的模型连接属于原主机平台。",
            "请在原平台恢复这次任务；在当前主机为队员选择本机程序和连接后，可另建新任务。",
        )
    } else if known("configuration.frozen_secret_unavailable")
        || known("configuration.frozen_secret_invalid")
        || known("configuration.provider_credential_required")
        || known("Private environment decryption failed")
    {
        (
            "configuration_retained_secret_unavailable",
            "这次任务保存的模型凭据在当前主机不可用。",
            "请恢复原工作区及其加密存储；也可以在模型服务商中保存新的凭据版本，再为队员的新任务选择连接。",
        )
    } else if known("configuration.provider_version_unavailable") {
        (
            "configuration_retained_provider_unavailable",
            "这次任务保存的服务商版本不在当前工作区。",
            "请恢复包含该版本的原工作区。为之后的新任务选择当前服务商，不会替换这次任务的历史连接。",
        )
    } else if known("configuration.native_scope_changed")
        || known("configuration.native_managed_files_changed")
        || known("configuration.native_catalog_changed")
        || known("configuration.native_managed_file_invalid")
    {
        (
            "configuration_native_scope_changed",
            "这次任务的独立模型配置已经变化。",
            "请保留当前目录，恢复原配置副本后再继续。若需改用新配置，请修改队员连接并另建任务。",
        )
    } else if known("configuration.live_catalog_unavailable")
        || known("configuration.live_catalog_invalid")
        || known("configuration.catalog_projection_incomplete")
    {
        (
            "configuration_runtime_catalog_unavailable",
            "当前程序无法提供这份连接所需的模型目录。",
            "请在智能体设置中核对当前主机的程序版本，再检查服务商的模型配置；本次没有改用其他模型。",
        )
    } else if known("configuration.frozen_connection_changed")
        || known("configuration.frozen_provider_changed")
    {
        (
            "configuration_retained_connection_changed",
            "这次任务的连接与保存的运行信息不一致。",
            "请恢复原工作区与程序记录；需要换连接时，为队员保存新设置后另建任务。",
        )
    } else {
        (
            "configuration_host_preparation_unavailable",
            "当前主机未能准备这次任务的模型连接。",
            "请核对本机程序、配置目录和原加密存储；原任务记录已保留，可以修复环境后再继续。",
        )
    };
    error.context(RuntimeFailureError::new(RuntimeFailureView::new(
        runtime.adapter_kind,
        RuntimeFailureOrigin::Environment,
        RuntimeFailurePhase::Spawn,
        code,
        summary,
        Some(detail.into()),
        false,
    )))
}
