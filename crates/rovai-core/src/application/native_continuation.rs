use super::*;
use crate::native_lifecycle::{self as native,continuation,storage};
use rusqlite::params;
use chrono::Utc;

impl Core{
    pub(super) async fn native_continuation_preview(&self,value:Value,client:&crate::draft_client::DraftClient)->Result<Value>{
        native::validate_operation("history.execution.preview",&value)?;
        let owner=value["sourceThreadId"].as_str().context("native_lifecycle.source_thread_required")?.to_owned();
        let id=value["importId"].as_str().context("native_lifecycle.import_required")?.to_owned();
        let revision=value["expectedImportRevision"].as_str().context("native_lifecycle.import_revision_required")?.to_owned();
        let agent=value["agentId"].as_str().context("native_lifecycle.member_required")?.to_owned();
        let version=value["expectedAgentVersion"].as_i64().context("native_lifecycle.member_version_required")?;
        let mode:native::NativeContinuationMode=serde_json::from_value(value["mode"].clone())?;
        let requested_point:Option<native::NativeForkPointV1>=serde_json::from_value(value["point"].clone())?;
        anyhow::ensure!((mode==native::NativeContinuationMode::ForkPoint)==requested_point.is_some(),"native_lifecycle.fork_point_mode_mismatch");
        let title=value["title"].as_str().context("native_lifecycle.title_required")?;
        anyhow::ensure!(title.chars().count()<=80,"native_lifecycle.title_too_long");
        let title=if title.trim().is_empty(){"原生会话工作副本".to_owned()}else{title.to_owned()};
        let database=self.database.clone();let data=self.data_dir.clone();let source_owner=owner.clone();let member=agent.clone();
        let (source,workspace,workspace_digest,runtime)=tokio::task::spawn_blocking(move||->Result<_>{
            let database=database.blocking_lock();let connection=database.connection();
            let source=continuation::imported(connection,&data,&source_owner,&id,&revision)?;
            let(workspace,digest)=continuation::workspace(connection,&source_owner)?;
            let runtime=continuation::member(connection,&member,version)?;Ok((source,workspace,digest,runtime))
        }).await??;
        let location=source.location.clone();let expected_revision=source.reference.source_revision.clone();
        tokio::task::spawn_blocking(move||->Result<()>{anyhow::ensure!(native::source::revision(&location)?==expected_revision,"native_lifecycle.source_changed");Ok(())}).await??;
        let point=if let Some(requested)=requested_point{
            let point=native::fork::resolve(&source.reference.provider,&source.turns,&requested.selected_turn_id)?;
            anyhow::ensure!(point==requested,"native_lifecycle.fork_point_changed");Some(point)
        }else{None};
        let origin=native::NativeSessionOriginV1{schema_version:1,origin_id:uuid::Uuid::new_v4().to_string(),command_id:uuid::Uuid::new_v4().to_string(),source:source.reference.clone(),mode:mode.clone(),point:point.clone(),workspace_digest:workspace_digest.clone()};origin.validate()?;
        let mut inspection_runtime=runtime.clone();
        native::freeze_intent(&mut inspection_runtime,Some(origin.clone()),None)?;
        let search=self.runtime_search_environment.read().await.clone();
        let private=self.data_dir.join("native-lifecycle").join("preparation").join(&origin.origin_id);
        let environment=crate::member_environment::LaunchEnvironmentSource{database:self.database.clone(),member_id:agent.clone(),kind:runtime.adapter_kind};
        let inspected=if runtime.adapter_kind==AdapterKind::CustomAcp{
            crate::member_environment::with_launch_source(environment,with_runtime_configuration(runtime.adapter_kind,&search,async{
                inspection_runtime.environment=crate::member_environment::load_for_launch().await?;
                acp::inspect_native_preparation(&workspace,&inspection_runtime,&origin,&source.location,&private).await
            })).await
        }else{Err(anyhow::anyhow!("native_lifecycle.explicit_matching_program_required"))};
        let(status,reason,initialize)=match inspected{Ok(digest)=>("available",None,Some(digest)),Err(_)=>(if runtime.adapter_kind==AdapterKind::CustomAcp{"unavailable"}else{"requires_acp_program"},Some(if runtime.adapter_kind==AdapterKind::CustomAcp{"所选程序未能确认同一来源或所需恢复能力；保留导入记录，请核对程序和环境"}else{"请选择你已添加、与来源相同且支持恢复的 ACP 程序；不会自动换程序或安装"}),None)};
        let mut database=self.database.lock().await;let tx=database.connection_mut().transaction()?;
        let current=continuation::member(&tx,&agent,version)?;anyhow::ensure!(current.config_digest==runtime.config_digest,"native_lifecycle.member_runtime_changed");
        let(_,current_workspace)=continuation::workspace(&tx,&owner)?;anyhow::ensure!(current_workspace==workspace_digest,"native_lifecycle.project_changed");
        let preview=storage::put_preview(&tx,"continuation",&owner,client.id(),json!({"schemaVersion":1,"source":source.reference,"mode":mode,"point":point,"sourceThreadId":owner,"newThreadId":ThreadId::new(),"newConversationId":uuid::Uuid::new_v4().to_string(),"agentId":agent,"expectedAgentVersion":version,"runtimeConfigDigest":runtime.config_digest,"workspaceDigest":workspace_digest,"title":title,"historyMessageCount":source.turns.len(),"capabilityStatus":status,"reason":reason}))?;
        tx.execute("INSERT INTO native_lifecycle_continuation_preview(preview_id,source_snapshot_digest,initialize_digest,runtime_json) VALUES(?1,?2,?3,?4)",params![preview["previewId"].as_str(),source.snapshot_digest,initialize,serde_json::to_string(&runtime)?])?;
        tx.commit()?;Ok(preview)
    }

