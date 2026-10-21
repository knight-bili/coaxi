//! User-selected native history import. Originals stay untouched. Imported
//! snapshots do not become Runs, live native-session addresses or file grants.
use super::*;
#[path="history_formats.rs"]
mod formats;
#[path="history_export.rs"]
mod export;
use anyhow::ensure;
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::{fs,path::Component,sync::atomic::AtomicBool};
const MAX_SOURCE:u64=128*1024*1024;
#[derive(Serialize,Deserialize,Clone)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Entry{id:String,name:String,bytes:u64,modified:u64,#[serde(default)]native_id:Option<String>}
#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Plan{root:PathBuf,provider:String,created_at:String,entries:Vec<Entry>,#[serde(default)]source_root:Option<PathBuf>,#[serde(default)]helper:bool,#[serde(default)]snapshot_digest:Option<String>}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request{scope:workbench::Scope,operation:String,#[serde(default)]root:Option<PathBuf>,#[serde(default)]provider:Option<String>,#[serde(default)]plan_id:Option<String>,#[serde(default)]entry_id:Option<String>,#[serde(default)]expected_digest:Option<String>,#[serde(default)]import_id:Option<String>,#[serde(default)]offset:usize,#[serde(default)]tags:Option<Vec<String>>,#[serde(default)]include_archived:bool,#[serde(default)]expected_revision:Option<String>,#[serde(default)]destination:Option<PathBuf>}
fn library_entry(mut value:Value)->Result<Value>{
    let revision=crate::command::canonical_json_digest(&value)?;
    if let Some(object)=value.as_object_mut(){object.remove("originalRoot");object.remove("nativeSessionId");let shared=object.remove("snapshotRoot").is_some_and(|value|!value.is_null());object.insert("sharedSourceSnapshot".into(),json!(shared));object.insert("revision".into(),json!(revision));object.entry("archived".to_string()).or_insert(json!(false));}
    Ok(value)
}
fn hash(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn identifier(value:&str)->Result<()>{ensure!(value.len()==64&&value.bytes().all(|byte|byte.is_ascii_hexdigit()),"history.invalid_identifier");Ok(())}
fn reject_link(path:&Path)->Result<()>{let metadata=fs::symlink_metadata(path)?;ensure!(!metadata.file_type().is_symlink(),"history.linked_path");#[cfg(windows)]{use std::os::windows::fs::MetadataExt;ensure!(metadata.file_attributes()&0x400==0,"history.linked_path");}Ok(())}
fn source(plan:&Plan,entry:&Entry)->Result<Vec<u8>>{
    let mut path=plan.root.clone();reject_link(&path)?;
    for component in Path::new(&entry.name).components(){match component{Component::Normal(name)=>{path.push(name);reject_link(&path)?},_=>anyhow::bail!("history.invalid_path")}}
    ensure!(fs::canonicalize(&path)?.starts_with(fs::canonicalize(&plan.root)?)&&fs::metadata(&path)?.len()<=MAX_SOURCE,"history.source_changed_or_too_large");
    use std::io::Read;let mut bytes=Vec::new();fs::File::open(path)?.take(MAX_SOURCE+1).read_to_end(&mut bytes)?;ensure!(bytes.len() as u64<=MAX_SOURCE,"history.source_too_large");Ok(bytes)
}
fn scan(root:PathBuf,provider:String)->Result<Plan>{
    ensure!(matches!(provider.as_str(),"codex"|"claude"|"qoder"|"grok"),"history.unsupported_adapter");reject_link(&root)?;let root=fs::canonicalize(root)?;ensure!(root.is_dir(),"history.directory_required");
    let mut pending=vec![(root.clone(),0_usize)];let mut entries=Vec::new();let mut visited=0_usize;
    while let Some((directory,depth))=pending.pop(){if depth>16{continue}for entry in fs::read_dir(directory)?{let entry=entry?;visited+=1;ensure!(visited<=100000,"history.scan_limit");let path=entry.path();if reject_link(&path).is_err(){continue}let metadata=entry.metadata()?;if metadata.is_dir(){if !matches!(entry.file_name().to_str(),Some(".git"|"node_modules"|"target"|".codegraph")){pending.push((path,depth+1))}continue}if !metadata.is_file()||metadata.len()>MAX_SOURCE||!matches!(path.extension().and_then(|part|part.to_str()),Some("jsonl"|"json")){continue}
        let name=path.strip_prefix(&root)?.to_string_lossy().replace('\\',"/");let id=hash(name.as_bytes());entries.push(Entry{id,name,bytes:metadata.len(),modified:metadata.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),native_id:None});ensure!(entries.len()<=5000,"history.candidate_limit");}}
    entries.sort_by(|a,b|b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));Ok(Plan{root,provider,created_at:Utc::now().to_rfc3339(),entries,source_root:None,helper:false,snapshot_digest:None})
}
impl Core{
    pub(super) async fn history_import(&self,local:bool,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value)?;
        let _root=self.workbench_root(&request.scope).await?;
        let directory=self.data_dir.join("imported-history");
        crate::platform::private_storage::prepare_private_directory(&directory)?;
        if request.operation=="list"{
            let mut entries=Vec::new();for entry in fs::read_dir(&directory)?{let entry=entry?;if entry.file_name().to_string_lossy().ends_with(".manifest.json"){let value:Value=serde_json::from_slice(&fs::read(entry.path())?)?;if request.include_archived||value["archived"]!=true{entries.push(library_entry(value)?)}}}
            entries.sort_by(|a,b|b["importedAt"].as_str().cmp(&a["importedAt"].as_str()));return Ok(json!({"entries":entries}));
        }
        if request.operation=="read"{
            let id=request.import_id.context("history.import_required")?;identifier(&id)?;let bytes=fs::read(directory.join(format!("{id}.transcript.json")))?;ensure!(request.offset<=bytes.len(),"history.invalid_offset");let end=(request.offset+512*1024).min(bytes.len());
            use base64::Engine;return Ok(json!({"digest":hash(&bytes),"totalBytes":bytes.len(),"offset":request.offset,"next":(end<bytes.len()).then_some(end),"bytes":base64::engine::general_purpose::STANDARD.encode(&bytes[request.offset..end])}));
        }
        if matches!(request.operation.as_str(),"tags"|"archive"|"restore"){
            let id=request.import_id.context("history.import_required")?;identifier(&id)?;let _lock=workbench::MUTATIONS.lock().await;let path=directory.join(format!("{id}.manifest.json"));let mut manifest:Value=serde_json::from_slice(&fs::read(&path)?)?;
            let revision=crate::command::canonical_json_digest(&manifest)?;ensure!(request.expected_revision.as_deref()==Some(revision.as_str()),"history.library_changed");
            if request.operation=="tags"{let tags=request.tags.context("history.tags_required")?;ensure!(tags.len()<=128&&tags.iter().all(|tag|!tag.trim().is_empty()&&tag.len()<=256),"history.invalid_tags");manifest["tags"]=json!(tags);}else{manifest["archived"]=json!(request.operation=="archive");manifest["archivedAt"]=if request.operation=="archive"{json!(Utc::now().to_rfc3339())}else{Value::Null};}
            crate::platform::private_storage::atomic_write_private_json(&path,&manifest)?;return Ok(json!({"saved":true,"entry":library_entry(manifest)?}));
        }
        ensure!(local,"history.local_owner_required");
        if request.operation=="export"{
            let id=request.import_id.context("history.import_required")?;let expected=request.expected_revision.context("history.revision_required")?;let destination=request.destination.context("history.destination_required")?;
            let _lock=workbench::MUTATIONS.lock().await;return tokio::task::spawn_blocking(move||export::export(&directory,&id,&expected,&destination)).await?;
        }
        if request.operation=="scan"{
            let root=request.root.context("history.root_required")?;let provider=request.provider.context("history.provider_required")?;
            let id=uuid::Uuid::new_v4().to_string();
            let plan=if formats::supported(&provider){formats::scan(self,root,provider,&id).await?}else{tokio::task::spawn_blocking(move||scan(root,provider)).await??};
            let plans=directory.join("plans");crate::platform::private_storage::prepare_private_directory(&plans)?;crate::platform::private_storage::atomic_write_private_json(&plans.join(format!("{id}.json")),&serde_json::to_value(&plan)?)?;
            return Ok(json!({"planId":id,"provider":plan.provider,"snapshot":plan.helper,"entries":plan.entries.iter().map(|entry|json!({"id":entry.id,"name":entry.name,"bytes":entry.bytes,"modified":entry.modified,"snapshot":plan.helper})).collect::<Vec<_>>()}));
        }
        let plan_id=request.plan_id.context("history.plan_required")?;uuid::Uuid::parse_str(&plan_id)?;
        let plan:Plan=serde_json::from_slice(&fs::read(directory.join("plans").join(format!("{plan_id}.json")))?)?;
        let entry=plan.entries.iter().find(|entry|Some(entry.id.as_str())==request.entry_id.as_deref()).context("history.entry_unavailable")?.clone();
        let name=entry.name.clone();let provider=plan.provider.clone();let origin=plan.source_root.clone().unwrap_or_else(||plan.root.clone());
        let snapshot_root=plan.helper.then(||plan.root.clone());let snapshot_digest=plan.snapshot_digest.clone();
        let (bytes,transcript)=if plan.helper{formats::read(&plan,&entry).await?}else{tokio::task::spawn_blocking(move||->Result<_>{let bytes=source(&plan,&entry)?;let transcript=crate::runtime_subagent_session::content::import_records(&plan.provider,&bytes,Utc::now(),&AtomicBool::new(false))?;Ok((bytes,transcript))}).await??};
        let digest=hash(&bytes);
        if request.operation=="preview"{return Ok(json!({"digest":digest,"summary":transcript["summary"],"notes":transcript["notes"],"turnCount":transcript["turns"].as_array().map(Vec::len)}));}
        ensure!(request.operation=="import"&&request.expected_digest.as_deref()==Some(digest.as_str()),"history.source_changed");
        let id=hash(format!("{provider}:{digest}").as_bytes());let manifest_path=directory.join(format!("{id}.manifest.json"));
        let _lock=workbench::MUTATIONS.lock().await;
        if manifest_path.exists(){let existing:Value=serde_json::from_slice(&fs::read(&manifest_path)?)?;return Ok(json!({"importId":id,"duplicate":true,"archived":existing["archived"]==true}));}
        let source_path=directory.join(format!("{id}.native.json"));
        crate::platform::private_storage::atomic_write_private_bytes(&source_path,&bytes)?;
        crate::platform::private_storage::atomic_write_private_json(&directory.join(format!("{id}.transcript.json")),&transcript)?;
        crate::platform::private_storage::atomic_write_private_json(&manifest_path,&json!({"id":id,"provider":provider,"sourceName":name,"originalRoot":origin,"sourceDigest":digest,"summary":transcript["summary"],"notes":transcript["notes"],"importedAt":Utc::now().to_rfc3339(),"linkedThreadId":request.scope.thread_id,"snapshotRoot":snapshot_root,"snapshotDigest":snapshot_digest,"tags":[],"archived":false}))?;
        Ok(json!({"importId":id,"duplicate":false}))
    }
}
