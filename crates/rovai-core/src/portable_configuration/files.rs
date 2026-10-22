//! 配置文件仍归原所有者；数据库事务只登记 outbox，提交后逐文件核对旧摘要。
use super::*;
use crate::platform::private_storage;
use rusqlite::{Connection,params};
use std::{io::Read,path::{Path,PathBuf}};

fn preferences(root:&Path)->Result<PathBuf>{
    let configured=crate::backup::owned_roots::configured(root)?;
    Ok(configured.into_iter().find(|value|value.id=="desktop_preferences"&&value.kind=="preferences").map(|value|value.path).unwrap_or_else(||root.into()))
}
pub(super) fn path(root:&Path,target:&str)->Result<PathBuf>{
    if super::channel_preferences::owns(target){let _=super::channel_preferences::read(root,target)?;return Ok(root.join("channel-adapters.json"))}
    Ok(match target{
        "messageLibrary"=>root.join("workspace-metadata/global-v1.json"),
        "taskTemplates"=>root.join("portable-configuration/task-templates-v1.json"),
        "agentSettings"=>root.join("portable-configuration/agent-settings-v1.json"),
        "appearance"=>preferences(root)?.join("appearance.json"),
        "general"=>preferences(root)?.join("general-preferences.json"),
        "windowClose"=>preferences(root)?.join("window-close.json"),
        "logging"=>preferences(root)?.join("logging.json"),
        _=>anyhow::bail!("configuration.file_target_not_admitted"),
    })
}
pub(crate) fn read(root:&Path,target:&str)->Result<Value>{
    if super::channel_preferences::owns(target){return super::channel_preferences::read(root,target)}
    let path=path(root,target)?;
    match std::fs::symlink_metadata(&path){
        Err(error)if error.kind()==std::io::ErrorKind::NotFound=>return Ok(Value::Null),
        Err(error)=>return Err(error.into()),
        Ok(metadata)=>ensure!(metadata.is_file()&&!metadata.file_type().is_symlink()&&metadata.len()<=1024*1024,"configuration.file_invalid"),
    }
    let mut file=private_storage::open_private_read_file(&path)?;let mut bytes=Vec::new();file.by_ref().take(1024*1024+1).read_to_end(&mut bytes)?;
    ensure!(bytes.len()<=1024*1024,"configuration.file_too_large");let value:Value=serde_json::from_slice(&bytes)?;super::bounded(&value,0)?;Ok(value)
}
pub(crate) fn basis(root:&Path,targets:&[String])->Result<Value>{
    let mut values=serde_json::Map::new();for target in targets{let destination=path(root,target)?;values.insert(target.clone(),json!({"ownerPath":destination,"value":read(root,target)?}));}Ok(Value::Object(values))
}
pub(crate) fn enqueue(connection:&Connection,root:&Path,command:&str,domain:&str,target:&str,before:&Value,after:&Value)->Result<()>{
    let value=json!({"before":before,"after":after,"ownerPath":path(root,target)?});ensure!(serde_json::to_vec(&value)?.len()<=MAX_REQUEST,"configuration.file_outbox_too_large");
    let now=chrono::Utc::now().to_rfc3339();connection.execute("INSERT INTO configuration_file_outbox(command_id,domain,target_id,state,expected_digest,value_json,created_at,updated_at) VALUES(?1,?2,?3,'pending',?4,?5,?6,?6)",params![command,domain,target,digest(before)?,serde_json::to_string(&value)?,now])?;Ok(())
}
pub(crate) fn statuses(connection:&Connection,command:&str)->Result<Vec<Value>>{
    let mut statement=connection.prepare("SELECT domain,target_id,state,error_code FROM configuration_file_outbox WHERE command_id=?1 ORDER BY domain,target_id")?;
    Ok(statement.query_map([command],|row|Ok(json!({"domain":row.get::<_,String>(0)?,"targetId":row.get::<_,String>(1)?,"state":row.get::<_,String>(2)?,"errorCode":row.get::<_,Option<String>>(3)?})))?.collect::<rusqlite::Result<Vec<_>>>()?)
}
fn apply_one(root:&Path,target:&str,expected:&str,value:&Value)->Result<&'static str>{
    if value["ownerPath"]!=serde_json::to_value(path(root,target)?)?{return Ok("conflict")}
    if super::channel_preferences::owns(target){return super::channel_preferences::apply(root,target,&value["before"],&value["after"])}
    let destination=path(root,target)?;let current=read(root,target)?;
    if current==value["after"]{return Ok("applied")}
    if digest(&current)?!=expected{return Ok("conflict")}
    private_storage::prepare_private_directory(destination.parent().context("configuration.file_parent_missing")?)?;
    if value["after"].is_null(){
        if !current.is_null(){let _file=private_storage::open_private_read_file(&destination)?;ensure!(read(root,target)?==current,"configuration.file_changed");std::fs::remove_file(&destination)?;}
    }else{
        ensure!(serde_json::to_vec(&value["after"])?.len()<=1024*1024,"configuration.file_too_large");
        ensure!(read(root,target)?==current,"configuration.file_changed");private_storage::atomic_write_private_json(&destination,&value["after"])?;
    }
    Ok("applied")
}
/// 调用者持有 workbench::MUTATIONS；同主机其他设置写入不能在比较和替换间穿插。
pub(crate) fn drain(connection:&Connection,root:&Path,command:&str)->Result<Vec<Value>>{
    let mut statement=connection.prepare("SELECT domain,target_id,expected_digest,value_json FROM configuration_file_outbox WHERE command_id=?1 AND state='pending' ORDER BY domain,target_id")?;
    let rows=statement.query_map([command],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;drop(statement);
    ensure!(rows.len()<=128,"configuration.file_target_limit");
    for(domain,target,expected,raw)in rows{
        // Desktop 偏好由原 Store 拥有，等待 Host 所有者接收；不从 Core 绕过其写队列。
        if matches!(target.as_str(),"appearance"|"general"|"windowClose"|"logging"){
            connection.execute("UPDATE configuration_file_outbox SET error_code='configuration.desktop_owner_required',updated_at=?4 WHERE command_id=?1 AND domain=?2 AND target_id=?3 AND state='pending'",params![command,domain,target,chrono::Utc::now().to_rfc3339()])?;continue
        }
        let value:Value=serde_json::from_str(&raw)?;
        let(state,error)=match apply_one(root,&target,&expected,&value){Ok("applied")=>("applied",None),Ok(_)=>("conflict",Some("configuration.file_changed")),Err(_)=>("failed",Some("configuration.file_write_failed"))};
        connection.execute("UPDATE configuration_file_outbox SET state=?4,error_code=?5,updated_at=?6 WHERE command_id=?1 AND domain=?2 AND target_id=?3 AND state='pending'",params![command,domain,target,state,error,chrono::Utc::now().to_rfc3339()])?;
    }
    statuses(connection,command)
}
