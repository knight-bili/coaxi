//! Portable user preferences. No Thread, Run, provider credential or model default
//! is copied; these JSON files remain owned by their existing Desktop/Core stores.
use super::*;
use serde_json::{Value,json};
use std::collections::BTreeMap;
const LIMIT:u64=64*1024*1024;
const DOMAINS:&[(&str,&str,&[&str])]=&[
    ("appearance","appearance.json",&["preference","chatFontSize","documentFontSize","codeFontSize","readingDensity","motionPreference","zoomPercentage"]),
    ("general","general-preferences.json",&["interfaceLanguage","startupLocationMode","lastSettingsSection","executionConsolePlacement","worldMapEnabled"]),
    ("windowClose","window-close.json",&["behavior"]),
    ("messageLibrary","workspace-metadata/global-v1.json",&["tags","quickMessages"]),
];
#[derive(Clone,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Snapshot{format_version:u32,product:String,app_version:String,created_at:String,pub domains:BTreeMap<String,Value>}
fn read_value(path:&Path)->Result<Value>{if !path.exists(){return Ok(Value::Null)}no_link(path)?;ensure!(fs::metadata(path)?.len()<=LIMIT,"config_sync.domain_too_large");Ok(serde_json::from_slice(&fs::read(path)?)?)}
fn target_path(root:&Path,preferences:&Path,id:&str,path:&str)->PathBuf{if id=="messageLibrary"{root.join(path)}else{preferences.join(path)}}
fn source(root:&Path,preferences:&Path)->Result<BTreeMap<String,Value>>{DOMAINS.iter().map(|(id,path,_)|Ok((id.to_string(),read_value(&target_path(root,preferences,id,path))?))).collect()}
fn source_digest(values:&BTreeMap<String,Value>)->Result<String>{crate::command::canonical_json_digest(&serde_json::to_value(values)?)}
fn validate(id:&str,value:&Value)->Result<()> {
    let (_,_,keys)=DOMAINS.iter().find(|(name,_,_)|*name==id).context("config_sync.unknown_domain")?;let object=value.as_object().context("config_sync.domain_must_be_object")?;ensure!(object.keys().all(|key|keys.contains(&key.as_str())),"config_sync.nonportable_key");
    for(key,value)in object{match(id,key.as_str()){
        ("appearance","preference")=>ensure!(["system","light","dark"].contains(&value.as_str().unwrap_or_default()),"config_sync.invalid_theme"),
        ("appearance","readingDensity")=>ensure!(["standard","relaxed"].contains(&value.as_str().unwrap_or_default()),"config_sync.invalid_density"),
        ("appearance","motionPreference")=>ensure!(["system","reduce"].contains(&value.as_str().unwrap_or_default()),"config_sync.invalid_motion"),
        ("appearance","zoomPercentage")=>ensure!(value.as_u64().is_some_and(|number|(10..=500).contains(&number)),"config_sync.invalid_zoom"),
        ("appearance",_)=>ensure!(value.as_u64().is_some_and(|number|(12..=24).contains(&number)),"config_sync.invalid_font"),
        ("general","interfaceLanguage")=>ensure!(["zh-CN","en"].contains(&value.as_str().unwrap_or_default()),"config_sync.invalid_language"),
        ("general","startupLocationMode")=>ensure!(["last_location","quick_chat"].contains(&value.as_str().unwrap_or_default()),"config_sync.invalid_startup"),
        ("general","executionConsolePlacement")=>ensure!(["right","inspector","bottom"].contains(&value.as_str().unwrap_or_default()),"config_sync.invalid_placement"),
        ("general","worldMapEnabled")=>ensure!(value.is_boolean(),"config_sync.invalid_boolean"),
        ("general","lastSettingsSection")=>ensure!(["remote","general","skills","toolbox","mcp","runtime","channels","appearance","notifications","monitoring","diagnostics","about"].contains(&value.as_str().unwrap_or_default()),"config_sync.invalid_settings_section"),
        ("windowClose","behavior")=>ensure!(["ask","tray","exit"].contains(&value.as_str().unwrap_or_default()),"config_sync.invalid_close_behavior"),
        ("messageLibrary","tags")=>{let rows=value.as_array().context("config_sync.invalid_tags")?;ensure!(rows.len()<=512,"config_sync.tag_limit");let mut seen=HashSet::new();for row in rows{let name=row.as_str().context("config_sync.invalid_tag")?;ensure!(!name.trim().is_empty()&&name.len()<=256&&seen.insert(name),"config_sync.invalid_tag");}},
        ("messageLibrary","quickMessages")=>{let rows=value.as_array().context("config_sync.invalid_messages")?;ensure!(rows.len()<=512,"config_sync.message_limit");let mut seen=HashSet::new();for row in rows{let object=row.as_object().context("config_sync.invalid_message")?;ensure!(object.len()==3&&object.keys().all(|key|["id","title","text"].contains(&key.as_str())),"config_sync.invalid_message");let id=row["id"].as_str().context("config_sync.invalid_message")?;uuid::Uuid::parse_str(id)?;ensure!(seen.insert(id)&&row["title"].as_str().is_some_and(|text|!text.trim().is_empty()&&text.len()<=512)&&row["text"].as_str().is_some_and(|text|text.len()<=1024*1024),"config_sync.invalid_message");}},
        _=>anyhow::bail!("config_sync.nonportable_key"),
    }}Ok(())
}
pub fn collect(root:&Path,preferences:&Path)->Result<(Snapshot,String)>{
    let before=source(root,preferences)?;let digest=source_digest(&before)?;let mut domains=BTreeMap::new();for(id,_,keys)in DOMAINS{if let Some(value)=before.get(*id).filter(|value|!value.is_null()){let object=value.as_object().context("config_sync.invalid_local_domain")?;let selected=Value::Object(object.iter().filter(|(key,_)|keys.contains(&key.as_str())).map(|(key,value)|(key.clone(),value.clone())).collect());validate(id,&selected)?;domains.insert(id.to_string(),selected);}}
    ensure!(source_digest(&source(root,preferences)?)?==digest,"config_sync.local_configuration_changed");Ok((Snapshot{format_version:1,product:"codeg-rovai-config".into(),app_version:env!("CARGO_PKG_VERSION").into(),created_at:chrono::Utc::now().to_rfc3339(),domains},digest))
}
fn read_snapshot(root:&Path,file:&Path,passphrase:&str,cancel:&CancellationToken)->Result<Snapshot>{
    ensure!(fs::metadata(file)?.len()<=LIMIT+1024*1024,"config_sync.snapshot_too_large");let directory=administrative(root)?.join(uuid::Uuid::new_v4().to_string());prepare_private_directory(&directory)?;
    let result=(||->Result<Snapshot>{let plaintext=directory.join("configuration.json");crypto::decrypt_file(file,&plaintext,passphrase,cancel)?;let value=read_value(&plaintext)?;let snapshot:Snapshot=serde_json::from_value(value)?;ensure!(snapshot.format_version==1&&snapshot.product=="codeg-rovai-config"&&snapshot.domains.len()<=DOMAINS.len(),"config_sync.unsupported_snapshot");for(id,value)in &snapshot.domains{validate(id,value)?;}Ok(snapshot)})();let _=fs::remove_dir_all(directory);result
}
pub fn export(root:&Path,preferences:&Path,destination:&Path,passphrase:&str,cancel:&CancellationToken)->Result<Value>{
    ensure!(passphrase.chars().count()>=8,"config_sync.passphrase_too_short");let root=fs::canonicalize(root)?;let parent=fs::canonicalize(destination.parent().context("config_sync.destination_parent_required")?)?;ensure!(!parent.starts_with(&root)&&!destination.exists(),"config_sync.destination_invalid");let(snapshot,digest)=collect(&root,preferences)?;let directory=administrative(&root)?.join(uuid::Uuid::new_v4().to_string());prepare_private_directory(&directory)?;
    let result=(||->Result<Value>{let plaintext=directory.join("configuration.json");atomic_write_private_json(&plaintext,&snapshot)?;let encrypted=directory.join("configuration.rovai-config");crypto::encrypt_file(&plaintext,&encrypted,passphrase,cancel)?;let publish=parent.join(format!(".rovai-config-{}.tmp",uuid::Uuid::new_v4()));let result=(||->Result<()>{let mut output=fs::OpenOptions::new().write(true).create_new(true).open(&publish)?;std::io::copy(&mut fs::File::open(encrypted)?,&mut output)?;output.sync_all()?;drop(output);fs::hard_link(&publish,destination)?;Ok(())})();let _=fs::remove_file(publish);result?;Ok(json!({"created":true,"localDigest":digest,"domains":snapshot.domains.keys().collect::<Vec<_>>(),"path":destination}))})();let _=fs::remove_dir_all(directory);result
}
pub fn inspect(root:&Path,preferences:&Path,file:&Path,passphrase:&str,cancel:&CancellationToken)->Result<Value>{let root=fs::canonicalize(root)?;let snapshot=read_snapshot(&root,file,passphrase,cancel)?;let(current,digest)=collect(&root,preferences)?;let changes=snapshot.domains.iter().map(|(id,value)|json!({"domain":id,"before":current.domains.get(id),"after":value})).collect::<Vec<_>>();Ok(json!({"formatVersion":snapshot.format_version,"appVersion":snapshot.app_version,"createdAt":snapshot.created_at,"localDigest":digest,"changes":changes}))}
pub fn apply(root:&Path,preferences:&Path,file:&Path,passphrase:&str,expected_digest:&str,cancel:&CancellationToken)->Result<Value>{
    let lease=CoreDataDirLease::acquire(root)?;let root=lease.data_dir();let snapshot=read_snapshot(root,file,passphrase,cancel)?;let before=source(root,preferences)?;ensure!(source_digest(&before)?==expected_digest,"config_sync.local_configuration_changed");
    let directory=administrative(root)?.join(format!("config-{}",uuid::Uuid::new_v4()));prepare_private_directory(&directory)?;atomic_write_private_json(&directory.join("before.json"),&before)?;let mut applied=Vec::new();
    let result=(||->Result<Value>{for(id,incoming)in &snapshot.domains{ensure!(!cancel.is_cancelled(),"backup.cancelled");let(_,path,_)=DOMAINS.iter().find(|(name,_,_)|*name==id).context("config_sync.unknown_domain")?;let target=target_path(root,preferences,id,path);let original=before.get(id).context("config_sync.before_missing")?;ensure!(read_value(&target)?==*original,"config_sync.local_configuration_changed");let mut next=original.as_object().cloned().unwrap_or_default();if id=="general"{next.entry("schemaVersion".to_owned()).or_insert(json!(5));}if id=="windowClose"{next.entry("schemaVersion".to_owned()).or_insert(json!(1));}if id=="messageLibrary"{next.entry("directoryAliases".to_owned()).or_insert(json!([]));}for(key,value)in incoming.as_object().context("config_sync.invalid_domain")?{next.insert(key.clone(),value.clone());}let next=Value::Object(next);prepare_private_directory(target.parent().context("config_sync.invalid_path")?)?;atomic_write_private_json(&target,&next)?;applied.push((id.clone(),target,next));atomic_write_private_json(&directory.join("progress.json"),&json!({"state":"applying","applied":applied.iter().map(|(id,_,_)|id).collect::<Vec<_>>()}))?;}
        atomic_write_private_json(&directory.join("progress.json"),&json!({"state":"applied","domains":snapshot.domains.keys().collect::<Vec<_>>()}))?;Ok(json!({"applied":true,"domains":snapshot.domains.keys().collect::<Vec<_>>(),"restartRequired":true}))})();
    if result.is_err(){for(id,path,written)in applied.iter().rev(){ensure!(read_value(path)?==*written,"config_sync.rollback_conflict_safety_preserved");let original=before.get(id).context("config_sync.before_missing")?;if original.is_null(){fs::remove_file(path)?}else{atomic_write_private_json(path,original)?;}}atomic_write_private_json(&directory.join("progress.json"),&json!({"state":"rolledBack"}))?;}result
}
