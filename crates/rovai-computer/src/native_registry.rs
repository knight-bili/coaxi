//! 原执行服务的本机范围容器；业务归属和模型资格由上游 Core 核对。
use anyhow::{Context,Result,ensure};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{collections::HashMap,sync::{Arc,atomic::{AtomicU64,Ordering}}};
use crate::{model_tools::{ComputerToolsConfig,ComputerToolsRuntimeConfig},native_events::ComputerEvents,native_service::{ComputerHost,ComputerService,NativeGrant}};

#[derive(Clone,Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeConfiguration {
    pub scope_key:String,
    pub bridge_instance_id:String,
    pub expected_stop_generation:u64,
    pub enabled:bool,
    pub launch_enabled:bool,
    pub clipboard_enabled:bool,
    pub allow_foreground:bool,
    pub screen_enabled:bool,
    pub grants:Vec<NativeGrant>,
}
struct Slot {service:Arc<ComputerService>,configuration:ComputerToolsRuntimeConfig,gate:tokio::sync::Mutex<Option<String>>,binding:std::sync::Mutex<String>}
pub struct NativeRegistry {
    slots:tokio::sync::Mutex<HashMap<String,Arc<Slot>>>,
    stops:AtomicU64,
    callback:Arc<dyn Fn(&str,Value)+Send+Sync>,
}
impl NativeRegistry {
    pub fn new(callback:impl Fn(&str,Value)+Send+Sync+'static)->Self {Self {slots:Default::default(),stops:AtomicU64::new(0),callback:Arc::new(callback)}}
    pub fn stop_generation(&self)->u64 {self.stops.load(Ordering::Acquire)}
    async fn slot(&self,key:&str)->Result<Arc<Slot>>{
        ensure!(key.len()==64&&key.bytes().all(|byte|byte.is_ascii_hexdigit()),"scope_changed");
        let mut slots=self.slots.lock().await;
        if let Some(slot)=slots.get(key){return Ok(slot.clone())}
        ensure!(slots.len()<64,"computer.scope_capacity");
        let config=ComputerToolsRuntimeConfig::new();
        let callback=self.callback.clone();let scope=key.to_owned();
        let events=ComputerEvents::new(move |kind,payload|callback(kind,json!({"scopeKey":scope,"payload":payload})));
        let service=ComputerService::start(ComputerHost{events},config.clone());
        let slot=Arc::new(Slot{service,configuration:config,gate:tokio::sync::Mutex::new(None),binding:std::sync::Mutex::new(String::new())});
        slots.insert(key.to_owned(),slot.clone());Ok(slot)
    }
    pub async fn configure(&self,config:ScopeConfiguration)->Result<Value>{
        uuid::Uuid::parse_str(&config.bridge_instance_id)?;
        ensure!(config.grants.len()<=256&&config.expected_stop_generation==self.stop_generation(),"cancelled");
        ensure!(config.enabled||!(config.launch_enabled||config.clipboard_enabled||config.screen_enabled),"interaction_disabled");
        for grant in &config.grants {
            ensure!(matches!(grant.target_kind.as_str(),"computer_window"|"computer_app"|"computer_desktop")&&grant.target_id.len()<=256,"grant_required");
            let expires=chrono::DateTime::parse_from_rfc3339(&grant.expires_at)?;
            ensure!(expires>chrono::Utc::now()&&expires<=chrono::Utc::now()+chrono::Duration::minutes(120),"grant_required");
        }
        let slot=self.slot(&config.scope_key).await?;
        let mut previous=slot.gate.lock().await;
        let digest=format!("{:x}",Sha256::digest(serde_json::to_vec(&config)?));
        if previous.as_deref()==Some(digest.as_str()){return Ok(json!({"configured":true,"stopGeneration":self.stop_generation()}))}
        // 绑定或授权改变时，原表先收回代次并释放按键，再重新应用当前批准资源。
        slot.service.stop().await;
        ensure!(config.expected_stop_generation==self.stop_generation(),"cancelled");
        let next=ComputerToolsConfig{enabled:config.enabled,launch_enabled:config.launch_enabled,clipboard_enabled:config.clipboard_enabled,allow_foreground:config.allow_foreground,screen_enabled:config.screen_enabled,show_indicator:false,..Default::default()};
        slot.configuration.set(next).await;
        let windows=slot.service.local_picker().await?;
        slot.service.apply_native_grants(&config.grants,&windows).await?;
        if config.expected_stop_generation!=self.stop_generation(){slot.service.stop().await;anyhow::bail!("cancelled")}
        *slot.binding.lock().map_err(|_|anyhow::anyhow!("computer.binding_lock"))?=config.bridge_instance_id;
        *previous=Some(digest);
        Ok(json!({"configured":true,"stopGeneration":self.stop_generation()}))
    }
    pub async fn picker(&self,scope:&str)->Result<Value>{let slot=self.slot(scope).await?;Ok(json!({"windows":slot.service.local_picker().await?,"screenId":crate::computer::targets::SCREEN_TARGET_ID,"stopGeneration":self.stop_generation()}))}
    pub async fn call(&self,scope:&str,bridge:&str,stop:u64,name:&str,arguments:&Value)->Result<Value>{
        ensure!(stop==self.stop_generation(),"cancelled");
        let slot=self.slot(scope).await?;
        ensure!(slot.binding.lock().map_err(|_|anyhow::anyhow!("computer.binding_lock"))?.as_str()==bridge,"binding_changed");
        let _gate=slot.gate.lock().await;
        ensure!(stop==self.stop_generation(),"cancelled");
        let result=slot.service.execute_external(name,arguments).await?;
        ensure!(stop==self.stop_generation(),"cancelled");
        Ok(result)
    }
    pub async fn stop_scope(&self,scope:&str)->Result<Value>{
        let slot={self.slots.lock().await.get(scope).cloned()};
        if let Some(slot)=slot {slot.binding.lock().map_err(|_|anyhow::anyhow!("computer.binding_lock"))?.clear();slot.service.stop().await;let mut config=slot.configuration.snapshot().await;config.enabled=false;slot.configuration.set(config).await;*slot.gate.lock().await=None;}
        Ok(json!({"stopped":true,"stopGeneration":self.stop_generation()}))
    }
    pub async fn stop_all(&self)->Value {
        let stop=self.stops.fetch_add(1,Ordering::AcqRel)+1;
        let slots:Vec<_>=self.slots.lock().await.values().cloned().collect();
        for slot in &slots{if let Ok(mut binding)=slot.binding.lock(){binding.clear()}}
        let mut tasks=tokio::task::JoinSet::new();
        for slot in slots {tasks.spawn(async move{slot.service.stop().await;let mut config=slot.configuration.snapshot().await;config.enabled=false;slot.configuration.set(config).await;*slot.gate.lock().await=None;});}
        while tasks.join_next().await.is_some(){}
        json!({"stopped":true,"stopGeneration":stop})
    }
    pub async fn status(&self,scope:&str)->Result<Value>{let slot=self.slot(scope).await?;Ok(json!({"status":slot.service.local_status().await,"stopGeneration":self.stop_generation()}))}
    pub async fn dispatch(&self,method:&str,params:Value)->Result<Value>{
        let text=|key:&str|params[key].as_str().context("invalid_arguments");
        match method {
            "configure"=>self.configure(serde_json::from_value(params)?).await,
            "picker"=>self.picker(text("scopeKey")?).await,
            "status"=>self.status(text("scopeKey")?).await,
            "stop_scope"=>self.stop_scope(text("scopeKey")?).await,
            "stop_all"=>Ok(self.stop_all().await),
            "call"=>self.call(text("scopeKey")?,text("bridgeInstanceId")?,params["expectedStopGeneration"].as_u64().context("invalid_arguments")?,text("toolName")?,&params["arguments"]).await,
            _=>anyhow::bail!("computer.operation_not_admitted"),
        }
    }
}
