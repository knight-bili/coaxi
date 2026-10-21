use super::*;
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request{scope:workbench::Scope,operation:String,#[serde(default)]hold_id:Option<String>,#[serde(default)]path:Option<String>,#[serde(default)]editable:bool}
impl Core{
    pub(super) async fn office_access(&self,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value)?;let root=self.workbench_root(&request.scope).await?;
        let scope=crate::command::canonical_json_digest(&json!({"scope":request.scope,"root":root,"core":self.data_dir}))?;
        if request.operation=="detect"{return Ok(crate::office_preview::detect())}
        let id=request.hold_id.context("office.hold_required")?;
        match request.operation.as_str(){
            "start"=>{let path=request.path.context("office.path_required")?;workbench::relative(&root,&path,false)?;crate::office_preview::start(&scope,&id,&root,&path,request.editable).await}
            "heartbeat"=>crate::office_preview::heartbeat(&scope,&id),
            "stop"=>crate::office_preview::release(&scope,&id).await,
            _=>anyhow::bail!("office.operation_not_admitted"),
        }
    }
}
