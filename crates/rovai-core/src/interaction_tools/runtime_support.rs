//! 工具接线源码能力和实测准入分开；候选不改变 Runtime 原有启动资格。
use anyhow::{Result,ensure};
use serde_json::{Value,json};
use crate::{agent_profile::AdapterKind,agent_runtime_adapter::{AgentRuntimeAdapterRegistry,ExternalMcpProjection},mcp::McpServerDefinition};

pub fn snapshot()->Value {
    let registry=AgentRuntimeAdapterRegistry::default();
    let runtimes=AdapterKind::ALL.into_iter().map(|kind|{
        let capability=registry.mcp_projection(kind);
        let (state,message)=match kind {
            AdapterKind::Pi=>("not_delivered_by_donor","固定原版 Pi 未将这组 MCP 交给模型；此配置不会使 Pi 获得浏览器或电脑工具。"),
            AdapterKind::CursorAgent=>("candidate","Cursor 独立会话接线仍为候选，尚未证明非空 MCP 投递与会话隔离，当前不会投递这组工具。"),
            _ if capability.external_mcp_projection==ExternalMcpProjection::Unsupported||!capability.supports_stdio=>("unsupported","当前运行器没有独立的标准输入输出 MCP 通道，不能投递这组工具。"),
            _=>("source_adapted","工具接线源码已接入；实际模型调用、图片和停止恢复尚未完成统一验证。"),
        };
        json!({"adapterKind":kind.as_str(),"state":state,"message":message,"qualified":false,"supportsStdio":capability.supports_stdio})
    }).collect::<Vec<_>>();
    json!({"schemaVersion":1,"runtimes":runtimes,"defaultFeaturesEnabled":false,"mobileExecutesPhoneOs":false,"headlessWithoutDesktop":"desktop_unavailable"})
}

/// 供隔离准入任务准备候选报文；不启动 Cursor、不写项目或用户的 .cursor 配置。
pub fn cursor_session_candidate(cwd:&str,definition:&McpServerDefinition,native_session_id:Option<&str>)->Result<Value>{
    ensure!(std::path::Path::new(cwd).is_absolute(),"interaction.absolute_workspace_required");
    let McpServerDefinition::Stdio{command,args,cwd:server_cwd,env}=definition else{anyhow::bail!("runtime_unavailable")};
    ensure!(std::path::Path::new(command).is_absolute()&&args.first().map(String::as_str)==Some("--interaction-mcp")&&args.get(1).map(String::as_str)==Some("--config")&&args.len()==3&&std::path::Path::new(&args[2]).is_absolute()&&env.is_empty(),"interaction.invalid_candidate_definition");
    if let Some(server_cwd)=server_cwd{ensure!(server_cwd==cwd,"scope_changed")}
    let server=json!({"name":super::SERVER_NAME,"command":command,"args":args,"env":[]});
    let new_params=json!({"cwd":cwd,"mcpServers":[server.clone()]});
    let load_params=if let Some(id)=native_session_id{ensure!(!id.is_empty()&&id.len()<=4096&&!id.contains('\0'),"interaction.invalid_native_session");json!({"sessionId":id,"cwd":cwd,"mcpServers":[server]})}else{Value::Null};
    Ok(json!({"schemaVersion":1,"adapterKind":AdapterKind::CursorAgent.as_str(),"state":"candidate","activated":false,"sessionNew":new_params,"sessionLoad":load_params,"sharedConfigurationWritten":false,"automaticApproval":false}))
}
