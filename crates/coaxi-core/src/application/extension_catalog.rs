//! Versioned descriptions of the existing extension seams. Descriptors never
//! grant execution, load arbitrary code, create another DB or replace Core.
use super::*;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Runtime,
    Channel,
    Viewer,
    Skill,
    Theme,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Query {
    #[serde(default = "version")]
    contract_version: u32,
    kind: Option<Kind>,
    id: Option<String>,
}
fn version() -> u32 {
    1
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Descriptor {
    id: String,
    kind: Kind,
    contract_version: u32,
    version: String,
    display_name: String,
    data_owner: &'static str,
    hosts: Vec<&'static str>,
    permissions: Vec<&'static str>,
    capabilities: Vec<String>,
    configuration_method: &'static str,
    source: Value,
}
fn source(coaxi: bool, path: &str) -> Value {
    if coaxi {
        json!({"repository":"https://github.com/spacering-net/codeg","revision":"aff1e6c278453ed8d0b96a5f664edcdca621aa3f","license":"Apache-2.0","path":path})
    } else {
        json!({"repository":"https://github.com/murray17/rovai-ai","revision":"cc465dda562536b94f23787759e3206324baa29e","license":"MIT","path":path})
    }
}
impl Core {
    pub(super) async fn extension_catalog(&self, value: Value) -> Result<Value> {
        let query: Query = serde_json::from_value(value)?;
        ensure!(
            query.contract_version == 1,
            "extension.contract_version_unsupported"
        );
        let mut entries = Vec::<Descriptor>::new();
        for runtime in crate::agent_profile::AdapterKind::ALL {
            entries.push(Descriptor {
                id: format!("runtime.{}", runtime.as_str()),
                kind: Kind::Runtime,
                contract_version: 1,
                version: env!("CARGO_PKG_VERSION").into(),
                display_name: runtime.as_str().into(),
                data_owner: "coaxi-core",
                hosts: vec!["desktop", "server"],
                permissions: vec![
                    "owned_process",
                    "workspace",
                    "network",
                    "user_selected_credentials",
                ],
                capabilities: vec![
                    "runtime_platform_admission".into(),
                    "frozen_member_configuration".into(),
                    "owned_stop_and_recovery".into(),
                ],
                configuration_method: "runtime.startup.get",
                source: source(false, "crates/coaxi-core/src/agent_runtime_adapter.rs"),
            });
        }
        for (id, label, coaxi) in [
            ("feishu", "飞书", false),
            ("lark", "Lark", false),
            ("dingtalk", "钉钉", false),
            ("telegram", "Telegram", true),
            ("weixin", "微信", true),
        ] {
            entries.push(Descriptor {
                id: format!("channel.{id}"),
                kind: Kind::Channel,
                contract_version: 1,
                version: env!("CARGO_PKG_VERSION").into(),
                display_name: label.into(),
                data_owner: "coaxi-core",
                hosts: vec!["desktop", "server"],
                permissions: vec!["network", "explicit_sender_allowlist", "bound_core_thread"],
                capabilities: vec![
                    "external_principal_identity".into(),
                    "durable_inbound".into(),
                    "core_execution".into(),
                    "delivery_reconciliation".into(),
                ],
                configuration_method: if coaxi {
                    "channels.adapters"
                } else {
                    "host.channels"
                },
                source: source(
                    coaxi,
                    if coaxi {
                        "src-tauri/src/chat_channel"
                    } else {
                        "apps/desktop/src/main/channel-settings.ts"
                    },
                ),
            });
        }
        for (id, label, method, permissions) in [
            (
                "text",
                "文本与代码",
                "workbench.files.read",
                vec!["authorized_workspace_read"],
            ),
            (
                "image",
                "图片",
                "attachments.filePreview",
                vec!["authorized_file_read"],
            ),
            (
                "html",
                "HTML 与交互卡片",
                "attachments.filePreview",
                vec!["isolated_renderer", "authorized_file_read"],
            ),
            (
                "office",
                "Office 实时预览",
                "office.watch",
                vec!["explicit_file_lease", "owned_process", "local_loopback"],
            ),
            (
                "diff",
                "文件差异",
                "workbench.git.read",
                vec!["authorized_workspace_read"],
            ),
        ] {
            entries.push(Descriptor {
                id: format!("viewer.{id}"),
                kind: Kind::Viewer,
                contract_version: 1,
                version: env!("CARGO_PKG_VERSION").into(),
                display_name: label.into(),
                data_owner: "coaxi-core",
                hosts: vec!["desktop", "web", "android", "ios"],
                permissions,
                capabilities: vec![
                    "owner_scoped_artifact".into(),
                    "host_authorized_bytes".into(),
                ],
                configuration_method: method,
                source: source(true, "src/components"),
            });
        }
        let skills = crate::coaxi_skill_catalog::list()?;
        for skill in skills.entries {
            entries.push(Descriptor{id:format!("skill.{}",skill.id),kind:Kind::Skill,contract_version:1,version:skills.commit.into(),display_name:skill.display_name.get("zh-CN").or_else(||skill.display_name.get("en")).cloned().unwrap_or(skill.id),data_owner:"rovai-core",hosts:vec!["desktop","server"],permissions:vec!["explicit_member_selection","frozen_session_delivery"],capabilities:vec!["immutable_pack_revision".into(),"preserved_history".into()],configuration_method:"skillPacks.list",source:json!({"repository":skills.repository,"revision":skills.commit,"license":"Apache-2.0","path":skill.source_path})});
        }
        for (id, label, coaxi) in [
            ("coaxi", "Coaxi 外观", false),
            ("coaxi", "Coaxi 主题与自定义样式", true),
        ] {
            entries.push(Descriptor {
                id: format!("theme.{id}"),
                kind: Kind::Theme,
                contract_version: 1,
                version: env!("CARGO_PKG_VERSION").into(),
                display_name: label.into(),
                data_owner: "coaxi-core",
                hosts: vec!["desktop", "web"],
                permissions: vec!["presentation_only", "sanitized_css"],
                capabilities: vec!["light_dark_palettes".into(), "import_export".into()],
                configuration_method: "appearance.extensions",
                source: source(
                    coaxi,
                    if coaxi {
                        "src/lib/themes.ts"
                    } else {
                        "apps/desktop/src/shared/appearance.ts"
                    },
                ),
            });
        }
        let items = entries
            .into_iter()
            .filter(|entry| {
                query.id.as_ref().is_none_or(|id| id == &entry.id)
                    && query.kind.is_none_or(|kind| {
                        std::mem::discriminant(&kind) == std::mem::discriminant(&entry.kind)
                    })
            })
            .collect::<Vec<_>>();
        Ok(
            json!({"contractVersion":1,"coreProtocolVersion":4,"productVersion":env!("CARGO_PKG_VERSION"),"descriptors":items,"registrationIsExecutionAdmission":false}),
        )
    }
}
