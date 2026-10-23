//! 客户确认的会话模型工具补齐 r1；与原 Built-in 授权分别管理。
use anyhow::{Context,Result,ensure};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use std::sync::LazyLock;

mod model;
mod schema;
pub mod policy;
pub(crate) mod scope;
pub(crate) mod requests;
pub(crate) mod sources;
pub(crate) mod source_projection;
pub(crate) mod grants;
pub(crate) mod questions;
pub(crate) mod question_delivery;
pub mod user_commands;
pub(crate) mod authoring;
pub(crate) mod calendar;
pub mod automation_owner;
pub mod user_api;
pub mod storage;
pub use model::*;
pub const SERVER_NAME:&str="rovai-conversation-tools";
pub const MAX_REQUEST:usize=128*1024;
pub const MAX_RESULT:usize=256*1024;
static CONTRACT:LazyLock<Value>=LazyLock::new(||serde_json::from_str(include_str!("contract.json")).expect("confirmed conversation model tools r1 contract"));

pub fn digest(value:&Value)->Result<String>{Ok(format!("sha256:{}",crate::command::canonical_json_digest(value)?))}
pub fn validate(name:&str,value:&Value)->Result<()>{
    ensure!(serde_json::to_vec(value)?.len()<=MAX_RESULT,"conversation_tools.value_too_large");
    schema::validate(&CONTRACT,CONTRACT["schemas"].get(name).context("conversation_tools.schema_missing")?,value,0)
}
pub fn validate_operation(name:&str,value:&Value)->Result<()>{
    ensure!(serde_json::to_vec(value)?.len()<=MAX_REQUEST,"conversation_tools.request_too_large");
    let schema=CONTRACT["userOperations"].get(name).context("conversation_tools.operation_unknown")?;
    schema::validate(&CONTRACT,&schema["input"],value,0)
}
pub(crate) fn validate_operation_output(name:&str,value:&Value)->Result<()>{
    let operation=CONTRACT["userOperations"].get(name).context("conversation_tools.operation_unknown")?;
    ensure!(serde_json::to_vec(value)?.len()<=MAX_RESULT,"conversation_tools.result_too_large");
    schema::validate(&CONTRACT,&operation["output"],value,0)
}
pub(crate) fn tool_definition(name:&str)->Result<&'static Value>{
    CONTRACT["modelTools"].as_array().context("conversation_tools.catalog_invalid")?.iter().find(|tool|tool["name"]==name).context("conversation_tools.tool_unknown")
}
pub(crate) fn validate_tool(name:&str,value:&Value)->Result<()>{
    ensure!(serde_json::to_vec(value)?.len()<=MAX_REQUEST,"conversation_tools.request_too_large");
    let definition=tool_definition(name)?;schema::validate(&definition["inputSchema"],&definition["inputSchema"],value,0)
}
/// 同一结果对象同时生成结构化内容和规范 JSON 文本，禁止双份结果漂移。
pub(crate) fn result(value:Value,error:bool)->Result<Value>{
    let mut bytes=Vec::new();crate::command::write_canonical_json(&mut bytes,&value)?;
    ensure!(bytes.len()<=MAX_RESULT,"conversation_tools.result_too_large");
    Ok(json!({"isError":error,"structuredContent":value,"content":[{"type":"text","text":String::from_utf8(bytes)?}]}))
}
