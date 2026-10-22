//! 客户确认的原生继续、分叉与插话 r1；新输入只绑定明确选择的新执行。
use anyhow::{Context,Result,ensure};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use std::sync::LazyLock;

pub mod model;
pub mod storage;
pub(crate) mod fork;
pub mod policy;
pub use model::*;
const MAX_REQUEST_BYTES:usize=2*1024*1024;
static CONTRACT:LazyLock<Value>=LazyLock::new(||serde_json::from_str(include_str!("contract.json")).expect("confirmed native continuation fork steer r1 contract"));

pub fn digest(value:&Value)->Result<String>{Ok(format!("sha256:{}",crate::command::canonical_json_digest(value)?))}
pub fn validate_shape(name:&str,value:&Value)->Result<()>{
    ensure!(serde_json::to_vec(value)?.len()<=MAX_REQUEST_BYTES,"native_lifecycle.request_too_large");
    crate::custom_runtime::validate_closed_shape(CONTRACT["schemas"].get(name).context("native_lifecycle.shape_unknown")?,value)
        .context("native_lifecycle.shape_invalid")
}
pub fn validate_operation(name:&str,value:&Value)->Result<()>{
    ensure!(serde_json::to_vec(value)?.len()<=MAX_REQUEST_BYTES,"native_lifecycle.request_too_large");
    crate::custom_runtime::validate_closed_shape(CONTRACT["operations"].get(name).context("native_lifecycle.operation_unknown")?,value)
        .context("native_lifecycle.operation_invalid")
}

/// 必须从没有原生意图的基础配置冻结；重复包裹不能产生新的身份。
pub(crate) fn freeze_intent(runtime:&mut crate::agent_profile::FrozenAgentRuntimeConfig,origin:Option<NativeSessionOriginV1>,steer:Option<FrozenNativeSteerV1>)->Result<()>{
    ensure!(runtime.native_origin.is_none()&&runtime.native_steer.is_none(),"native_lifecycle.intent_already_frozen");
    if let Some(origin)=&origin{origin.validate()?}
    if let Some(steer)=&steer{steer.validate()?}
    if origin.is_none()&&steer.is_none(){return Ok(())}
    let mut identity=serde_json::Map::new();
    if let Some(origin)=&origin{identity.insert("nativeOrigin".into(),serde_json::to_value(origin)?);}
    if let Some(steer)=&steer{identity.insert("nativeSteer".into(),serde_json::to_value(steer)?);}
    runtime.host_config_digest=crate::command::canonical_json_digest(&json!({"native":runtime.host_config_digest,"nativeIntent":identity}))?;
    runtime.binding_compatibility_digest=crate::command::canonical_json_digest(&json!({"native":runtime.binding_compatibility_digest,"nativeIntent":identity}))?;
    runtime.native_origin=origin;runtime.native_steer=steer;runtime.refresh_config_digest()
}
