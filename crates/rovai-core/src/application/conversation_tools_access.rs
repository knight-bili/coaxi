use super::*;
use crate::conversation_tools::{self as conversation_tools,broker::Binding,stdio::{IpcRequest,ProcessConfig}};
use tokio::io::{AsyncBufReadExt,AsyncReadExt,AsyncWriteExt,BufReader};

impl Core{
    pub(super) async fn conversation_tools_for_run(&self,execution:&AgentRunExecution)->Result<Option<conversation_tools::FrozenConversationToolsV1>>{
        let Some(frozen)=execution.runtime.conversation_tools.as_ref().filter(|frozen|frozen.flags.question||frozen.flags.session_info||frozen.flags.automations||frozen.flags.work_tasks)else{return Ok(None)};
        frozen.validate()?;let database=self.database.lock().await;
        let exposure:Option<String>=database.connection().query_row("SELECT mcp_exposure_json FROM context_manifest WHERE agent_run_id=?1",[&execution.agent_run_id],|row|row.get(0)).optional()?;
        if let Some(exposure)=exposure{let value:Value=serde_json::from_str(&exposure)?;if !value["servers"].as_array().is_some_and(|rows|rows.iter().any(|row|row["serverId"]==conversation_tools::SERVER_NAME)){return Ok(None)}}
        let capability=AgentRuntimeAdapterRegistry::default().mcp_projection_for_runtime(&execution.runtime);
        ensure!(capability.supports_stdio&&capability.external_mcp_projection!=crate::agent_runtime_adapter::ExternalMcpProjection::Unsupported,"conversation_tools.runtime_unavailable");
        Ok(Some(frozen.clone()))
    }
    pub(super) async fn prepare_conversation_tools_transport(self:&Arc<Self>,execution:&AgentRunExecution,frozen:&conversation_tools::FrozenConversationToolsV1,credential:&BuiltinToolBindingCredential)->Result<(crate::mcp::McpServerDefinition,String)>{
        let key=format!("{}:{}",execution.agent_run_id,execution.execution_epoch);let mut cache=self.conversation_tools_transports.lock().await;
        if let Some(value)=cache.get(&key){return Ok(value.clone())}ensure!(cache.len()<4096,"conversation_tools.capacity_reached");
        let bridge=uuid::Uuid::new_v4().to_string();let frozen_digest=conversation_tools::digest(&serde_json::to_value(frozen)?)?;
        let binding=Binding{schema_version:1,core_instance_id:self.interaction_tools.core_instance().into(),bridge_instance_id:bridge.clone(),agent_run_id:execution.agent_run_id.clone(),execution_epoch:execution.execution_epoch,native_binding_id:credential.native_binding_id.clone(),native_binding_generation:credential.native_binding_generation,native_session_id:credential.native_session_id.clone(),frozen_digest:frozen_digest.clone(),catalog_digest:frozen.catalog_digest.clone(),expires_at:(Utc::now()+chrono::Duration::seconds(7200)).to_rfc3339()};
        let token={let mut database=self.database.lock().await;self.conversation_tools.register(&mut database,self.interaction_tools.core_instance(),binding.clone())?};
        #[cfg(windows)]let endpoint=format!(r"\\.\pipe\rovai-ai-{}-{}",std::process::id(),uuid::Uuid::new_v4());
        #[cfg(unix)]let endpoint=std::env::temp_dir().join(format!("rvc-{}",uuid::Uuid::new_v4().simple())).join("i").to_string_lossy().into_owned();
        #[cfg(not(any(windows,unix)))]anyhow::bail!("conversation_tools.runtime_unavailable");
        let mut listener=LocalIpcListener::bind(&conversation_tools::stdio::endpoint(&endpoint)?)?;
        let directory=self.data_dir.join("conversation-tools/bridges").join(&bridge);crate::platform::private_storage::prepare_private_directory(&directory)?;
        let config=ProcessConfig{schema_version:1,binding:binding.clone(),binding_digest:conversation_tools::digest(&serde_json::to_value(&binding)?)?,core_endpoint:endpoint.clone(),authorization_token:token,max_frame_bytes:conversation_tools::MAX_FRAME};
        let path=directory.join("process.json");crate::platform::private_storage::atomic_write_private_json(&path,&config)?;
        let definition=crate::mcp::McpServerDefinition::Stdio{command:std::env::current_exe()?.to_string_lossy().into_owned(),args:vec!["--conversation-tools-mcp".into(),"--config".into(),path.to_string_lossy().into_owned()],cwd:Some(execution.workspace.execution_root.clone()),env:Default::default()};
        crate::platform::private_storage::atomic_write_private_json(&directory.join("delivery-evidence.json"),&json!({"schemaVersion":1,"bindingDigest":config.binding_digest,"catalogDigest":frozen.catalog_digest,"configurationDigest":conversation_tools::digest(&serde_json::to_value(&definition)?)?,"adapterKind":execution.runtime.adapter_kind.as_str(),"state":"source_prepared_unverified","realToolCallEvidenceDigest":null,"recordedAt":Utc::now().to_rfc3339()}))?;
        let weak=Arc::downgrade(self);
        tokio::spawn(async move{
            let mut tasks=tokio::task::JoinSet::new();let capacity=Arc::new(tokio::sync::Semaphore::new(16));let mut revoked=None;
            loop{
                while tasks.try_join_next().is_some(){}
                let Some(core)=weak.upgrade()else{break};
                let valid={let database=core.database.lock().await;core.conversation_tools.valid(database.connection(),core.interaction_tools.core_instance(),&binding)};
                if !valid&&revoked.is_none(){let mut database=core.database.lock().await;let _=core.conversation_tools.revoke(database.connection(),&binding);let _=conversation_tools::user_api::maintain(&mut database,false);revoked=Some(tokio::time::Instant::now());emit(&core.output,"conversationTools.changed",json!({"agentRunId":binding.agent_run_id}));}
                if revoked.is_some_and(|time:tokio::time::Instant|time.elapsed()>Duration::from_secs(30)){let _=core.conversation_tools.remove(&bridge);break}
                let stream=match tokio::time::timeout(Duration::from_secs(1),listener.accept()).await{Ok(Ok(stream))=>stream,Err(_)=>continue,Ok(Err(_))=>break};
                let Ok(permit)=capacity.clone().try_acquire_owned()else{drop(stream);continue};
                tasks.spawn(async move{let _permit=permit;let _=serve_conversation_connection(core,stream).await;});
            }
            tasks.abort_all();drop(listener);
            #[cfg(unix)]if let Some(parent)=Path::new(&endpoint).parent(){let _=std::fs::remove_dir(parent);}
        });
        let result=(definition,frozen_digest);cache.insert(key,result.clone());Ok(result)
    }
    pub(super) async fn conversation_tools_access(&self,method:&str,value:Value,client:&crate::draft_client::DraftClient)->Result<Value>{
        let mut database=self.database.lock().await;
        let result=rovai_core::conversation_tools::user_api::call(&mut database,&self.data_dir,method,value,client)?;
        drop(database);
        if matches!(method,"conversationTools.policy.set"|"conversationTools.questions.answer"|"conversationTools.questions.decline"|"conversationTools.grants.confirm"|"conversationTools.grants.revoke"|"conversationTools.requests.resolve"|"conversationTools.automationOwner.set"){
            emit(&self.output,"conversationTools.changed",json!({"operation":method}));
        }
        Ok(result)
    }
}
async fn serve_conversation_connection(core:Arc<Core>,stream:crate::platform::local_ipc::LocalIpcServerStream)->Result<()>{
    let(reader,mut writer)=tokio::io::split(stream);let mut reader=BufReader::new(reader.take(conversation_tools::MAX_FRAME as u64+1));let mut frame=Vec::new();
    tokio::time::timeout(Duration::from_secs(30),reader.read_until(b'\n',&mut frame)).await??;
    ensure!(frame.last()==Some(&b'\n')&&frame.len()<=conversation_tools::MAX_FRAME,"conversation_tools.invalid_input");let request:IpcRequest=serde_json::from_slice(&frame)?;
    let changed=matches!(request.operation.as_str(),"call"|"wrote"|"cancel"|"disconnect");
    let result={let mut database=core.database.lock().await;core.conversation_tools.request(&mut database,&core.data_dir,core.interaction_tools.core_instance(),request)};
    let response=match result{Ok(result)=>json!({"schemaVersion":1,"ok":true,"result":result}),Err(error)=>json!({"schemaVersion":1,"ok":false,"code":conversation_tools::broker::error_code(&error)})};
    if changed{emit(&core.output,"conversationTools.changed",json!({}));}
    let mut bytes=serde_json::to_vec(&response)?;ensure!(bytes.len()<conversation_tools::MAX_FRAME,"conversation_tools.result_too_large");bytes.push(b'\n');writer.write_all(&bytes).await?;writer.shutdown().await?;Ok(())
}
