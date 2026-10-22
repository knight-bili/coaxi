//! Host-selected storage locations; archives cannot choose restore destinations.
use super::*;
pub const REGISTRY_FILE:&str="backup-owned-roots-v1.json";
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct OwnedRoot {pub id:String,pub kind:String,pub path:PathBuf,pub shared:bool}
fn validate(root:&Path,value:&OwnedRoot)->Result<()> {
    ensure!(["skill_library","runtime_files","mcp_config","desktop_preferences"].contains(&value.id.as_str())&&["directory","file","preferences"].contains(&value.kind.as_str()),"backup.invalid_owned_root");
    ensure!(value.path.is_absolute()&&((value.id=="desktop_preferences"&&value.kind=="preferences"&&value.path==root)||(value.path!=root&&!root.starts_with(&value.path)))&&!value.path.components().any(|part|matches!(part,Component::CurDir|Component::ParentDir)),"backup.invalid_owned_path");
    let mut current=Some(value.path.as_path());while let Some(path)=current{if path.exists(){no_link(path)?;}current=path.parent();}Ok(())
}
pub fn register(root:&Path,skills:&Path,runtime_files:&Path,mcp:&Path)->Result<()> {
    let root=fs::canonicalize(root)?;let mut entries=Vec::new();
    for(id,kind,path,shared)in [("skill_library","directory",skills,true),("runtime_files","directory",runtime_files,false),("mcp_config","file",mcp,true)]{
        let value=OwnedRoot{id:id.into(),kind:kind.into(),path:path.into(),shared:shared&&!path.starts_with(&root)};validate(&root,&value)?;entries.push(value);
    }
    let desktop=root.join("backup-desktop-preferences-v1.json");if desktop.exists(){no_link(&desktop)?;ensure!(fs::metadata(&desktop)?.len()<=65536,"backup.desktop_registry_limit");let value:OwnedRoot=serde_json::from_slice(&fs::read(desktop)?)?;ensure!(value.id=="desktop_preferences"&&value.kind=="preferences","backup.invalid_desktop_root");validate(&root,&value)?;entries.push(value);}
    for (index,value)in entries.iter().enumerate(){for other in entries.iter().skip(index+1){ensure!(!effective_paths(value).iter().any(|a|effective_paths(other).iter().any(|b|a.starts_with(b)||b.starts_with(a))),"backup.overlapping_owned_roots");}}
    atomic_write_private_json(&root.join(REGISTRY_FILE),&entries)?;let issue=root.join("backup-owned-roots-issue.json");if issue.exists(){no_link(&issue)?;fs::remove_file(issue)?;}Ok(())
}
pub fn unavailable(root:&Path,message:&str)->Result<()>{atomic_write_private_json(&root.join("backup-owned-roots-issue.json"),&serde_json::json!({"code":"backup.owned_storage_registration_unavailable","message":message}))}
pub fn effective_paths(value:&OwnedRoot)->Vec<PathBuf>{if value.kind=="preferences"{super::desktop_preferences::UNITS.iter().map(|(name,_)|value.path.join(name)).collect()}else{vec![value.path.clone()]}}
pub fn register_desktop(root:&Path,preferences:&Path)->Result<()> {let root=fs::canonicalize(root)?;let value=OwnedRoot{id:"desktop_preferences".into(),kind:"preferences".into(),path:fs::canonicalize(preferences)?,shared:false};validate(&root,&value)?;atomic_write_private_json(&root.join("backup-desktop-preferences-v1.json"),&value)?;let mut entries=configured(&root)?;entries.retain(|entry|entry.id!="desktop_preferences");entries.push(value);atomic_write_private_json(&root.join(REGISTRY_FILE),&entries)}
pub fn configured(root:&Path)->Result<Vec<OwnedRoot>> {
    ensure!(!root.join("backup-owned-roots-issue.json").exists(),"backup.owned_storage_registration_unavailable");let path=root.join(REGISTRY_FILE);if !path.exists(){return Ok(Vec::new())}no_link(&path)?;ensure!(fs::metadata(&path)?.len()<=65536,"backup.owned_registry_limit");let entries:Vec<OwnedRoot>=serde_json::from_slice(&fs::read(path)?)?;ensure!(entries.len()<=4,"backup.owned_registry_limit");let mut ids=HashSet::new();for value in &entries{validate(root,value)?;ensure!(ids.insert(&value.id),"backup.duplicate_owned_root");}Ok(entries)
}
pub fn snapshot_roots(root:&Path)->Result<Vec<OwnedRoot>>{let mut values=Vec::new();for mut value in configured(root)?{if value.path.exists(){let metadata=fs::metadata(&value.path)?;ensure!(if value.kind=="file"{metadata.is_file()}else{metadata.is_dir()},"backup.owned_root_kind_changed");value.path=fs::canonicalize(&value.path)?;validate(root,&value)?;}values.push(value)}for(index,value)in values.iter().enumerate(){for other in values.iter().skip(index+1){ensure!(!effective_paths(value).iter().any(|a|effective_paths(other).iter().any(|b|a.starts_with(b)||b.starts_with(a))),"backup.overlapping_owned_roots");}}Ok(values)}
pub fn coverage(root:&Path)->Result<serde_json::Value>{let root=fs::canonicalize(root)?;Ok(serde_json::json!({"coreRoot":root,"coreIncludes":["database","canvas","uiPreferences","attachments","managedBlobs"],"ownedRoots":configured(&root)?.into_iter().map(|value|serde_json::json!({"id":value.id,"kind":value.kind,"path":value.path,"shared":value.shared,"exists":value.path.exists()})).collect::<Vec<_>>(),"projectFilesIncluded":false,"nativeProviderHomesIncluded":false}))}

