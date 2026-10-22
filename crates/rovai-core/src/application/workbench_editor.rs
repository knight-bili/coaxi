//! 工程编辑器草稿只在此保存；恢复不会写回工程文件或进入模型上下文。
use super::*;
use super::workbench::Scope;
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{collections::HashSet,fs,path::Path};
static WRITES:tokio::sync::Mutex<()>=tokio::sync::Mutex::const_new(());

#[derive(Clone,Deserialize,Serialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Document {path:String,text:String,saved:String,digest:Option<String>,#[serde(default)]conflict:bool}
#[derive(Clone,Deserialize,Serialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct EditorState {documents:Vec<Document>,selected:Option<String>,#[serde(default)]layout:Option<Value>}
#[derive(Clone,Deserialize,Serialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Record {schema_version:u32,scope:Scope,workspace_root:String,editor_id:String,revision:String,command_id:Option<String>,request_digest:Option<String>,value:EditorState,updated_at:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request {operation:String,scope:Scope,editor_id:String,expected_revision:Option<String>,command_id:Option<String>,value:Option<EditorState>}
fn validate(value:&EditorState)->Result<()> {
    ensure!(value.documents.len()<=128&&serde_json::to_vec(value)?.len()<=32*1024*1024,"workbench.editor_limit");
    let mut paths=HashSet::new();
    for document in &value.documents{
        ensure!(!document.path.is_empty()&&document.path.len()<=8192&&!document.path.contains(['\0','\r','\n',':','\\'])&&Path::new(&document.path).components().all(|part|matches!(part,std::path::Component::Normal(_)))&&!Path::new(&document.path).components().any(|part|part.as_os_str().to_string_lossy().eq_ignore_ascii_case(".git"))&&paths.insert(&document.path),"workbench.invalid_editor_path");
        ensure!(document.text.len()<=8*1024*1024&&document.saved.len()<=8*1024*1024,"workbench.editor_document_limit");
        if let Some(expected)=&document.digest{ensure!(*expected==format!("{:x}",Sha256::digest(document.saved.as_bytes())),"workbench.editor_base_digest_changed")}else{ensure!(document.saved.is_empty(),"workbench.editor_base_digest_required")}
    }
    ensure!(value.selected.as_ref().is_none_or(|selected|paths.contains(selected)),"workbench.editor_selection_missing");
    if let Some(layout)=&value.layout{ensure!(serde_json::to_vec(layout)?.len()<=65536,"workbench.editor_layout_limit")}
    Ok(())
}
impl Core {
    pub(super) async fn workbench_editor(&self,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value)?;ensure!(matches!(request.operation.as_str(),"get"|"save"),"workbench.operation_not_admitted");uuid::Uuid::parse_str(&request.editor_id)?;
        let root=self.workbench_root(&request.scope).await?;
        let key=crate::command::canonical_json_digest(&json!({"scope":request.scope,"root":root,"editorId":request.editor_id}))?;
        let directory=self.data_dir.join("workspace-editors");let path=directory.join(format!("{key}.json"));let _write=WRITES.lock().await;
        let current:Option<Record>=if path.exists(){let metadata=fs::symlink_metadata(&path)?;ensure!(metadata.is_file()&&!metadata.file_type().is_symlink()&&metadata.len()<=33*1024*1024,"workbench.invalid_editor_record");let record:Record=serde_json::from_slice(&fs::read(&path)?)?;ensure!(record.schema_version==1&&record.editor_id==request.editor_id&&record.workspace_root==root.to_string_lossy()&&serde_json::to_value(&record.scope)?==serde_json::to_value(&request.scope)?,"workbench.editor_owner_changed");validate(&record.value)?;Some(record)}else{None};
        if request.operation=="get"{return Ok(json!({"revision":current.as_ref().map(|record|&record.revision),"value":current.as_ref().map(|record|&record.value),"updatedAt":current.as_ref().map(|record|&record.updated_at)}))}
        let value=request.value.context("workbench.editor_value_required")?;validate(&value)?;
        let command=request.command_id.context("workbench.command_id_required")?;uuid::Uuid::parse_str(&command)?;
        let request_digest=crate::command::canonical_json_digest(&json!({"expectedRevision":request.expected_revision,"value":value}))?;
        if let Some(current)=&current{if current.command_id.as_deref()==Some(command.as_str()){ensure!(current.request_digest.as_deref()==Some(request_digest.as_str()),"workbench.command_conflict");return Ok(json!({"revision":current.revision,"value":current.value,"updatedAt":current.updated_at}))}}
        ensure!(current.as_ref().map(|record|record.revision.as_str())==request.expected_revision.as_deref(),"workbench.editor_changed");
        let record=Record{schema_version:1,scope:request.scope,workspace_root:root.to_string_lossy().into_owned(),editor_id:request.editor_id,revision:uuid::Uuid::new_v4().to_string(),command_id:Some(command),request_digest:Some(request_digest),value,updated_at:Utc::now().to_rfc3339()};
        crate::platform::private_storage::prepare_private_directory(&directory)?;crate::platform::private_storage::atomic_write_private_json(&path,&record)?;
        Ok(json!({"revision":record.revision,"value":record.value,"updatedAt":record.updated_at}))
    }
}
