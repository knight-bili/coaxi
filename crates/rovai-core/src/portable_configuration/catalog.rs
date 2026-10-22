//! 仅供 User 的迁移选择目录；不回显密钥、原环境值或主机绝对路径。
use super::*;
use rusqlite::{Connection,params};
use std::path::Path;

pub(crate) fn archives(connection:&Connection,client:&str,after:Option<&str>,limit:u32)->Result<Value>{
    ensure!((1..=100).contains(&limit),"configuration.limit_invalid");if let Some(after)=after{uuid::Uuid::parse_str(after)?;}
    let mut statement=connection.prepare("SELECT id,source_digest,source_kind,metadata_json,created_at FROM configuration_import_archive WHERE client_id=?1 AND (?2 IS NULL OR id>?2) ORDER BY id LIMIT ?3")?;
    let mut items=statement.query_map(params![client,after,limit+1],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?)))?.map(|row|{let(id,digest,kind,metadata,created)=row?;let metadata:Value=serde_json::from_str(&metadata)?;Ok(json!({"archiveId":id,"sourceDigest":digest,"sourceKind":kind,"counts":metadata["counts"],"createdAt":created}))}).collect::<Result<Vec<_>>>()?;
    let more=items.len()>limit as usize;items.truncate(limit as usize);let next=more.then(||items.last().and_then(|item|item["archiveId"].as_str()).map(str::to_owned)).flatten();Ok(json!({"items":items,"next":next}))
}
pub(crate) fn targets(connection:&Connection,root:&Path,domain:&str,after:Option<&str>,limit:u32)->Result<Value>{
    ensure!((1..=100).contains(&limit),"configuration.limit_invalid");
    if matches!(domain,"modelProviders"|"customAgents"){
        let page=if domain=="modelProviders"{providers::list(connection,after,limit)?}else{crate::custom_runtime::storage::list(connection,after,limit)?};
        let items=page["items"].as_array().context("configuration.catalog_invalid")?.iter().map(|item|json!({"id":item["id"],"name":item["name"],"revision":item["revision"]})).collect::<Vec<_>>();return Ok(json!({"items":items,"next":page["next"]}))
    }
    if let Some(target)=library::file_target(domain){
        let library=files::read(root,target)?;let key=if domain=="quickMessages"{"quickMessages"}else{"items"};
        let mut items=library[key].as_array().into_iter().flatten().filter_map(|item|{
            let id=item["id"].as_str()?;if after.is_some_and(|after|id<=after){return None}
            Some(json!({"id":id,"name":item.get("name").or_else(||item.get("title")).or_else(||item.get("registryId")).unwrap_or(&Value::Null),"revision":item["revision"].as_i64().unwrap_or(1)}))
        }).collect::<Vec<_>>();items.sort_by(|left,right|left["id"].as_str().cmp(&right["id"].as_str()));let more=items.len()>limit as usize;items.truncate(limit as usize);let next=more.then(||items.last().and_then(|item|item["id"].as_str()).map(str::to_owned)).flatten();return Ok(json!({"items":items,"next":next}))
    }
    ensure!(domain=="preferences","configuration.catalog_domain_unknown");
    let entries=[("system_language_settings","语言"),("appearance_mode","主题"),("appearance_zoom_level","缩放"),("system_close_behavior_settings","关闭窗口方式")];let mut items=Vec::new();
    for channel in channel_preferences::portable(root)?{
        let channel_id=channel["sourceAdapterId"].as_str().context("configuration.channel_target_invalid")?;uuid::Uuid::parse_str(channel_id)?;
        for(key,label)in[("chat_command_prefix","命令前缀"),("chat_message_language","消息语言"),("chat_event_filter","事件通知")]{
            let id=format!("channel:{channel_id}:{key}");if after.is_some_and(|after|id.as_str()<=after){continue}
            items.push(json!({"id":id,"name":format!("{} · {label}",channel["name"].as_str().unwrap_or("本机渠道")),"revision":1}));
        }
    }
    for(key,field)in preferences::NATIVE_FIELDS{
        if after.is_some_and(|after|*key<=after){continue}
        let target=key.split('.').nth(1).context("configuration.catalog_key_invalid")?;
        if target=="windowClose"&&!cfg!(windows){continue}
        let current=files::read(root,target)?;
        if !current[*field].is_null(){items.push(json!({"id":key,"name":key,"revision":1}));}
    }
    let policy=policies::basis(connection,"delegationPolicy")?;
    for(key,name)in[("delegation.enabled","启用帮手"),("delegation.depth_limit","帮手层数"),("delegation.agent_defaults","帮手程序默认选项"),("delegation.completed_cache_max_mb","帮手缓存上限")]{
        if after.is_none_or(|after|key>after){items.push(json!({"id":key,"name":name,"revision":policy["version"]}));}
    }
    for(key,name)in entries{
        if key=="system_close_behavior_settings"&&!cfg!(windows){continue}
        if after.is_some_and(|after|key<=after){continue}
        let raw=match key{"system_language_settings"=>"{}","appearance_mode"=>"system","appearance_zoom_level"=>"100",_=>"{}"};
        let(target,patch)=preferences::file_patch(&json!({"key":key,"value":raw}))?;let current=files::read(root,target)?;
        let fields=preferences::before_fields(&current,&patch);if fields.as_object().is_some_and(|map|map.values().any(|value|!value.is_null())){items.push(json!({"id":key,"name":name,"revision":1}));}
    }
    items.sort_by(|left,right|left["id"].as_str().cmp(&right["id"].as_str()));let more=items.len()>limit as usize;items.truncate(limit as usize);let next=if more{items.last().map(|value|value["id"].clone()).unwrap_or(Value::Null)}else{Value::Null};Ok(json!({"items":items,"next":next}))
}
