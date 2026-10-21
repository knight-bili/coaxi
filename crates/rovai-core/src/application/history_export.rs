//! Explicit local-owner export. Shared SQLite sources stay intact and are labelled.
use super::*;
use std::io::Write;

pub(super) fn export(directory:&Path,id:&str,expected:&str,destination:&Path)->Result<Value>{
    identifier(id)?;
    let manifest:Value=serde_json::from_slice(&fs::read(directory.join(format!("{id}.manifest.json")))?)?;
    ensure!(crate::command::canonical_json_digest(&manifest)?==expected,"history.library_changed");
    ensure!(destination.is_absolute()&&!destination.exists(),"history.export_choose_new_file");
    let parent=destination.parent().context("history.invalid_destination")?;reject_link(parent)?;
    let parent=fs::canonicalize(parent)?;
    ensure!(!parent.starts_with(fs::canonicalize(directory)?),"history.export_outside_library_required");
    let mut inputs=vec![("session.native.json".to_string(),directory.join(format!("{id}.native.json"))),("session.transcript.json".to_string(),directory.join(format!("{id}.transcript.json")))];
    let shared=manifest["snapshotRoot"].as_str();let mut source_hashes=Vec::new();
    if let Some(root)=shared{
        let root=PathBuf::from(root);reject_link(&root)?;let root=fs::canonicalize(root)?;
        let plans=fs::canonicalize(directory.join("plans"))?;ensure!(root.starts_with(&plans)&&root!=plans,"history.invalid_snapshot_root");
        let mut pending=vec![(root.clone(),0usize)];
        while let Some((folder,depth))=pending.pop(){ensure!(depth<=24,"history.depth_limit");for entry in fs::read_dir(folder)?{let entry=entry?;let path=entry.path();reject_link(&path)?;let metadata=entry.metadata()?;
            if metadata.is_dir(){pending.push((path,depth+1));continue}ensure!(metadata.is_file(),"history.invalid_snapshot_file");
            let relative=path.strip_prefix(&root)?.to_string_lossy().replace('\\',"/");source_hashes.push((relative.clone(),crate::backup::file_digest(&path)?));inputs.push((format!("source/{relative}"),path));ensure!(inputs.len()<=50002,"history.export_file_limit");
        }}
        source_hashes.sort_by(|a,b|a.0.cmp(&b.0));ensure!(manifest["snapshotDigest"].as_str()==Some(crate::command::canonical_json_digest(&json!(source_hashes))?.as_str()),"history.snapshot_changed");
    }
    let stage=directory.join(format!("export-{}.zip",uuid::Uuid::new_v4()));
    let result=(||->Result<Value>{
        let file=fs::OpenOptions::new().write(true).create_new(true).open(&stage)?;crate::platform::private_storage::repair_private_file(&stage)?;
        let mut zip=zip::ZipWriter::new(file);let options=zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).unix_permissions(0o600);
        let mut inventory=Vec::new();let mut total=0u64;
        for (name,path) in inputs{reject_link(&path)?;let size=fs::metadata(&path)?.len();ensure!(size<=512*1024*1024,"history.export_file_limit");total=total.checked_add(size).context("history.size_overflow")?;ensure!(total<=1024*1024*1024,"history.export_size_limit");
            let before=crate::backup::file_digest(&path)?;zip.start_file(&name,options)?;std::io::copy(&mut fs::File::open(&path)?,&mut zip)?;ensure!(crate::backup::file_digest(&path)?==before,"history.snapshot_changed");inventory.push(json!({"path":name,"bytes":size,"sha256":before}));
        }
        let public=library_entry(manifest.clone())?;zip.start_file("manifest.json",options)?;zip.write_all(&serde_json::to_vec_pretty(&json!({"format":"rovai-native-history-export","formatVersion":1,"entry":public,"sharedSourceSnapshot":shared.is_some(),"notice":if shared.is_some(){"Source is a shared native storage snapshot and can contain other sessions."}else{"Source contains the selected native log."},"files":inventory}))?)?;zip.finish()?.sync_all()?;
        let digest=crate::backup::file_digest(&stage)?;let mut output=fs::OpenOptions::new().write(true).create_new(true).open(destination)?;
        let copied=(||->Result<()>{crate::platform::private_storage::repair_private_file(destination)?;std::io::copy(&mut fs::File::open(&stage)?,&mut output)?;output.sync_all()?;Ok(())})();drop(output);
        if let Err(error)=copied{let _=fs::remove_file(destination);return Err(error)}
        Ok(json!({"exported":true,"sha256":digest,"bytes":fs::metadata(destination)?.len(),"sharedSourceSnapshot":shared.is_some()}))
    })();let _=fs::remove_file(stage);result
}