    pub(super) async fn native_continuation_confirm(&self,value:Value,client:&crate::draft_client::DraftClient)->Result<Value>{
        native::validate_operation("history.execution.confirm",&value)?;
        let command=value["commandId"].as_str().context("native_lifecycle.command_required")?.to_owned();
        let input_digest=native::digest(&value)?;let instance=self.interaction_tools.core_instance().to_owned();
        let (preview,snapshot,initialize)={
            let database=self.database.lock().await;let connection=database.connection();
            if let Some(prior)=continuation_prior(connection,&command,client.id(),&input_digest,&instance)?{return Ok(prior)}
            let owner:String=connection.query_row("SELECT owner_thread_id FROM native_lifecycle_preview WHERE id=?1 AND client_id=?2 AND kind='continuation'",params![value["previewId"].as_str(),client.id()],|row|row.get(0))?;
            let preview=storage::preview(connection,&owner,value["previewId"].as_str().context("native_lifecycle.preview_required")?,value["previewDigest"].as_str().context("native_lifecycle.preview_digest_required")?,"continuation")?;
            anyhow::ensure!(preview["capabilityStatus"]=="available","native_lifecycle.program_not_available");
            let(snapshot,initialize):(String,String)=connection.query_row("SELECT source_snapshot_digest,initialize_digest FROM native_lifecycle_continuation_preview WHERE preview_id=?1",[preview["previewId"].as_str()],|row|Ok((row.get(0)?,row.get(1)?)))?;(preview,snapshot,initialize)
        };
        let reference:native::NativeSourceReferenceV1=serde_json::from_value(preview["source"].clone())?;
        let owner=preview["sourceThreadId"].as_str().context("native_lifecycle.source_thread_required")?.to_owned();
        let member=preview["agentId"].as_str().context("native_lifecycle.member_required")?.to_owned();
        let expected=preview["expectedAgentVersion"].as_i64().context("native_lifecycle.member_version_required")?;
        let database=self.database.clone();let data=self.data_dir.clone();let source_owner=owner.clone();let selected=reference.clone();let expected_snapshot=snapshot.clone();
        let source=tokio::task::spawn_blocking(move||->Result<_>{
            let database=database.blocking_lock();let source=continuation::imported(database.connection(),&data,&source_owner,&selected.import_id,&selected.import_revision)?;
            anyhow::ensure!(source.reference==selected&&source.snapshot_digest==expected_snapshot,"native_lifecycle.import_changed");
            anyhow::ensure!(native::source::revision(&source.location)?==selected.source_revision,"native_lifecycle.source_changed");Ok(source)
        }).await??;
        let origin=native::NativeSessionOriginV1{schema_version:1,origin_id:uuid::Uuid::new_v4().to_string(),command_id:command.clone(),source:reference,mode:serde_json::from_value(preview["mode"].clone())?,point:serde_json::from_value(preview["point"].clone())?,workspace_digest:preview["workspaceDigest"].as_str().context("native_lifecycle.workspace_missing")?.into()};origin.validate()?;
        let mut database=self.database.lock().await;let tx=database.connection_mut().transaction()?;
        if let Some(prior)=continuation_prior(&tx,&command,client.id(),&input_digest,&instance)?{return Ok(prior)}
        storage::preview(&tx,&owner,preview["previewId"].as_str().context("native_lifecycle.preview_required")?,preview["previewDigest"].as_str().context("native_lifecycle.preview_digest_required")?,"continuation")?;
        let current=continuation::member(&tx,&member,expected)?;anyhow::ensure!(preview["runtimeConfigDigest"]==current.config_digest,"native_lifecycle.member_runtime_changed");
        continuation::release_closed(&tx)?;
        let(thread,conversation)=continuation::create_working_copy(&tx,&preview,client)?;
        let mut runtime=crate::agent_profile::resolve_frozen_runtime(&tx,&conversation,&member)?.map_err(|blocker|anyhow::anyhow!("native_lifecycle.runtime_blocked: {}",blocker.code))?;
        let base_digest=runtime.config_digest.clone();let policy=storage::freeze_policy(&tx,&member)?;native::freeze_intent(&mut runtime,Some(origin.clone()),policy)?;
        let(workspace,_)=continuation::workspace(&tx,&owner)?;
        let record=json!({"schemaVersion":1,"commandId":command,"state":"prepared","originId":origin.origin_id,"threadId":thread,"conversationId":conversation,"reportedSessionId":null,"error":null});native::validate_shape("NativeContinuationReceiptV1",&record)?;
        storage::begin_intent(&tx,&command,&owner,client.id(),&instance,"continuation",preview["previewId"].as_str().context("native_lifecycle.preview_required")?,&input_digest,&record)?;
        continuation::reserve(&tx,&command,&origin.source)?;
        let now=Utc::now().to_rfc3339();
        tx.execute("INSERT INTO native_lifecycle_continuation(origin_id,command_id,source_thread_id,thread_id,conversation_id,source_snapshot_digest,workspace_path,workspace_digest,base_runtime_digest,runtime_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)",params![origin.origin_id,command,owner,thread,conversation,snapshot,workspace.to_string_lossy(),origin.workspace_digest,base_digest,serde_json::to_string(&runtime)?,now])?;
        let activity=native::steer_delivery::Activity::register(&command)?;tx.commit()?;drop(database);
        let prepared_record=record.clone();
        let database=self.database.clone();let root=self.data_dir.clone();let output=self.output.clone();let search=self.runtime_search_environment.read().await.clone();
        tokio::spawn(async move{
            let _activity=activity;
            let starting=async{
                let mut database=database.lock().await;let tx=database.connection_mut().transaction()?;
                let mut next=record.clone();next["state"]=json!("starting");
                let changed=tx.execute("UPDATE native_lifecycle_intent SET state='starting',value_json=?3,updated_at=?4 WHERE command_id=?1 AND core_instance_id=?2 AND state='prepared'",params![command,instance,serde_json::to_string(&next)?,Utc::now().to_rfc3339()])?;anyhow::ensure!(changed==1,"native_lifecycle.intent_changed");tx.commit()?;Ok::<_,anyhow::Error>(())
            }.await;
            if starting.is_err(){return}
            let environment=crate::member_environment::LaunchEnvironmentSource{database:database.clone(),member_id:member,kind:runtime.adapter_kind};
            let kind=runtime.adapter_kind;
            let observed=crate::member_environment::with_launch_source(environment,with_runtime_configuration(kind,&search,async{
                runtime.environment=crate::member_environment::load_for_launch().await.map_err(|_|acp::NativePreparationFailure{outcome_unknown:false,code:"native_lifecycle.environment_unavailable"})?;
                let private=root.join("native-lifecycle").join("preparation").join(&origin.origin_id);
                acp::prepare_native_session(&workspace,&runtime,&origin,&source.location,&initialize,&private).await
            })).await;
            let mut database=database.lock().await;
            if let Err(error)=settle_continuation(database.connection_mut(),&command,&instance,&record,&runtime,observed){eprintln!("Native creation receipt retained unknown: {error:#}");}
            emit(&output,"nativeLifecycle.changed",json!({"commandId":command,"reason":"continuation_settled"}));
        });
        // 返回最初的准备回执；后台拥有生命周期，调用者断开不会取消持久化。
        Ok(prepared_record)
    }
}