#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(super) struct RestoreRoot{pub id:String,pub kind:String,pub target:PathBuf,pub operation_id:String,#[serde(default="present_by_default")]pub present:bool,#[serde(default)]pub unit:Option<String>}
fn present_by_default()->bool{true}
fn resolved_target(root:&Path,value:&OwnedRoot)->Result<PathBuf>{validate(root,value)?;if value.path.exists(){return Ok(fs::canonicalize(&value.path)?)}let parent=value.path.parent().context("backup.owned_parent_required")?;if parent.exists(){Ok(fs::canonicalize(parent)?.join(value.path.file_name().context("backup.owned_name_required")?))}else{ensure!(value.path.starts_with(root),"backup.owned_parent_missing");Ok(value.path.clone())}}
pub(super) fn prepare_restore(root:&Path,_operation:&Path,manifest:&Manifest,cancel:&CancellationToken)->Result<Vec<RestoreRoot>>{
    let configured=configured(root)?;let mut result=Vec::new();for source in &manifest.roots{ensure!(!cancel.is_cancelled(),"backup.cancelled");let target=configured.iter().find(|target|target.id==source.id&&target.kind==source.kind).context("backup.restore_root_not_configured")?;let path=resolved_target(root,target)?;
        if source.kind=="preferences"{for unit in super::desktop_preferences::restore_units(&source.units)?{result.push(RestoreRoot{id:source.id.clone(),kind:unit.kind.clone(),target:path.join(&unit.name),operation_id:uuid::Uuid::new_v4().to_string(),present:unit.present,unit:Some(unit.name)});}continue}
        if source.present&&source.kind=="file"{ensure!(manifest.entries.iter().filter(|entry|entry.root_id==source.id).count()==1,"backup.file_root_content_missing");}
        result.push(RestoreRoot{id:source.id.clone(),kind:source.kind.clone(),target:path,operation_id:uuid::Uuid::new_v4().to_string(),present:source.present,unit:None});
    }Ok(result)
}
fn paths(value:&RestoreRoot)->Result<(PathBuf,PathBuf,PathBuf)>{uuid::Uuid::parse_str(&value.operation_id)?;ensure!(["skill_library","runtime_files","mcp_config","desktop_preferences"].contains(&value.id.as_str()),"backup.invalid_restore_root");let parent=value.target.parent().context("backup.owned_parent_required")?;Ok((parent.join(format!(".rovai-restore-{}-{}",value.operation_id,value.id)),parent.join(format!(".rovai-before-{}-{}",value.operation_id,value.id)),parent.join(format!(".rovai-after-{}-{}",value.operation_id,value.id))))}
fn validate_plan(root:&Path,plan:&[RestoreRoot])->Result<()> {let configured=configured(root)?;let mut seen=HashSet::new();for value in plan{ensure!(seen.insert((&value.id,&value.unit)),"backup.duplicate_restore_root");let parent=configured.iter().find(|candidate|candidate.id==value.id).context("backup.restore_root_not_configured")?;
    let expected=if let Some(unit)=&value.unit{ensure!(parent.kind=="preferences"&&super::desktop_preferences::UNITS.iter().any(|(name,kind)|*name==unit&&*kind==value.kind),"backup.invalid_desktop_unit");resolved_target(root,parent)?.join(unit)}else{ensure!(parent.kind==value.kind,"backup.owned_kind_changed");resolved_target(root,parent)?};ensure!(expected==value.target,"backup.restore_root_configuration_changed");let(stage,before,after)=paths(value)?;for path in [&stage,&before,&after]{if path.exists(){no_link(path)?}ensure!(path.parent()==value.target.parent()&&path!=&value.target,"backup.invalid_owned_operation_path");}}Ok(())}
