//! Host-selected storage locations; archives cannot choose restore destinations.
use super::*;
pub const REGISTRY_FILE:&str="backup-owned-roots-v1.json";
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct OwnedRoot {pub id:String,pub kind:String,pub path:PathBuf,pub shared:bool}
fn validate(root:&Path,value:&OwnedRoot)->Result<()> {
    ensure!(["skill_library","runtime_files","mcp_config"].contains(&value.id.as_str())&&["directory","file"].contains(&value.kind.as_str()),"backup.invalid_owned_root");
    ensure!(value.path.is_absolute()&&value.path!=root&&!root.starts_with(&value.path)&&!value.path.components().any(|part|matches!(part,Component::CurDir|Component::ParentDir)),"backup.invalid_owned_path");
    let mut current=Some(value.path.as_path());while let Some(path)=current{if path.exists(){no_link(path)?;}current=path.parent();}Ok(())
}
pub fn register(root:&Path,skills:&Path,runtime_files:&Path,mcp:&Path)->Result<()> {
    let root=fs::canonicalize(root)?;let mut entries=Vec::new();
    for(id,kind,path,shared)in [("skill_library","directory",skills,true),("runtime_files","directory",runtime_files,false),("mcp_config","file",mcp,true)]{
        if path.starts_with(&root){continue}let value=OwnedRoot{id:id.into(),kind:kind.into(),path:path.into(),shared};validate(&root,&value)?;entries.push(value);
    }
    for (index,value)in entries.iter().enumerate(){for other in entries.iter().skip(index+1){ensure!(!value.path.starts_with(&other.path)&&!other.path.starts_with(&value.path),"backup.overlapping_owned_roots");}}
    atomic_write_private_json(&root.join(REGISTRY_FILE),&entries)
}
pub fn configured(root:&Path)->Result<Vec<OwnedRoot>> {
    let path=root.join(REGISTRY_FILE);if !path.exists(){return Ok(Vec::new())}no_link(&path)?;ensure!(fs::metadata(&path)?.len()<=65536,"backup.owned_registry_limit");let entries:Vec<OwnedRoot>=serde_json::from_slice(&fs::read(path)?)?;ensure!(entries.len()<=3,"backup.owned_registry_limit");let mut ids=HashSet::new();for value in &entries{validate(root,value)?;ensure!(ids.insert(&value.id),"backup.duplicate_owned_root");}Ok(entries)
}
pub fn existing(root:&Path)->Result<Vec<OwnedRoot>>{let mut values=Vec::new();for mut value in configured(root)?{if !value.path.exists(){continue}let metadata=fs::metadata(&value.path)?;ensure!(if value.kind=="file"{metadata.is_file()}else{metadata.is_dir()},"backup.owned_root_kind_changed");value.path=fs::canonicalize(&value.path)?;validate(root,&value)?;if !value.path.starts_with(root){values.push(value)}}Ok(values)}
pub fn coverage(root:&Path)->Result<serde_json::Value>{let root=fs::canonicalize(root)?;Ok(serde_json::json!({"coreRoot":root,"coreIncludes":["database","canvas","uiPreferences","attachments","managedBlobs"],"ownedRoots":configured(&root)?.into_iter().map(|value|serde_json::json!({"id":value.id,"kind":value.kind,"path":value.path,"shared":value.shared,"exists":value.path.exists()})).collect::<Vec<_>>(),"projectFilesIncluded":false,"nativeProviderHomesIncluded":false}))}
