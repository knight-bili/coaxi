//! 只在本产品新建的安装目录解包，不覆盖项目或原应用目录。
use super::*;
use std::{collections::BTreeSet,fs,io::{Read,Write},path::{Component,Path,PathBuf}};
use sha2::{Digest,Sha256};
pub const MAX_ARCHIVE:u64=512*1024*1024;
const MAX_EXPANDED:u64=1024*1024*1024;
const MAX_FILES:usize=100_000;
pub(crate) fn relative(value:&str)->Result<PathBuf>{
    let value=value.strip_prefix("./").unwrap_or(value);let path=Path::new(value);
    ensure!(!value.is_empty()&&!value.contains(['\\',':','\0'])&&path.components().all(|v|matches!(v,Component::Normal(_))),"custom_runtime.archive_path_invalid");
    for part in value.split('/'){
        let stem=part.split('.').next().unwrap_or_default().to_ascii_uppercase();
        let numbered_device=stem.strip_prefix("COM").or_else(||stem.strip_prefix("LPT")).is_some_and(|number|matches!(number,"1"|"2"|"3"|"4"|"5"|"6"|"7"|"8"|"9"));
        ensure!(!part.ends_with(['.',' '])&&!matches!(stem.as_str(),"CON"|"PRN"|"AUX"|"NUL")&&!numbered_device,"custom_runtime.archive_path_invalid");
    }Ok(path.to_path_buf())
}
fn write_file(root:&Path,name:&str,mut input:impl Read,size:u64,total:&mut u64,seen:&mut BTreeSet<String>,executable:bool)->Result<()>{
    let relative=relative(name)?;let key=if cfg!(windows){name.to_lowercase()}else{name.into()};ensure!(seen.insert(key)&&seen.len()<=MAX_FILES,"custom_runtime.archive_duplicate_or_capacity");
    *total=total.checked_add(size).context("custom_runtime.archive_capacity")?;ensure!(*total<=MAX_EXPANDED,"custom_runtime.archive_capacity");
    let target=root.join(relative);if let Some(parent)=target.parent(){fs::create_dir_all(parent)?}let mut file=fs::OpenOptions::new().create_new(true).write(true).open(&target)?;
    let count=std::io::copy(&mut input.by_ref().take(size+1),&mut file)?;ensure!(count==size,"custom_runtime.archive_size_changed");file.sync_all()?;
    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;fs::set_permissions(&target,fs::Permissions::from_mode(if executable{0o700}else{0o600}))?;}
    #[cfg(not(unix))]let _=executable;
    Ok(())
}
fn extract_tar(reader:impl Read,root:&Path,total:&mut u64,seen:&mut BTreeSet<String>)->Result<()>{
    let mut archive=tar::Archive::new(reader);let mut entries=0usize;
    for entry in archive.entries()?{entries+=1;ensure!(entries<=MAX_FILES,"custom_runtime.archive_capacity");let mut entry=entry?;let kind=entry.header().entry_type();ensure!(kind.is_file()||kind.is_dir(),"custom_runtime.archive_link_rejected");let name=entry.path()?.to_str().context("custom_runtime.archive_encoding_invalid")?.to_owned();if kind.is_dir(){relative(name.trim_end_matches('/'))?;continue}let size=entry.size();let executable=entry.header().mode()?&0o111!=0;write_file(root,&name,&mut entry,size,total,seen,executable)?;}
    Ok(())
}
pub(crate) fn extract(archive:&Path,root:&Path,command:&str)->Result<PathBuf>{
    crate::platform::private_storage::prepare_private_directory(root)?;
    let mut magic=[0u8;512];let mut file=fs::File::open(archive)?;let count=file.read(&mut magic)?;
    let mut total=0;let mut seen=BTreeSet::new();
    if count>=4&&magic[..2]==*b"PK"{
        let mut archive=zip::ZipArchive::new(fs::File::open(archive)?)?;ensure!(archive.len()<=MAX_FILES,"custom_runtime.archive_capacity");
        for index in 0..archive.len(){let mut entry=archive.by_index(index)?;let name=entry.name().to_owned();let mode=entry.unix_mode().unwrap_or(0);ensure!((mode&0o170000)!=0o120000,"custom_runtime.archive_link_rejected");if entry.is_dir(){relative(name.trim_end_matches('/'))?;continue}let size=entry.size();write_file(root,&name,&mut entry,size,&mut total,&mut seen,mode&0o111!=0)?;}
    }else if count>=2&&magic[..2]==[0x1f,0x8b]{
        extract_tar(flate2::read::GzDecoder::new(fs::File::open(archive)?),root,&mut total,&mut seen)?;
    }else if count>=262&&&magic[257..262]==b"ustar"{
        extract_tar(fs::File::open(archive)?,root,&mut total,&mut seen)?;
    }else{
        let size=fs::metadata(archive)?.len();write_file(root,command,fs::File::open(archive)?,size,&mut total,&mut seen,true)?;
    }
    let executable=root.join(relative(command)?);let metadata=fs::symlink_metadata(&executable)?;ensure!(metadata.is_file()&&!metadata.file_type().is_symlink(),"custom_runtime.binary_entry_missing");
    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;fs::set_permissions(&executable,fs::Permissions::from_mode(0o700))?;}
    Ok(executable.canonicalize()?)
}
/// 安装锁绑定受管目录里的全部文件、符号链接文本与实际入口。
/// 包管理器创建的内部链接可以保留；下载压缩包中的链接在解包处已拒绝。
pub(crate) fn tree_digest(root:&Path,entrypoint:&Path)->Result<String>{
    fn walk(root:&Path,path:&Path,items:&mut Vec<(String,PathBuf,bool)>)->Result<()>{
        ensure!(items.len()<=MAX_FILES,"custom_runtime.artifact_capacity");
        for entry in fs::read_dir(path)?{let entry=entry?;let metadata=fs::symlink_metadata(entry.path())?;let relative=entry.path().strip_prefix(root)?.to_string_lossy().replace('\\',"/");if metadata.file_type().is_symlink(){items.push((relative,entry.path(),true))}else if metadata.is_dir(){walk(root,&entry.path(),items)?}else{ensure!(metadata.is_file(),"custom_runtime.artifact_type_invalid");items.push((relative,entry.path(),false))}}
        Ok(())
    }
    let mut items=Vec::new();walk(root,root,&mut items)?;items.sort_by(|a,b|a.0.cmp(&b.0));let mut digest=Sha256::new();let mut total=0u64;
    for(name,path,link)in items{digest.update(name.as_bytes());digest.update([0]);if link{digest.update(b"link\0");digest.update(fs::read_link(path)?.to_string_lossy().as_bytes())}else{let mut file=fs::File::open(path)?;let mut buffer=[0u8;65536];loop{let n=file.read(&mut buffer)?;if n==0{break}total=total.checked_add(n as u64).context("custom_runtime.artifact_capacity")?;ensure!(total<=2*MAX_EXPANDED,"custom_runtime.artifact_capacity");digest.update(&buffer[..n]);}}digest.update([0]);}
    digest.update(b"entrypoint\0");digest.update(crate::agent_runtime_adapter::executable_fingerprint(entrypoint)?.as_bytes());Ok(format!("sha256:{:x}",digest.finalize()))
}
pub(crate) async fn download(url:&str,path:&Path,expected:Option<&str>,cancel:&tokio_util::sync::CancellationToken)->Result<String>{
    let client=reqwest::Client::builder().https_only(true).timeout(std::time::Duration::from_secs(180)).redirect(reqwest::redirect::Policy::limited(5)).build()?;
    let mut response=tokio::select!{result=client.get(url).send()=>result?.error_for_status()?,_=cancel.cancelled()=>anyhow::bail!("custom_runtime.install_cancelled")};
    ensure!(response.content_length().is_none_or(|size|size<=MAX_ARCHIVE),"custom_runtime.archive_capacity");
    let mut file=fs::OpenOptions::new().create_new(true).write(true).open(path)?;let mut total=0u64;let mut digest=Sha256::new();
    loop{let chunk=tokio::select!{result=response.chunk()=>result?,_=cancel.cancelled()=>anyhow::bail!("custom_runtime.install_cancelled")};let Some(chunk)=chunk else{break};total+=chunk.len()as u64;ensure!(total<=MAX_ARCHIVE,"custom_runtime.archive_capacity");digest.update(&chunk);file.write_all(&chunk)?;}file.sync_all()?;
    let observed=format!("{:x}",digest.finalize());ensure!(expected.is_none_or(|value|value.eq_ignore_ascii_case(&observed)),"custom_runtime.archive_digest_mismatch");Ok(format!("sha256:{observed}"))
}
