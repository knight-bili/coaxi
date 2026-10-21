use super::*;
use anyhow::ensure;
use serde::{Serialize,Deserialize};
use crate::backup::webdav::{Config,WebDav};
#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Settings{revision:String,config:Config}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request{operation:String,#[serde(default)]expected_revision:Option<String>,#[serde(default)]endpoint:Option<String>,#[serde(default)]username:Option<String>,#[serde(default)]password:Option<String>,#[serde(default)]clear_password:bool,#[serde(default)]allow_insecure:bool,#[serde(default)]command_id:Option<String>,#[serde(default)]path:Option<PathBuf>,#[serde(default)]snapshot_id:Option<String>,#[serde(default)]expected_digest:Option<String>}
fn view(settings:&Settings)->Value{json!({"revision":settings.revision,"endpoint":settings.config.endpoint,"username":settings.config.username,"passwordSet":!settings.config.password.is_empty(),"allowInsecure":settings.config.allow_insecure})}
impl Core{
    pub(super) async fn webdav_access(&self,local:bool,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value.clone())?;let path=self.data_dir.join("webdav-settings-v1.json");
        if request.operation=="get"{return if path.exists(){Ok(view(&serde_json::from_slice(&std::fs::read(path)?)?))}else{Ok(json!({"revision":null,"endpoint":"","username":"","passwordSet":false,"allowInsecure":false}))}}
        if request.operation=="set"{
            let _lock=workbench::MUTATIONS.lock().await;let previous=if path.exists(){Some(serde_json::from_slice::<Settings>(&std::fs::read(&path)?)?)}else{None};
            ensure!(previous.as_ref().map(|settings|settings.revision.as_str())==request.expected_revision.as_deref(),"webdav.settings_changed");let endpoint=request.endpoint.context("webdav.endpoint_required")?.trim().to_owned();let username=request.username.unwrap_or_default().trim().to_owned();
            let same_account=previous.as_ref().is_some_and(|settings|settings.config.endpoint==endpoint&&settings.config.username==username);
            let password=if request.clear_password{String::new()}else{request.password.filter(|password|!password.is_empty()).or_else(||previous.as_ref().filter(|_|same_account).map(|settings|settings.config.password.clone())).unwrap_or_default()};
            let settings=Settings{revision:uuid::Uuid::new_v4().to_string(),config:Config{endpoint,username,password,allow_insecure:request.allow_insecure}};
            WebDav::new(settings.config.clone())?;crate::platform::private_storage::atomic_write_private_json(&path,&settings)?;return Ok(view(&settings));
        }
        ensure!(local,"webdav.local_owner_required");
        let settings:Settings=serde_json::from_slice(&std::fs::read(path)?)?;ensure!(request.expected_revision.as_deref()==Some(settings.revision.as_str()),"webdav.settings_changed");let client=WebDav::new(settings.config)?;
        if request.operation=="list"{return Ok(json!({"entries":client.list().await?}))}
        if request.operation=="probe"{return client.probe().await}
        if request.operation=="create_collection"{return client.create_collection().await}
        let id=request.command_id.context("webdav.command_required")?;uuid::Uuid::parse_str(&id)?;
        let directory=self.data_dir.join("webdav-operations");crate::platform::private_storage::prepare_private_directory(&directory)?;let receipt=directory.join(format!("{id}.json"));let identity=crate::command::canonical_json_digest(&value)?;
        if request.operation=="status"{let previous:Value=serde_json::from_slice(&std::fs::read(receipt)?)?;return Ok(json!({"state":previous["state"],"result":previous["result"],"message":previous["message"]}));}
        ensure!(matches!(request.operation.as_str(),"push"|"pull"),"webdav.operation_not_admitted");let selected=request.path.context("webdav.path_required")?;ensure!(selected.is_absolute(),"webdav.absolute_path_required");
        if request.operation=="push"{let expected=request.expected_digest.as_deref().context("webdav.source_digest_required")?;let actual=tokio::task::spawn_blocking({let selected=selected.clone();move||crate::backup::file_digest(&selected)}).await??;ensure!(actual.1==expected,"webdav.source_changed");}
        else{let parent=std::fs::canonicalize(selected.parent().context("webdav.destination_parent_required")?)?;ensure!(!parent.starts_with(std::fs::canonicalize(&self.data_dir)?)&&!selected.exists(),"webdav.destination_invalid");}
        {
            let _lock=workbench::MUTATIONS.lock().await;if receipt.exists(){let previous:Value=serde_json::from_slice(&std::fs::read(&receipt)?)?;ensure!(previous["identity"].as_str()==Some(identity.as_str()),"webdav.command_identity_changed");if previous["state"]=="completed"{return Ok(previous["result"].clone())}anyhow::bail!("webdav.original_operation_outcome_unknown");}
            crate::platform::private_storage::atomic_write_private_json(&receipt,&json!({"identity":identity,"state":"started","at":Utc::now().to_rfc3339()}))?;
        }
        let result=if request.operation=="push"{client.push(&selected,&id,request.expected_digest.as_deref()).await}else{client.pull(request.snapshot_id.as_deref().context("webdav.snapshot_required")?,&selected,request.expected_digest.as_deref()).await};
        let receipt_value=match &result{Ok(snapshot)=>json!({"identity":identity,"state":"completed","result":{"snapshot":snapshot,"path":selected},"at":Utc::now().to_rfc3339()}),Err(error)=>json!({"identity":identity,"state":"unknown","message":error.to_string(),"at":Utc::now().to_rfc3339()})};
        crate::platform::private_storage::atomic_write_private_json(&receipt,&receipt_value)?;result?;Ok(receipt_value["result"].clone())
    }
}
