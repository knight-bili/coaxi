use super::*;
use crate::native_lifecycle::{self as native,storage};

impl Core{
    pub(super) async fn native_lifecycle_access(self:&Arc<Self>,method:&str,value:Value,client:crate::draft_client::DraftClient)->Result<Value>{
        if matches!(method,"history.execution.records"|"runtime.steer.evidence"){return self.native_lifecycle_records(method,value,&client).await}
        if matches!(method,"runtime.steer.normal.preview"|"runtime.steer.normal.confirm"|"runtime.steer.normal.receipt"){return self.native_normal_access(method,value,client).await}
        if method=="runtime.steer.materials.prepare"{return self.native_steer_materials(value,&client).await}
        native::validate_operation(method,&value)?;
        match method{
            "history.execution.preview"=>self.native_continuation_preview(value,&client).await,
            "history.execution.confirm"=>self.native_continuation_confirm(value,&client).await,
            "history.execution.receipt"=>{
                let command=value["commandId"].as_str().context("native_lifecycle.command_id_required")?;
                let database=self.database.lock().await;native::continuation::receipt(database.connection(),&self.data_dir,command,client.id(),self.interaction_tools.core_instance())
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
                let sql_limit=i64::try_from(limit).context("native_lifecycle.limit_invalid")?;
                let fetch_limit=sql_limit.checked_add(1).context("native_lifecycle.limit_invalid")?;
                let limit=usize::try_from(limit).context("native_lifecycle.limit_invalid")?;
                let mut rows=query.query_map(rusqlite::params![client.id(),scope.thread_id,scope.scope_kind,scope.conversation_id,value["after"].as_str(),fetch_limit],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,Option<String>>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
                let more=rows.len()>limit;rows.truncate(limit);let next=more.then(||rows.last().map(|row|row.0.clone())).flatten();
                let items=rows.into_iter().map(|(_,raw,command)|Ok(json!({"preview":serde_json::from_str::<Value>(&raw)?,"receipt":command.map(|command|native::steer_delivery::receipt(database.connection(),&command,client.id(),self.interaction_tools.core_instance())).transpose()?}))).collect::<Result<Vec<_>>>()?;
                Ok(json!({"schemaVersion":1,"items":items,"next":next}))
            },
            _=>anyhow::bail!("native_lifecycle.operation_unknown"),
        }
    }
    async fn native_lifecycle_records(&self,method:&str,value:Value,client:&crate::draft_client::DraftClient)->Result<Value>{
        anyhow::ensure!(serde_json::to_vec(&value)?.len()<=2*1024*1024,"native_lifecycle.request_too_large");
        let database=self.database.lock().await;let connection=database.connection();
        if method=="history.execution.records"{
            anyhow::ensure!(value.get("importId").is_some()&&value.get("after").is_some(),"native_lifecycle.records_request_invalid");
            #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Request{schema_version:u32,source_thread_id:String,import_id:Option<String>,after:Option<String>,limit:u32}
            let input:Request=serde_json::from_value(value)?;anyhow::ensure!(input.schema_version==1&&(1..=50).contains(&input.limit),"native_lifecycle.records_request_invalid");ThreadId::parse(&input.source_thread_id)?;
            if let Some(id)=&input.import_id{anyhow::ensure!(id.len()==64&&id.bytes().all(|byte|byte.is_ascii_hexdigit()),"native_lifecycle.import_id_invalid");}if let Some(after)=&input.after{uuid::Uuid::parse_str(after)?;}
            let mut statement=connection.prepare("SELECT p.id,p.value_json,i.command_id,n.preparation_evidence_json FROM native_lifecycle_preview p LEFT JOIN native_lifecycle_intent i ON i.preview_id=p.id LEFT JOIN native_lifecycle_continuation n ON n.command_id=i.command_id WHERE p.kind='continuation' AND p.owner_thread_id=?1 AND p.client_id=?2 AND (?3 IS NULL OR json_extract(p.value_json,'$.source.importId')=?3) AND (?4 IS NULL OR p.id>?4) ORDER BY p.id LIMIT ?5")?;
            let mut rows=statement.query_map(rusqlite::params![input.source_thread_id,client.id(),input.import_id,input.after,input.limit+1],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,Option<String>>(2)?,row.get::<_,Option<String>>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            let more=rows.len()>input.limit as usize;rows.truncate(input.limit as usize);let next=more.then(||rows.last().map(|row|row.0.clone())).flatten();
            let items=rows.into_iter().map(|(_,raw,command,proof)|->Result<Value>{
                let preview:Value=serde_json::from_str(&raw)?;native::validate_shape("NativeContinuationPreviewV1",&preview)?;
                let receipt=command.as_ref().map(|command|native::continuation::receipt(connection,&self.data_dir,&command,client.id(),self.interaction_tools.core_instance())).transpose()?;
                let preparation=proof.map(|raw|serde_json::from_str::<acp::NativePreparationProof>(&raw)).transpose()?.or(command.as_deref().map(|command|native::preparation_facts::read(connection,&self.data_dir,command)).transpose()?.flatten());
                Ok(json!({"preview":preview,"receipt":receipt,"preparation":preparation}))
            }).collect::<Result<Vec<_>>>()?;
            return Ok(json!({"schemaVersion":1,"items":items,"next":next}))
        }
        #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Request{schema_version:u32,command_id:String}
        let input:Request=serde_json::from_value(value)?;anyhow::ensure!(input.schema_version==1,"native_lifecycle.version_invalid");uuid::Uuid::parse_str(&input.command_id)?;
        let(preview,delivery):(String,String)=connection.query_row("SELECT p.value_json,d.value_json FROM native_lifecycle_intent i JOIN native_lifecycle_preview p ON p.id=i.preview_id JOIN native_lifecycle_input_delivery d ON d.command_id=i.command_id WHERE i.command_id=?1 AND i.kind='steer' AND i.client_id=?2",rusqlite::params![input.command_id,client.id()],|row|Ok((row.get(0)?,row.get(1)?)))?;
        let preview:Value=serde_json::from_str(&preview)?;let delivery:Value=serde_json::from_str(&delivery)?;native::validate_shape("NativeSteerPreviewV1",&preview)?;native::validate_shape("NativeInputDeliveryV1",&delivery)?;
        let receipt=native::steer_delivery::receipt(connection,&input.command_id,client.id(),self.interaction_tools.core_instance())?;
        let normal:Option<String>=connection.query_row("SELECT result_json FROM native_lifecycle_normal_continuation WHERE source_command_id=?1 AND client_id=?2",rusqlite::params![input.command_id,client.id()],|row|row.get(0)).optional()?;
        let normal=normal.map(|raw|serde_json::from_str::<crate::command::CommandHandlerResult>(&raw)).transpose()?;
        Ok(json!({"schemaVersion":1,"preview":preview,"receipt":receipt,"delivery":delivery,"normalContinuation":normal}))
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