fn continuation_prior(connection:&rusqlite::Connection,command:&str,client:&str,input:&str,instance:&str)->Result<Option<Value>>{
    let row:Option<(String,String,String)>=connection.query_row("SELECT kind,client_id,input_digest FROM native_lifecycle_intent WHERE command_id=?1",[command],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    row.map(|(kind,owner,digest)|{anyhow::ensure!(kind=="continuation"&&owner==client&&digest==input,"native_lifecycle.command_payload_changed");continuation::receipt(connection,command,client,instance)}).transpose()
}
fn settle_continuation(connection:&mut rusqlite::Connection,command:&str,instance:&str,record:&Value,runtime:&crate::agent_profile::FrozenAgentRuntimeConfig,observed:std::result::Result<acp::NativePreparationProof,acp::NativePreparationFailure>)->Result<()>{
    let tx=connection.transaction()?;let mut next=record.clone();
    let(state,lease)=match observed{
        Ok(proof)=>{
            next["state"]=json!("binding_recorded");next["reportedSessionId"]=json!(proof.reported_session_id);
            let now=Utc::now().to_rfc3339();
            let changed=tx.execute("UPDATE conversation SET native_adapter_installation_id=?2,native_session_id=?3,native_binding_compatibility_digest=?4,native_installation_generation=?5,native_session_compatibility_key=?6,version=version+1,updated_at=?7 WHERE id=?1 AND native_session_id IS NULL AND ended_at IS NULL AND last_input_sequence=0",params![record["conversationId"].as_str(),runtime.installation_id,proof.reported_session_id,runtime.binding_compatibility_digest,runtime.installation_generation,runtime.native_session_compatibility_key,now])?;anyhow::ensure!(changed==1,"native_lifecycle.prepared_conversation_changed");
            tx.execute("UPDATE native_lifecycle_continuation SET reported_session_id=?2,preparation_evidence_json=?3,updated_at=?4 WHERE command_id=?1",params![command,proof.reported_session_id,serde_json::to_string(&proof)?,now])?;
            ("binding_recorded","bound")
        },
        Err(error)=>{let state=if error.outcome_unknown{"unknown"}else{"failed"};next["state"]=json!(state);next["error"]=json!(error.code);(state,if error.outcome_unknown{"unknown"}else{"released"})}
    };
    native::validate_shape("NativeContinuationReceiptV1",&next)?;
    let changed=tx.execute("UPDATE native_lifecycle_intent SET state=?3,value_json=?4,updated_at=?5 WHERE command_id=?1 AND core_instance_id=?2 AND state='starting'",params![command,instance,state,serde_json::to_string(&next)?,Utc::now().to_rfc3339()])?;anyhow::ensure!(changed==1,"native_lifecycle.intent_changed");
    tx.execute("UPDATE native_lifecycle_source_lease SET state=?2 WHERE command_id=?1",params![command,lease])?;tx.commit()?;Ok(())
}
