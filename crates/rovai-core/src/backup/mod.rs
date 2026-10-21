//! Offline, encrypted snapshots of the unified Core data root (canvas included).
//! A Core instance lease is held for every snapshot/swap. No live DB is copied.
pub mod crypto;
pub mod webdav;
use anyhow::{Result,Context,ensure};
use serde::{Serialize,Deserialize};
use sha2::{Sha256,Digest};
use std::{fs,io::{Read,Write},path::{Path,PathBuf,Component},collections::{HashMap,HashSet}};
use crate::platform::private_storage::{prepare_private_directory,atomic_write_private_json};
use crate::core_data_dir_lock::{CoreDataDirLease,CORE_DATA_DIR_LOCK_FILE};
use tokio_util::sync::CancellationToken;
const BACKUP_I18N_KEY_BAD_PASSPHRASE:&str="backup.bad_passphrase_or_archive";
#[derive(Debug)]
pub struct BackupError{code:String,message:String}
impl std::fmt::Display for BackupError{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"{}: {}",self.code,self.message)}}
impl std::error::Error for BackupError{}
impl BackupError{
    fn io(error:impl std::fmt::Display)->Self{Self{code:"backup.io".into(),message:error.to_string()}}
    fn task_execution_failed(message:impl Into<String>)->Self{Self{code:"backup.failed".into(),message:message.into()}}
    fn authentication_failed(message:impl Into<String>)->Self{Self{code:"backup.authentication_failed".into(),message:message.into()}}
    fn invalid_input(message:impl Into<String>)->Self{Self{code:"backup.invalid_input".into(),message:message.into()}}
    fn with_detail(mut self,detail:impl AsRef<str>)->Self{self.message.push_str(": ");self.message.push_str(detail.as_ref());self}
    fn with_i18n(mut self,key:&str,_parameters:std::collections::BTreeMap<String,String>)->Self{self.code=key.into();self}
}
fn cancelled_error()->BackupError{BackupError{code:"backup.cancelled".into(),message:"备份操作已取消".into()}}
#[derive(Serialize,Deserialize,Clone)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Entry{pub path:String,pub size:u64,pub sha256:String}
#[derive(Serialize,Deserialize,Clone)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Manifest{pub format_version:u32,pub product:String,pub app_version:String,pub created_at:String,pub entries:Vec<Entry>}
#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Pending{root:PathBuf,staging:PathBuf,safety:PathBuf,manifest:Manifest,state:String,units:Vec<String>}
pub fn file_digest(path:&Path)->Result<(u64,String)>{hash_file(path)}
fn hash_file(path:&Path)->Result<(u64,String)>{let mut file=fs::File::open(path)?;let mut hash=Sha256::new();let mut bytes=0_u64;let mut buffer=[0_u8;64*1024];loop{let count=file.read(&mut buffer)?;if count==0{break}bytes+=count as u64;hash.update(&buffer[..count]);}Ok((bytes,format!("{:x}",hash.finalize())))}
fn no_link(path:&Path)->Result<()>{let metadata=fs::symlink_metadata(path)?;ensure!(!metadata.file_type().is_symlink(),"backup.linked_path: {}",path.display());#[cfg(windows)]{use std::os::windows::fs::MetadataExt;ensure!(metadata.file_attributes()&0x400==0,"backup.linked_path");}Ok(())}
fn name(value:&str)->Result<()>{ensure!(!value.is_empty()&&!value.contains(['\0',':','\\'])&&Path::new(value).components().all(|part|matches!(part,Component::Normal(_))),"backup.invalid_entry");Ok(())}
fn excluded(value:&str)->bool{value==CORE_DATA_DIR_LOCK_FILE||matches!(value.split('/').next(),Some("pets-market-cache"|"logs"|"run-tmp"|"tmp"|"crash-dumps"|"Crashpad"|"Cache"|"Code Cache"|"GPUCache"|".codegraph"))}
fn files(root:&Path)->Result<Vec<(String,PathBuf)>>{
    let mut queue=vec![root.to_path_buf()];let mut result=Vec::new();
    while let Some(directory)=queue.pop(){no_link(&directory)?;for entry in fs::read_dir(directory)?{let entry=entry?;let path=entry.path();let relative=path.strip_prefix(root)?.to_string_lossy().replace('\\',"/");if excluded(&relative){continue}name(&relative)?;no_link(&path)?;let metadata=entry.metadata()?;if metadata.is_dir(){queue.push(path)}else{ensure!(metadata.is_file(),"backup.special_file: {relative}");ensure!(result.len()<200000,"backup.entry_limit");result.push((relative,path))}}}
    result.sort_by(|a,b|a.0.cmp(&b.0));Ok(result)
}
fn administrative(root:&Path)->Result<PathBuf>{let parent=root.parent().context("backup.root_parent_required")?;let key=format!("{:x}",Sha256::digest(root.to_string_lossy().as_bytes()));let directory=parent.join(format!(".rovai-backup-{}",&key[..16]));prepare_private_directory(&directory)?;Ok(directory)}
fn progress(done:u64,total:u64,phase:&str){eprintln!("{}",serde_json::json!({"phase":phase,"done":done,"total":total}));}
pub fn create(root:&Path,destination:&Path,passphrase:&str,cancel:&CancellationToken)->Result<Manifest>{
    ensure!(passphrase.chars().count()>=8,"backup.passphrase_too_short");let lease=CoreDataDirLease::acquire(root)?;let root=lease.data_dir();
    let parent=fs::canonicalize(destination.parent().context("backup.destination_parent_required")?)?;
    ensure!(!parent.starts_with(root)&&!destination.exists(),"backup.destination_inside_data_or_exists");
    let administrative=administrative(root)?;let operation=administrative.join(uuid::Uuid::new_v4().to_string());prepare_private_directory(&operation)?;
    let result=(||->Result<Manifest>{
        let inputs=files(root)?;let total=inputs.len() as u64;let zip_path=operation.join("snapshot.zip");let file=fs::OpenOptions::new().write(true).create_new(true).open(&zip_path)?;let mut archive=zip::ZipWriter::new(file);let options=zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).unix_permissions(0o600);
        let mut manifest=Manifest{format_version:1,product:"codeg-rovai-core".into(),app_version:env!("CARGO_PKG_VERSION").into(),created_at:chrono::Utc::now().to_rfc3339(),entries:Vec::new()};
        for (index,(relative,path)) in inputs.iter().enumerate(){ensure!(!cancel.is_cancelled(),"backup.cancelled");let before=hash_file(path)?;archive.start_file(format!("data/{relative}"),options)?;let mut input=fs::File::open(path)?;std::io::copy(&mut input,&mut archive)?;ensure!(hash_file(path)?==before,"backup.file_changed: {relative}");manifest.entries.push(Entry{path:relative.clone(),size:before.0,sha256:before.1});progress(index as u64+1,total,"archive");}
        ensure!(files(root)?.iter().map(|entry|&entry.0).eq(inputs.iter().map(|entry|&entry.0)),"backup.root_changed");
        archive.start_file("manifest.json",options)?;archive.write_all(&serde_json::to_vec(&manifest)?)?;archive.finish()?.sync_all()?;
        let encrypted=operation.join("snapshot.rovai-backup");crypto::encrypt_file(&zip_path,&encrypted,passphrase,cancel)?;fs::File::open(&encrypted)?.sync_all()?;
        // Publish without replacing an existing user file, including across volumes.
        let temporary=parent.join(format!(".rovai-backup-{}.tmp",uuid::Uuid::new_v4()));
        let publish=(||->Result<()>{let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;std::io::copy(&mut fs::File::open(&encrypted)?,&mut file)?;file.sync_all()?;drop(file);fs::hard_link(&temporary,destination)?;Ok(())})();let _=fs::remove_file(&temporary);publish?;Ok(manifest)
    })();
    if operation.starts_with(&administrative){let _=fs::remove_dir_all(&operation);}result
}
fn decrypt(archive:&Path,temporary:&Path,passphrase:&str,cancel:&CancellationToken)->Result<PathBuf>{ensure!(crypto::is_encrypted(archive)?,"backup.encrypted_archive_required");let zip=temporary.join("snapshot.zip");crypto::decrypt_file(archive,&zip,passphrase,cancel)?;Ok(zip)}
fn read_manifest(archive:&mut zip::ZipArchive<fs::File>)->Result<Manifest>{let entry=archive.by_name("manifest.json")?;ensure!(entry.size()<=32*1024*1024,"backup.manifest_limit");let manifest:Manifest=serde_json::from_reader(entry)?;ensure!(manifest.format_version==1&&manifest.product=="codeg-rovai-core"&&manifest.entries.len()<=200000,"backup.unsupported_format");let mut seen=HashSet::new();let mut total=0_u64;for entry in &manifest.entries{name(&entry.path)?;ensure!(!excluded(&entry.path)&&entry.size<=8*1024*1024*1024&&entry.sha256.len()==64&&entry.sha256.bytes().all(|byte|byte.is_ascii_hexdigit())&&seen.insert(entry.path.clone()),"backup.invalid_manifest");total=total.checked_add(entry.size).context("backup.total_overflow")?;ensure!(total<=64*1024*1024*1024,"backup.total_limit");}Ok(manifest)}
pub fn inspect(root:&Path,archive:&Path,passphrase:&str,cancel:&CancellationToken)->Result<Manifest>{
    let root=fs::canonicalize(root)?;let administrative=administrative(&root)?;let temporary=administrative.join(uuid::Uuid::new_v4().to_string());prepare_private_directory(&temporary)?;
    let result=(||{let zip=decrypt(archive,&temporary,passphrase,cancel)?;let mut archive=zip::ZipArchive::new(fs::File::open(zip)?)?;read_manifest(&mut archive)})();let _=fs::remove_dir_all(&temporary);result
}
pub fn stage(root:&Path,archive:&Path,passphrase:&str,cancel:&CancellationToken)->Result<Manifest>{
    let lease=CoreDataDirLease::acquire(root)?;let root=lease.data_dir();let administrative=administrative(root)?;ensure!(!administrative.join("pending.json").exists(),"backup.restore_already_pending");
    let operation=administrative.join(uuid::Uuid::new_v4().to_string());prepare_private_directory(&operation)?;let staging=operation.join("staging");prepare_private_directory(&staging)?;
    let result=(||->Result<Manifest>{
        let zip=decrypt(archive,&operation,passphrase,cancel)?;let mut archive=zip::ZipArchive::new(fs::File::open(&zip)?)?;let manifest=read_manifest(&mut archive)?;
        let expected=manifest.entries.iter().map(|entry|(format!("data/{}",entry.path),entry)).collect::<HashMap<_,_>>();let mut seen=HashSet::new();
        for index in 0..archive.len(){ensure!(!cancel.is_cancelled(),"backup.cancelled");let mut entry=archive.by_index(index)?;if entry.name()=="manifest.json"{continue}let declared=expected.get(entry.name()).context("backup.undeclared_entry")?;ensure!(seen.insert(entry.name().to_owned())&&entry.size()==declared.size&&entry.enclosed_name().is_some()&&entry.unix_mode().is_none_or(|mode|mode&0o170000!=0o120000),"backup.invalid_zip_entry");let destination=staging.join(&declared.path);prepare_private_directory(destination.parent().context("backup.invalid_parent")?)?;let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&destination)?;let copied=std::io::copy(&mut (&mut entry).take(declared.size+1),&mut file)?;file.sync_all()?;drop(file);crate::platform::private_storage::repair_private_file(&destination)?;ensure!(copied==declared.size&&hash_file(&destination)?==(declared.size,declared.sha256.clone()),"backup.checksum_mismatch");progress(index as u64+1,manifest.entries.len() as u64,"verify");}
        ensure!(seen.len()==expected.len(),"backup.missing_entries");
        fs::remove_file(zip)?;
        let mut units=fs::read_dir(root)?.map(|entry|entry.map(|entry|entry.file_name().to_string_lossy().into_owned())).collect::<std::io::Result<HashSet<_>>>()?;
        units.extend(manifest.entries.iter().filter_map(|entry|entry.path.split('/').next().map(str::to_owned)));units.retain(|unit|!excluded(unit));let mut units=units.into_iter().collect::<Vec<_>>();units.sort();
        let safety=operation.join("before");prepare_private_directory(&safety)?;
        let pending=Pending{root:root.into(),staging,safety,manifest:manifest.clone(),state:"staged".into(),units};atomic_write_private_json(&administrative.join("pending.json"),&pending)?;Ok(manifest)
    })();
    if result.is_err()&&!administrative.join("pending.json").exists(){let _=fs::remove_dir_all(&operation);}result
}
/// Called before opening Core. Each path remains under the admitted root or its
/// private operation directory; the safety copy remains available after success.
pub fn apply_pending(root:&Path)->Result<Option<Manifest>>{let lease=CoreDataDirLease::acquire(root)?;apply_pending_with_lease(&lease)}
pub fn apply_pending_with_lease(lease:&CoreDataDirLease)->Result<Option<Manifest>>{
    let root=lease.data_dir();let key=format!("{:x}",Sha256::digest(root.to_string_lossy().as_bytes()));let administrative=root.parent().context("backup.root_parent_required")?.join(format!(".rovai-backup-{}",&key[..16]));let marker=administrative.join("pending.json");if !marker.exists(){return Ok(None)}
    prepare_private_directory(&administrative)?;
    let mut pending:Pending=serde_json::from_slice(&fs::read(&marker)?)?;ensure!(pending.root==root&&pending.staging.starts_with(&administrative)&&pending.safety.starts_with(&administrative),"backup.invalid_pending_paths");no_link(&pending.staging)?;no_link(&pending.safety)?;
    ensure!(matches!(pending.state.as_str(),"staged"|"applying"),"backup.restore_requires_recovery");pending.state="applying".into();atomic_write_private_json(&marker,&pending)?;
    for unit in &pending.units{
        name(unit)?;ensure!(!unit.contains('/')&&!excluded(unit),"backup.invalid_restore_unit");
        let live=root.join(unit);let staged=pending.staging.join(unit);let before=pending.safety.join(unit);
        let receipt=pending.safety.join(format!(".{:x}.swap.json",Sha256::digest(unit.as_bytes())));
        if !receipt.exists(){let state=serde_json::json!({"unit":unit,"hadOriginal":live.exists(),"hasReplacement":staged.exists(),"done":false});atomic_write_private_json(&receipt,&state)?;}
        let mut state:serde_json::Value=serde_json::from_slice(&fs::read(&receipt)?)?;if state["done"]==true{continue}
        if state["hadOriginal"]==true&&!before.exists(){ensure!(live.exists(),"backup.original_unit_missing");no_link(&live)?;fs::rename(&live,&before)?;}
        if state["hasReplacement"]==true&&staged.exists(){ensure!(!live.exists(),"backup.restore_destination_changed");no_link(&staged)?;fs::rename(&staged,&live)?;}
        ensure!(state["hasReplacement"]!=true||live.exists(),"backup.replacement_missing");state["done"]=serde_json::json!(true);atomic_write_private_json(&receipt,&state)?;
    }
    for entry in &pending.manifest.entries{ensure!(hash_file(&root.join(&entry.path))?==(entry.size,entry.sha256.clone()),"backup.applied_checksum_mismatch");}
    pending.state="applied".into();atomic_write_private_json(&marker,&pending)?;fs::rename(&marker,administrative.join(format!("restore-{}.json",uuid::Uuid::new_v4())))?;Ok(Some(pending.manifest))
}

