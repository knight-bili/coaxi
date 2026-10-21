//! External source relationships reference canonical business IDs, not copied tasks.
use super::*;
use super::workbench::{Scope,MUTATIONS};
use anyhow::ensure;
use serde::Deserialize;
use std::fs;
#[derive(Deserialize)] #[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request{scope:Scope,operation:String,#[serde(default="origin")]remote:String,account_id:Option<String>,source_kind:String,number:u64,target_kind:Option<String>,target_id:Option<String>,command_id:Option<String>,expected_revision:Option<String>,expected_repository_key:Option<String>}
fn origin()->String{"origin".into()}
impl Core{
    pub(super) async fn forge_links(&self,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value.clone())?;ensure!(["issue","pr"].contains(&request.source_kind.as_str())&&request.number>0,"forge.invalid_source");
        let metadata=self.forge_operation(json!({"scope":request.scope,"action":"repository","remote":request.remote,"accountId":request.account_id})).await?;
        let repository=metadata;let key=repository["repositoryKey"].as_str().context("forge.repository_unavailable")?;
        let directory=self.data_dir.join("forge-links");crate::platform::private_storage::prepare_private_directory(&directory)?;let path=directory.join(format!("{key}.json"));
        if request.operation=="list"{let links=if path.exists(){serde_json::from_slice::<Value>(&fs::read(path)?)?}else{json!([])};return Ok(json!({"revision":crate::command::canonical_json_digest(&links)?,"repository":repository,"links":links.as_array().context("forge.invalid_links")?.iter().filter(|link|link["sourceKind"]==request.source_kind&&link["number"]==request.number&&link["removed"]!=true).collect::<Vec<_>>()}))}
        ensure!(["link","unlink"].contains(&request.operation.as_str())&&request.expected_repository_key.as_deref()==Some(key),"forge.repository_changed_refresh");
        let kind=request.target_kind.as_deref().context("forge.target_required")?;let target=request.target_id.as_deref().context("forge.target_required")?;ensure!(["mission","thread","task"].contains(&kind),"forge.invalid_target");
        let root=self.workbench_root(&request.scope).await?;
        {
            let database=self.database.lock().await;
            let camp=match kind{
                "mission"=>database.connection().query_row("SELECT camp_id FROM mission WHERE id=?1",[target],|row|row.get::<_,String>(0)).context("forge.target_unavailable")?,
                "thread"=>target.to_owned(),
                _=>{let task=CollaborationService::default().get_visible_task(&database,&request.scope.thread_id,target,&ActorRef::User{user_id:CURRENT_USER_ID.into()},None)?.context("forge.target_unavailable")?;let _=task;request.scope.thread_id.clone()}
            };
            let persisted:String=database.connection().query_row("SELECT project_path FROM camp WHERE id=?1",[camp],|row|row.get(0)).context("forge.target_unavailable")?;
            ensure!(fs::canonicalize(persisted)?==root,"forge.target_workspace_mismatch");
        }
        let id=request.command_id.as_deref().context("forge.command_id_required")?;uuid::Uuid::parse_str(id)?;let receipts=self.data_dir.join("forge-link-operations");crate::platform::private_storage::prepare_private_directory(&receipts)?;let receipt=receipts.join(format!("{id}.json"));let identity=crate::command::canonical_json_digest(&json!({"repository":repository,"request":value}))?;
        let prepared=if receipt.exists(){let saved:Value=serde_json::from_slice(&fs::read(&receipt)?)?;ensure!(saved["identity"]==identity,"forge.command_conflict");if saved["state"]=="finished"{return Ok(saved["result"].clone())}saved["prepared"].clone()}else{
            let source=self.forge_operation(json!({"scope":request.scope,"action":request.source_kind,"number":request.number,"remote":request.remote,"accountId":request.account_id})).await?;ensure!(source["success"]==true&&source["repository"]["repositoryKey"]==key,"forge.source_unavailable");
            let data=&source["data"];let base=repository["webUrl"].as_str().context("forge.repository_unavailable")?;let provider=repository["provider"].as_str().unwrap_or_default();let route=match(provider,request.source_kind.as_str()){("gitlab","pr")=>"-/merge_requests",("gitlab",_)=>"-/issues",(_,"pr")=>"pull",_=>"issues"};
            let snapshot=json!({"title":data["title"].as_str().unwrap_or_default().chars().take(200).collect::<String>(),"body":data["body"].as_str().or_else(||data["description"].as_str()).unwrap_or_default().chars().take(262144).collect::<String>(),"url":format!("{base}/{route}/{}",request.number),"labels":data["labels"],"capturedAt":Utc::now().to_rfc3339()});
            crate::platform::private_storage::atomic_write_private_json(&receipt,&json!({"identity":identity,"state":"prepared","prepared":snapshot}))?;snapshot
        };
        let _guard=MUTATIONS.lock().await;
        let mut links:Vec<Value>=if path.exists(){serde_json::from_slice(&fs::read(&path)?)?}else{Vec::new()};
        let revision=crate::command::canonical_json_digest(&json!(links))?;
        let link_id=crate::command::canonical_json_digest(&json!({"repository":key,"sourceKind":request.source_kind,"number":request.number,"targetKind":kind,"targetId":target}))?;
        let already=links.iter().find(|link|link["id"]==link_id&&link["commandId"]==id).cloned();
        if already.is_none(){ensure!(request.expected_revision.as_deref()==Some(revision.as_str()),"forge.links_changed_refresh");ensure!(links.len()<10000,"forge.link_limit");
            if kind=="mission"&&request.source_kind=="pr"{
                let envelope=user_command_envelope(id.to_owned(),crate::mission::LinkMissionPrCommand{mission_id:target.into(),url:prepared["url"].as_str().context("forge.url_missing")?.into(),title:prepared["title"].as_str().unwrap_or_default().into(),remove:request.operation=="unlink"});
                let mut database=self.database.lock().await;let result=crate::mission::MissionService::default().link_pr(&mut database,&envelope)?;ensure!(result.result.status!=crate::command::CommandResultStatus::Rejected,"forge.mission_link_rejected");
            }
            links.retain(|link|link["id"]!=link_id);links.push(json!({"id":link_id,"commandId":id,"sourceKind":request.source_kind,"number":request.number,"targetKind":kind,"targetId":target,"source":prepared,"removed":request.operation=="unlink","updatedAt":Utc::now().to_rfc3339()}));
            crate::platform::private_storage::atomic_write_private_json(&path,&links)?;
        }
        let result=json!({"applied":true,"id":link_id,"revision":crate::command::canonical_json_digest(&json!(links))?});crate::platform::private_storage::atomic_write_private_json(&receipt,&json!({"identity":identity,"state":"finished","result":result}))?;Ok(result)
    }
}