fn relative_entry(value:&RestoreRoot,entry:&Entry)->Option<String>{if entry.root_id!=value.id{return None}match value.unit.as_deref(){None=>Some(entry.path.clone()),Some(unit)if entry.path==unit=>Some("content".into()),Some(unit)=>entry.path.strip_prefix(&format!("{unit}/")).map(str::to_owned)}}
fn verify_root(value:&RestoreRoot,path:&Path,manifest:&Manifest)->Result<()> {if !value.present{ensure!(!path.exists(),"backup.owned_root_should_be_absent");return Ok(())}no_link(path)?;for entry in manifest.entries.iter().filter(|entry|relative_entry(value,entry).is_some()){let file=if value.kind=="file"{path.to_path_buf()}else{path.join(relative_entry(value,entry).context("backup.invalid_owned_entry")?)};no_link(&file)?;ensure!(hash_file(&file)?==(entry.size,entry.sha256.clone()),"backup.owned_checksum_mismatch");}Ok(())}
fn copy_stage(value:&RestoreRoot,stage:&Path,source:&Path,manifest:&Manifest)->Result<()> {
    ensure!(!stage.exists(),"backup.owned_staging_exists");fs::create_dir(stage)?;prepare_private_directory(stage)?;
    for entry in manifest.entries.iter().filter(|entry|relative_entry(value,entry).is_some()){let from=source.join(&value.id).join(&entry.path);let target=stage.join(relative_entry(value,entry).context("backup.invalid_owned_entry")?);prepare_private_directory(target.parent().context("backup.invalid_parent")?)?;let mut file=crate::platform::private_storage::create_private_new_file(&target)?;let mut input=fs::File::open(&from)?;let copied=std::io::copy(&mut input,&mut file)?;file.sync_all()?;ensure!(copied==entry.size,"backup.owned_copy_size_changed");}
    verify_root(value,&if value.kind=="file"{stage.join("content")}else{stage.to_path_buf()},manifest)
}
pub(super) fn apply_restore(root:&Path,plan:&[RestoreRoot],manifest:&Manifest,safety:&Path)->Result<()> {
    validate_plan(root,plan)?;let source=safety.parent().context("backup.invalid_operation")?.join("owned");
    for value in plan{let(stage,before,_)=paths(value)?;let marker=safety.join(format!("owned-{}-{}.json",value.id,value.unit.as_deref().unwrap_or("root")));
        let mut state=if marker.exists(){serde_json::from_slice::<serde_json::Value>(&fs::read(&marker)?)?}else{let state=serde_json::json!({"hadOriginal":value.target.exists(),"prepared":false,"done":false});atomic_write_private_json(&marker,&state)?;state};
        if state["done"]==true{verify_root(value,&value.target,manifest)?;continue}
        if value.present&&state["prepared"]!=true{
            if stage.exists(){no_link(&stage)?;if stage.is_dir(){fs::remove_dir_all(&stage)?}else{fs::remove_file(&stage)?}}
            let parent=value.target.parent().context("backup.owned_parent_required")?;if !parent.exists(){ensure!(parent.starts_with(root),"backup.owned_parent_missing");prepare_private_directory(parent)?;}
            copy_stage(value,&stage,&source,manifest)?;state["prepared"]=serde_json::json!(true);atomic_write_private_json(&marker,&state)?;
        }
        if state["hadOriginal"]==true&&!before.exists(){ensure!(value.target.exists(),"backup.owned_original_missing");no_link(&value.target)?;fs::rename(&value.target,&before)?;}
        if value.present&&stage.exists(){let replacement=if value.kind=="file"{stage.join("content")}else{stage.clone()};if replacement.exists(){ensure!(!value.target.exists(),"backup.owned_destination_changed");no_link(&replacement)?;fs::rename(&replacement,&value.target)?;}if value.kind=="file"{let _=fs::remove_dir(&stage);}}
        verify_root(value,&value.target,manifest)?;state["done"]=serde_json::json!(true);atomic_write_private_json(&marker,&state)?;
    }Ok(())
}
pub(super) fn rollback_restore(root:&Path,plan:&[RestoreRoot],safety:&Path)->Result<()> {
    validate_plan(root,plan)?;for value in plan.iter().rev(){let(_,before,after)=paths(value)?;let marker=safety.join(format!("owned-{}-{}.json",value.id,value.unit.as_deref().unwrap_or("root")));if !marker.exists(){continue}let mut state:serde_json::Value=serde_json::from_slice(&fs::read(&marker)?)?;if state["rolledBack"]==true{continue}
        if before.exists(){if value.target.exists(){ensure!(!after.exists(),"backup.owned_rollback_destination_changed");no_link(&value.target)?;fs::rename(&value.target,&after)?;}fs::rename(&before,&value.target)?;}
        else if state["hadOriginal"]==false&&value.target.exists(){ensure!(!after.exists(),"backup.owned_rollback_destination_changed");no_link(&value.target)?;fs::rename(&value.target,&after)?;}
        state["rolledBack"]=serde_json::json!(true);atomic_write_private_json(&marker,&state)?;
    }Ok(())
}