/// Restore the safety snapshot of the latest applied/partly-applied operation.
pub fn rollback(root:&Path)->Result<()> {
    let lease=CoreDataDirLease::acquire(root)?;let root=lease.data_dir();let administrative=administrative(root)?;
    let pending=administrative.join("pending.json");
    let marker=if pending.exists(){pending}else{let mut candidates=fs::read_dir(&administrative)?.filter_map(|entry|entry.ok()).filter(|entry|entry.file_name().to_string_lossy().starts_with("restore-")&&entry.path().extension().is_some_and(|extension|extension=="json")).collect::<Vec<_>>();candidates.sort_by_key(|entry|entry.metadata().and_then(|metadata|metadata.modified()).ok());candidates.pop().context("backup.no_restore_to_rollback")?.path()};
    let mut operation:Pending=serde_json::from_slice(&fs::read(&marker)?)?;ensure!(operation.root==root&&operation.safety.starts_with(&administrative)&&operation.staging.starts_with(&administrative)&&matches!(operation.state.as_str(),"applying"|"applied"|"rolling_back"),"backup.invalid_rollback");no_link(&operation.safety)?;let after=operation.safety.parent().context("backup.invalid_operation")?.join("after");prepare_private_directory(&after)?;
    operation.state="rolling_back".into();atomic_write_private_json(&marker,&operation)?;
    for unit in operation.units.iter().rev(){
        name(unit)?;ensure!(!unit.contains('/')&&!excluded(unit),"backup.invalid_unit");let receipt=operation.safety.join(format!(".{:x}.swap.json",Sha256::digest(unit.as_bytes())));if !receipt.exists(){continue}
        let mut state:serde_json::Value=serde_json::from_slice(&fs::read(&receipt)?)?;if state["rolledBack"]==true{continue}
        let live=root.join(unit);let before=operation.safety.join(unit);let saved=after.join(unit);
        if before.exists(){if live.exists(){ensure!(!saved.exists(),"backup.rollback_destination_changed");no_link(&live)?;fs::rename(&live,&saved)?;}no_link(&before)?;fs::rename(&before,&live)?;}
        else if state["hadOriginal"]==false&&live.exists(){ensure!(!saved.exists(),"backup.rollback_destination_changed");no_link(&live)?;fs::rename(&live,&saved)?;}
        state["rolledBack"]=serde_json::json!(true);atomic_write_private_json(&receipt,&state)?;
    }
    operation.state="rolled_back".into();atomic_write_private_json(&marker,&operation)?;
    if marker.file_name().is_some_and(|name|name=="pending.json"){fs::rename(marker,administrative.join(format!("rollback-{}.json",uuid::Uuid::new_v4())))?;}Ok(())
}
