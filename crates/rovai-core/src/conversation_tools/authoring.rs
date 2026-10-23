//! 模型创建入口只使用冻结的工程和已配置队员；保存使命不启动运行。
use super::*;
use super::scope::RunLease;
use crate::{collaboration::ProjectBindingKind,command::{ActorRef,CommandEnvelope,CommandExecution,CommandHandlerResult,DomainCommand,DomainCommandGateway,sealed}};
use rusqlite::{Connection,OptionalExtension,Transaction,params};
use std::path::Path;

/// 不能从JSON构造，只有本模块核对真实Run或持久自动化所有者后产生。
pub(crate) struct CreationAuthority{actor:ActorRef,epoch:Option<i64>,request:String,automation:Option<String>}
impl CreationAuthority{
    pub(crate) fn actor(&self)->&ActorRef{&self.actor}
    pub(crate) fn epoch(&self)->Option<i64>{self.epoch}
    pub(crate) fn source_request(&self)->&str{&self.request}
    pub(crate) fn source_automation(&self)->Option<&str>{self.automation.as_deref()}
}
pub(crate) struct Target{pub path:String,pub binding:ProjectBindingKind,pub member:String,pub adapter:String}
pub(crate) fn target(connection:&Connection,lease:&RunLease,input:&Value)->Result<Target>{
    let requested=input["folder_path"].as_str().filter(|value|!value.is_empty());
    let current:String=connection.query_row("SELECT project_path FROM camp WHERE id=?1 AND deletion_operation_id IS NULL",[&lease.frozen.scope.thread_id],|row|row.get(0))?;
    let selected=requested.unwrap_or(&current);ensure!(Path::new(selected).is_absolute()&&Path::new(selected).is_dir(),"conversation_tools.project_not_authorized");
    let canonical=Path::new(selected).canonicalize()?.to_string_lossy().into_owned();
    let grants=lease.frozen.project_grants.iter().filter(|grant|Path::new(&grant.project_path).canonicalize().is_ok_and(|path|path.to_string_lossy()==canonical)).collect::<Vec<_>>();
    ensure!(grants.len()==1,"conversation_tools.project_not_authorized");let grant=grants[0];
    let actual:Option<(String,String)>=connection.query_row("SELECT id,project_binding_kind FROM camp WHERE project_path=?1 AND version=?2 AND deletion_operation_id IS NULL AND activation_state='active' ORDER BY CASE WHEN id=?3 THEN 0 ELSE 1 END,id LIMIT 1",params![grant.project_path,grant.revision,lease.frozen.scope.thread_id],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
    let(camp,binding)=actual.context("conversation_tools.project_not_authorized")?;
    let selector=input["agent_type"].as_str().filter(|value|!value.is_empty());let mut candidates=Vec::new();
    for member in &grant.member_agent_ids{
        let row:Option<(String,String)>=connection.query_row("SELECT a.display_name,a.selected_runtime_adapter_kind FROM agent_profile a JOIN camp_member m ON m.agent_id=a.id WHERE m.camp_id=?1 AND a.id=?2 AND m.status='active' AND m.leave_requested_at IS NULL AND a.profile_status='present' AND a.default_runtime_installation_id IS NOT NULL AND a.default_model_selection_json IS NOT NULL",params![camp,member],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
        if let Some((name,adapter))=row{if selector.is_some_and(|selector|selector==member||selector==name||selector==adapter)||selector.is_none()&&member==&lease.frozen.agent_id{candidates.push((member.clone(),adapter));}}
    }
    ensure!(candidates.len()==1,"conversation_tools.member_unavailable");let(member,adapter)=candidates.remove(0);
    let binding=match binding.as_str(){"directory"=>ProjectBindingKind::Directory,"quick_chat"=>ProjectBindingKind::QuickChat,_=>anyhow::bail!("conversation_tools.project_not_authorized")};
    Ok(Target{path:grant.project_path.clone(),binding,member,adapter})
}
#[derive(Clone,Serialize)]
pub(crate) struct ApplyWorkTask{request_id:String}
impl sealed::Sealed for ApplyWorkTask{}
impl DomainCommand for ApplyWorkTask{const TYPE:&'static str="conversationTool.create_work_task";const EXACT_REPLAY:bool=true;}
pub(crate) fn create_mission(database:&mut crate::db::Database,lease:&RunLease,id:&str)->Result<CommandExecution>{
    let envelope=CommandEnvelope{command_id:id.into(),actor:ActorRef::Agent{agent_id:lease.frozen.agent_id.clone(),source_agent_run_id:lease.run_id.clone()},camp_id:Some(lease.frozen.scope.thread_id.clone()),expected_versions:vec![],execution_epoch:Some(lease.epoch),payload:ApplyWorkTask{request_id:id.into()}};
    DomainCommandGateway.execute(database,&envelope,|tx|{
        let input=requests::prepared(tx,lease,id,"create_work_task")?;
        let target=match target(tx,lease,&input){Ok(target)=>target,Err(error) if error.to_string().starts_with("conversation_tools.")=>{
            let note=if error.to_string()=="conversation_tools.member_unavailable"{"未创建：没有唯一获准且已配置程序的队员。请由用户修复工程队员选择。"}else{"未创建：原工程授权已变化或目录不可用，请由用户重新选择已有工程。"};
            let value=json!({"created":false,"kind":"work_task","note":note});requests::finish(tx,id,"mission",&value)?;return Ok(CommandHandlerResult::applied("conversation_tools.not_created",value,None))
        },Err(error)=>return Err(error)};
        let authority=CreationAuthority{actor:envelope.actor.clone(),epoch:Some(lease.epoch),request:id.into(),automation:None};
        let title=input["title"].as_str().context("conversation_tools.title_required")?;let prompt=input["prompt"].as_str().context("conversation_tools.prompt_required")?;
        let mission=crate::mission::create_conversation_tool_mission_in_tx(tx,&authority,title,prompt,&target.path,target.binding,&target.member)?;
        let value=json!({"created":true,"kind":"work_task","id":mission,"title":title,"folder_name":Path::new(&target.path).file_name().and_then(|name|name.to_str()).unwrap_or(""),"folder_path":target.path,"agent_type":target.adapter,"target_kind":"mission"});
        requests::finish(tx,id,"mission",&value)?;Ok(CommandHandlerResult::applied("conversation_tools.mission_created",value,None))
    })
}
pub(crate) fn scheduled_authority(connection:&Connection,automation:&str,request:&str)->Result<CreationAuthority>{
    let exists:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tool_automation_owner owner JOIN automation a ON a.id=owner.automation_id WHERE owner.automation_id=?1 AND owner.source_request_id=?2 AND owner.action='enqueue_task')",params![automation,request],|row|row.get(0))?;
    ensure!(exists,"conversation_tools.automation_owner_changed");
    Ok(CreationAuthority{actor:ActorRef::System{component_id:"conversation_tool_automation".into()},epoch:None,request:request.into(),automation:Some(automation.into())})
}
