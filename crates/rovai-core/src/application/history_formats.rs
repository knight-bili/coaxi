//! Read-only native parser helper over an owned, immutable source snapshot.
use super::*;
use crate::managed_process::{ManagedProcess,ManagedProcessLaunchSpec,ManagedProcessPurpose,ManagedStdinPolicy,ManagedWindowsArgvDialect};
use std::{io::{Read,Write},time::Duration};
use tokio::io::{AsyncReadExt,AsyncWriteExt};
const FORMATS:&[&str]=&["opencode","gemini","openclaw","cline","hermes","codebuddy","kimi_code","pi","cursor","deepseek","antigravity"];
pub(super) fn supported(provider:&str)->bool{FORMATS.contains(&provider)}
fn excluded(path:&Path)->bool{
    path.components().any(|component|{let name=component.as_os_str().to_string_lossy().to_ascii_lowercase();matches!(name.as_str(),".git"|"node_modules"|"target"|"dist"|".ssh"|".aws"|"cache"|".cache"|"bin"|"locks"|"credentials"|"oauth")})||path.file_name().is_some_and(|name|{let name=name.to_string_lossy().to_ascii_lowercase();name==".env"||name.starts_with(".env.")||matches!(name.as_str(),"auth.json"|"credentials.json"|"oauth_creds.json"|"acp_token.json"|"acp_business_token.json"|"secrets.json"|"config.toml"|"settings.json")})
}
fn snapshot(root:&Path,target:&Path)->Result<String>{
    reject_link(root)?;let root=fs::canonicalize(root)?;ensure!(root.is_dir()&&!target.starts_with(&root),"history.snapshot_cannot_contain_itself");crate::platform::private_storage::prepare_private_directory(target)?;
    let mut pending=vec![(root.clone(),0usize)];let mut files=Vec::new();let mut visited=0usize;let mut total=0u64;
    while let Some((directory,depth))=pending.pop(){ensure!(depth<=24,"history.depth_limit");for entry in fs::read_dir(directory)?{let entry=entry?;visited+=1;ensure!(visited<=50000,"history.scan_limit");let path=entry.path();reject_link(&path)?;let relative=path.strip_prefix(&root)?;if excluded(relative){continue}let metadata=entry.metadata()?;if metadata.is_dir(){pending.push((path,depth+1));continue}if !metadata.is_file(){continue}
            let filename=path.file_name().unwrap_or_default().to_string_lossy();if filename.ends_with("-wal")||filename.ends_with("-shm"){continue}
            let ext=path.extension().and_then(|extension|extension.to_str()).unwrap_or_default().to_ascii_lowercase();if !["json","jsonl","db","sqlite","sqlite3","vscdb","pb","meta","idx"].contains(&ext.as_str()){continue}
            ensure!(metadata.len()<=128*1024*1024,"history.source_too_large");total=total.checked_add(metadata.len()).context("history.size_overflow")?;ensure!(total<=512*1024*1024,"history.snapshot_limit");
            let destination=target.join(relative);crate::platform::private_storage::prepare_private_directory(destination.parent().context("history.invalid_path")?)?;
            let mut header=[0u8;16];let count=fs::File::open(&path)?.read(&mut header)?;
            if count==16&&header==*b"SQLite format 3\0"{
                let connection=rusqlite::Connection::open_with_flags(&path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY|rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)?;connection.backup(rusqlite::MAIN_DB,&destination,None)?;
            }else{
                let bytes=fs::read(&path)?;ensure!(bytes.len()as u64<=128*1024*1024,"history.source_too_large");let before=hash(&bytes);crate::platform::private_storage::atomic_write_private_bytes(&destination,&bytes)?;ensure!(hash(&fs::read(&path)?)==before,"history.source_changed_during_snapshot");
            }
            crate::platform::private_storage::repair_private_file(&destination)?;files.push((relative.to_string_lossy().replace('\\',"/"),crate::backup::file_digest(&destination)?));
        }}files.sort_by(|left,right|left.0.cmp(&right.0));crate::command::canonical_json_digest(&json!(files))
}
async fn helper(root:&Path,provider:&str,operation:&str,id:Option<&str>)->Result<Value>{
    ensure!(supported(provider),"history.unsupported_adapter");let executable=std::env::current_exe()?;let name=if cfg!(windows){"rovai-native-history.exe"}else{"rovai-native-history"};let mut program=executable.parent().context("history.helper_unavailable")?.join(name);
    if cfg!(debug_assertions){if let Some(path)=std::env::var_os("ROVAI_NATIVE_HISTORY_HELPER"){let path=PathBuf::from(path);ensure!(path.is_absolute(),"history.helper_absolute_path_required");program=path;}}
    reject_link(&program)?;ensure!(program.is_absolute()&&program.is_file(),"history.helper_unavailable");
    let mut command=tokio::process::Command::new(program);command.current_dir(root).env_clear().env("HOME",root).env("USERPROFILE",root);
    for name in ["SystemRoot","WINDIR","TEMP","TMP"]{if let Some(value)=std::env::var_os(name){command.env(name,value);}}
    let spec=ManagedProcessLaunchSpec::capture(&command,ManagedProcessPurpose::RuntimeOneShot,ManagedStdinPolicy::Piped,ManagedWindowsArgvDialect::MicrosoftCrt,"native-history-read")?;
    let mut process=ManagedProcess::spawn(spec)?;let mut input=process.take_stdin().context("history.helper_pipe_unavailable")?;let output=process.take_stdout().context("history.helper_pipe_unavailable")?;let error=process.take_stderr().context("history.helper_pipe_unavailable")?;
    let stdout=tokio::spawn(async move{let mut bytes=Vec::new();output.take(64*1024*1024+1).read_to_end(&mut bytes).await?;Ok::<_,std::io::Error>(bytes)});
    let stderr=tokio::spawn(async move{let mut bytes=Vec::new();error.take(65537).read_to_end(&mut bytes).await?;Ok::<_,std::io::Error>(bytes)});
    let request=serde_json::to_vec(&json!({"protocolVersion":1,"root":root,"provider":provider,"operation":operation,"id":id}))?;input.write_all(&request).await?;input.shutdown().await?;drop(input);
    let waited=tokio::time::timeout(Duration::from_secs(90),process.wait()).await;
    let status=match waited{Ok(value)=>value?,Err(_)=>{let _=process.force_terminate_tree();let _=tokio::time::timeout(Duration::from_secs(3),process.wait()).await;stdout.abort();stderr.abort();anyhow::bail!("history.helper_timeout")}};
    let bytes=tokio::time::timeout(Duration::from_secs(3),stdout).await???;let errors=tokio::time::timeout(Duration::from_secs(3),stderr).await???;ensure!(status.success(),"history.helper_read_failed: {}",String::from_utf8_lossy(&errors).chars().take(1000).collect::<String>());ensure!(bytes.len()<=64*1024*1024,"history.helper_result_too_large");let value:Value=serde_json::from_slice(&bytes)?;ensure!(value["protocolVersion"]==1,"history.helper_protocol_mismatch");Ok(value["result"].clone())
}
pub(super) async fn scan(core:&Core,original:PathBuf,provider:String,plan_id:&str)->Result<Plan>{
    let target=core.data_dir.join("imported-history/plans").join(plan_id).join("source");let source=original.clone();let copied=target.clone();let digest=tokio::task::spawn_blocking(move||snapshot(&source,&copied)).await??;
    let result=helper(&target,&provider,"list",None).await?;let summaries=result.as_array().context("history.invalid_candidates")?;ensure!(summaries.len()<=5000,"history.candidate_limit");let mut entries=Vec::new();
    for summary in summaries{if !summary["parent_id"].is_null(){continue}let id=summary["id"].as_str().context("history.invalid_native_id")?;let key=hash(format!("{provider}:{id}").as_bytes());let title=summary["title"].as_str().filter(|title|!title.trim().is_empty()).unwrap_or(id);entries.push(Entry{id:key,name:title.chars().take(240).collect(),bytes:0,modified:summary["started_at"].as_str().and_then(|value|chrono::DateTime::parse_from_rfc3339(value).ok()).map(|date|date.timestamp().max(0)as u64).unwrap_or(0),native_id:Some(id.into())});}
    entries.sort_by(|left,right|right.modified.cmp(&left.modified).then(left.name.cmp(&right.name)));Ok(Plan{root:target,provider,created_at:Utc::now().to_rfc3339(),entries,source_root:Some(original),helper:true,snapshot_digest:Some(digest)})
}
pub(super) async fn read(plan:&Plan,entry:&Entry)->Result<(Vec<u8>,Value)>{
    let id=entry.native_id.as_deref().context("history.invalid_native_id")?;let detail=helper(&plan.root,&plan.provider,"read",Some(id)).await?;
    let raw=serde_json::to_vec(&json!({"format":"codeg-native-snapshot-v1","provider":plan.provider,"snapshotDigest":plan.snapshot_digest,"nativeSessionId":id,"summary":detail["summary"]}))?;
    let transcript=json!({"schema_version":1,"turns":detail["turns"],"context_tokens":null,"session_stats":detail["session_stats"],"summary":detail["summary"],"notes":{"malformed_records":0,"incomplete_tail":false,"unavailable_children":0,"unavailable_advisories":0},"child_references":[]});Ok((raw,transcript))
}
