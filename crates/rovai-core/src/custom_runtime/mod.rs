//! 客户确认的自定义智能体接入 r1。定义、秘密引用和安装锁由 Core 拥有。
use anyhow::{Context,Result,ensure};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use std::sync::LazyLock;

pub mod storage;
pub(crate) mod artifacts;
pub(crate) mod packages;
pub(crate) mod installer;
pub(crate) mod launch;
mod schema;
pub const MAX_BYTES:usize=2*1024*1024;
pub const MAX_DEFINITIONS:i64=4096;
static CONTRACT:LazyLock<Value>=LazyLock::new(||serde_json::from_str(include_str!("contract.json")).expect("confirmed custom ACP r1 contract"));

pub type CustomRuntimeDefinitionV1=Value;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct CustomRuntimeRefV1{pub id:String,pub definition_revision:i64,pub definition_digest:String}
impl CustomRuntimeRefV1{
    pub fn validate(&self)->Result<()>{validate("reference",&serde_json::to_value(self)?)}
    pub fn from_definition(value:&Value)->Result<Self>{let result=Self{id:text(value,"id")?.into(),definition_revision:value["revision"].as_i64().context("custom_runtime.revision_invalid")?,definition_digest:text(value,"definitionDigest")?.into()};result.validate()?;Ok(result)}
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct FrozenCustomRuntimeV1{
    pub schema_version:u32,pub reference:CustomRuntimeRefV1,pub registry_id:String,pub source:String,
    pub distribution_kind:String,pub resolved_definition:CustomRuntimeDefinitionV1,pub launch_lock_digest:String,
    pub declared_mcp:bool,pub declared_skill_roots:Vec<String>,pub observed_capabilities_digest:Option<String>,
}
pub fn text<'a>(value:&'a Value,key:&str)->Result<&'a str>{value[key].as_str().context("custom_runtime.field_invalid")}
pub fn digest(value:&Value)->Result<String>{Ok(format!("sha256:{}",crate::command::canonical_json_digest(value)?))}
pub fn validate(name:&str,value:&Value)->Result<()>{ensure!(serde_json::to_vec(value)?.len()<=MAX_BYTES,"custom_runtime.definition_too_large");schema::validate(CONTRACT["schemas"].get(name).context("custom_runtime.shape_unknown")?,value,0)}
pub fn validate_operation(operation:&str,value:&Value)->Result<()>{ensure!(serde_json::to_vec(value)?.len()<=MAX_BYTES,"custom_runtime.request_too_large");schema::validate(CONTRACT["operations"].get(operation).and_then(|v|v.get("input")).context("custom_runtime.operation_unknown")?,value,0)}
pub fn validate_definition(value:&Value,input:bool)->Result<()>{
    validate(if input{"definitionInput"}else{"definitionView"},value)?;
    let kind=text(value,"distributionKind")?;let spec=&value["spec"];
    ensure!(match kind{"npx"=>spec["npx"].is_object()&&spec["uvx"].is_null()&&spec["binary"].as_object().is_some_and(|v|v.is_empty()),"uvx"=>spec["uvx"].is_object()&&spec["npx"].is_null()&&spec["binary"].as_object().is_some_and(|v|v.is_empty()),"binary"=>spec["npx"].is_null()&&spec["uvx"].is_null()&&spec["binary"].as_object().is_some_and(|v|!v.is_empty()),_=>false},"custom_runtime.distribution_invalid");
    if let Some(directory)=value["skillsDir"].as_str(){ensure!(std::path::Path::new(directory).is_absolute(),"custom_runtime.skill_directory_not_absolute")}
    if let Some(platforms)=spec["binary"].as_object(){for platform in platforms.values(){let url=reqwest::Url::parse(text(platform,"archive")?).context("custom_runtime.archive_url_invalid")?;ensure!(url.scheme()=="https"&&url.host_str().is_some()&&url.username().is_empty()&&url.password().is_none(),"custom_runtime.archive_https_required")}}
    Ok(())
}
/// 指针由 Core 固定生成，秘密的归属包括定义、分发通道及环境变量名。
pub(crate) fn environments(value:&Value)->Vec<String>{
    let mut paths=Vec::new();for kind in ["npx","uvx"]{if value["spec"][kind].is_object(){paths.push(format!("/spec/{kind}/env"))}}
    if let Some(platforms)=value["spec"]["binary"].as_object(){for platform in platforms.keys(){paths.push(format!("/spec/binary/{platform}/env"))}}paths
}
pub fn default_definition()->Value{json!({"schemaVersion":1,"id":uuid::Uuid::new_v4().to_string(),"registryId":"custom-agent","name":"","description":"","version":"1.0.0","distributionKind":"npx","spec":{"npx":{"package":"","args":[],"env":{},"cmd":null,"nodeRequired":null},"uvx":null,"binary":{}},"iconUrl":null,"skillsSharedStore":false,"skillsDir":null,"source":"manual","sourceRef":null,"versionProbe":null,"supportsMcp":true,"enabled":true})}
