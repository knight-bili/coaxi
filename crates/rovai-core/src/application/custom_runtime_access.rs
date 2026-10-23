use super::*;
use crate::custom_runtime::{self as custom,CustomRuntimeRefV1,storage};

impl Core{
    pub(super) async fn custom_runtime_access(&self,method:&str,value:Value)->Result<Value>{
        if method=="runtime.custom.registry"{
            #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Request{refresh:bool}
            let input:Request=serde_json::from_value(value)?;let mut catalog=custom::registry::catalog(input.refresh).await?;
            let registered={let database=self.database.lock().await;let mut query=database.connection().prepare("SELECT registry_id FROM custom_runtime_head WHERE removed=0 ORDER BY registry_id")?;query.query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?};catalog["registeredIds"]=json!(registered);catalog["platform"]=json!(custom::installer::platform().ok());return Ok(catalog)
        }
        custom::validate_operation(method,&value)?;
        match method{
            "runtime.custom.list"=>{let database=self.database.lock().await;storage::list(database.connection(),value["after"].as_str(),value["limit"].as_u64().context("custom_runtime.limit_invalid")? as u32)},
            "runtime.custom.upsert"=>{
                let command_id=custom::text(&value,"commandId")?.to_owned();let mut payload=value;payload.as_object_mut().context("custom_runtime.request_invalid")?.remove("commandId");
                let input:storage::Upsert=serde_json::from_value(payload).map_err(|_|anyhow::anyhow!("custom_runtime.definition_invalid"))?;let mut database=self.database.lock().await;
                let execution=storage::upsert(&mut database,&user_command_envelope(command_id,input))?;let result=serde_json::to_value(execution.result)?;emit(&self.output,"runtime.custom.changed",json!({"reason":"definition_saved"}));Ok(result)
            },
            "runtime.custom.remove"=>{
                let command_id=custom::text(&value,"commandId")?.to_owned();let mut payload=value;payload.as_object_mut().context("custom_runtime.request_invalid")?.remove("commandId");let input:storage::Remove=serde_json::from_value(payload)?;
                let mut database=self.database.lock().await;let execution=storage::remove(&mut database,&user_command_envelope(command_id,input))?;let result=serde_json::to_value(execution.result)?;emit(&self.output,"runtime.custom.changed",json!({"reason":"definition_removed"}));Ok(result)
            },
            "runtime.custom.install"=>{
                let reference:CustomRuntimeRefV1=serde_json::from_value(value["reference"].clone())?;
                let(definition,selectable)={let database=self.database.lock().await;let definition=storage::resolve(database.connection(),&reference,false)?;let selectable=storage::resolve(database.connection(),&reference,true).is_ok();(definition,selectable)};
                custom::installer::start(&self.data_dir,definition,reference,custom::text(&value,"platform")?.into(),custom::text(&value,"commandId")?.into(),selectable,self.database.clone()).await
            },
            "runtime.custom.inspect"=>{
                let reference:CustomRuntimeRefV1=serde_json::from_value(value["reference"].clone())?;let database=self.database.lock().await;let definition=storage::resolve(database.connection(),&reference,false)?;
                let platform=custom::installer::platform()?;
                let installation:Option<(Option<String>,Option<String>,Option<String>,String)>=database.connection().query_row("SELECT installation_id,operation_id,lock_json,status FROM custom_runtime_installation WHERE definition_id=?1 AND definition_revision=?2 AND platform=?3",rusqlite::params![reference.id,reference.definition_revision,platform],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
                drop(database);
                let detail=if let Some((installation_id,operation_id,lock,status))=installation{
                    let receipt=operation_id.as_deref().map(|id|custom::installer::receipt(&self.data_dir,id)).transpose()?;let lock=lock.map(|raw|serde_json::from_str::<Value>(&raw)).transpose()?;let lock_digest=lock.as_ref().map(custom::digest).transpose()?;
                    let environment=custom::availability::inspect(&self.data_dir,&reference,platform,operation_id.as_deref(),lock.as_ref()).await?;
                    json!({"installationId":installation_id,"operationId":operation_id,"status":status,"receipt":receipt,"launchLock":lock,"launchLockDigest":lock_digest,"qualification":"not_verified","environment":environment})
                }else{json!({"status":"not_installed","qualification":"not_verified","environment":{"status":"not_installed","unresolvedLinkCount":0,"qualification":"not_verified"}})};
                Ok(json!({"reference":reference,"definition":definition,"platform":platform,"installation":detail}))
            },
            _=>anyhow::bail!("custom_runtime.operation_unknown")
        }
    }
}
