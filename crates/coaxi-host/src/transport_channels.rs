//! Original Telegram/Weixin transports; Core remains the message/run owner.
use anyhow::{Context, Result, ensure};
use coaxi_channels::{
    ButtonStyle, ChannelMessageTarget, ChatChannelBackend, IncomingCommand, InteractiveMessage,
    MessageButton, RichMessage, TelegramBackend, WeixinBackend,
};
use coaxi_core::{
    application::CoreService,
    platform::{atomic_write_private_bytes, open_private_read_file, prepare_private_directory},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, io::Read, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    id: String,
    local_id: i32,
    provider: String,
    enabled: bool,
    token: String,
    base_url: String,
    chat_id: String,
    topic_mode: bool,
    allowed_senders: Vec<String>,
}
struct Running {
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
pub struct TransportChannels {
    core: CoreService,
    data_dir: PathBuf,
    running: Mutex<Option<Running>>,
}
impl TransportChannels {
    pub fn new(core: CoreService, data_dir: PathBuf) -> Self {
        Self {
            core,
            data_dir,
            running: Mutex::new(None),
        }
    }
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if running.is_some() {
            return Ok(());
        }
        let directory = prepare_private_directory(&self.data_dir.join("channel-adapter-inbox"))?;
        let cancel = CancellationToken::new();
        let stopping = cancel.clone();
        let core = self.core.clone();
        let task = tokio::spawn(async move {
            let (sender, mut incoming) = mpsc::channel::<IncomingCommand>(32);
            let mut backends: HashMap<i32, (String, Arc<dyn ChatChannelBackend>)> = HashMap::new();
            let mut revision = String::new();
            let mut configs = Vec::<Config>::new();
            let mut inbox_after = 0usize;
            let mut timer = tokio::time::interval(Duration::from_secs(2));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _=stopping.cancelled()=>break,
                    received=incoming.recv()=>{let Some(mut message)=received else{break};
                        let durable=message.durable.take();let Some(config)=configs.iter().find(|config|config.local_id==message.channel_id&&config.enabled)else{continue};
                        if !config.allowed_senders.contains(&message.sender_id){if let Some(durable)=durable{let _=durable.send(());}continue}
                        let external=message.metadata.get("update_id").or_else(||message.metadata.get("message_id")).or_else(||message.metadata.get("msg_id")).map(|value|value.to_string()).unwrap_or_else(||format!("metadata:{:x}",Sha256::digest(serde_json::to_vec(&message.metadata).unwrap_or_default())));
                        let key=format!("{:x}",Sha256::digest(format!("{}\0{}\0{}",config.id,message.sender_id,external).as_bytes()));
                        let entry=json!({"params":{"adapterId":config.id,"expectedRevision":revision,"externalId":external,"senderId":message.sender_id,"text":message.command_text,"target":message.target,"callbackData":message.callback_data}});
                        let path=directory.join(format!("{key}.json"));let bytes=match serde_json::to_vec(&entry){Ok(bytes)if bytes.len()<=1024*1024=>bytes,_=>continue};
                        // A full/unwritable inbox withholds acknowledgment, so the
                        // receiver does not advance the provider's polling cursor.
                        let count=std::fs::read_dir(&directory).map(|entries|entries.take(4097).count()).unwrap_or(4097);
                        if count<=4096&&atomic_write_private_bytes(&path,&bytes).is_ok(){if let Some(durable)=durable{let _=durable.send(());}}
                    },
                    _=timer.tick()=>{
                        let _=call(&core,"backup.config.local",json!({"operation":"tick"})).await;
                        if let Ok(snapshot)=call(&core,"channels.adapters.private",json!({"operation":"list"})).await{
                            let next=snapshot["revision"].as_str().unwrap_or_default();
                            if next!=revision{for (_,(_,backend))in backends.drain(){let _=backend.stop().await;}configs=serde_json::from_value(snapshot["adapters"].clone()).unwrap_or_default();revision=next.into();
                                for config in configs.iter().filter(|config|config.enabled){
                                    let backend:Arc<dyn ChatChannelBackend>=match config.provider.as_str(){"telegram"=>Arc::new(TelegramBackend::new(config.local_id,config.token.clone(),config.chat_id.clone(),config.topic_mode)),"weixin"=>Arc::new(WeixinBackend::new(config.local_id,config.token.clone(),config.base_url.clone()).with_allowed_senders(config.allowed_senders.clone())),_=>continue};
                                    if backend.start(sender.clone()).await.is_ok(){backends.insert(config.local_id,(config.id.clone(),backend));}
                                }
                            }
                        }
                        let mut files=std::fs::read_dir(&directory).into_iter().flatten().filter_map(|entry|entry.ok()).map(|entry|entry.path()).filter(|path|path.extension().is_some_and(|extension|extension=="json"||extension=="callback-pending")).collect::<Vec<_>>();files.sort();
                        if !files.is_empty(){let count=files.len();for offset in 0..count.min(32){let path=&files[(inbox_after+offset)%count];let result=(||->Result<Value>{let mut file=open_private_read_file(path)?;let mut bytes=Vec::new();(&mut file).take(1024*1024+1).read_to_end(&mut bytes)?;ensure!(bytes.len()<=1024*1024,"channel_inbox_too_large");Ok(serde_json::from_slice(&bytes)?)})();
                                if let Ok(mut entry)=result{if !entry["callbackData"].is_null(){entry["params"]["callbackData"]=entry["callbackData"].take();}
                                    if let Ok(receipt)=call(&core,"channels.adapters.inbound",entry["params"].clone()).await{match receipt["status"].as_str(){Some("applied"|"accepted")=>{let _=std::fs::remove_file(path);},Some("rejected")=>{let _=std::fs::rename(path,path.with_extension("rejected"));},_=>{}}}
                                }
                            }inbox_after=(inbox_after+32)%count;
                        }
                        let mut states=Vec::new();for (_, (id,backend))in &backends{states.push(json!({"adapterId":id,"state":backend.status().await}));}
                        let entries=std::fs::read_dir(&directory).into_iter().flatten().filter_map(|entry|entry.ok()).map(|entry|entry.path()).collect::<Vec<_>>();
                        let pending_inbox=entries.iter().filter(|path|path.extension().is_some_and(|value|value=="json")).count();let pending_callbacks=entries.iter().filter(|path|path.extension().is_some_and(|value|value=="callback-pending")).count();let rejected=entries.iter().filter(|path|path.extension().is_some_and(|value|value=="rejected")).count();
                        let _=call(&core,"channels.adapters.hostStatus",json!({"states":states,"pendingInbox":pending_inbox,"pendingCallbacks":pending_callbacks,"rejectedInbox":rejected})).await;
                        // Scan bounded pages. Sending/unknown receipts never enter
                        // the automatic send path again after a process restart.
                        let mut after=0i64;for _ in 0..8{
                            let Ok(page)=call(&core,"channels.adapters.outbox",json!({"operation":"list","after":after})).await else{break};
                            if let Some(items)=page["items"].as_array(){for output in items.iter().filter(|output|output["state"]=="ready").take(32){
                                let data=&output["data"];let Some((_,backend))=backends.values().find(|(id,_)|data["adapterId"]==id.as_str())else{continue};
                                let Ok(target)=serde_json::from_value::<ChannelMessageTarget>(data["target"].clone())else{continue};let Some(body)=data["body"].as_str()else{continue};
                                let id=&output["id"];let digest=&output["digest"];let Ok(claim)=call(&core,"channels.adapters.outbox",json!({"operation":"claim","id":id,"expectedDigest":digest})).await else{continue};if claim["claimed"]!=true{continue}
                                let buttons=data["buttons"].as_array().map(|rows|rows.iter().filter_map(|row|Some(MessageButton{id:row["id"].as_str()?.into(),label:row["label"].as_str()?.into(),style:ButtonStyle::Default})).collect::<Vec<_>>()).unwrap_or_default();
                                let message=RichMessage::info(body);let sent=tokio::time::timeout(Duration::from_secs(50),async{if buttons.is_empty(){backend.send_rich_message_to(&message,&target).await}else{backend.send_interactive_message_to(&InteractiveMessage{base:message,buttons,callback_context:json!({"adapterId":data["adapterId"]})},&target).await}}).await;
                                let (state,remote)=match sent{Ok(Ok(id))=>("sent",Some(id.0)),_=>("unknown",None)};
                                let _=call(&core,"channels.adapters.outbox",json!({"operation":"finish","id":id,"expectedDigest":digest,"result":state,"remoteId":remote})).await;
                            }}
                            if page["hasMore"]!=true{break}let Some(next)=page["scanAfter"].as_i64()else{break};if next<=after{break}after=next;
                        }
                    }
                }
            }
            for (_, (_, backend)) in backends {
                let _ = backend.stop().await;
            }
        });
        *running = Some(Running { cancel, task });
        Ok(())
    }
    pub async fn shutdown(&self) {
        if let Some(mut running) = self.running.lock().await.take() {
            running.cancel.cancel();
            if tokio::time::timeout(Duration::from_secs(5), &mut running.task)
                .await
                .is_err()
            {
                running.task.abort();
                let _ = running.task.await;
            }
        }
    }
}
async fn call(core: &CoreService, method: &str, params: Value) -> Result<Value> {
    let reply = tokio::time::timeout(Duration::from_secs(30), core.request(method, params))
        .await
        .context("channel_core_result_unknown")??;
    ensure!(reply.error.is_none(), "channel_core_operation_rejected");
    reply.result.context("channel_core_result_unavailable")
}
