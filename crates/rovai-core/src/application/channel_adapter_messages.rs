//! Local Host ingress and outbox for the original Telegram/Weixin transports.
//! There is no public RPC for impersonating an external sender.
use super::*;
use anyhow::ensure;
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::fs;
#[derive(Clone,Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Inbound{adapter_id:String,expected_revision:String,external_id:String,sender_id:String,text:String,target:rovai_channels::ChannelMessageTarget}
impl crate::command::sealed::Sealed for Inbound{}
impl crate::command::DomainCommand for Inbound{const TYPE:&'static str="channel.adapter.inbound";}
fn source_id(value:&Value)->Result<String>{let hash=crate::command::canonical_json_digest(value)?;Ok(format!("adapter_{}",hash.trim_start_matches("sha256:")))}
impl Core{
    pub(super) async fn channel_adapter_inbound(&self,value:Value)->Result<Value>{
        let input:Inbound=serde_json::from_value(value)?;
        ensure!(!input.text.trim().is_empty()&&input.text.len()<=256*1024&&!input.external_id.is_empty()&&input.external_id.len()<=512&&input.sender_id.len()<=512,"channel_adapter.invalid_message");
        let adapters=self.read_channel_adapters()?;
        ensure!(input.expected_revision==crate::command::canonical_json_digest(&serde_json::to_value(&adapters)?)?,"channel_adapter.configuration_changed");
        let config=adapters.iter().find(|config|config.id==input.adapter_id&&config.enabled).context("channel_adapter.disabled")?;
        ensure!(config.local_id==input.target.channel_id&&config.allowed_senders.contains(&input.sender_id),"channel_adapter.sender_not_authorized");
        let chat=input.target.chat_id.as_deref().unwrap_or(&config.chat_id);
        let binding=config.bindings.iter().find(|binding|binding.chat_id==chat&&binding.thread_key==input.target.thread_key).context("channel_adapter.target_not_bound")?.clone();
        let principal=source_id(&json!({"provider":config.provider,"adapterId":config.id,"senderId":input.sender_id}))?;
        let digest=Sha256::digest(format!("{}\0{}\0{}",config.id,input.sender_id,input.external_id).as_bytes());let mut bytes:[u8;16]=digest[..16].try_into()?;bytes[6]=(bytes[6]&15)|0x50;bytes[8]=(bytes[8]&63)|0x80;let id=uuid::Uuid::from_bytes(bytes).to_string();
        let envelope=crate::command::CommandEnvelope{command_id:id.clone(),actor:ActorRef::System{component_id:"channel-adapter".into()},camp_id:Some(binding.thread_id.clone()),expected_versions:vec![],execution_epoch:None,payload:input.clone()};
        let mut database=self.database.lock().await;
        let result=crate::command::DomainCommandGateway.execute(&mut database,&envelope,|transaction|{
            let now=chrono::Utc::now().to_rfc3339();
            transaction.execute("INSERT INTO external_principal(id,provider,tenant_key,external_user_id,display_name,version,created_at,updated_at) VALUES(?1,?2,?3,?4,?4,1,?5,?5) ON CONFLICT(provider,tenant_key,external_user_id) DO NOTHING",rusqlite::params![principal,config.provider,config.id,input.sender_id,now])?;
            let mut content=binding.agent_ids.iter().map(|agent|crate::camp_content::StructuredThreadMessageSegment::MemberMention{agent_id:agent.clone()}).collect::<Vec<_>>();content.push(crate::camp_content::StructuredThreadMessageSegment::Text{text:input.text.clone()});
            let admission=crate::collaboration::CollaborationService::default().admit_external_channel_message(transaction,crate::collaboration::ExternalChannelAdmissionInput{source_attachments:vec![],camp_id:binding.thread_id.clone(),external_principal_id:principal.clone(),body:input.text.clone(),structured_content:content,addressed_agent_ids:binding.agent_ids.clone(),command_id:id.clone(),now})?;
            match admission{
                Err(rejected)=>Ok(rejected),
                Ok(accepted)=>Ok(crate::command::CommandHandlerResult::applied("channel.adapter.admitted",json!({"adapterId":config.id,"threadId":binding.thread_id,"messageId":accepted.camp_message_id,"senderId":input.sender_id,"externalId":input.external_id,"target":input.target}),Some(crate::command::EntityReference{entity_type:"camp_message".into(),entity_id:accepted.camp_message_id}))),
            }
        })?;
        drop(database);self.execution_wake.execution_changed();emit(&self.output,"thread.changed",json!({"threadId":binding.thread_id,"reason":"channel_adapter_message"}));Ok(serde_json::to_value(result.result)?)
    }

