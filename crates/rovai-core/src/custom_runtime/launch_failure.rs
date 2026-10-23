//! 恢复失败沿既有公开错误展示，保留原安装与历史任务的精确关系。
use crate::agent_profile::FrozenAgentRuntimeConfig;
use crate::runtime_failure::{RuntimeFailureError,RuntimeFailureOrigin,RuntimeFailurePhase,RuntimeFailureView};

pub(super) fn public(runtime:&FrozenAgentRuntimeConfig,error:anyhow::Error)->anyhow::Error{
    let known=|code:&str|error.chain().any(|cause|cause.to_string()==code);
    let(code,summary,detail)=if known("custom_runtime.artifacts_changed")||known("custom_runtime.entrypoint_changed"){
        ("custom_runtime_retained_artifacts_unavailable","这次任务的原安装在当前主机不可用或已经变化。","保留原任务和安装目录。在自定义智能体中查看状态，修复原环境后再继续；重新安装的版本供之后的新任务明确选择。")
    }else if known("custom_runtime.frozen_definition_changed")||known("custom_runtime.installation_scope_changed")||known("custom_runtime.launch_lock_changed"){
        ("custom_runtime_retained_installation_changed","这次任务的原程序版本与保存的安装记录不一致。","请恢复包含原程序版本和安装记录的工作区；当前设置不会替换旧任务的冻结配置。")
    }else if known("custom_runtime.secret_scope_mismatch")||known("Private environment decryption failed"){
        ("custom_runtime_retained_secret_unavailable","这次任务保存的程序环境在当前主机无法读取。","请恢复原工作区的加密存储。需要新凭据时，保存新版本并为之后的任务重新选择，原任务记录继续保留。")
    }else{
        ("custom_runtime_environment_unavailable","当前主机未能准备这次任务的自定义程序。","请在自定义智能体中核对本机安装、原版本和所需运行环境；本次未改用其他程序。")
    };
    error.context(RuntimeFailureError::new(RuntimeFailureView::new(runtime.adapter_kind,RuntimeFailureOrigin::Environment,RuntimeFailurePhase::Spawn,code,summary,Some(detail.into()),false)))
}
