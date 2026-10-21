//! Immutable encrypted snapshots and an ETag-protected WebDAV index. Account
//! matching and URL construction follow Codeg network/webdav's closed base URL.
use anyhow::{Result,Context,ensure};
use serde::{Serialize,Deserialize};
use reqwest::{Client,Method,StatusCode,Url};
use sha2::{Digest,Sha256};
use std::{path::{Path,PathBuf},time::Duration};
use tokio::io::AsyncWriteExt;
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Config{pub endpoint:String,pub username:String,pub password:String,#[serde(default)]pub allow_insecure:bool}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Snapshot{pub id:String,pub created_at:String,pub sha256:String,pub bytes:u64}
#[derive(Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Index{format_version:u32,entries:Vec<Snapshot>}
pub struct WebDav{client:Client,config:Config,base:Url}
impl WebDav{
    pub fn new(config:Config)->Result<Self>{
        let mut base=Url::parse(&config.endpoint)?;
        ensure!(matches!(base.scheme(),"https"|"http")&&(base.scheme()=="https"||config.allow_insecure)&&base.username().is_empty()&&base.password().is_none()&&base.query().is_none()&&base.fragment().is_none()&&base.host_str().is_some(),"webdav.invalid_endpoint");
        ensure!(!config.username.contains(['\r','\n'])&&!config.password.contains(['\r','\n'])&&config.username.len()<=1024&&config.password.len()<=16000,"webdav.invalid_credentials");
        if !base.path().ends_with('/'){base.set_path(&(base.path().to_owned()+"/"));}
        let client=Client::builder().redirect(reqwest::redirect::Policy::none()).connect_timeout(Duration::from_secs(15)).build()?;
        Ok(Self{client,config,base})
    }
    fn url(&self,name:&str)->Result<Url>{ensure!(!name.contains(['/',':','\\','?','#','\r','\n'])&&!name.is_empty(),"webdav.invalid_name");let url=self.base.join(name)?;ensure!(url.origin()==self.base.origin()&&url.path().starts_with(self.base.path()),"webdav.path_escaped");Ok(url)}
    fn request(&self,method:Method,url:Url)->reqwest::RequestBuilder{self.client.request(method,url).basic_auth(&self.config.username,Some(&self.config.password))}
    pub async fn probe(&self)->Result<serde_json::Value>{let response=self.request(Method::from_bytes(b"PROPFIND")?,self.base.clone()).header("depth","0").timeout(Duration::from_secs(15)).send().await?;ensure!(response.status().is_success(),"webdav.request_rejected: {}",response.status());Ok(serde_json::json!({"connected":true}))}
    async fn index(&self)->Result<(Index,Option<String>)>{
        let mut response=self.request(Method::GET,self.url("rovai-index-v1.json")?).timeout(Duration::from_secs(30)).send().await?;
        if response.status()==StatusCode::NOT_FOUND{return Ok((Index{format_version:1,entries:Vec::new()},None))}
        ensure!(response.status().is_success(),"webdav.index_read_failed: {}",response.status());let etag=response.headers().get("etag").and_then(|value|value.to_str().ok()).map(str::to_owned);
        ensure!(etag.as_deref().is_none_or(|etag|!etag.starts_with("W/")),"webdav.strong_etag_required");
        let mut bytes=Vec::new();while let Some(chunk)=response.chunk().await?{ensure!(bytes.len()+chunk.len()<=8*1024*1024,"webdav.index_limit");bytes.extend_from_slice(&chunk)}
        let index:Index=serde_json::from_slice(&bytes)?;ensure!(index.format_version==1&&index.entries.len()<=10000,"webdav.unsupported_index");let mut seen=std::collections::HashSet::new();for entry in &index.entries{uuid::Uuid::parse_str(&entry.id)?;ensure!(entry.sha256.len()==64&&entry.sha256.bytes().all(|byte|byte.is_ascii_hexdigit())&&entry.bytes<=64*1024*1024*1024&&seen.insert(&entry.id),"webdav.invalid_snapshot");}
        ensure!(etag.is_some(),"webdav.server_does_not_support_conditional_updates");Ok((index,etag))
    }
    pub async fn list(&self)->Result<Vec<Snapshot>>{let (mut index,_)=self.index().await?;index.entries.sort_by(|a,b|b.created_at.cmp(&a.created_at));Ok(index.entries)}
    fn blob_name(snapshot:&Snapshot)->String{format!("rovai-{}-{}.rovai-backup",snapshot.id,snapshot.sha256)}
    async fn verify_remote(&self,snapshot:&Snapshot)->Result<()>{
        let mut response=self.request(Method::GET,self.url(&Self::blob_name(snapshot))?).timeout(Duration::from_secs(43200)).send().await?.error_for_status()?;let mut hash=Sha256::new();let mut length=0_u64;
        while let Some(chunk)=response.chunk().await?{length=length.checked_add(chunk.len() as u64).context("webdav.length_overflow")?;ensure!(length<=snapshot.bytes,"webdav.remote_size_changed");hash.update(&chunk)}
        ensure!(length==snapshot.bytes&&format!("{:x}",hash.finalize())==snapshot.sha256,"webdav.remote_checksum_mismatch");Ok(())
    }
    pub async fn push(&self,path:&Path,id:&str)->Result<Snapshot>{
        uuid::Uuid::parse_str(id)?;ensure!(super::crypto::is_encrypted(path)?,"webdav.encrypted_snapshot_required");
        let (bytes,sha256)=tokio::task::spawn_blocking({let path=path.to_path_buf();move||super::hash_file(&path)}).await??;ensure!(bytes<=64*1024*1024*1024,"webdav.snapshot_limit");
        let snapshot=Snapshot{id:id.into(),created_at:chrono::Utc::now().to_rfc3339(),sha256,bytes};let (index,_)=self.index().await?;
        if let Some(existing)=index.entries.iter().find(|entry|entry.id==id){ensure!(existing.sha256==snapshot.sha256&&existing.bytes==snapshot.bytes,"webdav.operation_identity_changed");return Ok(existing.clone())}
        let stream=tokio_util::io::ReaderStream::new(tokio::fs::File::open(path).await?);
        let upload=self.request(Method::PUT,self.url(&Self::blob_name(&snapshot))?).header("if-none-match","*").header("content-type","application/octet-stream").header("content-length",bytes).timeout(Duration::from_secs(43200)).body(reqwest::Body::wrap_stream(stream)).send().await;
        match upload{Ok(response)if response.status().is_success()=>{},Ok(response)if response.status()==StatusCode::PRECONDITION_FAILED=>self.verify_remote(&snapshot).await?,Ok(response)=>anyhow::bail!("webdav.upload_rejected: {}",response.status()),Err(_)=>self.verify_remote(&snapshot).await.context("webdav.original_upload_outcome_unknown")?}
        ensure!(tokio::task::spawn_blocking({let path=path.to_path_buf();move||super::hash_file(&path)}).await??==(snapshot.bytes,snapshot.sha256.clone()),"webdav.local_snapshot_changed");
        for _ in 0..3{
            let (mut index,etag)=self.index().await?;if let Some(existing)=index.entries.iter().find(|entry|entry.id==id){ensure!(existing.sha256==snapshot.sha256&&existing.bytes==snapshot.bytes,"webdav.operation_identity_changed");return Ok(existing.clone())}
            index.entries.push(snapshot.clone());let mut request=self.request(Method::PUT,self.url("rovai-index-v1.json")?).header("content-type","application/json").timeout(Duration::from_secs(30));request=if let Some(etag)=etag{request.header("if-match",etag)}else{request.header("if-none-match","*")};
            match request.body(serde_json::to_vec(&index)?).send().await{Ok(response)if response.status().is_success()=>return Ok(snapshot),Ok(response)if response.status()==StatusCode::PRECONDITION_FAILED=>continue,Ok(response)=>anyhow::bail!("webdav.index_write_rejected: {}",response.status()),Err(_)=>{let (index,_)=self.index().await?;if index.entries.iter().any(|entry|entry.id==id&&entry.sha256==snapshot.sha256){return Ok(snapshot)}anyhow::bail!("webdav.original_index_write_outcome_unknown")}}
        }
        anyhow::bail!("webdav.concurrent_index_update; encrypted blob retained")
    }
    pub async fn pull(&self,id:&str,destination:&Path,expected_digest:Option<&str>)->Result<Snapshot>{
        uuid::Uuid::parse_str(id)?;let (index,_)=self.index().await?;let snapshot=index.entries.into_iter().find(|entry|entry.id==id).context("webdav.snapshot_missing")?;
        ensure!(expected_digest.is_none_or(|digest|digest==snapshot.sha256),"webdav.remote_selection_changed");
        ensure!(!destination.exists(),"webdav.destination_exists");let parent=std::fs::canonicalize(destination.parent().context("webdav.destination_parent_required")?)?;let temporary:PathBuf=parent.join(format!(".rovai-download-{}.tmp",uuid::Uuid::new_v4()));
        let result=async{let mut response=self.request(Method::GET,self.url(&Self::blob_name(&snapshot))?).timeout(Duration::from_secs(43200)).send().await?.error_for_status()?;let mut file=tokio::fs::OpenOptions::new().write(true).create_new(true).open(&temporary).await?;let mut hash=Sha256::new();let mut length=0_u64;
            while let Some(chunk)=response.chunk().await?{length=length.checked_add(chunk.len() as u64).context("webdav.length_overflow")?;ensure!(length<=snapshot.bytes,"webdav.download_limit");hash.update(&chunk);file.write_all(&chunk).await?;progress(length,snapshot.bytes,"download");}
            file.sync_all().await?;drop(file);ensure!(length==snapshot.bytes&&format!("{:x}",hash.finalize())==snapshot.sha256,"webdav.download_checksum_mismatch");ensure!(super::crypto::is_encrypted(&temporary)?,"webdav.encrypted_snapshot_required");std::fs::hard_link(&temporary,destination)?;Ok::<_,anyhow::Error>(snapshot)
        }.await;let _=tokio::fs::remove_file(&temporary).await;result
    }
}
fn progress(done:u64,total:u64,phase:&str){eprintln!("{}",serde_json::json!({"phase":phase,"done":done,"total":total}));}