    pub(super) async fn channel_adapter_outbox(&self,value:Value)->Result<Value>{
        #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Request{operation:String,after:Option<i64>,id:Option<String>,expected_digest:Option<String>,result:Option<String>,remote_id:Option<String>}
        let request:Request=serde_json::from_value(value)?;
        let directory=self.data_dir.join("channel-adapter-outbox");crate::platform::private_storage::prepare_private_directory(&directory)?;
        if request.operation=="list"{
            let adapters=self.read_channel_adapters()?;let database=self.database.lock().await;
            let mut statement=database.connection().prepare("SELECT global_sequence,result_payload_json FROM event_log WHERE command_type='channel.adapter.inbound' AND result_status='applied' AND global_sequence>?1 ORDER BY global_sequence LIMIT 100")?;
            let receipts=statement.query_map([request.after.unwrap_or(0).max(0)],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
            let mut outputs=Vec::new();let mut last=request.after.unwrap_or(0);let mut pending=false;
            for (sequence,payload)in &receipts{
                let source:Value=serde_json::from_str(payload)?;let Some(config)=adapters.iter().find(|config|config.enabled&&source["adapterId"]==config.id)else{last=*sequence;continue};
                if !config.allowed_senders.iter().any(|sender|source["senderId"]==sender.as_str()){last=*sequence;continue}
                let target=&source["target"];let chat=target["chat_id"].as_str().unwrap_or(&config.chat_id);let thread_key=target["thread_key"].as_str();if !config.bindings.iter().any(|binding|source["threadId"]==binding.thread_id&&binding.chat_id==chat&&binding.thread_key.as_deref()==thread_key){last=*sequence;continue}
                let message=source["messageId"].as_str().context("channel_adapter.invalid_source")?;
                let mut query=database.connection().prepare("SELECT ar.id,ar.status,cm.id,cm.body FROM agent_run_input ari JOIN agent_run ar ON ar.id=ari.agent_run_id LEFT JOIN camp_message cm ON cm.id=ar.final_camp_message_id AND cm.tombstoned_at IS NULL WHERE ari.message_id=?1 ORDER BY ar.created_at,ar.id")?;
                let runs=query.query_map([message],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,Option<String>>(2)?,row.get::<_,Option<String>>(3)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
                let mut source_pending=runs.is_empty();
                for (run,status,message_id,body)in runs{
                    if let (Some(message_id),Some(body))=(message_id,body){
                        let id=source_id(&json!({"adapterId":config.id,"sourceMessage":message,"runId":run,"messageId":message_id}))?;
                        let data=json!({"id":id,"adapterId":config.id,"target":source["target"],"messageId":message_id,"body":body,"senderId":source["senderId"]});let digest=crate::command::canonical_json_digest(&data)?;let receipt=directory.join(format!("{id}.json"));
                        let state=if receipt.exists(){serde_json::from_slice::<Value>(&fs::read(receipt)?)?}else{Value::Null};
                        if state["state"]!="sent"{source_pending=true;outputs.push(json!({"id":id,"digest":digest,"state":state["state"].as_str().unwrap_or("ready"),"data":data}));}
                    }else if !["succeeded","failed","cancelled"].contains(&status.as_str()){source_pending=true;}
                }
                if source_pending{pending=true;}else if !pending{last=*sequence;}
            }
            return Ok(json!({"items":outputs,"after":last,"hasMore":receipts.len()==100,"pending":pending,"scanAfter":receipts.last().map(|(sequence,_)|*sequence),"firstPending":if pending{Some(last+1)}else{None}}))
        }
        ensure!(["claim","finish"].contains(&request.operation.as_str()),"channel_adapter.operation_not_admitted");
        let id=request.id.as_deref().context("channel_adapter.output_id_required")?;ensure!(id.starts_with("adapter_")&&id[8..].len()==64&&id[8..].bytes().all(|byte|byte.is_ascii_hexdigit()),"channel_adapter.invalid_output_id");
        let _guard=super::workbench::MUTATIONS.lock().await;let file=directory.join(format!("{id}.json"));let mut saved=if file.exists(){serde_json::from_slice::<Value>(&fs::read(&file)?)?}else{json!({"digest":request.expected_digest,"state":"ready"})};
        ensure!(saved["digest"]==request.expected_digest.as_deref().context("channel_adapter.output_digest_required")?,"channel_adapter.output_changed");
        if request.operation=="claim"{if saved["state"]!="ready"{return Ok(json!({"claimed":false,"state":saved["state"]}))}saved["state"]=json!("sending");}
        else{ensure!(saved["state"]=="sending","channel_adapter.output_not_owned");let state=request.result.as_deref().context("channel_adapter.output_result_required")?;ensure!(["sent","unknown"].contains(&state),"channel_adapter.invalid_output_result");saved["state"]=json!(state);saved["remoteId"]=json!(request.remote_id);}
        saved["updatedAt"]=json!(chrono::Utc::now().timestamp());
        crate::platform::private_storage::atomic_write_private_json(&file,&saved)?;Ok(json!({"claimed":request.operation=="claim","state":saved["state"]}))
    }
}
