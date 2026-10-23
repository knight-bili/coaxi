//! Explicit offline program switch. Acquiring the data lease never opens SQLite.
use anyhow::{Context,Result,ensure};
use clap::{Args,ValueEnum};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::{fs,io::Read,path::{Path,PathBuf}};

#[derive(Clone,Copy,ValueEnum)]
pub enum Action{List,Apply,Recover}
#[derive(Args)]
pub struct Arguments{
    #[arg(long,value_enum)]pub action:Action,
    #[arg(long)]pub directory:PathBuf,
    #[arg(long)]pub data_dir:Option<PathBuf>,
    #[arg(long)]pub snapshot:Option<String>,
    #[arg(long)]pub expected_current:Option<String>,
    #[arg(long)]pub expected_target:Option<String>,
    #[arg(long)]pub confirmed:bool,
}
#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Pending{
    version:u32,id:String,selected:String,current_digest:String,
    target_digest:String,holding:String,phase:String,
}
fn no_link(path:&Path)->Result<()>{
    let metadata=fs::symlink_metadata(path)?;
    ensure!(!metadata.file_type().is_symlink(),"program_recovery.link_not_admitted");
    #[cfg(windows)]{use std::os::windows::fs::MetadataExt;ensure!(metadata.file_attributes()&0x400==0,"program_recovery.reparse_not_admitted");}
    Ok(())
}
fn digest(path:&Path)->Result<String>{
    no_link(path)?;let mut input=fs::File::open(path)?;let mut sha=Sha256::new();let mut buffer=[0u8;65536];
    loop{let n=input.read(&mut buffer)?;if n==0{break}sha.update(&buffer[..n]);}
    Ok(format!("{:x}",sha.finalize()))
}
fn metadata(root:&Path)->Result<(String,Value)>{
    let path=root.join("manifest.json");no_link(&path)?;
    ensure!(fs::metadata(&path)?.len()<=16*1024*1024,"program_recovery.manifest_limit");
    let value:Value=serde_json::from_slice(&fs::read(&path)?)?;
    ensure!(value["schemaVersion"]==1&&value["productId"]=="codeg-rovai-workbench"&&value["files"].is_object(),"program_recovery.product_mismatch");
    Ok((digest(&path)?,value))
}
fn verify(root:&Path)->Result<String>{
    let(identity,manifest)=metadata(root)?;let files=manifest["files"].as_object().context("program_recovery.manifest_invalid")?;
    ensure!(files.len()<=200000,"program_recovery.file_limit");
    for(relative,hash)in files{
        ensure!(!relative.is_empty()&&!relative.contains(['\\',':','\0'])&&Path::new(relative).components().all(|part|matches!(part,std::path::Component::Normal(_))),"program_recovery.path_invalid");
        let mut path=root.to_path_buf();for part in Path::new(relative).components(){path.push(part);no_link(&path)?;}
        let actual=digest(&path)?;ensure!(Some(actual.as_str())==hash.as_str(),"program_recovery.package_changed: {relative}");
    }
    ensure!(digest(&root.join("manifest.json"))?==identity,"program_recovery.manifest_changed");Ok(identity)
}
fn snapshot(root:&Path,id:&str)->Result<PathBuf>{
    ensure!(!id.is_empty()&&!id.contains(['/',':','\\','\0']),"program_recovery.invalid_snapshot");
    let path=if id.len()==64&&id.bytes().all(|c|c.is_ascii_hexdigit()){
        let revisions=root.join("revisions");no_link(&revisions)?;revisions.join(id)
    }else{
        ensure!(id.starts_with(".previous-")&&id.len()==46&&id[10..].bytes().all(|c|c.is_ascii_hexdigit()||c==b'-'),"program_recovery.invalid_snapshot");root.join(id)
    };
    no_link(&path)?;ensure!(path.is_dir(),"program_recovery.snapshot_missing");Ok(path)
}
fn current(root:&Path)->Result<PathBuf>{
    let path=root.join("current");
    #[cfg(unix)]{
        if fs::symlink_metadata(&path)?.file_type().is_symlink(){
            let selected=fs::read_link(&path)?;
            ensure!(!selected.is_absolute()&&selected.components().all(|c|matches!(c,std::path::Component::Normal(_)))&&selected.parent()==Some(Path::new("revisions")),"program_recovery.current_link_invalid");
            return snapshot(root,selected.file_name().and_then(|s|s.to_str()).context("program_recovery.current_link_invalid")?)
        }
    }
    no_link(&path)?;Ok(path)
}
fn holding(id:&str)->String{format!(".previous-{}-{}-{}-{}-{}",&id[..8],&id[8..12],&id[12..16],&id[16..20],&id[20..32])}
fn store(root:&Path,value:&Pending)->Result<()>{rovai_core::platform::atomic_write_private_bytes(&root.join("program-recovery-pending-v1.json"),&serde_json::to_vec_pretty(value)?)}
fn finish(root:&Path,value:&Pending,outcome:&str)->Result<Value>{
    let directory=root.join("program-recovery-history");rovai_core::platform::prepare_private_directory(&directory)?;
    let result=json!({"operationId":value.id,"outcome":outcome,"snapshot":value.selected,"businessDataChanged":false,"requiresCompatibleCoreSchema":true});
    rovai_core::platform::atomic_write_private_bytes(&directory.join(format!("{}.json",value.id)),&serde_json::to_vec_pretty(&result)?)?;
    fs::remove_file(root.join("program-recovery-pending-v1.json"))?;Ok(result)
}
fn recover(root:&Path)->Result<Value>{
    let path=root.join("program-recovery-pending-v1.json");if !path.exists(){return Ok(json!({"recovered":false}))}
    no_link(&path)?;ensure!(fs::metadata(&path)?.len()<65536,"program_recovery.pending_limit");let value:Pending=serde_json::from_slice(&fs::read(path)?)?;
    ensure!(value.version==1&&value.id.len()==64&&value.id.bytes().all(|c|c.is_ascii_hexdigit())&&value.holding==holding(&value.id),"program_recovery.pending_identity_invalid");
    let current_path=root.join("current");let previous=root.join(&value.holding);
    if let Ok(active)=current(root){let found=verify(&active)?;if found==value.target_digest{return finish(root,&value,"applied")}if found==value.current_digest{return finish(root,&value,"not_applied")}}
    if !current_path.try_exists()?&&previous.try_exists()?{
        no_link(&previous)?;ensure!(verify(&previous)?==value.current_digest,"program_recovery.previous_changed");fs::rename(&previous,&current_path)?;return finish(root,&value,"rolled_back")
    }
    anyhow::bail!("program_recovery.manual_intervention_required; original copies retained")
}
fn native_program()->&'static str{if cfg!(windows){"rovai-server.exe"}else{"rovai-server"}}
pub fn run(arguments:Arguments)->Result<()>{
    ensure!(arguments.directory.is_absolute()&&arguments.directory.components().all(|c|!matches!(c,std::path::Component::ParentDir|std::path::Component::CurDir)),"program_recovery.absolute_directory_required");
    no_link(&arguments.directory)?;let root=fs::canonicalize(&arguments.directory)?;let marker=root.join("INSTALLER-V1");no_link(&marker)?;
    ensure!(fs::read_to_string(marker)?.trim()=="codeg-rovai-workbench-server","program_recovery.installation_owner_required");
    if matches!(arguments.action,Action::List){
        let active=current(&root)?;let(current_digest,info)=metadata(&active)?;let mut items=Vec::new();
        for parent in [root.clone(),root.join("revisions")]{
            if !parent.is_dir(){continue}no_link(&parent)?;
            for entry in fs::read_dir(parent)?.take(1001){let entry=entry?;let id=entry.file_name().to_string_lossy().to_string();if let Ok(path)=snapshot(&root,&id){if path==active{continue}let item=match metadata(&path){Ok((digest,manifest))=>json!({"id":id,"manifestDigest":digest,"version":manifest["version"],"target":manifest["target"]}),Err(error)=>json!({"id":id,"unavailable":true,"message":error.to_string()})};items.push(item)}}
        }
        println!("{}",json!({"current":{"manifestDigest":current_digest,"version":info["version"],"target":info["target"]},"snapshots":items,"businessDataChanged":false}));return Ok(())
    }
    ensure!(arguments.confirmed,"program_recovery.confirmation_required");
    let data=arguments.data_dir.as_deref().context("program_recovery.data_directory_required")?;
    ensure!(data.is_absolute()&&!fs::canonicalize(data)?.starts_with(&root),"program_recovery.data_must_be_outside_program");
    let _data_lease=rovai_core::core_data_dir_lock::CoreDataDirLease::acquire(data)?;
    let lock_path=root.join(".program-recovery.lock");if lock_path.exists(){no_link(&lock_path)?;}
    let lock=fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(lock_path)?;lock.try_lock().context("program_recovery.operation_in_progress")?;
    let active=current(&root).ok();let program=active.as_ref().map(|p|fs::File::open(p.join(native_program()))).transpose()?;
    if let Some(program)=&program{program.try_lock().context("program_recovery.stop_this_server_first")?;}
    if matches!(arguments.action,Action::Recover){println!("{}",recover(&root)?);return Ok(())}
    ensure!(!root.join("program-recovery-pending-v1.json").exists(),"program_recovery.recovery_required");
    let active=active.context("program_recovery.current_missing")?;let selected=arguments.snapshot.as_deref().context("program_recovery.snapshot_required")?;let target=snapshot(&root,selected)?;
    ensure!(target!=active,"program_recovery.already_current");let before=verify(&active)?;let after=verify(&target)?;
    ensure!(Some(before.as_str())==arguments.expected_current.as_deref()&&Some(after.as_str())==arguments.expected_target.as_deref(),"program_recovery.preview_changed");
    let(_,a)=metadata(&active)?;let(_,b)=metadata(&target)?;ensure!(a["target"]==b["target"],"program_recovery.platform_mismatch");
    // A copied maintenance CLI outside current/target is required on Windows.
    #[cfg(windows)]{let executable=fs::canonicalize(std::env::current_exe()?)?;ensure!(!executable.starts_with(&active)&&!executable.starts_with(&target),"program_recovery.copy_host_cli_outside_program_before_switch");}
    let id=format!("{:x}",Sha256::digest(format!("{before}:{after}:{}:{}",std::process::id(),chrono::Utc::now().to_rfc3339()).as_bytes()));
    let mut pending=Pending{version:1,holding:holding(&id),id,selected:selected.into(),current_digest:before,target_digest:after,phase:"prepared".into()};
    #[cfg(unix)]{
        if fs::symlink_metadata(root.join("current"))?.file_type().is_symlink(){
            ensure!(selected.len()==64,"program_recovery.managed_revision_required");store(&root,&pending)?;
            let next=root.join(format!(".current-{}",pending.id));std::os::unix::fs::symlink(Path::new("revisions").join(selected),&next)?;fs::rename(next,root.join("current"))?;
            pending.phase="applied".into();store(&root,&pending)?;println!("{}",finish(&root,&pending,"applied")?);return Ok(())
        }
    }
    store(&root,&pending)?;fs::rename(root.join("current"),root.join(&pending.holding))?;
    pending.phase="current_moved".into();store(&root,&pending)?;fs::rename(&target,root.join("current"))?;
    pending.phase="applied".into();store(&root,&pending)?;println!("{}",finish(&root,&pending,"applied")?);Ok(())
}
