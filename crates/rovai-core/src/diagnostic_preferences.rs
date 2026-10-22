//! 非模型设置：独立 Host 的结构化诊断级别，不影响事件、审批或回执。
use anyhow::{Context,Result,ensure};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use std::{io::Read,path::Path};

#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Target{pub target:String,pub level:String}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Settings{pub level:String,pub targets:Vec<Target>}
impl Default for Settings{fn default()->Self{Self{level:"info".into(),targets:vec![]}}}
fn rank(level:&str)->Option<u8>{Some(match level{"off"=>0,"error"=>1,"warn"=>2,"info"=>3,"debug"=>4,"trace"=>5,_=>return None})}
impl Settings{
    pub fn validate(&self)->Result<()>{
        ensure!(rank(&self.level).is_some()&&self.targets.len()<=1024,"logging.settings_invalid");
        let mut seen=std::collections::BTreeSet::new();
        for row in &self.targets{ensure!(!row.target.is_empty()&&row.target.len()<=256&&row.target.bytes().all(|value|value.is_ascii_alphanumeric()||b"_.:-".contains(&value))&&seen.insert(&row.target)&&rank(&row.level).is_some(),"logging.target_invalid");}Ok(())
    }
    pub fn enabled(&self,target:&str,level:&str)->bool{
        let threshold=self.targets.iter().filter(|row|target.starts_with(&row.target)).max_by_key(|row|row.target.len()).map(|row|row.level.as_str()).unwrap_or(&self.level);
        rank(level).is_some_and(|level|level>0&&level<=rank(threshold).unwrap_or(0))
    }
}
fn raw(root:&Path)->Result<Value>{
    let path=root.join("logging.json");
    if !path.try_exists()?{return Ok(Value::Null)}
    let mut file=crate::platform::private_storage::open_private_read_file(&path)?;
    ensure!(file.metadata()?.is_file()&&file.metadata()?.len()<=1024*1024,"logging.file_invalid");
    let mut bytes=Vec::new();file.by_ref().take(1024*1024+1).read_to_end(&mut bytes)?;ensure!(bytes.len()<=1024*1024,"logging.file_invalid");
    Ok(serde_json::from_slice(&bytes)?)
}
fn settings(value:&Value)->Result<Settings>{
    if value.is_null(){return Ok(Settings::default())}
    let mut value=value.clone();let map=value.as_object_mut().context("logging.settings_invalid")?;
    if let Some(version)=map.remove("schemaVersion"){ensure!(version==1,"logging.schema_unsupported");}
    map.entry("targets").or_insert(json!([]));let value:Settings=serde_json::from_value(value)?;value.validate()?;Ok(value)
}
pub fn load(root:&Path)->Result<Settings>{settings(&raw(root)?)}
/// 调用者在 Core 文件写队列内；Desktop 的原 Store 仍是其唯一写入者。
pub(crate) fn access(root:&Path,input:Value)->Result<Value>{
    let desktop=crate::backup::owned_roots::configured(root)?.iter().any(|owner|owner.id=="desktop_preferences");
    #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
    struct Input{operation:String,expected_digest:Option<String>,settings:Option<Settings>}
    let input:Input=serde_json::from_value(input)?;ensure!(matches!(input.operation.as_str(),"get"|"set"),"logging.operation_invalid");
    if desktop{ensure!(input.operation=="get","logging.desktop_owner_required");return Ok(json!({"settings":Settings::default(),"digest":"desktop-owner","writable":false,"scope":"structuredHost"}))}
    let mut current=raw(root)?;
    if input.operation=="set"{
        ensure!(input.expected_digest.as_deref()==Some(crate::portable_configuration::digest(&current)?.as_str()),"logging.settings_changed");
        let next=input.settings.context("logging.settings_required")?;next.validate()?;
        let mut value=serde_json::to_value(&next)?;value["schemaVersion"]=json!(1);
        ensure!(raw(root)?==current,"logging.settings_changed");crate::platform::private_storage::atomic_write_private_json(&root.join("logging.json"),&value)?;current=value;
    }else{ensure!(input.expected_digest.is_none()&&input.settings.is_none(),"logging.read_input_invalid");}
    Ok(json!({"settings":settings(&current)?,"digest":crate::portable_configuration::digest(&current)?,"writable":true,"scope":"structuredHost"}))
}
