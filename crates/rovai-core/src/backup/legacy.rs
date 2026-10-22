//! 旧 Rovai Core 根目录的离线副本迁入；模型材料和冻结证据原字节保留。
use super::*;

#[derive(Serialize,Deserialize,Clone)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Preview {
    pub source:PathBuf,
    pub digest:String,
    pub authority:String,
    pub file_count:usize,
    pub total_bytes:u64,
    pub external_roots:Vec<owned_roots::OwnedRoot>,
}
fn source_root(source:&Path,target:&Path)->Result<PathBuf>{
    ensure!(source.is_absolute()&&source.is_dir(),"migration.source_directory_required");
    no_link(source)?;
    let source=fs::canonicalize(source)?;let target=fs::canonicalize(target)?;
    ensure!(!source.starts_with(&target)&&!target.starts_with(&source),"migration.roots_overlap");
    ensure!(!source.join("config-sync-pending-v1.json").exists(),"migration.source_recovery_required");
    Ok(source)
}
fn snapshot(source:&Path,cancel:&CancellationToken)->Result<(Preview,Vec<Entry>)>{
    let candidates=["rovai.sqlite","lumen.sqlite"].into_iter().filter(|name|source.join(name).exists()).collect::<Vec<_>>();
    ensure!(candidates.len()==1,"migration.single_authority_required");
    let authority=candidates[0];let database=source.join(authority);no_link(&database)?;
    let mut header=[0_u8;16];fs::File::open(database)?.read_exact(&mut header)?;ensure!(&header==b"SQLite format 3\0","migration.invalid_authority_header");
    let administrative=administrative(source)?;
    ensure!(!administrative.join("pending.json").exists(),"migration.source_recovery_required");
    let mut entries=Vec::new();let mut total=0_u64;
    for (index,(path,file))in files(source)?.into_iter().enumerate(){
        ensure!(!cancel.is_cancelled(),"backup.cancelled");
        let(size,sha256)=hash_file(&file)?;total=total.checked_add(size).context("backup.total_overflow")?;
        ensure!(size<=8*1024*1024*1024&&total<=64*1024*1024*1024,"backup.total_limit");
        entries.push(Entry{root_id:"data".into(),path,size,sha256});progress(index as u64+1,index as u64+1,"migration_inspect");
    }
    let external_roots=owned_roots::configured(source)?.into_iter().filter(|root|!root.path.starts_with(source)).collect::<Vec<_>>();
    let digest=format!("{:x}",Sha256::digest(serde_json::to_vec(&(source,authority,&entries,&external_roots))?));
    Ok((Preview{source:source.into(),digest,authority:authority.into(),file_count:entries.len(),total_bytes:total,external_roots},entries))
}
pub fn preview(target:&Path,source:&Path,cancel:&CancellationToken)->Result<Preview>{
    let source=source_root(source,target)?;let lease=CoreDataDirLease::acquire(&source)?;
    Ok(snapshot(lease.data_dir(),cancel)?.0)
}
pub fn stage(target:&Path,source:&Path,expected:&str,cancel:&CancellationToken)->Result<Manifest>{
    ensure!(expected.len()==64&&expected.bytes().all(|byte|byte.is_ascii_hexdigit()),"migration.preview_required");
    let source=source_root(source,target)?;
    let source_lease=CoreDataDirLease::acquire(&source)?;let target_lease=CoreDataDirLease::acquire(target)?;
    let target=target_lease.data_dir();let source=source_lease.data_dir();
    ensure!(!target.join("config-sync-pending-v1.json").exists(),"backup.configuration_recovery_required");
    let administrative=administrative(target)?;ensure!(!administrative.join("pending.json").exists(),"backup.restore_already_pending");
    let (preview,entries)=snapshot(source,cancel)?;ensure!(preview.digest==expected,"migration.source_changed");
    let operation=administrative.join(uuid::Uuid::new_v4().to_string());prepare_private_directory(&operation)?;
    let staging=operation.join("staging");prepare_private_directory(&staging)?;
    let result=(||->Result<Manifest>{
        for(index,entry)in entries.iter().enumerate(){
            ensure!(!cancel.is_cancelled(),"backup.cancelled");
            let destination=staging.join(&entry.path);prepare_private_directory(destination.parent().context("backup.invalid_parent")?)?;
            let mut input=fs::File::open(source.join(&entry.path))?;let mut output=crate::platform::private_storage::create_private_new_file(&destination)?;
            let size=std::io::copy(&mut input,&mut output)?;output.sync_all()?;
            ensure!(size==entry.size&&hash_file(&destination)?==(entry.size,entry.sha256.clone()),"backup.checksum_mismatch");
            progress(index as u64+1,entries.len()as u64,"migration_copy");
        }
        ensure!(snapshot(source,cancel)?.0.digest==expected,"migration.source_changed");
        let manifest=Manifest{runtime_links:Vec::new(),file_modes:Default::default(),format_version:2,product:"codeg-rovai-core".into(),app_version:"legacy-rovai".into(),created_at:chrono::Utc::now().to_rfc3339(),entries,roots:Vec::new()};
        let mut units=fs::read_dir(target)?.map(|entry|entry.map(|entry|entry.file_name().to_string_lossy().into_owned())).collect::<std::io::Result<HashSet<_>>>()?;
        units.extend(manifest.entries.iter().filter_map(|entry|entry.path.split('/').next().map(str::to_owned)));units.retain(|unit|!excluded(unit));let mut units=units.into_iter().collect::<Vec<_>>();units.sort();
        let safety=operation.join("before");prepare_private_directory(&safety)?;
        atomic_write_private_json(&operation.join("legacy-source.json"),&preview)?;
        let pending=Pending{root:target.into(),staging:staging.clone(),safety,manifest:manifest.clone(),state:"staged".into(),units,owned:Vec::new()};
        atomic_write_private_json(&administrative.join("pending.json"),&pending)?;Ok(manifest)
    })();
    if result.is_err()&&!administrative.join("pending.json").exists(){let _=fs::remove_dir_all(operation);}result
}
