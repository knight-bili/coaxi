//! File-tree actions retain recoverable originals and require captured content.
use super::*;
use super::workbench::{Scope,relative,MUTATIONS};
use anyhow::ensure;
use serde::{Deserialize,Serialize};
use std::{fs,io::{Read,Write}};
use sha2::{Digest,Sha256};
#[derive(Deserialize)] #[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request{scope:Scope,operation:String,#[serde(default)]path:String,destination:Option<String>,command_id:Option<String>,expected_digest:Option<String>,trash_id:Option<String>,query:Option<String>,after:Option<String>,#[serde(default)]content:bool,#[serde(default)]case_sensitive:bool,#[serde(default)]regex:bool,#[serde(default)]include_ignored:bool,paths:Option<Vec<String>>}
#[derive(Serialize,Deserialize)] #[serde(rename_all="camelCase")]
struct Entry{path:String,directory:bool,bytes:u64,digest:String,#[serde(default)]executable:bool}
fn snapshot(root:&Path)->Result<(String,Vec<Entry>)>{
    let mut pending=vec![(root.to_path_buf(),String::new())];let mut entries=Vec::new();let mut total=0u64;
    while let Some((path,name))=pending.pop(){
        let metadata=fs::symlink_metadata(&path)?;ensure!(!metadata.file_type().is_symlink(),"workbench.link_not_allowed");
        #[cfg(windows)]{use std::os::windows::fs::MetadataExt;ensure!(metadata.file_attributes()&0x400==0,"workbench.link_not_allowed");}
        ensure!(entries.len()<10000&&name.matches('/').count()<32,"workbench.tree_limit");
        if metadata.is_dir(){
            entries.push(Entry{path:name.clone(),directory:true,bytes:0,digest:String::new(),executable:false});
            for child in fs::read_dir(&path)?{let child=child?;let child_name=child.file_name().into_string().map_err(|_|anyhow::anyhow!("workbench.non_utf8_name"))?;ensure!(!child_name.eq_ignore_ascii_case(".git"),"workbench.nested_git_metadata");pending.push((child.path(),if name.is_empty(){child_name}else{format!("{name}/{child_name}")}));}
        }else{
            ensure!(metadata.is_file(),"workbench.special_file");total+=metadata.len();ensure!(total<=512*1024*1024,"workbench.tree_size_limit");
            let mut file=fs::File::open(path)?;let mut hasher=Sha256::new();let mut buffer=[0u8;64*1024];let mut bytes=0u64;
            loop{let count=file.read(&mut buffer)?;if count==0{break}bytes+=count as u64;ensure!(bytes<=metadata.len(),"workbench.file_changed");hasher.update(&buffer[..count]);}ensure!(bytes==metadata.len(),"workbench.file_changed");
            #[cfg(unix)]let executable={use std::os::unix::fs::PermissionsExt;metadata.permissions().mode()&0o111!=0};#[cfg(not(unix))]let executable=false;
            entries.push(Entry{path:name,directory:false,bytes,digest:format!("{:x}",hasher.finalize()),executable});
        }
    }
    entries.sort_by(|a,b|a.path.cmp(&b.path));Ok((crate::command::canonical_json_digest(&json!(entries))?,entries))
}
fn trash_root(root:&Path)->Result<PathBuf>{let parent=root.parent().context("workbench.workspace_parent_required")?;let key=format!("{:x}",Sha256::digest(root.to_string_lossy().as_bytes()));Ok(parent.join(format!(".rovai-trash-{}",&key[..16])))}
fn copy_tree(source:&Path,destination:&Path,expected:&str)->Result<()> {
    let (before,entries)=snapshot(source)?;ensure!(before==expected,"workbench.file_changed");
    ensure!(!destination.exists()&&!destination.starts_with(source),"workbench.destination_exists");
    let parent=destination.parent().context("workbench.parent_missing")?;ensure!(parent.is_dir(),"workbench.parent_missing");
    let temporary=parent.join(format!(".rovai-copy-{}",uuid::Uuid::new_v4()));
    let result=(||->Result<()>{
        for entry in &entries{let input=if entry.path.is_empty(){source.to_path_buf()}else{source.join(&entry.path)};let output=if entry.path.is_empty(){temporary.clone()}else{temporary.join(&entry.path)};if entry.directory{fs::create_dir(&output)?;}else{let mut input=fs::File::open(&input)?;let mut output=fs::OpenOptions::new().write(true).create_new(true).open(&output)?;std::io::copy(&mut input,&mut output)?;output.sync_all()?;#[cfg(unix)]{use std::os::unix::fs::PermissionsExt;output.set_permissions(fs::Permissions::from_mode(if entry.executable{0o700}else{0o600}))?;}}}
        ensure!(snapshot(source)?.0==before&&snapshot(&temporary)?.0==before,"workbench.file_changed");ensure!(!destination.exists(),"workbench.destination_exists");fs::rename(&temporary,destination)?;Ok(())
    })();if result.is_err(){if temporary.is_dir(){let _=fs::remove_dir_all(&temporary);}else{let _=fs::remove_file(&temporary);}}result
}
fn search(root:&Path,request:&Request)->Result<Value>{
    let query=request.query.as_deref().context("workbench.query_required")?;ensure!(!query.is_empty()&&query.len()<=1024,"workbench.query_limit");
    let regex=regex::RegexBuilder::new(&if request.regex{query.to_owned()}else{regex::escape(query)}).case_insensitive(!request.case_sensitive).size_limit(1024*1024).build()?;
    let start=relative(root,&request.path,false)?;ensure!(start.is_dir(),"workbench.directory_required");
    let mut pending=vec![start];let mut results=Vec::new();let mut scanned=0usize;let mut bytes=0u64;let mut limited=false;
    while let Some(directory)=pending.pop(){
        let mut entries=fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;entries.sort_by_key(|entry|entry.file_name());
        for entry in entries{
            scanned+=1;if scanned>50000||bytes>64*1024*1024{limited=true;break}
            let name=entry.file_name().to_string_lossy().into_owned();if name.eq_ignore_ascii_case(".git"){continue}
            let path=entry.path().strip_prefix(root)?.to_string_lossy().replace('\\',"/");let metadata=fs::symlink_metadata(entry.path())?;
            if relative(root,&path,false).is_err(){continue}
            if metadata.is_dir(){if path.matches('/').count()<32&&(request.include_ignored||!matches!(name.as_str(),"node_modules"|"target"|"dist"|".next"|".venv"|"vendor")){pending.push(entry.path())}continue}
            if !metadata.is_file(){continue}
            if !request.content{if regex.is_match(&path){results.push(json!({"path":path,"line":null,"column":null,"text":name}))}continue}
            if metadata.len()>2*1024*1024{continue}bytes+=metadata.len();let data=fs::read(entry.path())?;let Ok(text)=std::str::from_utf8(&data)else{continue};if text.contains('\0'){continue}
            for (line_no,line) in text.lines().enumerate(){for found in regex.find_iter(line).take(20){let column=line[..found.start()].chars().count()+1;results.push(json!({"path":path,"line":line_no+1,"column":column,"text":line.chars().take(400).collect::<String>()}));if results.len()>=2000{limited=true;break}}if limited{break}}
            if results.len()>=2000{limited=true;break}
        }
        if limited{break}
    }
    results.sort_by_key(|item|format!("{}:{:010}:{:010}",item["path"].as_str().unwrap_or_default(),item["line"].as_u64().unwrap_or(0),item["column"].as_u64().unwrap_or(0)));
    if let Some(after)=&request.after{results.retain(|item|format!("{}:{:010}:{:010}",item["path"].as_str().unwrap_or_default(),item["line"].as_u64().unwrap_or(0),item["column"].as_u64().unwrap_or(0))>*after)}
    let next=if results.len()>250{let item=&results[249];Some(format!("{}:{:010}:{:010}",item["path"].as_str().unwrap_or_default(),item["line"].as_u64().unwrap_or(0),item["column"].as_u64().unwrap_or(0)))}else{None};results.truncate(250);
    Ok(json!({"results":results,"next":next,"limited":limited,"scanned":scanned}))
}
impl Core{
    pub(super) async fn workbench_file_tools(&self,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value.clone())?;let root=self.workbench_root(&request.scope).await?;let data_dir=self.data_dir.clone();
        if request.operation=="search"{return tokio::task::spawn_blocking(move||search(&root,&request)).await?}
        let _guard=MUTATIONS.lock().await;
        tokio::task::spawn_blocking(move||{
            if request.operation=="versions"{let paths=request.paths.as_ref().context("workbench.paths_required")?;ensure!(paths.len()<=64,"workbench.path_limit");let mut versions=Vec::new();for path in paths{let target=relative(&root,path,true)?;let value=if target.is_file(){let metadata=fs::metadata(&target)?;if metadata.len()>16*1024*1024{json!({"path":path,"state":"too_large"})}else{json!({"path":path,"state":"present","digest":crate::backup::file_digest(&target)?.1})}}else{json!({"path":path,"state":"missing"})};versions.push(value)}return Ok(json!({"versions":versions}))}
            if request.operation=="inspect"{let path=relative(&root,&request.path,false)?;let (digest,entries)=snapshot(&path)?;return Ok(json!({"path":request.path,"digest":digest,"directory":path.is_dir(),"files":entries.len(),"bytes":entries.iter().map(|entry|entry.bytes).sum::<u64>()}))}
            let trash=trash_root(&root)?;
            if request.operation=="trash_list"{let mut entries=Vec::new();if trash.exists(){crate::platform::private_storage::prepare_private_directory(&trash)?;for entry in fs::read_dir(&trash)?.take(10001){let entry=entry?;if !entry.file_type()?.is_dir(){continue}let metadata=entry.path().join("entry.json");if metadata.is_file(){let value:Value=serde_json::from_slice(&fs::read(metadata)?)?;if value["root"]==json!(root)&&entry.path().join("content").exists(){entries.push(value)}}}}return Ok(json!({"entries":entries}))}
            ensure!(matches!(request.operation.as_str(),"copy"|"trash"|"restore"|"move"),"workbench.operation_not_admitted");
            let id=request.command_id.as_deref().context("workbench.command_id_required")?;uuid::Uuid::parse_str(id)?;
            let receipts=data_dir.join("workspace-operations");crate::platform::private_storage::prepare_private_directory(&receipts)?;let receipt=receipts.join(format!("{id}.json"));let identity=crate::command::canonical_json_digest(&json!({"root":root,"request":value}))?;
            if receipt.exists(){let previous:Value=serde_json::from_slice(&fs::read(&receipt)?)?;ensure!(previous["identity"]==identity,"workbench.command_conflict");ensure!(previous["state"]=="applied","workbench.operation_unknown_check_state");return Ok(previous["result"].clone())}
            ensure!(!request.path.is_empty(),"workbench.root_immutable");
            let path=relative(&root,&request.path,request.operation=="restore")?;ensure!(path!=root,"workbench.root_immutable");
            crate::platform::private_storage::atomic_write_private_json(&receipt,&json!({"identity":identity,"state":"started"}))?;
            let result=(||->Result<Value>{
                if request.operation=="restore"{let trash_id=request.trash_id.as_deref().context("workbench.trash_id_required")?;uuid::Uuid::parse_str(trash_id)?;let source=trash.join(trash_id);let metadata:Value=serde_json::from_slice(&fs::read(source.join("entry.json"))?)?;ensure!(metadata["root"]==json!(root)&&metadata["path"]==request.path,"workbench.trash_identity_changed");ensure!(!path.exists(),"workbench.destination_exists");let (digest,_)=snapshot(&source.join("content"))?;ensure!(metadata["digest"]==digest,"workbench.trash_changed");fs::rename(source.join("content"),&path)?;return Ok(json!({"applied":true,"path":request.path}))}
                let (digest,_)=snapshot(&path)?;ensure!(request.expected_digest.as_deref()==Some(digest.as_str()),"workbench.file_changed");
                if request.operation=="trash"{crate::platform::private_storage::prepare_private_directory(&trash)?;let directory=trash.join(id);crate::platform::private_storage::prepare_private_directory(&directory)?;crate::platform::private_storage::atomic_write_private_json(&directory.join("entry.json"),&json!({"root":root,"path":request.path,"digest":digest,"trashId":id,"deletedAt":Utc::now().to_rfc3339()}))?;fs::rename(&path,directory.join("content"))?;Ok(json!({"applied":true,"trashId":id}))}
                else{let destination=relative(&root,request.destination.as_deref().context("workbench.destination_required")?,true)?;ensure!(destination!=root&&!destination.exists()&&!destination.starts_with(&path),"workbench.invalid_destination");if request.operation=="copy"{copy_tree(&path,&destination,&digest)?}else{ensure!(snapshot(&path)?.0==digest,"workbench.file_changed");fs::rename(&path,&destination)?;}Ok(json!({"applied":true}))}
            })();
            match result{Ok(result)=>{crate::platform::private_storage::atomic_write_private_json(&receipt,&json!({"identity":identity,"state":"applied","result":result}))?;Ok(result)},Err(error)=>{crate::platform::private_storage::atomic_write_private_json(&receipt,&json!({"identity":identity,"state":"failed","error":error.to_string()}))?;Err(error)}}
        }).await?
    }
}
