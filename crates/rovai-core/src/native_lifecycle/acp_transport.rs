//! ACP 模块的私有子模块：写出时核对真实 Host/Run/轮次，回执仍由 Core 记账。
use super::*;
use crate::native_lifecycle::{self as native,acp_policy};

#[derive(Debug)]
pub(crate) struct NativeSteerObservation{
    pub connection_written:bool,pub prompt_digest:String,pub request_bytes:u64,
    pub outcome:Option<String>,pub receipt_digest:Option<String>,pub error:Option<String>,
}
impl AcpRuntime{
    pub(crate) async fn native_steer_capability(&self)->Result<Option<String>>{
        if self.host.adapter_kind!=AdapterKind::CustomAcp||!self.host.is_alive()||self.host.native_steering_disabled.load(Ordering::Acquire){return Ok(None)}
        let initialize=self.host.initialize_result.read().await;
        let Some(initialize)=initialize.as_ref()else{return Ok(None)};
        if !acp_policy::steering_available(initialize){return Ok(None)}
        Ok(Some(native::digest(&json!({"host":self.host.host_instance_id,"runtimeCompatibility":self.runtime_compatibility_digest,"initialize":initialize}))?))
    }
    /// 参数已由 Core 从受管快照投影；本入口拒绝另一个执行或代次的轮次。
    pub(crate) async fn native_steer(&self,active_turn:&str,expected_capability:&str,blocks:Vec<Value>)->Result<NativeSteerObservation>{
        ensure!(self.native_steer_capability().await?.as_deref()==Some(expected_capability),"native_lifecycle.capability_changed");
        ensure!(!blocks.is_empty(),"native_lifecycle.empty_input");
        let session=self.session_id().await.context("native_lifecycle.native_session_missing")?;
        let params=acp_policy::steer_params(&session,&blocks);let prompt_digest=native::digest(&json!(blocks))?;
        let id=self.host.next_id.fetch_add(1,Ordering::Relaxed);
        let request=json!({"jsonrpc":"2.0","method":"_session/steering","id":id,"params":params});
        let bytes=serde_json::to_vec(&request)?.len().checked_add(1).context("native_lifecycle.native_request_too_large")?;
        ensure!(bytes<=64*1024*1024,"native_lifecycle.native_request_too_large");
        let(sender,receiver)=oneshot::channel();
        self.host.pending.lock().await.insert(id,PendingRpc::Response{method:"_session/steering".into(),sender});
        let routes=self.host.routes.read().await;
        let current=routes.get(&session).is_some_and(|route|route.owner==self.owner&&matches!(&route.phase,AcpSessionPhase::PromptActive(prompt) if prompt.prompt_id==active_turn));
        if !current{drop(routes);self.host.pending.lock().await.remove(&id);anyhow::bail!("native_lifecycle.active_turn_changed")}
        // 只在写出期间保留路由读锁；等待回执时必须释放，允许原轮次结束。
        let wrote=match timeout(Duration::from_secs(15),self.host.send(request)).await{Ok(value)=>value,Err(_)=>{self.host.protocol_violated.store(true,Ordering::Release);Err(anyhow::anyhow!("native_lifecycle.native_write_timeout"))}};drop(routes);
        if let Err(error)=wrote{
            self.host.pending.lock().await.remove(&id);
            return Ok(NativeSteerObservation{connection_written:false,prompt_digest,request_bytes:bytes as u64,outcome:None,receipt_digest:None,error:Some(self.redact_text(&error.to_string()))})
        }
        let response=match timeout(Duration::from_secs(45),receiver).await{
            Ok(Ok(Ok(value)))=>Ok(value),
            Ok(Ok(Err(error)))=>Err(self.redact_text(&error.to_string())),
            Ok(Err(_))=>Err("native_lifecycle.native_reply_closed".into()),
            Err(_)=>{self.host.pending.lock().await.remove(&id);Err("native_lifecycle.native_reply_timeout".into())},
        };
        let receipt_digest=self.host.native_response_digests.lock().await.remove(&id);
        let(outcome,error)=match response{
            Ok(value)=>{
                match acp_policy::native_outcome(&value){
                    Ok(outcome) if receipt_digest.is_some()=>{if outcome=="startedNewTurn"{self.host.native_steering_disabled.store(true,Ordering::Release)}(Some(outcome.to_owned()),None)},
                    _=>(None,Some("native_lifecycle.native_receipt_unknown".into())),
                }
            },
            Err(error)=>(None,Some(error)),
        };
        Ok(NativeSteerObservation{connection_written:true,prompt_digest,request_bytes:bytes as u64,outcome,receipt_digest,error})
    }
    pub(crate) async fn native_fork(&self,origin:&native::NativeSessionOriginV1)->Result<String>{
        origin.validate()?;ensure!(origin.mode!=native::NativeContinuationMode::Continue,"native_lifecycle.fork_mode_required");
        ensure!(self.prompt_id().await.is_none(),"native_lifecycle.active_turn_exists");
        let initialize=self.host.initialize_result.read().await.clone().context("native_lifecycle.initialize_missing")?;
        ensure!(acp_policy::provider_matches(&origin.source.provider,&initialize),"native_lifecycle.provider_program_mismatch");
        ensure!(initialize.pointer("/agentCapabilities/sessionCapabilities/fork").is_some_and(Value::is_object),"native_lifecycle.fork_unavailable");
        if origin.point.is_some(){ensure!(acp_policy::exact_fork_available(&origin.source.provider,&initialize),"native_lifecycle.exact_fork_unavailable")}
        let params=native::fork::request(&origin.source.native_session_id,&acp_protocol_path(&self.execution_root),origin.point.as_ref())?;
        let result=self.host.rpc_with_timeout("session/fork",params,ACP_HISTORY_RESTORE_TIMEOUT).await?;
        let id=result["sessionId"].as_str().filter(|id|!id.is_empty()&&id.len()<=4096&&!id.contains('\0')).context("native_lifecycle.fork_session_id_missing")?;
        ensure!(id!=origin.source.native_session_id,"native_lifecycle.fork_returned_original_session");
        Ok(id.into())
    }
}
