//! 客户确认的完整模型配置迁移 r1；旧任务省略新连接字段。
use anyhow::{Context,Result,ensure};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use std::sync::LazyLock;

pub mod storage;
pub(crate) mod secrets;
pub(crate) mod providers;
pub(crate) mod members;
pub(crate) mod archive;
mod fusion_archive;
mod codeg_crypto;
pub(crate) mod codex_catalog;
pub(crate) mod normalize;
pub(crate) mod archive_store;
pub(crate) mod migration;
pub(crate) mod files;
pub(crate) mod desktop_owner;
pub(crate) mod library;
pub(crate) mod preferences;
pub(crate) mod policies;
pub(crate) mod launch;
pub(crate) mod catalog;
mod model;
pub use model::*;

pub const MAX_REQUEST:usize=2*1024*1024;
pub const MAX_ARCHIVE:usize=64*1024*1024;
static CONTRACT:LazyLock<Value>=LazyLock::new(||serde_json::from_str(include_str!("contract.json")).expect("confirmed portable configuration r1 contract"));

pub fn digest(value:&Value)->Result<String>{Ok(format!("sha256:{}",crate::command::canonical_json_digest(value)?))}

fn resolved_shape(shape:&Value,depth:usize)->Result<Value>{
    ensure!(depth<=64,"configuration.schema_depth_exceeded");
    if let Some(reference)=shape.get("$ref").and_then(Value::as_str){
        let name=reference.strip_prefix("#/schemas/").context("configuration.external_schema_forbidden")?;
        return resolved_shape(CONTRACT["schemas"].get(name).context("configuration.schema_reference_missing")?,depth+1)
    }
    match shape{
        Value::Object(map)=>{
            let mut output=serde_json::Map::new();
            for(key,value)in map{
                // 本合同 anyOf 仅用于互斥的 nullable 分支；复用已有闭集校验器。
                output.insert(if key=="anyOf"{"oneOf".into()}else{key.clone()},resolved_shape(value,depth+1)?);
            }
            Ok(Value::Object(output))
        },
        Value::Array(items)=>Ok(Value::Array(items.iter().map(|item|resolved_shape(item,depth+1)).collect::<Result<_>>()?)),
        value=>Ok(value.clone()),
    }
}
fn bounded(value:&Value,depth:usize)->Result<()>{
    ensure!(depth<=32,"configuration.value_too_deep");
    match value{
        Value::Object(map)=>{ensure!(map.len()<=4096,"configuration.too_many_properties");for(key,value)in map{ensure!(key.len()<=4096&&!key.contains('\0'),"configuration.key_invalid");bounded(value,depth+1)?;}},
        Value::Array(items)=>{ensure!(items.len()<=8192,"configuration.too_many_items");for value in items{bounded(value,depth+1)?;}},
        Value::String(text)=>ensure!(text.len()<=1024*1024&&!text.contains('\0'),"configuration.text_too_large"),
        _=>{},
    }
    Ok(())
}
fn validate_at(section:&str,name:&str,value:&Value,limit:usize)->Result<()>{
    ensure!(serde_json::to_vec(value)?.len()<=limit,"configuration.request_too_large");bounded(value,0)?;
    let shape=CONTRACT[section].get(name).context("configuration.shape_unknown")?;
    crate::custom_runtime::validate_closed_shape(&resolved_shape(shape,0)?,value).context("configuration.shape_invalid")
}
pub fn validate(name:&str,value:&Value)->Result<()>{validate_at("schemas",name,value,if name=="ConfigurationArchiveV2"{MAX_ARCHIVE}else{MAX_REQUEST})}
pub fn validate_operation(name:&str,value:&Value)->Result<()>{validate_at("operations",name,value,MAX_REQUEST)}

/// 保留原生配置的旧身份；只有明确选用本次连接的新配置增加这个包裹。
pub(crate) fn freeze(runtime:&mut crate::agent_profile::FrozenAgentRuntimeConfig,connection:Option<FrozenPortableConnectionV1>)->Result<()>{
    ensure!(runtime.portable_connection.is_none()&&runtime.native_origin.is_none()&&runtime.native_steer.is_none(),"configuration.intent_already_frozen");
    let Some(connection)=connection else{return Ok(())};connection.validate()?;
    ensure!(runtime.custom_api.is_none(),"configuration.native_api_overlap");
    ensure!(connection.installation_id==runtime.installation_id&&connection.installation_generation==runtime.installation_generation,"configuration.installation_changed");
    ensure!(connection.adapter_kind()?==runtime.adapter_kind,"configuration.runtime_kind_mismatch");
    runtime.host_config_digest=crate::command::canonical_json_digest(&json!({"native":runtime.host_config_digest,"portableConnection":connection}))?;
    runtime.binding_compatibility_digest=crate::command::canonical_json_digest(&json!({"native":runtime.binding_compatibility_digest,"portableConnection":connection}))?;
    runtime.portable_connection=Some(connection);runtime.refresh_config_digest()
}
