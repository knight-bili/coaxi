//! 看板只读公开价目。与 Runtime 模型目录、凭据和已冻结的成本证据分开。
use super::*;
use anyhow::ensure;
use serde::{Deserialize,Serialize};
use chrono::{DateTime,Utc};
const SOURCE:&str="https://models.dev/api.json";
const LIMIT:usize=32*1024*1024;
static REFRESH:tokio::sync::Mutex<()>=tokio::sync::Mutex::const_new(());
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Request{#[serde(default)]refresh:bool}
#[derive(Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Cache{version:u32,fetched_at:String,origin_digest:String,content_digest:String,providers:Value}
fn cached(path:&Path)->Result<Option<Cache>>{
    if !path.exists(){return Ok(None)}let metadata=std::fs::symlink_metadata(path)?;ensure!(metadata.is_file()&&!metadata.file_type().is_symlink()&&metadata.len()as usize<=LIMIT,"usage_prices.invalid_cache");
    let cache:Cache=serde_json::from_slice(&std::fs::read(path)?)?;ensure!(cache.version==1&&crate::command::canonical_json_digest(&cache.providers)?==cache.content_digest,"usage_prices.invalid_cache");DateTime::parse_from_rfc3339(&cache.fetched_at)?;Ok(Some(cache))
}
fn public_value(cache:Option<&Cache>,error:Option<String>)->Value{
    let stale=cache.is_some_and(|cache|DateTime::parse_from_rfc3339(&cache.fetched_at).map(|date|Utc::now().signed_duration_since(date.with_timezone(&Utc))>chrono::Duration::hours(24)).unwrap_or(true));
    json!({"source":SOURCE,"available":cache.is_some(),"fetchedAt":cache.map(|cache|cache.fetched_at.as_str()),"contentDigest":cache.map(|cache|cache.content_digest.as_str()),"originDigest":cache.map(|cache|cache.origin_digest.as_str()),"providers":cache.map(|cache|cache.providers.clone()).unwrap_or_else(||json!([])),"stale":stale,"refreshError":error,"ratesArePerMillionUsd":true,"actualBill":false})
}
fn positive(value:&Value)->Value{value.as_f64().filter(|value|value.is_finite()&&*value>=0.0).map(|value|json!(value)).unwrap_or(Value::Null)}
fn project(value:Value)->Result<Value>{
    let mut providers=Vec::new();let source=value.as_object().context("usage_prices.invalid_catalog")?;ensure!(source.len()<=1000,"usage_prices.catalog_too_large");let mut count=0;
    for(id,provider)in source{ensure!(id.len()<=512,"usage_prices.invalid_identity");let Some(models)=provider["models"].as_object()else{continue};let mut list=Vec::new();
        for(key,model)in models{count+=1;ensure!(count<=100000&&key.len()<=1024,"usage_prices.catalog_too_large");let cost=&model["cost"];list.push(json!({"id":model["id"].as_str().unwrap_or(key),"name":model["name"].as_str().unwrap_or(key),"cost_in":positive(&cost["input"]),"cost_out":positive(&cost["output"]),"cost_cache_read":positive(&cost["cache_read"]),"cost_cache_write":positive(&cost["cache_write"])}));}
        providers.push(json!({"id":id,"name":provider["name"].as_str().unwrap_or(id),"models":list}));
    }ensure!(!providers.is_empty(),"usage_prices.empty_catalog");Ok(json!(providers))
}
impl Core {
    pub(super) async fn usage_price_catalog(&self,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value)?;let path=self.data_dir.join("usage-price-catalog-v1.json");let existing=cached(&path);
        if !request.refresh{return match existing{Ok(cache)=>Ok(public_value(cache.as_ref(),None)),Err(error)=>Ok(public_value(None,Some(error.to_string())))}}
        let _guard=REFRESH.lock().await;
        let fetched=async{
            let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(20)).build()?;
            let mut response=client.get(SOURCE).header(reqwest::header::ACCEPT,"application/json").send().await?.error_for_status()?;
            ensure!(response.content_length().is_none_or(|length|length<=LIMIT as u64),"usage_prices.catalog_too_large");let mut bytes=Vec::new();while let Some(chunk)=response.chunk().await?{ensure!(bytes.len().saturating_add(chunk.len())<=LIMIT,"usage_prices.catalog_too_large");bytes.extend_from_slice(&chunk)}
            use sha2::{Digest,Sha256};let origin_digest=format!("{:x}",Sha256::digest(&bytes));let providers=project(serde_json::from_slice(&bytes)?)?;let cache=Cache{version:1,fetched_at:Utc::now().to_rfc3339(),origin_digest,content_digest:crate::command::canonical_json_digest(&providers)?,providers};
            crate::platform::private_storage::atomic_write_private_json(&path,&cache)?;Ok::<Cache,anyhow::Error>(cache)
        }.await;
        match fetched{Ok(cache)=>Ok(public_value(Some(&cache),None)),Err(error)=>{let cache=existing.ok().flatten();Ok(public_value(cache.as_ref(),Some(error.to_string())))}}
    }
}
