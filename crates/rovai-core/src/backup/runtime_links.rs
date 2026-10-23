//! 自定义包安装的链接作为加密清单的一部分保留，不遍历链接目标。
use super::*;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct RuntimeLink{pub path:String,pub target:String,pub directory:bool}
fn payload(path:&str)->Result<PathBuf>{
    name(path)?;let parts=path.split('/').collect::<Vec<_>>();
    ensure!(parts.len()>=7&&parts[0]=="custom-runtimes"&&parts[5]=="payload","backup.runtime_link_outside_payload");
    uuid::Uuid::parse_str(parts[1])?;uuid::Uuid::parse_str(parts[4])?;
    ensure!(parts[2].parse::<u64>().is_ok_and(|revision|revision>0)&&!parts[3].is_empty(),"backup.runtime_link_invalid_installation");
    Ok(parts[..6].iter().collect())
}
fn relative_target(link:&RuntimeLink)->Result<Option<PathBuf>>{
    let scope=payload(&link.path)?;ensure!(!link.target.is_empty()&&link.target.len()<=16384&&!link.target.contains('\0'),"backup.runtime_link_target_invalid");
    let target=Path::new(&link.target);if target.is_absolute()||link.target.contains(':')||link.target.starts_with(['\\','/']){return Ok(None)}
    let mut resolved=Path::new(&link.path).parent().context("backup.runtime_link_parent_required")?.to_path_buf();
    for part in target.components(){match part{Component::Normal(value)=>resolved.push(value),Component::CurDir=>{},Component::ParentDir=>{if !resolved.pop(){return Ok(None)}},_=>return Ok(None)}}
    Ok(resolved.starts_with(scope).then_some(resolved))
}
pub(super) fn scan(root:&Path,ignored:&[PathBuf])->Result<(Vec<(String,PathBuf)>,Vec<RuntimeLink>)>{
    let mut pending=vec![(root.to_path_buf(),0usize)];let mut files=Vec::new();let mut links=Vec::new();let mut seen=0usize;
    while let Some((directory,depth))=pending.pop(){ensure!(depth<=128,"backup.runtime_directory_depth_limit");no_link(&directory)?;
        for entry in fs::read_dir(directory)?{let entry=entry?;let path=entry.path();let relative=path.strip_prefix(root)?.to_str().context("backup.path_encoding_invalid")?.replace('\\',"/");
            if excluded(&relative)||ignored.iter().any(|ignored|path.starts_with(ignored)){continue}name(&relative)?;seen+=1;ensure!(seen<=200000,"backup.entry_limit");
            let metadata=fs::symlink_metadata(&path)?;
            let linked=metadata.file_type().is_symlink();
            #[cfg(windows)]let linked={use std::os::windows::fs::MetadataExt;linked||metadata.file_attributes()&0x400!=0};
            if linked{
                payload(&relative)?;let target=fs::read_link(&path)?.to_str().context("backup.link_encoding_invalid")?.to_owned();
                #[cfg(windows)]let directory={use std::os::windows::fs::MetadataExt;metadata.file_attributes()&0x10!=0};
                #[cfg(not(windows))]let directory=fs::metadata(&path).is_ok_and(|metadata|metadata.is_dir());
                let link=RuntimeLink{path:relative,target,directory};relative_target(&link)?;links.push(link);continue
            }
            no_link(&path)?;if metadata.is_dir(){pending.push((path,depth+1))}else{ensure!(metadata.is_file(),"backup.special_file");files.push((relative,path));}
        }
    }
    files.sort_by(|left,right|left.0.cmp(&right.0));links.sort_by(|left,right|left.path.cmp(&right.path));Ok((files,links))
}
pub(super) fn validate(manifest:&Manifest)->Result<()>{
    ensure!(manifest.entries.len()+manifest.runtime_links.len()<=200000,"backup.entry_limit");
    if manifest.format_version<3{ensure!(manifest.runtime_links.is_empty()&&manifest.file_modes.is_empty(),"backup.legacy_extra_runtime_metadata");}
    let archive_paths=manifest.entries.iter().map(|entry|archive_name(&entry.root_id,&entry.path)).collect::<HashSet<_>>();
    let mut paths=manifest.entries.iter().filter(|entry|entry.root_id=="data").map(|entry|entry.path.clone()).collect::<std::collections::BTreeSet<_>>();
    for link in &manifest.runtime_links{ensure!(paths.insert(link.path.clone()),"backup.duplicate_runtime_link");}
    for link in &manifest.runtime_links{
        relative_target(link)?;let prefix=format!("{}/",link.path);
        ensure!(!paths.range(prefix.clone()..).next().is_some_and(|path|path.starts_with(&prefix)),"backup.entry_below_runtime_link");
    }
    for(path,mode)in &manifest.file_modes{ensure!(*mode<=0o777&&archive_paths.contains(path),"backup.invalid_file_mode");}Ok(())
}
pub(super) fn mode(path:&Path)->Result<Option<u32>>{
    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;Ok(Some(fs::metadata(path)?.permissions().mode()&0o777))}
    #[cfg(not(unix))]{let _=path;Ok(None)}
}
pub(super) fn restore_mode(path:&Path,mode:Option<u32>)->Result<()>{
    #[cfg(unix)]if let Some(mode)=mode{use std::os::unix::fs::PermissionsExt;ensure!(mode<=0o777,"backup.invalid_file_mode");fs::set_permissions(path,fs::Permissions::from_mode(mode))?;}
    #[cfg(not(unix))]let _=(path,mode);
    Ok(())
}
pub(super) fn restore(staging:&Path,manifest:&Manifest)->Result<()>{
    validate(manifest)?;let data=staging.to_path_buf();let mut unresolved=Vec::new();let mut created=Vec::new();
    for link in &manifest.runtime_links{
        let Some(_)=relative_target(link)?else{unresolved.push(serde_json::json!({"link":link,"reason":"external_or_absolute_target_requires_environment_repair"}));continue};
        let destination=data.join(&link.path);ensure!(!destination.exists()&&fs::symlink_metadata(&destination).is_err(),"backup.runtime_link_destination_exists");
        prepare_private_directory(destination.parent().context("backup.runtime_link_parent_required")?)?;
        #[cfg(unix)]let result=std::os::unix::fs::symlink(&link.target,&destination);
        #[cfg(windows)]let result=if link.directory{std::os::windows::fs::symlink_dir(&link.target,&destination)}else{std::os::windows::fs::symlink_file(&link.target,&destination)};
        #[cfg(not(any(unix,windows)))]let result=Err(std::io::Error::new(std::io::ErrorKind::Unsupported,"symbolic links unavailable"));
        match result{Err(error)=>unresolved.push(serde_json::json!({"link":link,"reason":"host_link_creation_unavailable","detail":error.to_string()})),Ok(())=>created.push(link)}
    }
    for link in created{
        let path=data.join(&link.path);let scope=fs::canonicalize(data.join(payload(&link.path)?))?;
        if !fs::canonicalize(&path).is_ok_and(|target|target.starts_with(&scope)){
            #[cfg(windows)]if link.directory{fs::remove_dir(&path)?;}else{fs::remove_file(&path)?;}
            #[cfg(not(windows))]fs::remove_file(&path)?;
            unresolved.push(serde_json::json!({"link":link,"reason":"target_missing_or_outside_owned_payload"}));
        }
    }
    if !unresolved.is_empty(){atomic_write_private_json(&data.join("custom-runtime-restore-needs-environment-v1.json"),&serde_json::json!({"schemaVersion":1,"unresolvedLinks":unresolved,"historicalLocksRetained":true,"automaticExecutionAllowed":false}))?;}
    Ok(())
}
