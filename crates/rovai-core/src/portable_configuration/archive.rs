//! 原配置只做有界解码，未知域和源行原文交给受管加密归档保留。
use super::*;
use std::collections::BTreeMap;
use sha2::{Digest,Sha256};

pub const CODEG_DOMAINS:[&str;6]=["modelProviders","agentSettings","customAgents","quickMessages","taskTemplates","preferences"];

#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
pub(super) struct CodegProvider{
    pub name:String,pub api_url:String,pub api_key:String,#[serde(default)]pub agent_types_json:String,
    pub agent_type:String,#[serde(default)]pub model:Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
pub(super) struct CodegProviderRef{pub agent_type:String,pub name:String}
#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
pub(super) struct CodegAgentSetting{
    pub agent_type:String,#[serde(default)]pub registry_id:String,pub enabled:bool,#[serde(default)]pub sort_order:i32,
    #[serde(default)]pub env_json:Option<String>,#[serde(default)]pub provider:Option<CodegProviderRef>,
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
pub(super) struct CodegCustomAgent{
    pub registry_id:String,pub name:String,#[serde(default)]pub description:String,#[serde(default)]pub version:String,
    #[serde(default)]pub distribution_kind:String,#[serde(default)]pub spec_json:String,#[serde(default)]pub icon_url:Option<String>,
    #[serde(default)]pub skills_shared_store:bool,#[serde(default)]pub source:String,#[serde(default)]pub version_probe:Option<String>,#[serde(default)]pub supports_mcp:bool,
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
pub(super) struct CodegQuickMessage{pub title:String,pub content:String,#[serde(default)]pub sort_order:i32}
#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
pub(super) struct CodegTaskTemplate{pub name:String,#[serde(default)]pub title:String,#[serde(default)]pub config:String}

pub(crate) struct ParsedArchive{
    pub source_kind:&'static str,pub source_digest:String,pub original:Value,pub domains:BTreeMap<String,Value>,
}
pub(crate) struct Entry{pub id:String,pub domain:String,pub index:usize,pub value:Value}

fn rows<T:serde::de::DeserializeOwned>(value:&Value)->Result<()>{
    if value.is_null(){return Ok(())}
    let values=value.as_array().context("configuration.domain_rows_invalid")?;ensure!(values.len()<=8192,"configuration.domain_too_many_rows");
    for value in values{let _:T=serde_json::from_value(value.clone()).map_err(|_|anyhow::anyhow!("configuration.domain_row_invalid"))?;}Ok(())
}
fn check_codeg_domain(name:&str,value:&Value)->Result<()>{
    match name{
        "modelProviders"=>rows::<CodegProvider>(value),"agentSettings"=>rows::<CodegAgentSetting>(value),
        "customAgents"=>rows::<CodegCustomAgent>(value),"quickMessages"=>rows::<CodegQuickMessage>(value),"taskTemplates"=>rows::<CodegTaskTemplate>(value),
        "preferences"=>{ensure!(value.is_null()||value.as_object().is_some_and(|map|map.values().all(Value::is_string)),"configuration.preference_values_invalid");Ok(())},
        _=>Ok(()),
    }
}
pub(crate) fn parse(bytes:&[u8],passphrase:&str,manifest:Option<&Value>)->Result<ParsedArchive>{
    ensure!(bytes.len()<=MAX_ARCHIVE,"configuration.archive_too_large");
    let source_digest=format!("sha256:{:x}",Sha256::digest(bytes));
    let mut value:Value=serde_json::from_slice(bytes).map_err(|_|anyhow::anyhow!("configuration.archive_json_invalid"))?;
    let encrypted=value.get("codegConfigEncryption").is_some();
    if let Some(manifest)=manifest{
        ensure!(serde_json::to_vec(manifest)?.len()<=64*1024&&manifest["schemaVersion"]==1,"configuration.manifest_invalid");
        ensure!(manifest["config"]["size"].as_u64()==Some(bytes.len()as u64)&&manifest["config"]["sha256"].as_str()==Some(source_digest.trim_start_matches("sha256:")),"configuration.manifest_content_changed");
        ensure!(manifest["encryption"].as_str()==Some(if encrypted{"aes-256-gcm"}else{"none"}),"configuration.manifest_encryption_mismatch");
    }
    if encrypted{let plain=super::codeg_crypto::decrypt(value,passphrase)?;value=serde_json::from_slice(&plain).map_err(|_|anyhow::anyhow!("configuration.archive_json_invalid"))?;}
    super::bounded(&value,0)?;
    let source_kind=if value["schemaVersion"]==1&&value.get("formatVersion").is_none(){"codeg_v1"}
    else if value["formatVersion"]==1&&value["product"]=="codeg-rovai-config"{"fusion_v1"}
    else if value["formatVersion"]==2&&value["product"]=="codeg-rovai-config"{validate("ConfigurationArchiveV2",&value)?;"fusion_v2"}
    else{anyhow::bail!("configuration.archive_version_unsupported")};
    let map=value["domains"].as_object().context("configuration.domains_required")?;ensure!(map.len()<=32,"configuration.too_many_domains");
    let mut total=0usize;
    for(name,domain)in map{
        ensure!(name.len()<=128,"configuration.domain_name_invalid");
        total=total.checked_add(domain.as_array().map_or_else(||domain.as_object().map_or(0,|map|map.len()),|items|items.len())).context("configuration.too_many_entries")?;
        ensure!(total<=8192,"configuration.too_many_entries");
        if source_kind=="codeg_v1"{check_codeg_domain(name,domain)?;}
    }
    let domains=map.iter().map(|(name,value)|(name.clone(),value.clone())).collect();
    Ok(ParsedArchive{source_kind,source_digest,original:value,domains})
}
impl ParsedArchive{
    pub(crate) fn entries(&self)->Result<Vec<Entry>>{
        let mut entries=Vec::new();
        for(domain,value)in &self.domains{
            if let Some(rows)=value.as_array(){for(index,value)in rows.iter().enumerate(){entries.push(Entry{id:digest(&json!({"source":self.source_digest,"domain":domain,"index":index}))?,domain:domain.clone(),index,value:value.clone()});}}
            else if let Some(map)=value.as_object(){for(index,(key,value))in map.iter().enumerate(){entries.push(Entry{id:digest(&json!({"source":self.source_digest,"domain":domain,"index":index,"key":key}))?,domain:domain.clone(),index,value:json!({"key":key,"value":value})});}}
        }
        ensure!(entries.len()<=8192,"configuration.too_many_entries");Ok(entries)
    }
}
