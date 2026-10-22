use super::*;
use crate::platform::private_storage;
use rusqlite::{Connection,OptionalExtension,params};
use std::{io::Read,path::{Path,PathBuf}};

fn context(id:&str,client:&str)->String{format!("portable-archive:{id}:{client}")}
fn directory(root:&Path)->Result<PathBuf>{
    let parent=root.join("portable-configuration");private_storage::prepare_private_directory(&parent)?;
    Ok(private_storage::prepare_private_directory(&parent.join("archives"))?)
}

/// 外层在有界读取和解密后调用；没有安装、运行或成员配置副作用。
pub(crate) fn register(connection:&Connection,root:&Path,client:&str,id:&str,parsed:archive::ParsedArchive)->Result<Value>{
    ensure!(!client.is_empty()&&client.len()<=1024,"configuration.client_required");
    uuid::Uuid::parse_str(id)?;
    if let Some(previous)=receipt(connection,client,id)?{
        ensure!(previous["sourceDigest"]==parsed.source_digest,"configuration.archive_operation_changed");return Ok(previous)
    }
    let count:i64=connection.query_row("SELECT COUNT(*) FROM configuration_import_archive WHERE client_id=?1",[client],|row|row.get(0))?;
    ensure!(count<1024,"configuration.archive_count_limit");
    let content_digest=digest(&parsed.original)?;
    let metadata=json!({"schemaVersion":1,"contentDigest":content_digest,"counts":parsed.domains.iter().map(|(name,value)|(name.clone(),value.as_array().map_or_else(||value.as_object().map_or(0,|map|map.len()),|rows|rows.len()))).collect::<std::collections::BTreeMap<_,_>>()});
    let cipher=crate::member_environment::encrypt(connection,&context(&id,client),&parsed.original)?;
    ensure!(cipher.len()<=MAX_ARCHIVE+28,"configuration.archive_too_large");
    let relative=format!("portable-configuration/archives/{id}.bin");let path=directory(root)?.join(format!("{id}.bin"));
    private_storage::atomic_write_private_bytes(&path,&cipher)?;
    connection.execute("INSERT INTO configuration_import_archive(id,client_id,source_digest,source_kind,relative_path,metadata_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,client,parsed.source_digest,parsed.source_kind,relative,serde_json::to_string(&metadata)?,chrono::Utc::now().to_rfc3339()])?;
    Ok(json!({"archiveId":id,"sourceDigest":parsed.source_digest,"sourceKind":parsed.source_kind,"counts":metadata["counts"]}))
}

pub(crate) fn receipt(connection:&Connection,client:&str,id:&str)->Result<Option<Value>>{
    uuid::Uuid::parse_str(id)?;
    let row:Option<(String,String,String)>=connection.query_row("SELECT source_digest,source_kind,metadata_json FROM configuration_import_archive WHERE id=?1 AND client_id=?2",params![id,client],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    row.map(|(digest,kind,metadata)|->Result<_>{let metadata:Value=serde_json::from_str(&metadata)?;Ok(json!({"archiveId":id,"sourceDigest":digest,"sourceKind":kind,"counts":metadata["counts"]}))}).transpose()
}

pub(crate) fn retained(connection:&Connection,root:&Path,client:&str,id:&str,expected:&str)->Result<archive::ParsedArchive>{
    uuid::Uuid::parse_str(id)?;
    let(source,kind,relative,metadata):(String,String,String,String)=connection.query_row("SELECT source_digest,source_kind,relative_path,metadata_json FROM configuration_import_archive WHERE id=?1 AND client_id=?2",params![id,client],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).context("configuration.archive_not_owned")?;
    ensure!(source==expected&&relative==format!("portable-configuration/archives/{id}.bin"),"configuration.archive_changed");
    let path=directory(root)?.join(format!("{id}.bin"));let mut file=private_storage::open_private_read_file(&path)?;
    ensure!(file.metadata()?.len()<=MAX_ARCHIVE as u64+28,"configuration.archive_too_large");let mut cipher=Vec::new();file.by_ref().take(MAX_ARCHIVE as u64+29).read_to_end(&mut cipher)?;
    ensure!(cipher.len()<=MAX_ARCHIVE+28,"configuration.archive_too_large");
    let original:Value=crate::member_environment::decrypt(connection,&context(id,client),&cipher)?;
    let metadata:Value=serde_json::from_str(&metadata)?;
    ensure!(metadata["contentDigest"].as_str()==Some(digest(&original)?.as_str()),"configuration.archive_content_changed");
    // 已保存的是解密后的完整源 JSON；不保存原导入口令，也不再次询问口令。
    let mut parsed=archive::parse_retained(&serde_json::to_vec(&original)?)?;
    ensure!(parsed.source_kind==kind,"configuration.archive_kind_changed");parsed.source_digest=source;Ok(parsed)
}
