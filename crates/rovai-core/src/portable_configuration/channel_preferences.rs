//! 渠道偏好必须明确选择本机渠道。凭据仍归原文件，outbox 只携带无秘密的投影。
use super::*;
use std::{io::Read,path::Path};
const FIELDS:[&str;3]=["commandPrefix","messageLanguage","notificationKinds"];
pub(crate) fn owns(target:&str)->bool{target.starts_with("channel:")}
fn id(target:&str)->Result<&str>{let value=target.strip_prefix("channel:").context("configuration.channel_target_invalid")?;uuid::Uuid::parse_str(value)?;Ok(value)}
fn raw(root:&Path)->Result<Value>{
    let path=root.join("channel-adapters.json");if !path.exists(){return Ok(json!([]))}
    let mut file=crate::platform::private_storage::open_private_read_file(&path)?;
    ensure!(file.metadata()?.len()<=4*1024*1024,"configuration.channel_file_too_large");let mut bytes=Vec::new();file.by_ref().take(4*1024*1024+1).read_to_end(&mut bytes)?;
    ensure!(bytes.len()<=4*1024*1024,"configuration.channel_file_too_large");let value:Value=serde_json::from_slice(&bytes)?;ensure!(value.as_array().is_some_and(|rows|rows.len()<=512),"configuration.channel_file_invalid");Ok(value)
}
fn projected(value:&Value)->Result<Value>{
    Ok(json!({"schemaVersion":1,"id":value["id"],"name":value["name"],"recordDigest":digest(value)?,"commandPrefix":value.get("commandPrefix").cloned().unwrap_or(json!("/")),"messageLanguage":value.get("messageLanguage").cloned().unwrap_or(json!("zh-cn")),"notificationKinds":value.get("notificationKinds").cloned().unwrap_or(json!([]))}))
}
pub(crate) fn read(root:&Path,target:&str)->Result<Value>{let key=id(target)?;let value=raw(root)?;value.as_array().unwrap().iter().find(|row|row["id"]==key).map(projected).transpose().map(|value|value.unwrap_or(Value::Null))}
pub(crate) fn portable(root:&Path)->Result<Vec<Value>>{
    Ok(raw(root)?.as_array().unwrap().iter().map(|row|json!({"sourceAdapterId":row["id"],"name":row["name"],"commandPrefix":row.get("commandPrefix").cloned().unwrap_or(json!("/")),"messageLanguage":row.get("messageLanguage").cloned().unwrap_or(json!("zh-cn")),"notificationKinds":row.get("notificationKinds").cloned().unwrap_or(json!([]))})).collect())
}
fn patch(entry:&Value)->Result<Option<Value>>{
    let key=entry["key"].as_str().context("configuration.preference_key_required")?;let value=entry["value"].as_str().context("configuration.preference_value_required")?;
    let(field,mapped)=match key{
        "chat_command_prefix"=>{ensure!(!value.is_empty()&&value.len()<=8&&!value.chars().any(|value|value.is_whitespace()||value.is_control()),"configuration.channel_prefix_invalid");("commandPrefix",json!(value))},
        "chat_message_language"=>{let language=value.to_ascii_lowercase();ensure!(["en","zh-cn","zh-tw","ja","ko","es","de","fr","pt","ar"].contains(&language.as_str()),"configuration.channel_language_invalid");("messageLanguage",json!(language))},
        "chat_event_filter"=>{
            let mut selected:Value=serde_json::from_str(value).map_err(|_|anyhow::anyhow!("configuration.channel_filter_invalid"))?;
            if selected.is_null(){selected=json!(["approval","message","round","collaboration","mission","task"])}
            let rows=selected.as_array().context("configuration.channel_filter_invalid")?;
            ensure!(rows.len()<=6&&rows.iter().all(|value|value.as_str().is_some_and(|value|["approval","message","round","collaboration","mission","task"].contains(&value))),"configuration.channel_filter_needs_explicit_mapping");
            let mut seen=std::collections::BTreeSet::new();ensure!(rows.iter().all(|value|seen.insert(value.as_str().unwrap())),"configuration.channel_filter_duplicate");("notificationKinds",selected)
        },
        _=>return Ok(None),
    };
    let mut result=serde_json::Map::new();result.insert(field.into(),mapped);Ok(Some(Value::Object(result)))
}
pub(crate) fn selected(entry:&Value,decision:&Value)->Result<Option<(String,Value)>>{
    let Some(patch)=patch(entry)?else{return Ok(None)};
    ensure!(decision["action"]=="replace"&&decision["expectedRevision"]==1,"configuration.channel_explicit_target_required");
    let value=decision["targetId"].as_str().context("configuration.channel_explicit_target_required")?;
    let target=value.strip_suffix(&format!(":{}",entry["key"].as_str().unwrap())).context("configuration.channel_target_field_mismatch")?;id(target)?;
    Ok(Some((target.into(),patch)))
}
fn apply_fields(row:&mut Value,value:&Value)->Result<()>{
    ensure!(value.as_object().is_some(),"configuration.channel_projection_invalid");
    for field in FIELDS{if let Some(value)=value.get(field){row[field]=value.clone();}}
    // 复用原渠道设置相同枚举和限值；不复制 token、地址、发送人、绑定或权限。
    patch(&json!({"key":"chat_command_prefix","value":row["commandPrefix"].as_str().context("configuration.channel_prefix_invalid")?}))?;
    patch(&json!({"key":"chat_message_language","value":row["messageLanguage"].as_str().context("configuration.channel_language_invalid")?}))?;
    patch(&json!({"key":"chat_event_filter","value":serde_json::to_string(&row["notificationKinds"])?}))?;Ok(())
}
pub(crate) fn merge(root:&Path,target:&str,original:&Value,patch:&Value)->Result<Value>{
    let key=id(target)?;ensure!(original["id"]==key,"configuration.channel_target_changed");let values=raw(root)?;
    let mut row=values.as_array().unwrap().iter().find(|row|row["id"]==key).context("configuration.channel_missing")?.clone();
    apply_fields(&mut row,original)?;apply_fields(&mut row,patch)?;projected(&row)
}
pub(crate) fn apply(root:&Path,target:&str,before:&Value,after:&Value)->Result<&'static str>{
    let key=id(target)?;let current=read(root,target)?;if current==*after{return Ok("applied")}
    if current!=*before{return Ok("conflict")}
    let mut values=raw(root)?;let row=values.as_array_mut().unwrap().iter_mut().find(|row|row["id"]==key).context("configuration.channel_missing")?;
    apply_fields(row,after)?;ensure!(projected(row)?==*after,"configuration.channel_projection_changed");
    ensure!(read(root,target)?==current,"configuration.channel_changed");
    crate::platform::private_storage::atomic_write_private_json(&root.join("channel-adapters.json"),&values)?;Ok("applied")
}
