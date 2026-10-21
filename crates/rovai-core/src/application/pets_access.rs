use super::*;
use anyhow::ensure;
use serde::Deserialize;
use sha2::{Digest,Sha256};
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request{operation:String,#[serde(default)]id:Option<String>,#[serde(default)]offset:usize,#[serde(default)]source:Option<PathBuf>,#[serde(default)]display_name:Option<String>,#[serde(default)]description:Option<String>,#[serde(default)]value:Option<Value>}
impl Core{
    pub(super) async fn pets_access(&self,local:bool,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value)?;let root=self.data_dir.join("pets");let settings=self.data_dir.join("pets-settings-v1.json");
        if request.operation=="list"{return Ok(json!({"entries":crate::pets::list(&root)?,"settings":if settings.exists(){serde_json::from_slice::<Value>(&std::fs::read(settings)?)?}else{json!({"enabled":false,"activeId":null,"scale":0.75})}}));}
        let _lock=workbench::MUTATIONS.lock().await;
        if request.operation=="settings"{
            let value=request.value.context("pets.settings_required")?;let object=value.as_object().context("pets.invalid_settings")?;ensure!(object.keys().all(|key|matches!(key.as_str(),"enabled"|"activeId"|"scale"))&&value["enabled"].is_boolean()&&value["scale"].as_f64().is_some_and(|scale|(0.25..=2.0).contains(&scale)),"pets.invalid_settings");
            if let Some(id)=value["activeId"].as_str(){crate::pets::read_manifest(&root,id)?;}else{ensure!(value["activeId"].is_null(),"pets.invalid_active_id")};crate::platform::private_storage::atomic_write_private_json(&settings,&value)?;return Ok(value);
        }
        if request.operation=="import_directory"{ensure!(local,"pets.local_owner_required");let source=request.source.context("pets.source_required")?;return crate::pets::import_directory(&root,&source);}
        let id=request.id.context("pets.id_required")?;crate::pets::validation::validate_pet_id(&id)?;
        if request.operation=="asset"{let (bytes,mime)=crate::pets::asset(&root,&id)?;ensure!(request.offset<=bytes.len(),"pets.invalid_offset");let end=(request.offset+512*1024).min(bytes.len());use base64::Engine;return Ok(json!({"id":id,"mime":mime,"digest":format!("{:x}",Sha256::digest(&bytes)),"offset":request.offset,"totalBytes":bytes.len(),"next":(end<bytes.len()).then_some(end),"bytes":base64::engine::general_purpose::STANDARD.encode(&bytes[request.offset..end])}));}
        if request.operation=="meta"{let mut manifest=crate::pets::read_manifest(&root,&id)?;if let Some(name)=request.display_name{ensure!(!name.trim().is_empty()&&name.len()<=256,"pets.invalid_name");manifest.display_name=name;}if let Some(description)=request.description{ensure!(description.len()<=16000,"pets.invalid_description");manifest.description=Some(description);}crate::platform::private_storage::atomic_write_private_json(&root.join(&id).join("pet.json"),&manifest)?;return Ok(serde_json::to_value(manifest)?);}
        if request.operation=="remove"{crate::pets::read_manifest(&root,&id)?;let trash=root.join(".trash");crate::platform::private_storage::prepare_private_directory(&trash)?;std::fs::rename(root.join(&id),trash.join(uuid::Uuid::new_v4().to_string()))?;if settings.exists(){let mut value:Value=serde_json::from_slice(&std::fs::read(&settings)?)?;if value["activeId"].as_str()==Some(id.as_str()){value["activeId"]=Value::Null;value["enabled"]=json!(false);crate::platform::private_storage::atomic_write_private_json(&settings,&value)?;}}return Ok(json!({"removed":true}));}
        anyhow::bail!("pets.operation_not_admitted")
    }
}
