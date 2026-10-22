use super::*;
use crate::native_lifecycle::{self as native,normal};
use rusqlite::params;

#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Confirm {
    schema_version:u32,command_id:String,reference:normal::Reference,
    #[serde(default)]draft_revision:Option<i64>,
    #[serde(default)]composer_scope:Option<crate::single_chat_composer_scope::SingleChatComposerScope>,
}
impl Core{
    pub(super) async fn native_normal_access(self:&Arc<Self>,method:&str,value:Value,client:crate::draft_client::DraftClient)->Result<Value>{
        anyhow::ensure!(serde_json::to_vec(&value)?.len()<=2*1024*1024,"native_lifecycle.request_too_large");
        match method{
            "runtime.steer.normal.preview"=>{
                #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Request{schema_version:u32,source_command_id:String}
                let input:Request=serde_json::from_value(value)?;anyhow::ensure!(input.schema_version==1,"native_lifecycle.version_invalid");let mut database=self.database.lock().await;
                normal::preview(database.connection_mut(),&input.source_command_id,&client)
            },
            "runtime.steer.normal.receipt"=>{
                #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Request{schema_version:u32,command_id:String}
                let input:Request=serde_json::from_value(value)?;anyhow::ensure!(input.schema_version==1,"native_lifecycle.version_invalid");let database=self.database.lock().await;
                normal::receipt(database.connection(),&input.command_id,client.id(),self.interaction_tools.core_instance())
            },
            "runtime.steer.normal.confirm"=>{
                let input:Confirm=serde_json::from_value(value)?;anyhow::ensure!(input.schema_version==1,"native_lifecycle.version_invalid");uuid::Uuid::parse_str(&input.command_id)?;
                let digest=native::digest(&serde_json::to_value(&input)?)?;let instance=self.interaction_tools.core_instance().to_owned();
                let mut database=self.database.lock().await;
                let previous:Option<(String,String)>=database.connection().query_row("SELECT client_id,request_digest FROM native_lifecycle_normal_request WHERE command_id=?1",[&input.command_id],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
                if let Some((owner,previous))=previous{anyhow::ensure!(owner==client.id()&&previous==digest,"native_lifecycle.normal_command_payload_changed");return normal::receipt(database.connection(),&input.command_id,client.id(),&instance)}
                let preview=normal::resolve(database.connection(),&input.reference,&client)?;
                if preview["scope"]["scopeKind"]=="public"{anyhow::ensure!(input.draft_revision.is_none()&&input.composer_scope.is_none(),"native_lifecycle.normal_scope_changed");}else{anyhow::ensure!(input.draft_revision.is_some_and(|revision|revision>=0),"native_lifecycle.normal_draft_revision_required");}
                let now=chrono::Utc::now().to_rfc3339();
                let tx=database.connection_mut().transaction()?;tx.execute("INSERT INTO native_lifecycle_normal_request(command_id,client_id,core_instance_id,request_digest,request_json,state,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'prepared',?6,?6)",params![input.command_id,client.id(),instance,digest,serde_json::to_string(&input)?,now])?;
                let activity=native::steer_delivery::Activity::register(&input.command_id)?;tx.commit()?;drop(database);
                let response=json!({"schemaVersion":1,"commandId":input.command_id,"state":"prepared","commandResult":null,"error":null});let core=self.clone();
                tokio::spawn(async move{
                    let _activity=activity;
                    let observed=core.send_native_normal(&input,client.clone()).await;
                    let mut database=core.database.lock().await;
                    let(state,error)=match observed{Ok(_)=>("recorded",None),Err(_)=>("failed",Some("native_lifecycle.normal_admission_failed"))};
                    if let Err(error)=database.connection_mut().execute("UPDATE native_lifecycle_normal_request SET state=?3,error=?4,updated_at=?5 WHERE command_id=?1 AND core_instance_id=?2 AND state='prepared'",params![input.command_id,instance,state,error,chrono::Utc::now().to_rfc3339()]){eprintln!("Native normal receipt retained unknown: {error:#}");}
                    emit(&core.output,"nativeLifecycle.changed",json!({"commandId":input.command_id,"reason":"normal_settled"}));
                });Ok(response)
            },
            _=>anyhow::bail!("native_lifecycle.normal_operation_unknown")
        }
    }
    async fn send_native_normal(self:&Arc<Self>,input:&Confirm,client:crate::draft_client::DraftClient)->Result<CommandExecution>{
        let packet={let database=self.database.lock().await;normal::input(database.connection(),&self.data_dir,input.reference.clone(),client.clone())?};
        let locations={let database=self.database.lock().await;packet.input.materials.iter().map(|material|native::material::require(database.connection(),&packet.scope,&client,material)).collect::<Result<Vec<_>>>()?};
        let root=self.data_dir.clone();let frozen=packet.input.clone();private_sources::worker(&self.private_source_read_capacity,move||native::material::blocks(&root,&frozen,&locations).map(|_|())).await?;
        let thread=packet.scope.thread_id.clone();
        let result=match packet.scope.scope_kind.as_str(){
            "public"=>{
                let body=packet.input.text.clone();let message=SendUserAutomationThreadMessageCommand{camp_id:thread.clone(),agent_id:packet.agent_id.clone(),body:body.clone(),execution:Some(ExecutionRequest{task_id:None,purpose:body,completion_role:"required".into(),budget:None})};
                let envelope=user_camp_command_envelope(input.command_id.clone(),thread.clone(),normal::PublicCommand{native_input:packet,message});let mut database=self.database.lock().await;
                CollaborationService::default().send_native_input_camp_message(&mut database,&envelope)?
            },
            "single_chat"|"delegation_child"=>{
                let child=packet.scope.scope_kind=="delegation_child";let message=SendSingleChatMessageCommand{draft_client:client.clone(),camp_id:thread.clone(),conversation_id:packet.scope.conversation_id.clone().context("native_lifecycle.normal_conversation_missing")?,body:packet.input.text.clone(),draft_revision:input.draft_revision.context("native_lifecycle.normal_draft_revision_required")?,composer_scope:input.composer_scope.clone()};
                if child{
                    let envelope=user_camp_command_envelope(input.command_id.clone(),thread.clone(),normal::ChildCommand{native_input:packet,message});
                    private_sources::command(&self.database,&self.private_source_read_capacity,&envelope,|database|SingleChatService::send_source_inputs(database,&envelope),|database,prepared|SingleChatService::for_client(client.clone()).with_prepared_sources(prepared).send_owned(database,&envelope)).await?
                }else{
                    let envelope=user_camp_command_envelope(input.command_id.clone(),thread.clone(),normal::SingleCommand{native_input:packet,message});
                    private_sources::command(&self.database,&self.private_source_read_capacity,&envelope,|database|SingleChatService::send_source_inputs(database,&envelope),|database,prepared|SingleChatService::for_client(client.clone()).with_prepared_sources(prepared).send_owned(database,&envelope)).await?
                }
            },
            _=>anyhow::bail!("native_lifecycle.normal_scope_changed")
        };
        if result.result.status!=CommandResultStatus::Rejected{
            self.request_camp_attachment_projection(&thread);if command_result_has_delivery_work(&result.result.payload){self.delivery_batch_scheduler_notify.notify_one();self.execution_wake.runs.notify_one();}
            emit(&self.output,"single_chat.changed",json!({"threadId":thread,"result":result.result}));
        }Ok(result)
    }
}
