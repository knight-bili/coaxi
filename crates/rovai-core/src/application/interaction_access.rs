//! 仅原生 Desktop 的控制方法；不加入 Web/Server 公共操作目录。
use super::*;
use anyhow::ensure;
use crate::interaction_tools::{self as tools, broker::{Broker, digest}, stdio::IpcRequest};
use rusqlite::OptionalExtension;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request { action: String, payload: Value }
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> { value[key].as_str().context("interaction.invalid_identity") }

fn require_scope(database: &Database, scope: &Value, agent: &str) -> Result<()> {
    let thread = string(scope, "threadId")?;
    let kind = string(scope, "scopeKind")?;
    let conversation = scope["conversationId"].as_str();
    ensure!(matches!((kind, conversation), ("public", None) | ("single_chat" | "delegation_child", Some(_))), "scope_changed");
    let admitted: bool = database.connection().query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation c JOIN camp ON camp.id=c.camp_id JOIN camp_member m ON m.camp_id=camp.id AND m.agent_id=c.agent_id WHERE camp.id=?1 AND c.agent_id=?2 AND c.ended_at IS NULL AND camp.activation_state='active' AND m.status='active' AND m.leave_requested_at IS NULL AND ((?3='public' AND c.kind='camp_member') OR (c.kind=?3 AND c.id=?4)))",
        rusqlite::params![thread, agent, kind, conversation], |row| row.get(0))?;
    ensure!(admitted, "scope_changed");
    Ok(())
}
fn require_binding(database: &Database, broker: &Broker, binding: &Value) -> Result<()> {
    tools::validate_shape("binding", binding)?;
    ensure!(binding["coreInstanceId"] == broker.core_instance(), "binding_changed");
    require_scope(database, &binding["scope"], string(binding, "agentId")?)?;
    let expected_conversation = binding["scope"]["conversationId"].as_str();
    let valid: bool = database.connection().query_row(r#"
        SELECT EXISTS(SELECT 1 FROM agent_run r JOIN conversation c ON c.id=r.conversation_id
        JOIN camp ON camp.id=c.camp_id JOIN camp_member m ON m.camp_id=camp.id AND m.agent_id=c.agent_id
        LEFT JOIN camp_turn t ON t.id=r.camp_turn_id
        WHERE r.id=?1 AND r.execution_epoch=?2 AND r.status='running' AND r.cancel_requested_at IS NULL
        AND c.camp_id=?3 AND c.agent_id=?4 AND c.native_binding_id=?5 AND c.native_binding_generation=?6
        AND camp.membership_generation=?7 AND c.ended_at IS NULL AND camp.activation_state='active'
        AND m.status='active' AND m.leave_requested_at IS NULL
        AND m.version=CAST(json_extract(r.effective_config_json,'$.campMemberVersion') AS INTEGER)
        AND ((?8='public' AND c.kind='camp_member' AND ?9 IS NULL) OR (c.kind=?8 AND c.id=?9))
        AND (?10 IS NULL OR c.native_session_id=?10)
        AND (r.camp_turn_id IS NULL OR (t.status IN ('running','waiting') AND t.cancel_requested_at IS NULL
        AND t.execution_budget_exhausted_at IS NULL AND (t.execution_budget_deadline_at>?11 OR (t.execution_budget_deadline_at IS NULL AND t.execution_budget_schema_version=2)))))
    "#, rusqlite::params![string(binding,"agentRunId")?,binding["executionEpoch"].as_i64(),string(&binding["scope"],"threadId")?,string(binding,"agentId")?,string(binding,"nativeBindingId")?,binding["nativeBindingGeneration"].as_i64(),binding["membershipGeneration"].as_i64(),string(&binding["scope"],"scopeKind")?,expected_conversation,binding["nativeSessionId"].as_str(),crate::execution_budget::camp_turn_execution_budget_now().to_rfc3339()], |row| row.get(0))?;
    ensure!(valid, "run_not_active");
    let policy = broker.policies.read()?;
    ensure!(binding["permissionRevision"] == policy.revision && binding["policyDigest"] == digest(&serde_json::to_value(&policy)?)?, "permission_changed");
    Ok(())
}
impl Core {
    pub(super) async fn interaction_policy_for_run(&self, execution: &AgentRunExecution) -> Result<Option<tools::Policy>> {
        let policy = self.interaction_tools.policies.read()?;
        if !policy.features(&execution.agent_id).any() { return Ok(None); }
        let capability = AgentRuntimeAdapterRegistry::default().mcp_projection(execution.runtime.adapter_kind);
        if capability.external_mcp_projection == crate::agent_runtime_adapter::ExternalMcpProjection::Unsupported || !capability.supports_stdio { return Ok(None); }
        let database = self.database.lock().await;
        // 已冻结且原来没配备工具的 Run 不在途中获得新目录。
        let exposure: Option<String> = database.connection().query_row("SELECT mcp_exposure_json FROM context_manifest WHERE agent_run_id=?1", [&execution.agent_run_id], |row| row.get(0)).optional()?;
        if let Some(exposure) = exposure {
            let value: Value = serde_json::from_str(&exposure)?;
            if !value["servers"].as_array().is_some_and(|servers| servers.iter().any(|entry| entry["serverId"] == tools::SERVER_NAME)) { return Ok(None); }
        }
        // 已确认的帮手续接合同要求保留原生绑定；没有隔离续接证据前只保留原能力。
        if crate::delegation_resume::validate_session(database.connection(), &execution.agent_run_id)?.is_some() { return Ok(None); }
        Ok(Some(policy))
    }
    pub(super) async fn prepare_interaction_transport(
        self: &Arc<Self>, execution: &AgentRunExecution, policy: &tools::Policy,
        credential: &BuiltinToolBindingCredential,
    ) -> Result<(crate::mcp::McpServerDefinition, String)> {
        use crate::builtin_tool_transport::LocalIpcEndpoint;
        use crate::platform::private_storage::{prepare_private_directory, atomic_write_private_json};
        let features = policy.features(&execution.agent_id);
        let bridge_id = uuid::Uuid::new_v4().to_string();
        let catalog_digest = digest(&json!({"schemaVersion":1,"tools":features.catalog()}))?;
        let policy_digest = digest(&serde_json::to_value(policy)?)?;
        let catalog_policy_digest = digest(&json!({"catalogDigest":catalog_digest,"policyDigest":policy_digest}))?;
        let database = self.database.lock().await;
        let (kind, membership): (String,i64) = database.connection().query_row("SELECT c.kind,camp.membership_generation FROM conversation c JOIN camp ON camp.id=c.camp_id WHERE c.id=?1 AND c.agent_id=?2",rusqlite::params![execution.conversation_id,execution.agent_id],|row|Ok((row.get(0)?,row.get(1)?)))?;
        let scope_kind = match kind.as_str() { "camp_member" => "public", "single_chat" => "single_chat", "delegation_child" => "delegation_child", _ => anyhow::bail!("scope_changed") };
        let peers = self.interaction_tools.peers()?;
        // 多个桌面同时在线时要求先由本机窗口确定唯一 peer，不能随机选机器。
        let usable: Vec<_> = peers.iter().filter(|peer| peer["capabilities"].as_array().is_some_and(|capabilities| (!features.browser || capabilities.iter().any(|capability| capability == "browser")) && (!features.computer || capabilities.iter().any(|capability| capability == "computer")))).collect();
        ensure!(usable.len() == 1, "desktop_unavailable");
        let binding = json!({"schemaVersion":1,"bridgeInstanceId":bridge_id,"coreInstanceId":self.interaction_tools.core_instance(),"desktopPeerId":usable[0]["desktopPeerId"],"scope":{"threadId":execution.camp_id,"conversationId":if scope_kind=="public"{None}else{Some(execution.conversation_id.as_str())},"scopeKind":scope_kind},"agentId":execution.agent_id,"agentRunId":execution.agent_run_id,"executionEpoch":execution.execution_epoch,"nativeBindingId":credential.native_binding_id,"nativeBindingGeneration":credential.native_binding_generation,"nativeSessionId":credential.native_session_id,"membershipGeneration":membership,"permissionRevision":policy.revision,"catalogDigest":catalog_digest,"policyDigest":policy_digest,"expiresAt":(Utc::now()+chrono::Duration::minutes(120)).to_rfc3339(),"revocationEpoch":0});
        require_binding(&database,&self.interaction_tools,&binding)?;
        let secret = self.interaction_tools.register(binding.clone(),features)?;
        drop(database);
        let directory = self.data_dir.join("interaction-tools/bridges").join(&bridge_id);
        prepare_private_directory(&directory)?;
        #[cfg(windows)]
        let endpoint = LocalIpcEndpoint::WindowsNamedPipe { name:format!(r"\\.\pipe\rovai-ai-{}-{}",std::process::id(),uuid::Uuid::new_v4()) };
        #[cfg(unix)]
        let endpoint = LocalIpcEndpoint::UnixSocket { path: std::env::temp_dir().join(format!("rvi-{}",uuid::Uuid::new_v4().simple())).join("i").to_string_lossy().into_owned() };
        #[cfg(not(any(unix,windows)))]
        anyhow::bail!("runtime_unavailable");
        let mut listener = LocalIpcListener::bind(&endpoint)?;
        let config = crate::interaction_tools::stdio::ProcessConfig { schema_version:1,core_endpoint:endpoint.clone(),bridge_instance_id:bridge_id.clone(),secret:secret.clone() };
        let config_path = directory.join("process.json");
        atomic_write_private_json(&config_path,&config)?;
        atomic_write_private_json(&directory.join("binding.json"),&binding)?;
        let definition = crate::mcp::McpServerDefinition::Stdio { command:std::env::current_exe()?.to_string_lossy().into_owned(),args:vec!["--interaction-mcp".into(),"--config".into(),config_path.to_string_lossy().into_owned()],cwd:Some(execution.workspace.execution_root.clone()),env:Default::default() };
        let evidence = json!({"schemaVersion":1,"bindingDigest":digest(&binding)?,"catalogDigest":catalog_digest,"configurationDigest":digest(&serde_json::to_value(&definition)?)?,"adapterKind":execution.runtime.adapter_kind.as_str(),"reportedRuntimeVersion":execution.runtime.reported_version,"nativeSessionId":credential.native_session_id,"deliveryKind":"mcp_stdio","state":"candidate","runtimeToolNames":[],"transportEvidenceDigest":null,"realToolCallEvidenceDigest":null,"modelImageInputEvidenceDigest":null,"recordedAt":Utc::now().to_rfc3339()});
        tools::validate_shape("deliveryEvidence",&evidence)?;
        atomic_write_private_json(&directory.join("delivery-evidence.json"),&evidence)?;
        let core = Arc::downgrade(self);
        tokio::spawn(async move {
            let capacity = Arc::new(tokio::sync::Semaphore::new(32));
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                while tasks.try_join_next().is_some() {}
                let Some(core) = core.upgrade() else { break; };
                if core.interaction_tools.authenticated_binding(&bridge_id,&secret).is_err() { break; }
                {
                    let database = core.database.lock().await;
                    if require_binding(&database,&core.interaction_tools,&binding).is_err() { let _=core.interaction_tools.disconnect(&bridge_id); break; }
                }
                let accepted = tokio::time::timeout(std::time::Duration::from_secs(1),listener.accept()).await;
                let stream = match accepted { Ok(Ok(stream))=>stream, Err(_)=>continue, Ok(Err(_))=>break };
                let Ok(permit) = capacity.clone().try_acquire_owned() else { drop(stream); continue; };
                tasks.spawn(async move { let _permit=permit; let _=serve_connection(core,stream).await; });
            }
            tasks.abort_all();
            drop(listener);
            #[cfg(unix)]
            if let LocalIpcEndpoint::UnixSocket{path} = endpoint { if let Some(parent)=Path::new(&path).parent(){let _=std::fs::remove_dir(parent);} }
        });
        Ok((definition,catalog_policy_digest))
    }
    pub(super) async fn interaction_access(&self, value: Value) -> Result<Value> {
        let input: Request = serde_json::from_value(value)?;
        let broker = &self.interaction_tools;
        let payload = &input.payload;
        match input.action.as_str() {
            "sharing.targets" => {
                #[derive(Deserialize)] #[serde(rename_all="camelCase",deny_unknown_fields)] struct Targets {thread_id:String,after:Option<String>}
                let input:Targets=serde_json::from_value(payload.clone())?;
                let database=self.database.lock().await;
                let mut query=database.connection().prepare("SELECT c.id,c.kind,c.agent_id,p.display_name,c.created_at FROM conversation c JOIN camp ON camp.id=c.camp_id JOIN camp_member m ON m.camp_id=camp.id AND m.agent_id=c.agent_id JOIN agent_profile p ON p.id=c.agent_id WHERE camp.id=?1 AND camp.activation_state='active' AND c.ended_at IS NULL AND m.status='active' AND m.leave_requested_at IS NULL AND (?2 IS NULL OR c.id>?2) ORDER BY c.id LIMIT 201")?;
                let mut rows=query.query_map(rusqlite::params![input.thread_id,input.after],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
                let more=rows.len()>200;rows.truncate(200);
                let after=if more{rows.last().map(|row|row.0.clone())}else{None};
                let targets=rows.into_iter().map(|(conversation,kind,agent,name,created)|{let public=kind=="camp_member";json!({"id":conversation,"agentId":agent,"displayName":name,"createdAt":created,"scope":{"threadId":input.thread_id,"conversationId":if public{None}else{Some(conversation.as_str())},"scopeKind":if public{"public"}else{kind.as_str()}})}).collect::<Vec<_>>();
                Ok(json!({"targets":targets,"next":after}))
            }
            "policy.get" => Ok(serde_json::to_value(broker.policies.read()?)?),
            "policy.replace" => {
                #[derive(Deserialize)] #[serde(rename_all="camelCase",deny_unknown_fields)] struct Replace { expected_revision:u64, value:tools::Policy }
                let input: Replace = serde_json::from_value(payload.clone())?;
                let database = self.database.lock().await;
                for member in &input.value.members {
                    let exists: bool = database.connection().query_row("SELECT EXISTS(SELECT 1 FROM agent_profile WHERE id=?1)", [&member.agent_id], |row| row.get(0))?;
                    ensure!(exists, "interaction.member_missing");
                }
                Ok(serde_json::to_value(broker.policies.replace(input.expected_revision, input.value)?)?)
            }
            "peer.attach" => {
                #[derive(Deserialize)] #[serde(rename_all="camelCase",deny_unknown_fields)] struct Attach { desktop_peer_id:Option<String>, owner_window_id:String, connection_generation:u64, capabilities:Vec<String> }
                let input: Attach = serde_json::from_value(payload.clone())?;
                let peer = json!({"schemaVersion":1,"desktopPeerId":input.desktop_peer_id.unwrap_or_else(||uuid::Uuid::new_v4().to_string()),"coreInstanceId":broker.core_instance(),"ownerWindowId":input.owner_window_id,"connectionGeneration":input.connection_generation,"capabilities":input.capabilities,"expiresAt":(Utc::now()+chrono::Duration::seconds(90)).to_rfc3339()});
                broker.attach_peer(peer)
            }
            "peer.list" => Ok(json!({"peers":broker.peers()?})),
            "peer.detach" => { broker.revoke_peer(string(payload,"desktopPeerId")?)?; Ok(json!({"revoked":true})) }
            "grant.share" => {
                tools::validate_shape("resourceGrant", payload)?;
                let database = self.database.lock().await;
                require_scope(&database, &payload["scope"], string(payload,"agentId")?)?;
                let features = broker.policies.read()?.features(string(payload,"agentId")?);
                ensure!(if payload["targetKind"] == "browser_tab" { features.browser } else { features.computer }, "interaction_disabled");
                broker.share(payload.clone())
            }
            "grant.revoke" => { broker.revoke_grant(string(payload,"grantId")?)?; Ok(json!({"revoked":true})) }
            "grant.list" => {
                let database = self.database.lock().await;
                require_scope(&database, &payload["scope"], string(payload,"agentId")?)?;
                Ok(json!({"grants":broker.grants(payload)?}))
            }
            "desktop.poll" => { let database = self.database.lock().await; broker.poll(payload, |binding| require_binding(&database, broker, binding)) }
            "desktop.targets" => broker.register_targets(payload.clone()),
            "desktop.check" => { let database = self.database.lock().await; broker.check_desktop_call(payload, |binding| require_binding(&database, broker, binding)) }
            "desktop.finish" => { let database = self.database.lock().await; broker.finish(payload, |binding| require_binding(&database, broker, binding)) }
            "pending" => Ok(json!({"items":broker.pending()?})),
            "eval.approve"|"operation.approve" => { broker.approve(string(payload,"bridgeCallId")?,string(payload,"argumentsDigest")?)?; Ok(json!({"approved":true})) }
            "computer.stop"=>broker.stop_computer(),
            "unknown.reconcile" => broker.reconcile_unknown(string(payload,"bridgeCallId")?,string(payload,"argumentsDigest")?),
            "run.revoke" => { broker.revoke_run(string(payload,"agentRunId")?)?; Ok(json!({"revoked":true})) }
            _ => anyhow::bail!("interaction.operation_not_admitted"),
        }
    }
    pub(super) async fn interaction_ipc(&self, request: IpcRequest) -> Result<Value> {
        ensure!(request.schema_version == 1, "transport_failed");
        let broker = &self.interaction_tools;
        let binding = broker.authenticated_binding(&request.bridge_instance_id,&request.secret)?;
        let database = self.database.lock().await;
        require_binding(&database, broker, &binding)?;
        match request.operation.as_str() {
            "catalog" => Ok(json!(broker.catalog(&request.bridge_instance_id,&request.secret)?)),
            "call" => {
                ensure!(request.params["bridgeInstanceId"] == request.bridge_instance_id, "binding_changed");
                broker.enqueue(request.params,&request.secret)
            }
            "result" => broker.result(&request.bridge_instance_id,&request.secret,string(&request.params,"bridgeCallId")?, |binding| require_binding(&database,broker,binding)),
            "cancel" => broker.cancel_call(&request.bridge_instance_id,string(&request.params,"bridgeCallId")?),
            "disconnect" => { broker.disconnect(&request.bridge_instance_id)?; Ok(json!({"revoked":true})) }
            _ => anyhow::bail!("transport_failed"),
        }
    }
}

pub(super) async fn serve_connection(core: Arc<Core>, stream: LocalIpcStream) -> Result<()> {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader.take(tools::MAX_REQUEST_BYTES as u64 + 1));
    let mut frame = Vec::new();
    tokio::time::timeout(std::time::Duration::from_secs(10),reader.read_until(b'\n',&mut frame)).await??;
    ensure!(frame.len() <= tools::MAX_REQUEST_BYTES && frame.last() == Some(&b'\n'), "invalid_arguments");
    let request: IpcRequest = serde_json::from_slice(&frame)?;
    let response = match core.interaction_ipc(request).await {
        Ok(result) => json!({"schemaVersion":1,"ok":true,"result":result}),
        Err(error) => {
            let message = error.to_string();
            let code = if matches!(message.as_str(),"interaction_disabled"|"runtime_unavailable"|"desktop_unavailable"|"scope_changed"|"run_not_active"|"binding_changed"|"membership_changed"|"permission_changed"|"grant_required"|"origin_changed"|"target_changed"|"generation_changed"|"approval_required"|"approval_rejected"|"cancelled"|"result_too_large"|"invalid_arguments"|"unknown_outcome"|"original_request_pending"|"transport_failed") {message.as_str()} else {"transport_failed"};
            json!({"schemaVersion":1,"ok":false,"code":code})
        }
    };
    let mut bytes = serde_json::to_vec(&response)?;
    ensure!(bytes.len() <= tools::MAX_RESULT_BYTES, "result_too_large");bytes.push(b'\n');
    writer.write_all(&bytes).await?;writer.shutdown().await?;
    Ok(())
}
