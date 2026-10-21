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
        let settings:Settings=serde_json::from_slice(&std::fs::read(path)?)?;ensure!(request.expected_revision.as_deref()==Some(settings.revision.as_str()),"webdav.settings_changed");let client=WebDav::new(settings.config.clone())?;
        if request.operation=="list"{return Ok(json!({"entries":client.list().await?}))}
        if request.operation=="probe"{return client.probe().await}
        if request.operation=="create_collection"{return client.create_collection().await}
        if request.operation=="transfers"{return super::webdav_transfer::list(&self.data_dir)}
        let id=request.command_id.context("webdav.command_required")?;uuid::Uuid::parse_str(&id)?;
        if request.operation=="status"{return super::webdav_transfer::status(&self.data_dir,&id)}
        if request.operation=="cancel"{return super::webdav_transfer::cancel(&self.data_dir,&id)}
        let directory=self.data_dir.join("webdav-operations");crate::platform::private_storage::prepare_private_directory(&directory)?;let receipt=directory.join(format!("{id}.json"));
        let _lock=workbench::MUTATIONS.lock().await;
        if request.operation=="resume"{
            let previous:Value=serde_json::from_slice(&std::fs::read(&receipt)?)?;
            if previous["state"]=="completed"||super::webdav_transfer::active(&self.data_dir,&id){return super::webdav_transfer::status(&self.data_dir,&id)}
            let original=previous["request"].clone();ensure!(original.is_object(),"webdav.legacy_receipt_requires_manual_reconciliation");ensure!(original["expectedRevision"]==settings.revision,"webdav.settings_changed");let identity=crate::command::canonical_json_digest(&original)?;ensure!(previous["identity"]==identity,"webdav.command_identity_changed");return super::webdav_transfer::start(&self.data_dir,settings.config,original,identity)
        }
        ensure!(matches!(request.operation.as_str(),"push"|"pull"),"webdav.operation_not_admitted");let identity=crate::command::canonical_json_digest(&value)?;
        if receipt.exists(){let previous:Value=serde_json::from_slice(&std::fs::read(&receipt)?)?;ensure!(previous["identity"]==identity,"webdav.command_identity_changed");return super::webdav_transfer::status(&self.data_dir,&id)}
        let selected=request.path.context("webdav.path_required")?;ensure!(selected.is_absolute(),"webdav.absolute_path_required");let digest=request.expected_digest.context("webdav.source_digest_required")?;ensure!(digest.len()==64&&digest.bytes().all(|byte|byte.is_ascii_hexdigit()),"webdav.invalid_source_digest");
        if request.operation=="pull"{let parent=std::fs::canonicalize(selected.parent().context("webdav.destination_parent_required")?)?;ensure!(!parent.starts_with(std::fs::canonicalize(&self.data_dir)?)&&!selected.exists(),"webdav.destination_invalid");let snapshot=request.snapshot_id.context("webdav.snapshot_required")?;uuid::Uuid::parse_str(&snapshot)?;}
        super::webdav_transfer::start(&self.data_dir,settings.config,value,identity)
    }
}
