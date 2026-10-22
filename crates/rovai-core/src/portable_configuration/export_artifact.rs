//! 客户端归属的冻结导出。口令只用于主机私有 HMAC 与内存加密，落盘均为密文。
use super::*;
use rusqlite::Connection;
use std::{io::Read,path::{Path,PathBuf}};
use crate::platform::private_storage;

#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct Prepare{pub operation_id:String,pub include_secrets:bool,pub passphrase:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ReadExport{pub operation_id:String}
pub(crate) struct Frozen{pub directory:PathBuf,pub snapshot:Value,pub metadata:Value,pub passphrase:String}
fn directory(root:&Path,client:&str,id:&str)->Result<PathBuf>{
    uuid::Uuid::parse_str(id)?;
    let client=digest(&json!(client))?;Ok(root.join("portable-configuration/exports").join(client.trim_start_matches("sha256:")).join(id))
}
fn read_json(path:&Path,limit:usize)->Result<Value>{
    let value=read_bytes(path,limit)?;Ok(serde_json::from_slice(&value)?)
}
fn read_bytes(path:&Path,limit:usize)->Result<Vec<u8>>{
    let metadata=std::fs::symlink_metadata(path)?;ensure!(metadata.is_file()&&!metadata.file_type().is_symlink()&&metadata.len()<=limit as u64,"configuration.export_file_invalid");
    let mut file=private_storage::open_private_read_file(path)?;let mut bytes=Vec::new();file.by_ref().take(limit as u64+1).read_to_end(&mut bytes)?;ensure!(bytes.len()<=limit,"configuration.export_file_too_large");Ok(bytes)
}
fn hash(bytes:&[u8])->String{use sha2::{Digest,Sha256};format!("sha256:{:x}",Sha256::digest(bytes))}
fn public(metadata:&Value)->Value{json!({"operationId":metadata["operationId"],"archiveId":metadata["archiveId"],"createdAt":metadata["createdAt"],"includeSecrets":metadata["includeSecrets"],"state":metadata["state"],"encryptedDigest":metadata["encryptedDigest"],"byteSize":metadata["byteSize"]})}

/// 调用方持有配置互斥与数据库锁；在释放锁前冻结原快照和请求证据。
pub(crate) fn freeze(connection:&Connection,root:&Path,client:&str,input:Prepare)->Result<Frozen>{
    ensure!(input.passphrase.chars().count()>=8&&input.passphrase.len()<=16000,"configuration.export_passphrase_invalid");
    let path=directory(root,client,&input.operation_id)?;
    let request=serde_json::to_vec(&json!({"includeSecrets":input.include_secrets,"passphrase":input.passphrase}))?;
    let context=format!("configuration-export:{client}:{}",input.operation_id);
    let hmac=crate::member_environment::private_input_hmac(connection,&context,&request)?;
    let receipt=path.join("receipt.json");let frozen=path.join("snapshot.bin");
    let(snapshot,metadata)=if receipt.exists(){
        let metadata=read_json(&receipt,64*1024)?;
        ensure!(metadata["requestHmac"].as_str()==Some(hmac.as_str())&&metadata["operationId"]==input.operation_id&&metadata["includeSecrets"]==input.include_secrets,"configuration.export_original_input_required");
        let cipher=read_bytes(&frozen,MAX_ARCHIVE+28)?;
        ensure!(metadata["snapshotCipherDigest"].as_str()==Some(hash(&cipher).as_str()),"configuration.export_snapshot_changed");
        let snapshot:Value=crate::member_environment::decrypt(connection,&context,&cipher)?;
        ensure!(metadata["snapshotDigest"].as_str()==Some(digest(&snapshot)?.as_str()),"configuration.export_snapshot_changed");(snapshot,metadata)
    }else{
        ensure!(!frozen.exists()&&!path.join("configuration.rovai-config").exists(),"configuration.export_preparation_incomplete");
        let snapshot=super::export::collect(connection,root,input.include_secrets)?;
        let cipher=crate::member_environment::encrypt(connection,&context,&snapshot)?;
        private_storage::prepare_private_directory(&path)?;
        private_storage::atomic_write_private_bytes(&frozen,&cipher)?;
        let metadata=json!({"schemaVersion":1,"operationId":input.operation_id,"archiveId":snapshot["archiveId"],"createdAt":snapshot["createdAt"],"includeSecrets":input.include_secrets,"requestHmac":hmac,"snapshotDigest":digest(&snapshot)?,"snapshotCipherDigest":hash(&cipher),"state":"prepared","encryptedDigest":null,"byteSize":null});
        private_storage::atomic_write_private_json(&receipt,&metadata)?;(snapshot,metadata)
    };
    Ok(Frozen{directory:path,snapshot,metadata,passphrase:input.passphrase})
}
pub(crate) fn finish(mut frozen:Frozen)->Result<Value>{
    let output=frozen.directory.join("configuration.rovai-config");
    let cipher=if output.exists(){
        let bytes=read_bytes(&output,MAX_ARCHIVE)?;
        if frozen.metadata["state"]=="ready"{
            ensure!(frozen.metadata["encryptedDigest"].as_str()==Some(hash(&bytes).as_str())&&frozen.metadata["byteSize"].as_u64()==Some(bytes.len()as u64),"configuration.export_artifact_changed");
        }else{
            let plain=crate::backup::crypto::decrypt_bytes(&bytes,&frozen.passphrase,MAX_ARCHIVE).map_err(|_|anyhow::anyhow!("configuration.export_artifact_changed"))?;
            let observed:Value=serde_json::from_slice(&plain)?;ensure!(observed==frozen.snapshot,"configuration.export_artifact_changed");
        }
        bytes
    }else{
        ensure!(frozen.metadata["state"]=="prepared","configuration.export_artifact_missing");
        let bytes=crate::backup::crypto::encrypt_bytes(&serde_json::to_vec(&frozen.snapshot)?,&frozen.passphrase,MAX_ARCHIVE).map_err(|_|anyhow::anyhow!("configuration.export_encryption_failed"))?;
        private_storage::atomic_write_private_bytes(&output,&bytes)?;bytes
    };
    frozen.metadata["state"]=json!("ready");frozen.metadata["encryptedDigest"]=json!(hash(&cipher));frozen.metadata["byteSize"]=json!(cipher.len());
    private_storage::atomic_write_private_json(&frozen.directory.join("receipt.json"),&frozen.metadata)?;Ok(public(&frozen.metadata))
}
pub(crate) fn read(root:&Path,client:&str,input:ReadExport,with_path:bool)->Result<Value>{
    let path=directory(root,client,&input.operation_id)?;let receipt=path.join("receipt.json");
    if !receipt.exists(){return Ok(json!({"state":"unknown","operationId":input.operation_id}))}
    let metadata=read_json(&receipt,64*1024)?;ensure!(metadata["operationId"]==input.operation_id,"configuration.export_receipt_changed");
    let mut result=public(&metadata);
    if with_path{
        ensure!(metadata["state"]=="ready","configuration.export_not_ready");let artifact=path.join("configuration.rovai-config");let bytes=read_bytes(&artifact,MAX_ARCHIVE)?;
        ensure!(metadata["encryptedDigest"].as_str()==Some(hash(&bytes).as_str())&&metadata["byteSize"].as_u64()==Some(bytes.len()as u64),"configuration.export_artifact_changed");
        result["artifactPath"]=serde_json::to_value(artifact)?;
    }
    Ok(result)
}
