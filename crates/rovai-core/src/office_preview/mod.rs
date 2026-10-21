//! Per-open authority for Codeg's ref-counted OfficeCLI watch processes.
mod watch;
pub use watch::{acquire_sse_lease,release_sse_lease,note_proxy_request};
use anyhow::{Result,Context,ensure};
use serde_json::{Value,json};
use std::{collections::HashMap,path::{Path,PathBuf,Component},sync::{LazyLock,Mutex},time::{Instant,Duration}};
struct Lease {scope:String,root:String,path:String,port:u16,underlying:String,cap:String,editable:bool,last:Instant}
static LEASES:LazyLock<Mutex<HashMap<String,Lease>>>=LazyLock::new(Default::default);
static START:tokio::sync::Mutex<()>=tokio::sync::Mutex::const_new(());
static SWEEP:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
fn leases()->std::sync::MutexGuard<'static,HashMap<String,Lease>>{LEASES.lock().unwrap_or_else(|error|error.into_inner())}
fn view(id:&str,lease:&Lease)->Value{json!({"holdId":id,"port":lease.port,"cap":lease.cap,"editable":lease.editable})}
pub fn validate_watch_cap(port:u16,cap:&str)->bool{leases().values().any(|lease|lease.port==port&&lease.cap==cap&&lease.last.elapsed()<Duration::from_secs(180)&&watch::validate_watch_cap(port,&lease.underlying))}
pub fn allows_edit(port:u16,cap:&str)->bool{leases().values().any(|lease|lease.port==port&&lease.cap==cap&&lease.editable&&lease.last.elapsed()<Duration::from_secs(180)&&watch::validate_watch_cap(port,&lease.underlying))}
pub fn detect()->Value{json!({"installed":resolve_officecli().is_some(),"formats":["docx","xlsx","pptx"]})}
pub async fn start(scope:&str,id:&str,root:&Path,path:&str,editable:bool)->Result<Value>{
    uuid::Uuid::parse_str(id)?;
    let _start=START.lock().await;
    {
        let mut leases=leases();
        if let Some(lease)=leases.get_mut(id){ensure!(lease.scope==scope&&lease.root==root.to_string_lossy()&&lease.path==path&&lease.editable==editable,"office.hold_identity_changed");ensure!(watch::validate_watch_cap(lease.port,&lease.underlying),"office.original_watch_expired");lease.last=Instant::now();return Ok(view(id,lease));}
        ensure!(leases.len()<128,"office.lease_limit");
    }
    let _=resolve_tree_path(root,path)?;
    let started=watch::start_office_watch_core(root.to_string_lossy().into_owned(),path.into()).await.map_err(|error|anyhow::anyhow!("office.{}: {error}",error.code()))?;
    let lease=Lease{scope:scope.into(),root:root.to_string_lossy().into_owned(),path:path.into(),port:started.port,underlying:started.cap,cap:uuid::Uuid::new_v4().simple().to_string(),editable,last:Instant::now()};
    let result=view(id,&lease);leases().insert(id.into(),lease);
    if !SWEEP.swap(true,std::sync::atomic::Ordering::AcqRel){tokio::spawn(async{loop{tokio::time::sleep(Duration::from_secs(30)).await;let expired={let mut leases=leases();let ids=leases.iter().filter(|(_,lease)|lease.last.elapsed()>=Duration::from_secs(180)).map(|(id,_)|id.clone()).collect::<Vec<_>>();ids.into_iter().filter_map(|id|leases.remove(&id)).collect::<Vec<_>>()};for lease in expired{let _=watch::stop_office_watch_core(lease.root,lease.path).await;}watch::sweep_office_watches(Duration::from_secs(300));}});}
    Ok(result)
}
pub fn heartbeat(scope:&str,id:&str)->Result<Value>{let mut leases=leases();let lease=leases.get_mut(id).context("office.lease_expired")?;ensure!(lease.scope==scope&&watch::validate_watch_cap(lease.port,&lease.underlying),"office.lease_expired");lease.last=Instant::now();Ok(view(id,lease))}
pub async fn release(scope:&str,id:&str)->Result<Value>{let lease={let mut leases=leases();if let Some(lease)=leases.get(id){ensure!(lease.scope==scope,"office.hold_identity_changed")};leases.remove(id)};if let Some(lease)=lease{watch::stop_office_watch_core(lease.root,lease.path).await?;}Ok(json!({"released":true}))}
pub fn stop_all(){leases().clear();watch::stop_all_office_watches();}
fn is_office_path(path:&Path)->bool{path.extension().and_then(|part|part.to_str()).is_some_and(|part|matches!(part.to_ascii_lowercase().as_str(),"docx"|"xlsx"|"pptx"))}
fn resolve_tree_path(root:&Path,path:&str)->Result<PathBuf>{
    ensure!(!path.is_empty()&&!path.contains(['\0',':','\\']),"office.invalid_path");let canonical=std::fs::canonicalize(root)?;let mut target=canonical.clone();
    for part in Path::new(path).components(){match part{Component::Normal(name)=>{target.push(name);let metadata=std::fs::symlink_metadata(&target)?;ensure!(!metadata.file_type().is_symlink(),"office.linked_path");#[cfg(windows)]{use std::os::windows::fs::MetadataExt;ensure!(metadata.file_attributes()&0x400==0,"office.linked_path");}},_=>anyhow::bail!("office.invalid_path")}}
    let target=std::fs::canonicalize(target)?;ensure!(target.starts_with(canonical)&&target.is_file(),"office.outside_root");Ok(target)
}
fn resolve_officecli()->Option<PathBuf>{
    let executable=if cfg!(windows){"officecli.exe"}else{"officecli"};
    if let Some(paths)=std::env::var_os("PATH"){for path in std::env::split_paths(&paths){if path.is_absolute(){let candidate=path.join(executable);if candidate.is_file(){return Some(candidate)}}}}
    #[cfg(windows)]{if let Some(root)=std::env::var_os("LOCALAPPDATA"){let path=PathBuf::from(root).join("OfficeCLI").join(executable);if path.is_file(){return Some(path)}}}
    #[cfg(not(windows))]{if let Some(root)=dirs::home_dir(){let path=root.join(".local/bin").join(executable);if path.is_file(){return Some(path)}}}
    None
}
fn tokio_command(program:impl AsRef<std::ffi::OsStr>)->tokio::process::Command{
    let mut command=tokio::process::Command::new(program);command.env_clear();
    for key in ["PATH","SystemRoot","WINDIR","HOME","USERPROFILE","TEMP","TMP","LOCALAPPDATA","DOTNET_ROOT"]{if let Some(value)=std::env::var_os(key){command.env(key,value);}}
    command.env("LANG","C.UTF-8").env("LC_ALL","C.UTF-8");
    #[cfg(windows)]command.creation_flags(0x0800_0000);
    command
}
