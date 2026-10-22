use super::*;
use crate::native_lifecycle::{self as native,storage};

impl Core{
    pub(super) async fn native_lifecycle_access(self:&Arc<Self>,method:&str,value:Value,client:crate::draft_client::DraftClient)->Result<Value>{
        if matches!(method,"runtime.steer.normal.preview"|"runtime.steer.normal.confirm"|"runtime.steer.normal.receipt"){return self.native_normal_access(method,value,client).await}
        if method=="runtime.steer.materials.prepare"{return self.native_steer_materials(value,&client).await}
        native::validate_operation(method,&value)?;
        match method{
            "history.execution.preview"=>self.native_continuation_preview(value,&client).await,
            "history.execution.confirm"=>self.native_continuation_confirm(value,&client).await,
            "history.execution.receipt"=>{
                let command=value["commandId"].as_str().context("native_lifecycle.command_id_required")?;
                let database=self.database.lock().await;native::continuation::receipt(database.connection(),command,client.id(),self.interaction_tools.core_instance())
            },
            "runtime.steer.policy.get"=>{let database=self.database.lock().await;storage::policy(database.connection())},
            "runtime.steer.policy.set"=>{
                let command_id=value["commandId"].as_str().context("native_lifecycle.command_id_required")?.to_owned();
                let mut payload=value;payload.as_object_mut().context("native_lifecycle.request_invalid")?.remove("commandId");
                let input:native::policy::SetPolicy=serde_json::from_value(payload)?;let mut database=self.database.lock().await;
                let execution=native::policy::set(&mut database,&user_command_envelope(command_id,input))?;
                let result=serde_json::to_value(execution.result)?;emit(&self.output,"nativeLifecycle.changed",json!({"reason":"policy_saved"}));Ok(result)
            },
            "runtime.steer.targets"=>{
                let scope:native::NativeSteerScopeV1=serde_json::from_value(value["scope"].clone())?;
                let candidates={let database=self.database.lock().await;native::target::candidates(database.connection(),&scope)?};
                let mut targets=Vec::new();
                for mut target in candidates{
                    let Some(runtime)=self.custom_acp.get_agent_run(&target.agent_run_id,target.execution_epoch).await else{continue};
                    if runtime.native_steer_capability().await?.is_none(){continue}
                    let Some(turn)=runtime.prompt_id().await else{continue};target.active_turn_id=turn;
                    let database=self.database.lock().await;if native::target::require(database.connection(),&target).is_ok(){targets.push(target);}
                }Ok(json!({"schemaVersion":1,"targets":targets}))
            },
            "runtime.steer.preview"=>self.native_steer_preview(value,&client).await,
            "runtime.steer.confirm"=>self.native_steer_confirm(value,&client).await,
            "runtime.steer.receipt"=>{
                let command=value["commandId"].as_str().context("native_lifecycle.command_id_required")?;
                let database=self.database.lock().await;native::steer_delivery::receipt(database.connection(),command,client.id(),self.interaction_tools.core_instance())
            },
            "runtime.steer.previews"=>{
                let scope:native::NativeSteerScopeV1=serde_json::from_value(value["scope"].clone())?;native::target::scope(&scope)?;
                let database=self.database.lock().await;let mut query=database.connection().prepare("SELECT p.id,p.value_json,i.command_id FROM native_lifecycle_preview p LEFT JOIN native_lifecycle_intent i ON i.preview_id=p.id WHERE p.kind='steer' AND p.client_id=?1 AND p.owner_thread_id=?2 AND json_extract(p.value_json,'$.target.scope.scopeKind')=?3 AND json_extract(p.value_json,'$.target.scope.conversationId') IS ?4 AND (?5 IS NULL OR p.id>?5) ORDER BY p.id LIMIT ?6")?;
                let limit=value["limit"].as_u64().context("native_lifecycle.limit_invalid")?;
                let mut rows=query.query_map(rusqlite::params![client.id(),scope.thread_id,scope.scope_kind,scope.conversation_id,value["after"].as_str(),limit+1],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,Option<String>>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
                let more=rows.len()>limit as usize;rows.truncate(limit as usize);let next=more.then(||rows.last().map(|row|row.0.clone())).flatten();
                let items=rows.into_iter().map(|(_,raw,command)|Ok(json!({"preview":serde_json::from_str::<Value>(&raw)?,"receipt":command.map(|command|native::steer_delivery::receipt(database.connection(),&command,client.id(),self.interaction_tools.core_instance())).transpose()?}))).collect::<Result<Vec<_>>>()?;
                Ok(json!({"schemaVersion":1,"items":items,"next":next}))
            },
            _=>anyhow::bail!("native_lifecycle.operation_unknown"),
        }
    }
    async fn native_steer_materials(&self,value:Value,client:&crate::draft_client::DraftClient)->Result<Value>{
        #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Request{schema_version:u32,target:native::NativeSteerTargetV1,source_ids:Vec<String>,composer_scope:Option<crate::single_chat_composer_scope::SingleChatComposerScope>}
        anyhow::ensure!(serde_json::to_vec(&value)?.len()<=2*1024*1024&&value.get("composerScope").is_some(),"native_lifecycle.material_prepare_invalid");let input:Request=serde_json::from_value(value)?;anyhow::ensure!(input.schema_version==1,"native_lifecycle.version_invalid");
        let candidates={let database=self.database.lock().await;native::material::selected_sources(database.connection(),&input.target,client,input.composer_scope.as_ref(),&input.source_ids)?};
        let root=self.data_dir.clone();let frozen=tokio::task::spawn_blocking(move||candidates.iter().map(|candidate|native::material::freeze(&root,candidate)).collect::<Result<Vec<_>>>()).await??;
        let mut database=self.database.lock().await;let tx=database.connection_mut().transaction()?;for material in &frozen{native::material::register(&tx,&input.target,client,material)?}tx.commit()?;
        Ok(json!({"schemaVersion":1,"materials":frozen.into_iter().map(|material|material.descriptor).collect::<Vec<_>>()}))
    }
    async fn native_steer_preview(&self,value:Value,client:&crate::draft_client::DraftClient)->Result<Value>{
        let target:native::NativeSteerTargetV1=serde_json::from_value(value["target"].clone())?;let input:native::NativeSteerInputV1=serde_json::from_value(value["input"].clone())?;input.validate()?;
        let locations={let database=self.database.lock().await;native::target::require(database.connection(),&target)?;input.materials.iter().map(|material|native::material::require(database.connection(),&target.scope,client,material)).collect::<Result<Vec<_>>>()?};
        let root=self.data_dir.clone();let check_input=input.clone();tokio::task::spawn_blocking(move||native::material::blocks(&root,&check_input,&locations)).await??;
        let runtime=self.custom_acp.get_agent_run(&target.agent_run_id,target.execution_epoch).await.context("native_lifecycle.runtime_unavailable")?;
        anyhow::ensure!(runtime.prompt_id().await.as_deref()==Some(target.active_turn_id.as_str()),"native_lifecycle.active_turn_changed");
        let capability=runtime.native_steer_capability().await?.context("native_lifecycle.native_steer_unavailable")?;
        let mut database=self.database.lock().await;let tx=database.connection_mut().transaction()?;native::target::require(&tx,&target)?;
        let value=storage::put_preview(&tx,"steer",&target.scope.thread_id,client.id(),json!({"schemaVersion":1,"target":target,"input":input,"inputDigest":native::digest(&serde_json::to_value(&input)?)?,"capabilityDigest":capability}))?;tx.commit()?;Ok(value)
    }
    async fn native_steer_confirm(&self,value:Value,client:&crate::draft_client::DraftClient)->Result<Value>{
        let command=value["commandId"].as_str().context("native_lifecycle.command_id_required")?.to_owned();let input_digest=native::digest(&value)?;let core_instance=self.interaction_tools.core_instance().to_string();
        let preview={let database=self.database.lock().await;
            if let Some(prior)=native::steer_delivery::prior(database.connection(),&command,client.id(),&input_digest,&core_instance)?{return Ok(prior)}
            let owner:String=database.connection().query_row("SELECT owner_thread_id FROM native_lifecycle_preview WHERE id=?1 AND client_id=?2 AND kind='steer'",rusqlite::params![value["previewId"].as_str(),client.id()],|row|row.get(0))?;
            storage::preview(database.connection(),&owner,value["previewId"].as_str().context("native_lifecycle.preview_required")?,value["previewDigest"].as_str().context("native_lifecycle.preview_digest_required")?,"steer")?
        };
        let target:native::NativeSteerTargetV1=serde_json::from_value(preview["target"].clone())?;let input:native::NativeSteerInputV1=serde_json::from_value(preview["input"].clone())?;input.validate()?;
        let runtime=self.custom_acp.get_agent_run(&target.agent_run_id,target.execution_epoch).await.context("native_lifecycle.runtime_unavailable")?;
        let capability=preview["capabilityDigest"].as_str().context("native_lifecycle.capability_missing")?.to_owned();
        anyhow::ensure!(runtime.native_steer_capability().await?.as_deref()==Some(capability.as_str())&&runtime.prompt_id().await.as_deref()==Some(target.active_turn_id.as_str()),"native_lifecycle.active_turn_changed");
        let mut database=self.database.lock().await;let tx=database.connection_mut().transaction()?;
        if let Some(prior)=native::steer_delivery::prior(&tx,&command,client.id(),&input_digest,&core_instance)?{return Ok(prior)}
        storage::preview(&tx,&target.scope.thread_id,preview["previewId"].as_str().context("native_lifecycle.preview_required")?,preview["previewDigest"].as_str().context("native_lifecycle.preview_digest_required")?,"steer")?;
        native::target::require(&tx,&target)?;
        let locations=input.materials.iter().map(|material|native::material::require(&tx,&target.scope,client,material)).collect::<Result<Vec<_>>>()?;
        let record=json!({"schemaVersion":1,"commandId":command,"previewId":preview["previewId"],"target":target,"inputDigest":preview["inputDigest"],"status":"prepared","nativeOutcome":null,"nativeReceiptDigest":null,"error":null});
        native::validate_shape("NativeSteerReceiptV1",&record)?;
        storage::begin_intent(&tx,&command,&target.scope.thread_id,client.id(),&core_instance,"steer",preview["previewId"].as_str().context("native_lifecycle.preview_required")?,&input_digest,&record)?;
        storage::reserve_images(&tx,&command,&target,&input)?;native::steer_delivery::record_initial(&tx,&command,&preview)?;
        let activity=native::steer_delivery::Activity::register(&command)?;tx.commit()?;drop(database);
        let database=self.database.clone();let root=self.data_dir.clone();let output=self.output.clone();
        tokio::spawn(async move{
            let _activity=activity;
            let observed=async{
                let blocks=tokio::task::spawn_blocking(move||native::material::blocks(&root,&input,&locations)).await??;
                {let database=database.lock().await;native::target::require(database.connection(),&target)?;}
                runtime.native_steer(&target.active_turn_id,&capability,blocks).await
            }.await;
            let mut database=database.lock().await;
            if let Err(error)=native::steer_delivery::settle(database.connection_mut(),&command,&core_instance,observed){eprintln!("Native input receipt retained unknown: {error:#}");}
            emit(&output,"nativeLifecycle.changed",json!({"commandId":command,"reason":"steer_settled"}));
        });Ok(record)
    }
}
