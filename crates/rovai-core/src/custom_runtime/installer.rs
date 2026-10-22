use super::*;
use super::packages::{Log,PackageEntry};
use std::{collections::BTreeSet,fs,path::{Path,PathBuf},sync::Arc,time::Duration};
use tokio_util::sync::CancellationToken;
use crate::{command::{ActorRef,CommandEnvelope,CommandHandlerResult,DomainCommand,DomainCommandGateway,sealed},db::Database};
use rusqlite::params;
use rusqlite::OptionalExtension;
static MUTATION:LazyLock<Arc<tokio::sync::Mutex<()>>>=LazyLock::new(||Arc::new(tokio::sync::Mutex::new(())));
static ACTIVE:LazyLock<std::sync::Mutex<BTreeSet<PathBuf>>>=LazyLock::new(Default::default);
pub fn platform()->Result<&'static str>{match(std::env::consts::OS,std::env::consts::ARCH){("windows","x86_64")=>Ok("windows-x64"),("windows","aarch64")=>Ok("windows-arm64"),("macos","x86_64")=>Ok("macos-x64"),("macos","aarch64")=>Ok("macos-arm64"),("linux","x86_64")=>Ok("linux-x64"),("linux","aarch64")=>Ok("linux-arm64"),_=>anyhow::bail!("custom_runtime.platform_unsupported")}}
pub(crate) fn receipt_path(root:&Path,id:&str)->Result<PathBuf>{uuid::Uuid::parse_str(id)?;Ok(root.join("custom-runtime-operations").join(format!("{id}.json")))}
pub fn receipt(root:&Path,id:&str)->Result<Value>{
    let path=receipt_path(root,id)?;ensure!(fs::metadata(&path)?.len()<=MAX_BYTES as u64,"custom_runtime.receipt_too_large");let mut value:Value=serde_json::from_slice(&fs::read(&path)?)?;
    if value["state"]=="running"&&!ACTIVE.lock().map_err(|_|anyhow::anyhow!("custom_runtime.install_lock"))?.contains(&path){value["state"]=json!("unknown");value["error"]=json!("安装任务已中断，原请求不会自动重新执行。请核对原安装产物。")}Ok(value)
}
fn binary_spec<'a>(definition:&'a Value,platform:&str)->Result<(&'a Value,String)>{
    let keys=match platform{"windows-x64"=>["windows-x86_64","x86_64-pc-windows-msvc"],"windows-arm64"=>["windows-aarch64","aarch64-pc-windows-msvc"],"macos-x64"=>["darwin-x86_64","x86_64-apple-darwin"],"macos-arm64"=>["darwin-aarch64","aarch64-apple-darwin"],"linux-x64"=>["linux-x86_64","x86_64-unknown-linux-gnu"],"linux-arm64"=>["linux-aarch64","aarch64-unknown-linux-gnu"],_=>anyhow::bail!("custom_runtime.platform_unsupported")};
    for key in keys{if let Some(value)=definition["spec"]["binary"].get(key){return Ok((value,format!("/spec/binary/{key}/env")))}}anyhow::bail!("custom_runtime.binary_platform_missing")
}
pub(crate) fn environment_path(definition:&Value,platform:&str)->Result<String>{match text(definition,"distributionKind")?{"npx"=>Ok("/spec/npx/env".into()),"uvx"=>Ok("/spec/uvx/env".into()),"binary"=>binary_spec(definition,platform).map(|v|v.1),_=>anyhow::bail!("custom_runtime.distribution_invalid")}}
fn skill_roots(definition:&Value)->Result<Vec<String>>{let mut paths=Vec::new();if let Some(path)=definition["skillsDir"].as_str(){paths.push(path.to_owned())}if definition["skillsSharedStore"]==true{paths.push(dirs::home_dir().context("custom_runtime.home_missing")?.join(".agents/skills").to_string_lossy().into_owned())}Ok(paths)}
async fn install(root:&Path,definition:&Value,reference:&CustomRuntimeRefV1,platform:&str,id:&str,log:Log)->Result<Value>{
    let directory=root.join("custom-runtimes").join(&reference.id).join(reference.definition_revision.to_string()).join(platform).join(id);ensure!(!directory.exists(),"custom_runtime.artifact_already_exists");crate::platform::private_storage::prepare_private_directory(&directory)?;let payload=directory.join("payload");fs::create_dir(&payload)?;let cancel=CancellationToken::new();
    let entry=match text(definition,"distributionKind")?{
        "npx"=>packages::install_npx(&payload,&definition["spec"]["npx"],log.clone(),cancel).await?,
        "uvx"=>packages::install_uvx(&payload,&definition["spec"]["uvx"],log.clone(),cancel).await?,
        "binary"=>{let(spec,_)=binary_spec(definition,platform)?;let archive=directory.join("download.archive");artifacts::download(text(spec,"archive")?,&archive,spec["sha256"].as_str(),&cancel).await?;let command=text(spec,"cmd")?;let executable=artifacts::extract(&archive,&payload,command)?;PackageEntry{executable,arguments:spec["args"].as_array().context("custom_runtime.arguments_invalid")?.iter().map(|v|v.as_str().map(str::to_owned).context("custom_runtime.arguments_invalid")).collect::<Result<_>>()?,package:None,version:text(definition,"version")?.into()}},
        _=>anyhow::bail!("custom_runtime.distribution_invalid")};
    if let Some(probe)=definition["versionProbe"].as_str(){
        let mut args=packages::words(probe)?;let first=args.first().context("custom_runtime.version_probe_invalid")?;
        if !first.starts_with('-'){let name=Path::new(first).file_name().and_then(|v|v.to_str()).context("custom_runtime.version_probe_invalid")?;let actual=entry.executable.file_name().and_then(|v|v.to_str()).unwrap_or_default();let spec=definition.pointer(environment_path(definition,platform)?.trim_end_matches("/env")).context("custom_runtime.spec_missing")?;let declared=spec["cmd"].as_str().unwrap_or_else(||entry.package.as_deref().and_then(|v|v.rsplit('/').next()).unwrap_or(actual));ensure!(name==actual||name==declared,"custom_runtime.version_probe_program_mismatch");args.remove(0);}
        let mut command=packages::command(&entry.executable);if definition["distributionKind"]=="npx"{command.arg(entry.arguments.first().context("custom_runtime.npm_entry_missing")?);}command.args(args).current_dir(&payload);crate::managed_operation::run_bounded_command(command,"custom-acp-version-probe",Duration::from_secs(15),CancellationToken::new(),log.clone()).await?;
    }
    let environment=definition.pointer(&environment_path(definition,platform)?).context("custom_runtime.environment_missing")?;
    let lock=json!({"schemaVersion":1,"definition":reference,"platform":platform,"executable":entry.executable,"arguments":entry.arguments,"environmentDigest":digest(environment)?,"artifactDigest":artifacts::tree_digest(&payload,&entry.executable)?,"resolvedPackage":entry.package,"resolvedVersion":entry.version,"skillRoots":skill_roots(definition)?,"supportsMcpDeclared":definition["supportsMcp"],"protocolCapabilitiesDigest":null});validate("launchLock",&lock)?;
    crate::platform::private_storage::atomic_write_private_json(&directory.join("launch-lock.json"),&lock)?;Ok(lock)
}
#[derive(Serialize)]#[serde(rename_all="camelCase")]
struct Complete{reference:CustomRuntimeRefV1,lock:Value}
impl sealed::Sealed for Complete{}
impl DomainCommand for Complete{const TYPE:&'static str="runtime.custom.install.complete";const EXACT_REPLAY:bool=true;}
fn register(database:&mut Database,reference:CustomRuntimeRefV1,lock:Value,id:&str)->Result<Value>{
    let fingerprint=crate::agent_runtime_adapter::executable_fingerprint(Path::new(text(&lock,"executable")?))?;
    let snapshot=crate::agent_runtime_adapter::AgentRuntimeAdapterRegistry::default().light_ready_snapshot(crate::agent_profile::AdapterKind::CustomAcp,Some(text(&lock,"resolvedVersion")?.into()),fingerprint,chrono::Utc::now().to_rfc3339())?;
    let envelope=CommandEnvelope{command_id:format!("{id}-complete"),actor:ActorRef::System{component_id:"custom-acp-installer".into()},camp_id:None,expected_versions:Vec::new(),execution_epoch:None,payload:Complete{reference:reference.clone(),lock:lock.clone()}};
    let result=DomainCommandGateway.execute(database,&envelope,|tx|{
        storage::resolve(tx,&reference,false)?;let installation_id=format!("custom-acp-{}-{}-{}-{id}",reference.id,reference.definition_revision,text(&lock,"platform")?);let now=chrono::Utc::now().to_rfc3339();
        tx.execute("INSERT INTO adapter_installation(id,adapter_kind,executable_path,command_name,installation_class,source,auth_scope,enabled,generation,path_state,version,created_at,updated_at) VALUES(?1,'custom-acp',?2,'custom-acp','custom','custom',?3,1,1,'valid',1,?4,?4) ON CONFLICT(id) DO NOTHING",params![installation_id,text(&lock,"executable")?,format!("custom:{}:{}",reference.id,reference.definition_revision),now])?;
        crate::agent_profile::upsert_static_capability_snapshot(tx,&installation_id,&snapshot)?;
        tx.execute("INSERT OR IGNORE INTO custom_runtime_installation_history(installation_id,definition_id,definition_revision,platform,operation_id,lock_json,created_at) SELECT installation_id,definition_id,definition_revision,platform,operation_id,lock_json,updated_at FROM custom_runtime_installation WHERE definition_id=?1 AND definition_revision=?2 AND platform=?3 AND installation_id IS NOT NULL AND operation_id IS NOT NULL AND lock_json IS NOT NULL",params![reference.id,reference.definition_revision,text(&lock,"platform")?])?;
        tx.execute("INSERT INTO custom_runtime_installation_history(installation_id,definition_id,definition_revision,platform,operation_id,lock_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![installation_id,reference.id,reference.definition_revision,text(&lock,"platform")?,id,serde_json::to_string(&lock)?,now])?;
        tx.execute("INSERT INTO custom_runtime_installation(definition_id,definition_revision,platform,installation_id,operation_id,lock_json,status,updated_at) VALUES(?1,?2,?3,?4,?5,?6,'installed_unverified',?7) ON CONFLICT(definition_id,definition_revision,platform) DO UPDATE SET installation_id=excluded.installation_id,operation_id=excluded.operation_id,lock_json=excluded.lock_json,status=excluded.status,updated_at=excluded.updated_at",params![reference.id,reference.definition_revision,text(&lock,"platform")?,installation_id,id,serde_json::to_string(&lock)?,now])?;
        Ok(CommandHandlerResult::applied("custom_runtime.installed",json!({"installationId":installation_id,"reference":reference,"launchLockDigest":digest(&lock)?,"qualification":"not_verified"}),None))
    })?;Ok(serde_json::to_value(result.result)?)
}
pub async fn start(root:&Path,definition:Value,reference:CustomRuntimeRefV1,requested_platform:String,id:String,selectable:bool,database:Arc<tokio::sync::Mutex<Database>>)->Result<Value>{
    ensure!(requested_platform==platform()?,"custom_runtime.install_requires_current_platform");let path=receipt_path(root,&id)?;let input_digest=digest(&json!({"reference":reference,"platform":requested_platform,"commandId":id}))?;
    if path.exists(){let result=receipt(root,&id)?;ensure!(result["inputDigest"].as_str()==Some(input_digest.as_str()),"custom_runtime.command_identity_changed");return Ok(result)}
    let guard=MUTATION.clone().try_lock_owned().map_err(|_|anyhow::anyhow!("custom_runtime.install_busy"))?;
    if path.exists(){let result=receipt(root,&id)?;ensure!(result["inputDigest"].as_str()==Some(input_digest.as_str()),"custom_runtime.command_identity_changed");return Ok(result)}
    ensure!(selectable,"custom_runtime.definition_changed");crate::platform::private_storage::prepare_private_directory(path.parent().context("custom_runtime.receipt_parent_missing")?)?;
    let record=json!({"commandId":id,"inputDigest":input_digest,"reference":reference,"platform":requested_platform,"state":"running","startedAt":chrono::Utc::now().to_rfc3339()});crate::platform::private_storage::atomic_write_private_json(&path,&record)?;
    {
        let database=database.lock().await;
        let installed:Option<String>=database.connection().query_row("SELECT lock_json FROM custom_runtime_installation WHERE definition_id=?1 AND definition_revision=?2 AND platform=?3 AND lock_json IS NOT NULL",params![reference.id,reference.definition_revision,requested_platform],|r|r.get(0)).optional()?;
        if let Some(lock)=installed{let lock:Value=serde_json::from_str(&lock)?;let mut existing=record;existing["state"]=json!("completed");existing["info"]=json!({"reference":reference,"launchLockDigest":digest(&lock)?,"qualification":"not_verified","reusedImmutableInstallation":true});crate::platform::private_storage::atomic_write_private_json(&path,&existing)?;return Ok(existing)}
        database.connection().execute("INSERT INTO custom_runtime_installation(definition_id,definition_revision,platform,operation_id,status,updated_at) VALUES(?1,?2,?3,?4,'installing',?5) ON CONFLICT(definition_id,definition_revision,platform) DO UPDATE SET operation_id=excluded.operation_id,status=excluded.status,updated_at=excluded.updated_at WHERE custom_runtime_installation.lock_json IS NULL",params![reference.id,reference.definition_revision,requested_platform,id,chrono::Utc::now().to_rfc3339()])?;
    }
    ACTIVE.lock().map_err(|_|anyhow::anyhow!("custom_runtime.install_lock"))?.insert(path.clone());let root=root.to_path_buf();let returned=record.clone();
    tokio::spawn(async move{let _guard=guard;let log=Log::default();let secrets=crate::runtime_discovery::capture_host_runtime_environment().into_iter().filter(|(key,_)|crate::member_environment::sensitive(key)).map(|(_,value)|value).collect();let redactor=crate::runtime_custom_api::CredentialRedactor::from_values(secrets);
        let result=async{let lock=install(&root,&definition,&reference,&requested_platform,&id,log.clone()).await?;let mut database=database.lock().await;register(&mut database,reference,lock,&id)}.await;
        let mut record=record;record["completedAt"]=json!(chrono::Utc::now().to_rfc3339());record["log"]=json!(redactor.text(&String::from_utf8_lossy(&log.lock().await)));
        match result{Ok(info)=>{record["state"]=json!("completed");record["info"]=info},Err(error)=>{record["state"]=json!("failed");record["error"]=json!(redactor.text(&error.to_string()))}}
        let _=crate::platform::private_storage::atomic_write_private_json(&path,&record);if let Ok(mut active)=ACTIVE.lock(){active.remove(&path);}
    });Ok(returned)
}
