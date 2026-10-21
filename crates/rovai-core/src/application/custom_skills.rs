//! User-authored source snapshots feed the existing installed-pack contract.
use super::*;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::BTreeMap, fs};
use crate::{codeg_skill_catalog::{InstallationFile, InstallationSource, SkillPackEntry, SkillPackFamily}, skill_pack_installation::{SkillPackStore, InstallBeginning, InstallSkillPackCommand}, command::DomainCommandGateway};

static SOURCES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
#[derive(Deserialize, Serialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct SourceFile { path: String, bytes: Vec<u8>, #[serde(default)] executable: bool }
#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Prepare { skill_id: String, expected_revision: Option<String>, files: Vec<SourceFile>, #[serde(default)] preserve_support_files: bool, copy_from: Option<CopySource> }
#[derive(Deserialize)] #[serde(rename_all="camelCase",deny_unknown_fields)]
struct CopySource {skill_id:String,source_revision:String}
#[derive(Deserialize, Serialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Source { skill_id: String, source_revision: String, snapshot_id: String, description: String, files: Vec<SourceFile> }
#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Install { command_id: String, command: InstallSkillPackCommand }

fn admit_id(id:&str)->Result<()> {
    crate::skill::validate_skill_name(id)?;
    ensure!(!crate::codeg_skill_catalog::list()?.entries.iter().any(|entry|entry.id==id),"skill.custom_reserved_id");
    Ok(())
}
fn source_root(root:&Path)->PathBuf { root.join("custom-skill-sources") }
fn load_source(root:&Path,id:&str)->Result<Source> {
    admit_id(id)?;
    let source:Source=serde_json::from_slice(&fs::read(source_root(root).join(id).join("current.json"))?)?;
    ensure!(source.skill_id==id,"skill.custom_source_mismatch");
    uuid::Uuid::parse_str(&source.snapshot_id)?;
    Ok(source)
}
fn snapshot_source(root:&Path,request:Prepare,expected_digest:Option<&str>)->Result<Value> {
    admit_id(&request.skill_id)?;
    ensure!(!request.files.is_empty()&&request.files.len()<=512,"skill.custom_file_limit");
    ensure!(request.files.iter().map(|file|file.bytes.len()).sum::<usize>()<=16*1024*1024,"skill.custom_size_limit");
    let current=source_root(root).join(&request.skill_id).join("current.json");
    let previous=if current.exists(){Some(load_source(root,&request.skill_id)?.source_revision)}else{None};
    ensure!(previous==request.expected_revision,"skill.custom_source_conflict");
    let mut files=request.files;
    let inherited=if let Some(copy)=request.copy_from {
        let source=load_source(root,&copy.skill_id)?;ensure!(source.source_revision==copy.source_revision,"skill.custom_source_conflict");Some(source.files)
    }else if request.preserve_support_files {Some(load_source(root,&request.skill_id)?.files)}else{None};
    if let Some(inherited)=inherited {for file in inherited {if !files.iter().any(|changed|changed.path.eq_ignore_ascii_case(&file.path)){files.push(file)}}}
    ensure!(files.len()<=512&&files.iter().map(|file|file.bytes.len()).sum::<usize>()<=16*1024*1024,"skill.custom_size_limit");
    files.sort_by(|a,b|a.path.cmp(&b.path));
    ensure!(!files.windows(2).any(|pair|pair[0].path.eq_ignore_ascii_case(&pair[1].path)),"skill.custom_duplicate_path");
    let directory=source_root(root).join(&request.skill_id);
    crate::platform::private_storage::prepare_private_directory(&source_root(root))?;
    crate::platform::private_storage::prepare_private_directory(&directory)?;
    let snapshot_id=uuid::Uuid::new_v4().to_string();
    let snapshot=directory.join(&snapshot_id);
    crate::platform::private_storage::prepare_private_directory(&snapshot)?;
    let written=(||->Result<String>{
        for file in &files {
            ensure!(!file.path.is_empty()&&file.path.split('/').all(|part|!part.is_empty()&&part!="."&&part!=".."&&!part.ends_with(['.',' '])),"skill.custom_invalid_path");
            ensure!(file.path!="source.json"&&file.path!="current.json","skill.custom_reserved_path");
            let target=super::workbench::relative(&snapshot,&file.path,true)?;
            let mut parent=target.parent().context("skill.custom_parent_missing")?.to_path_buf();
            let mut missing=Vec::new();
            while !parent.exists(){missing.push(parent.clone());parent=parent.parent().context("skill.custom_parent_missing")?.to_path_buf();}
            for path in missing.iter().rev(){crate::platform::private_storage::prepare_private_directory(path)?;}
            crate::platform::private_storage::atomic_write_private_bytes(&target,&file.bytes)?;
            #[cfg(unix)] if file.executable { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&target,fs::Permissions::from_mode(0o700))?; }
        }
        let (digest,description)=crate::skill::inspect_installed_pack_tree_with_description(&snapshot,&request.skill_id)?;
        if let Some(expected)=expected_digest {ensure!(digest==expected,"skill.custom_import_changed");}
        let revision=format!("custom:{digest}");
        // Metadata contains a bounded copy so a directory rename never changes source bytes.
        let source=Source{skill_id:request.skill_id.clone(),source_revision:revision.clone(),snapshot_id:snapshot_id.clone(),description,files};
        crate::platform::private_storage::atomic_write_private_json(&snapshot.join("source.json"),&source)?;
        crate::platform::private_storage::atomic_write_private_json(&current,&source)?;
        Ok(revision)
    })();
    match written { Ok(revision)=>Ok(json!({"skillId":request.skill_id,"sourceRevision":revision})), Err(error)=>{let _=fs::remove_dir_all(&snapshot);Err(error)} }
}
fn catalog_entries(root:&Path)->Result<Vec<SkillPackEntry>> {
    let directory=source_root(root); if !directory.exists(){return Ok(Vec::new())}
    let mut entries=Vec::new();
    for entry in fs::read_dir(directory)?.take(4097) {
        let entry=entry?;ensure!(entries.len()<4096,"skill.custom_catalog_limit");
        let metadata=entry.file_type()?; if !metadata.is_dir()||metadata.is_symlink(){continue}
        let id=entry.file_name().to_string_lossy().into_owned();let source=load_source(root,&id)?;
        entries.push(SkillPackEntry{id:id.clone(),family:SkillPackFamily::Custom,category:"custom".into(),icon:None,sort_order:0,display_name:BTreeMap::from([("zh".into(),id.clone()),("en".into(),id)]),description:BTreeMap::from([("zh".into(),source.description.clone()),("en".into(),source.description)]),featured:false,accent:None,needs_key:false,needs_env:false,source_path:"SKILL.md".into(),source_url:String::new(),content_source:"custom",required_capability:None,load_id:Some(source.source_revision)});
    }
    entries.sort_by(|a,b|a.id.cmp(&b.id));Ok(entries)
}
impl Core {
    pub(super) fn skill_pack_catalog(&self)->Result<Value>{let mut catalog=crate::codeg_skill_catalog::list()?;catalog.entries.extend(catalog_entries(&self.data_dir)?);Ok(serde_json::to_value(catalog)?)}
    pub(super) fn skill_pack_read(&self,value:Value)->Result<Value>{
        let params:crate::codeg_skill_catalog::ReadSkillPackParams=serde_json::from_value(value)?;
        if crate::codeg_skill_catalog::list()?.entries.iter().any(|entry|entry.id==params.skill_id){return Ok(serde_json::to_value(crate::codeg_skill_catalog::read(&params)?)?)}
        let source=load_source(&self.data_dir,&params.skill_id)?;
        let file=source.files.iter().find(|file|file.path==params.path).context("skill.custom_file_missing")?;
        let (status,content)=if file.bytes.len()>1024*1024 {("too_large",None)}else{match std::str::from_utf8(&file.bytes){Ok(text)=>("text",Some(text.to_owned())),Err(_)=>("binary",None)}};
        Ok(serde_json::to_value(crate::skill::SkillContentView{path:params.path,content,status,files:source.files.iter().map(|file|crate::skill::SkillContentFile{path:file.path.clone(),bytes:file.bytes.len() as u64}).collect()})?)
    }
    pub(super) async fn custom_skill_prepare(&self,value:Value)->Result<Value>{
        let request:Prepare=serde_json::from_value(value)?;let _guard=SOURCES.lock().await;snapshot_source(&self.data_dir,request,None)
    }
    pub(super) async fn custom_skill_local(&self,value:Value)->Result<Value>{
        #[derive(Deserialize)] #[serde(rename_all="camelCase",deny_unknown_fields)] struct Import{source:PathBuf,skill_id:String,expected_revision:Option<String>}
        let request:Import=serde_json::from_value(value)?;admit_id(&request.skill_id)?;
        let before=crate::skill::inspect_installed_pack_tree(&request.source,&request.skill_id)?;
        let mut pending=vec![(request.source.clone(),String::new())];let mut files=Vec::new();let mut total=0usize;
        while let Some((directory,prefix))=pending.pop(){
            for entry in fs::read_dir(directory)?{let entry=entry?;let name=entry.file_name().into_string().map_err(|_|anyhow::anyhow!("skill.custom_non_unicode_path"))?;let path=if prefix.is_empty(){name}else{format!("{prefix}/{name}")};
                let resolved=super::workbench::relative(&request.source,&path,false)?;let metadata=fs::symlink_metadata(&resolved)?;
                if metadata.is_dir(){ensure!(path.matches('/').count()<16,"skill.custom_depth_limit");pending.push((resolved,path));continue}
                ensure!(metadata.is_file()&&files.len()<512,"skill.custom_file_limit");ensure!(metadata.len()<=16*1024*1024,"skill.custom_size_limit");let bytes=fs::read(&resolved)?;total+=bytes.len();ensure!(total<=16*1024*1024,"skill.custom_size_limit");
                #[cfg(unix)] let executable={use std::os::unix::fs::PermissionsExt;metadata.permissions().mode()&0o111!=0};
                #[cfg(not(unix))] let executable=false;
                files.push(SourceFile{path,bytes,executable});
            }
        }
        ensure!(crate::skill::inspect_installed_pack_tree(&request.source,&request.skill_id)?==before,"skill.custom_import_changed");
        let _guard=SOURCES.lock().await;snapshot_source(&self.data_dir,Prepare{skill_id:request.skill_id,expected_revision:request.expected_revision,files,preserve_support_files:false,copy_from:None},Some(&before))
    }
    pub(super) async fn custom_skill_install(&self,value:Value)->Result<Value>{
        let request:Install=serde_json::from_value(value)?;let envelope=user_command_envelope(request.command_id,request.command);let store=SkillPackStore::new(&self.data_dir)?;
        {let database=self.database.lock().await;if let Some(execution)=DomainCommandGateway.replay_if_recorded(&database,&envelope)?{return Ok(serde_json::to_value(execution.result)?);}}
        let current=load_source(&self.data_dir,&envelope.payload.skill_id)?;
        let source:Source=serde_json::from_slice(&fs::read(source_root(&self.data_dir).join(&current.skill_id).join(&current.snapshot_id).join("source.json"))?)?;
        let source=InstallationSource{revision:Cow::Owned(source.source_revision),files:source.files.into_iter().map(|file|InstallationFile{path:file.path,bytes:Cow::Owned(file.bytes),executable:file.executable}).collect()};
        let beginning={let mut database=self.database.lock().await;store.begin_install_with_source(&mut database,&envelope,Ok(source))?};
        let execution=match beginning{InstallBeginning::Complete(execution)=>execution,InstallBeginning::Prepare(plan)=>{let preparing=store.clone();let prepared=tokio::task::spawn_blocking(move||preparing.prepare(plan)).await??;let mut database=self.database.lock().await;store.commit_install(&mut database,&envelope,prepared)?}};
        Ok(serde_json::to_value(execution.result)?)
    }
}
