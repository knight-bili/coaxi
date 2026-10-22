//! 已认证用户的反馈操作与每个 Run 独立的私有 MCP 连接。
use super::*;
use anyhow::ensure;
use crate::ui_feedback::{self as feedback,Binding,Scope,stdio::{IpcRequest,ProcessConfig}};
use tokio::io::{AsyncBufReadExt,AsyncReadExt,AsyncWriteExt,BufReader};

#[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Request{action:String,payload:Value}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Target{scope:Scope}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Listing{scope:Scope,agent_id:String,after:Option<String>}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct CardTarget{scope:Scope,agent_id:String,message_id:String}
impl Core{
    pub(super) async fn feedback_policy_for_run(&self,execution:&AgentRunExecution)->Result<Option<feedback::Policy>>{
        let database=self.database.lock().await;let policy=feedback::storage::policy(database.connection())?;if !policy.enabled(&execution.agent_id){return Ok(None)}
        let capability=AgentRuntimeAdapterRegistry::default().mcp_projection(execution.runtime.adapter_kind);
        if capability.external_mcp_projection==crate::agent_runtime_adapter::ExternalMcpProjection::Unsupported||!capability.supports_stdio{return Ok(None)}
        let exposure:Option<String>=database.connection().query_row("SELECT mcp_exposure_json FROM context_manifest WHERE agent_run_id=?1",[&execution.agent_run_id],|row|row.get(0)).optional()?;
        if let Some(exposure)=exposure{let value:Value=serde_json::from_str(&exposure)?;if !value["servers"].as_array().is_some_and(|rows|rows.iter().any(|row|row["serverId"]==feedback::SERVER_NAME)){return Ok(None)}}
        if crate::delegation_resume::validate_session(database.connection(),&execution.agent_run_id)?.is_some(){return Ok(None)}
        Ok(Some(policy))
    }
    pub(super) async fn prepare_feedback_transport(self:&Arc<Self>,execution:&AgentRunExecution,policy:&feedback::Policy,credential:&BuiltinToolBindingCredential)->Result<(crate::mcp::McpServerDefinition,String)>{
        let key=format!("{}:{}",execution.agent_run_id,execution.execution_epoch);let mut cache=self.feedback_transports.lock().await;
        if let Some(value)=cache.get(&key){return Ok(value.clone())}ensure!(cache.len()<4096,"feedback.binding_capacity");
        let bridge=uuid::Uuid::new_v4().to_string();let catalog_digest=feedback::digest(&feedback::catalog())?;
        let database=self.database.lock().await;
        let(kind,membership):(String,i64)=database.connection().query_row("SELECT c.kind,camp.membership_generation FROM conversation c JOIN camp ON camp.id=c.camp_id WHERE c.id=?1 AND c.agent_id=?2",rusqlite::params![execution.conversation_id,execution.agent_id],|row|Ok((row.get(0)?,row.get(1)?)))?;
        let scope_kind=match kind.as_str(){"camp_member"=>"public","single_chat"=>"single_chat","delegation_child"=>"delegation_child",_=>anyhow::bail!("scope_changed")};
        let binding=Binding{schema_version:1,bridge_instance_id:bridge.clone(),core_instance_id:self.interaction_tools.core_instance().to_string(),scope:Scope{thread_id:execution.camp_id.clone(),conversation_id:if scope_kind=="public"{None}else{Some(execution.conversation_id.clone())},scope_kind:scope_kind.into()},agent_id:execution.agent_id.clone(),agent_run_id:execution.agent_run_id.clone(),execution_epoch:execution.execution_epoch as u64,native_binding_id:credential.native_binding_id.clone(),native_binding_generation:credential.native_binding_generation as u64,native_session_id:credential.native_session_id.clone(),membership_generation:membership as u64,policy_revision:policy.revision,catalog_digest:catalog_digest.clone(),expires_at:(Utc::now()+chrono::Duration::minutes(120)).to_rfc3339()};
        feedback::storage::require_binding(database.connection(),&binding,self.interaction_tools.core_instance())?;let token=self.feedback_tools.register(binding.clone())?;drop(database);
        #[cfg(windows)]let endpoint=format!(r"\\.\pipe\rovai-feedback-{}-{}",std::process::id(),uuid::Uuid::new_v4());
        #[cfg(unix)]let endpoint=std::env::temp_dir().join(format!("rvf-{}",uuid::Uuid::new_v4().simple())).join("i").to_string_lossy().into_owned();
        #[cfg(not(any(windows,unix)))]anyhow::bail!("runtime_unavailable");
        let mut listener=LocalIpcListener::bind(&feedback::stdio::endpoint(&endpoint)?)?;
        let directory=self.data_dir.join("ui-feedback/bridges").join(&bridge);crate::platform::private_storage::prepare_private_directory(&directory)?;
        let binding_digest=feedback::digest(&serde_json::to_value(&binding)?)?;let config=ProcessConfig{schema_version:1,binding:binding.clone(),binding_digest:binding_digest.clone(),core_endpoint:endpoint.clone(),authorization_token:token,max_frame_bytes:feedback::MAX_FRAME_BYTES};
        let path=directory.join("process.json");crate::platform::private_storage::atomic_write_private_json(&path,&config)?;
        let definition=crate::mcp::McpServerDefinition::Stdio{command:std::env::current_exe()?.to_string_lossy().into_owned(),args:vec!["--ui-feedback-mcp".into(),"--config".into(),path.to_string_lossy().into_owned()],cwd:Some(execution.workspace.execution_root.clone()),env:Default::default()};
        let frozen_digest=feedback::digest(&json!({"catalogDigest":catalog_digest,"policy":policy}))?;
        crate::platform::private_storage::atomic_write_private_json(&directory.join("delivery-evidence.json"),&json!({"schemaVersion":1,"bindingDigest":binding_digest,"catalogDigest":catalog_digest,"configurationDigest":feedback::digest(&serde_json::to_value(&definition)?)?,"adapterKind":execution.runtime.adapter_kind.as_str(),"state":"candidate","realToolCallEvidenceDigest":null,"recordedAt":Utc::now().to_rfc3339()}))?;
        let weak=Arc::downgrade(self);let binding_for_listener=binding.clone();
        tokio::spawn(async move{
            let binding=binding_for_listener;let capacity=Arc::new(tokio::sync::Semaphore::new(16));let mut tasks=tokio::task::JoinSet::new();let mut revoked_at=None;
            loop{
                while tasks.try_join_next().is_some(){}
                let Some(core)=weak.upgrade()else{break};
                let valid={let database=core.database.lock().await;feedback::storage::require_binding(database.connection(),&binding,core.interaction_tools.core_instance()).is_ok()};
                if !valid&&revoked_at.is_none(){let _=core.feedback_tools.revoke(&bridge);let database=core.database.lock().await;let _=feedback::delivery::revoke(database.connection(),&binding);revoked_at=Some(tokio::time::Instant::now());emit(&core.output,"feedback.changed",json!({"threadId":binding.scope.thread_id,"agentRunId":binding.agent_run_id}));}
                if revoked_at.is_some_and(|time:tokio::time::Instant|time.elapsed()>Duration::from_secs(30)){let _=core.feedback_tools.remove(&bridge);break}
                let stream=match tokio::time::timeout(Duration::from_secs(1),listener.accept()).await{Ok(Ok(stream))=>stream,Err(_)=>continue,Ok(Err(_))=>break};let Ok(permit)=capacity.clone().try_acquire_owned()else{drop(stream);continue};
                tasks.spawn(async move{let _permit=permit;let _=serve_connection(core,stream).await;});
            }
            tasks.abort_all();drop(listener);
            #[cfg(unix)]if let Some(parent)=Path::new(&endpoint).parent(){let _=std::fs::remove_dir(parent);}
        });
        let result=(definition,frozen_digest);cache.insert(key,result.clone());Ok(result)
    }
    pub(super) async fn feedback_operation(&self,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value)?;
        match request.action.as_str(){
            "policy.get"=>{ensure!(request.payload==json!({}),"feedback.invalid_arguments");let database=self.database.lock().await;Ok(serde_json::to_value(feedback::storage::policy(database.connection())?)?)},
            "policy.set"=>{feedback::validate_shape("memberPolicyUpdate",&request.payload)?;#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Update{schema_version:u32,expected_revision:u64,members:Vec<feedback::MemberPolicy>}let input:Update=serde_json::from_value(request.payload)?;ensure!(input.schema_version==1,"feedback.invalid_version");let mut database=self.database.lock().await;let result=feedback::storage::set_policy(database.connection_mut(),input.expected_revision,input.members)?;emit(&self.output,"feedback.changed",json!({"policyRevision":result.revision}));Ok(serde_json::to_value(result)?)},
            "targets"=>{
                let input:Target=serde_json::from_value(request.payload)?;input.scope.validate()?;let database=self.database.lock().await;let policy=feedback::storage::policy(database.connection())?;
                let mut query=database.connection().prepare("SELECT r.id,r.execution_epoch,c.agent_id,p.display_name,json_extract(r.effective_config_json,'$.runtime.adapterKind'),c.native_binding_id FROM agent_run r JOIN conversation c ON c.id=r.conversation_id JOIN camp ON camp.id=c.camp_id JOIN camp_member m ON m.camp_id=camp.id AND m.agent_id=c.agent_id JOIN agent_profile p ON p.id=c.agent_id WHERE camp.id=?1 AND camp.activation_state='active' AND camp.deletion_operation_id IS NULL AND c.ended_at IS NULL AND m.status='active' AND m.leave_requested_at IS NULL AND r.status='running' AND r.cancel_requested_at IS NULL AND ((?2='public' AND c.kind='camp_member' AND ?3 IS NULL)OR(c.kind=?2 AND c.id=?3)) ORDER BY r.created_at DESC,r.id LIMIT 201")?;
                let mut rows=query.query_map(rusqlite::params![input.scope.thread_id,input.scope.scope_kind,input.scope.conversation_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,i64>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,Option<String>>(4)?,row.get::<_,Option<String>>(5)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
                let truncated=rows.len()>200;rows.truncate(200);Ok(json!({"targets":rows.into_iter().map(|(run,epoch,agent,name,runtime,binding)|json!({"agentRunId":run,"executionEpoch":epoch,"agentId":agent,"displayName":name,"runtimeKind":runtime,"feedbackEnabled":policy.enabled(&agent),"nativeBindingReady":binding.is_some(),"scope":input.scope})).collect::<Vec<_>>(),"truncated":truncated}))
            },
            "cards.locate"=>{let input:CardTarget=serde_json::from_value(request.payload)?;let database=self.database.lock().await;let body=feedback::cards::source_body(database.connection(),&input.scope,&input.agent_id,&input.message_id)?;let sources=feedback::cards::fences(&body)?;let origins=sources.iter().enumerate().map(|(ordinal,source)|Ok(json!({"messageId":input.message_id,"conversationId":input.scope.conversation_id,"messageKind":input.scope.scope_kind,"cardOrdinal":ordinal,"sourceDigests":[feedback::digest(&Value::String(source.clone()))?,feedback::digest(&Value::String(source.strip_suffix('\n').unwrap_or(source).into()))?]}))).collect::<Result<Vec<_>>>()?;Ok(json!({"origins":origins}))},
            "preview"=>{let input:feedback::PreviewRequest=serde_json::from_value(request.payload.clone())?;feedback::validate_shape("previewRequest",&request.payload)?;let mut database=self.database.lock().await;Ok(serde_json::to_value(feedback::storage::preview(database.connection_mut(),input)?)?)},
            "submit"=>{let input:feedback::SubmitRequest=serde_json::from_value(request.payload.clone())?;feedback::validate_shape("submitRequest",&request.payload)?;let mut database=self.database.lock().await;let receipt=feedback::storage::submit(database.connection_mut(),input)?;emit(&self.output,"feedback.changed",json!({"threadId":receipt.scope.thread_id,"feedbackId":receipt.feedback_id}));Ok(serde_json::to_value(receipt)?)},
            "list"=>{let input:Listing=serde_json::from_value(request.payload)?;let database=self.database.lock().await;feedback::storage::list(database.connection(),&input.scope,&input.agent_id,input.after.as_deref())},
            _=>anyhow::bail!("feedback.invalid_operation")
        }
    }
    async fn feedback_ipc(&self,input:IpcRequest)->Result<Value>{
        ensure!(input.schema_version==1,"binding_changed");let binding=self.feedback_tools.authenticate(&input.bridge_instance_id,&input.authorization_token,&input.binding_digest,input.operation=="wrote")?;
        let mut database=self.database.lock().await;
        match input.operation.as_str(){
            "catalog"=>{ensure!(input.payload==json!({}),"binding_changed");feedback::storage::require_binding(database.connection(),&binding,self.interaction_tools.core_instance())?;Ok(json!([feedback::catalog()]))},
            "prepare"=>{feedback::validate_shape("privatePrepareRequest",&input.payload)?;ensure!(input.payload["bindingDigest"]==input.binding_digest,"binding_changed");feedback::delivery::prepare(database.connection_mut(),&binding,self.interaction_tools.core_instance(),input.payload["transportRpcId"].clone(),input.payload["argumentsDigest"].as_str().context("binding_changed")?)},
            "wrote"=>{feedback::validate_shape("privateWriteReceipt",&input.payload)?;let result=feedback::delivery::wrote(database.connection_mut(),&binding,input.payload["attemptId"].as_str().context("binding_changed")?,input.payload["bindingDigest"].as_str().context("binding_changed")?,input.payload["resultDigest"].as_str().context("binding_changed")?,input.payload["writeStatus"].as_str().context("binding_changed")?)?;emit(&self.output,"feedback.changed",json!({"threadId":binding.scope.thread_id,"agentRunId":binding.agent_run_id}));Ok(result)},
            "disconnect"=>{ensure!(input.payload==json!({}),"binding_changed");self.feedback_tools.revoke(&binding.bridge_instance_id)?;feedback::delivery::revoke(database.connection(),&binding)?;Ok(json!({}))},
            _=>anyhow::bail!("binding_changed")
        }
    }
}
async fn serve_connection(core:Arc<Core>,stream:crate::platform::local_ipc::LocalIpcServerStream)->Result<()>{
    let(reader,mut writer)=tokio::io::split(stream);let mut reader=BufReader::new(reader.take(feedback::MAX_FRAME_BYTES as u64+1));let mut frame=Vec::new();tokio::time::timeout(Duration::from_secs(30),reader.read_until(b'\n',&mut frame)).await??;
    ensure!(frame.last()==Some(&b'\n')&&frame.len()<=feedback::MAX_FRAME_BYTES,"transport_failed");let request:IpcRequest=serde_json::from_slice(&frame)?;
    let response=match core.feedback_ipc(request).await{Ok(result)=>json!({"schemaVersion":1,"ok":true,"result":result}),Err(error)=>{let message=error.to_string();let code=match message.as_str(){"feature_disabled"|"runtime_unavailable"|"scope_changed"|"run_not_active"|"epoch_changed"|"binding_changed"|"member_changed"|"policy_changed"|"result_too_large"|"original_request_pending"|"feedback_unknown"=>message.as_str(),_=>"transport_failed"};json!({"schemaVersion":1,"ok":false,"code":code})}};
    let mut bytes=serde_json::to_vec(&response)?;ensure!(bytes.len()<=feedback::MAX_FRAME_BYTES*2,"result_too_large");bytes.push(b'\n');writer.write_all(&bytes).await?;writer.shutdown().await?;Ok(())
}
