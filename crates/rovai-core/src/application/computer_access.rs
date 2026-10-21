//! Native owner admission; deliberately absent from the public operation catalog.
use super::*;
use anyhow::ensure;
use serde::Deserialize;
use std::{collections::HashMap,sync::{Arc,LazyLock}};
static SERVICES:LazyLock<tokio::sync::Mutex<HashMap<String,Arc<rovai_computer::service::Service>>>>=LazyLock::new(Default::default);
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request {scope:workbench::Scope,request:Value,command_id:String}
impl Core {
    pub(super) async fn computer_access(&self,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value)?;
        uuid::Uuid::parse_str(&request.command_id)?;
        let root=self.workbench_root(&request.scope).await?;
        let scope=crate::command::canonical_json_digest(&json!({"scope":request.scope,"root":root,"core":self.data_dir}))?;
        let service={let mut services=SERVICES.lock().await;ensure!(services.contains_key(&scope)||services.len()<64,"computer.scope_limit");services.entry(scope.clone()).or_default().clone()};
        let operation:rovai_computer::service::Request=serde_json::from_value(request.request.clone())?;
        let mutation=matches!(operation.operation.as_str(),"act"|"request_permission");
        if !mutation{return service.handle(operation).await}
        let identity=crate::command::canonical_json_digest(&json!({"scope":scope,"request":request.request}))?;
        let directory=self.data_dir.join("computer-operations");
        let path=directory.join(format!("{}.json",request.command_id));
        {
            let _lock=workbench::MUTATIONS.lock().await;
            crate::platform::private_storage::prepare_private_directory(&directory)?;
            if path.exists(){let previous:Value=serde_json::from_slice(&std::fs::read(&path)?)?;ensure!(previous["identity"].as_str()==Some(identity.as_str()),"computer.command_identity_changed");anyhow::bail!("computer.original_request_already_admitted: {}",previous["state"]);}
            crate::platform::private_storage::atomic_write_private_json(&path,&json!({"identity":identity,"state":"started","at":Utc::now().to_rfc3339()}))?;
        }
        let result=service.handle(operation).await;
        let evidence=match &result{Ok(value)=>json!({"identity":identity,"state":"completed","resultDigest":crate::command::canonical_json_digest(value)?,"at":Utc::now().to_rfc3339()}),Err(error)=>json!({"identity":identity,"state":"unknown","message":error.to_string(),"at":Utc::now().to_rfc3339()})};
        crate::platform::private_storage::atomic_write_private_json(&path,&evidence)?;
        result
    }
}
