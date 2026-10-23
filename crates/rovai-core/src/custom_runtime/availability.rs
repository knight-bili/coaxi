//! 供用户检查恢复后的本机产物；不启动程序，也不修改历史安装锁。
use super::*;
use std::{fs,io::Read,path::Path};

fn unresolved(root:&Path,prefix:&str)->Result<usize>{
    let path=root.join("custom-runtime-restore-needs-environment-v1.json");
    if let Err(error)=fs::symlink_metadata(&path){if error.kind()==std::io::ErrorKind::NotFound{return Ok(0)}return Err(error.into())}
    let mut bytes=Vec::new();crate::platform::private_storage::open_private_read_file(&path)?.take(4*1024*1024+1).read_to_end(&mut bytes)?;
    ensure!(bytes.len()<=4*1024*1024,"custom_runtime.restore_report_too_large");
    let value:Value=serde_json::from_slice(&bytes)?;
    ensure!(value["schemaVersion"]==1&&value["automaticExecutionAllowed"]==false,"custom_runtime.restore_report_invalid");
    let rows=value["unresolvedLinks"].as_array().context("custom_runtime.restore_report_invalid")?;
    ensure!(rows.len()<=200000,"custom_runtime.restore_report_too_large");
    Ok(rows.iter().filter(|row|row.pointer("/link/path").and_then(Value::as_str).is_some_and(|path|path.starts_with(prefix))).count())
}

pub(crate) async fn inspect(root:&Path,reference:&CustomRuntimeRefV1,platform:&str,operation:Option<&str>,lock:Option<&Value>)->Result<Value>{
    let Some((operation,lock))=operation.zip(lock)else{return Ok(json!({"status":"not_installed","unresolvedLinkCount":0,"qualification":"not_verified"}))};
    uuid::Uuid::parse_str(operation)?;validate("launchLock",lock)?;
    let root=root.to_path_buf();let prefix=format!("custom-runtimes/{}/{}/{}/{operation}/payload/",reference.id,reference.definition_revision,platform);
    let payload=root.join(prefix.trim_end_matches('/'));let lock=lock.clone();
    tokio::task::spawn_blocking(move||{
        let count=match unresolved(&root,&prefix){Ok(count)=>count,Err(_)=>return json!({"status":"unreadable","unresolvedLinkCount":null,"qualification":"not_verified"})};
        let current=artifacts::tree_digest(&payload,Path::new(lock["executable"].as_str().unwrap_or_default())).is_ok_and(|observed|lock["artifactDigest"]==observed);
        json!({"status":if current{"artifacts_current"}else{"needs_environment"},"unresolvedLinkCount":if current{0}else{count},"qualification":"not_verified"})
    }).await.map_err(Into::into)
}
