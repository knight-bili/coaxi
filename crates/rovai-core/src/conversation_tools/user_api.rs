//! 仅由Host已认证User路由调用；四个模型工具使用独立运行租约，不能调用这里。
use super::*;
use crate::{command::{ActorRef,CommandEnvelope,CommandExecution,CommandResultStatus},draft_client::DraftClient};
use std::path::Path;

pub const OPERATIONS:[&str;14]=[
    "conversationTools.policy.get","conversationTools.policy.set",
    "conversationTools.questions.list","conversationTools.questions.answer","conversationTools.questions.decline","conversationTools.questions.reconcile",
    "conversationTools.grants.preview","conversationTools.grants.confirm","conversationTools.grants.revoke","conversationTools.grants.list",
    "conversationTools.requests.get","conversationTools.requests.resolve","conversationTools.automationOwner.get","conversationTools.automationOwner.set",
];
fn envelope<T>(id:&str,payload:T)->CommandEnvelope<T>{CommandEnvelope{command_id:id.into(),actor:ActorRef::User{user_id:crate::current_user::CURRENT_USER_ID.into()},camp_id:None,expected_versions:vec![],execution_epoch:None,payload}}
fn payload(execution:CommandExecution)->Result<Value>{
    ensure!(execution.result.status!=CommandResultStatus::Rejected,"{}",execution.result.code);Ok(execution.result.payload)
}
pub fn call(database:&mut crate::db::Database,root:&Path,method:&str,mut value:Value,client:&DraftClient)->Result<Value>{
    validate_operation(method,&value)?;
    let result=match method{
        "conversationTools.policy.get"=>serde_json::to_value(policy::get(database.connection())?)?,
        "conversationTools.questions.list"=>questions::list(database.connection(),&value)?,
        "conversationTools.questions.reconcile"|"conversationTools.requests.get"=>user_commands::request_status(database.connection(),value["requestId"].as_str().context("conversation_tools.request_id_required")?)?,
        "conversationTools.grants.list"=>grants::list(database.connection(),&value)?,
        "conversationTools.automationOwner.get"=>automation_owner::get(database.connection(),value["automationId"].as_str().context("conversation_tools.automation_required")?)?.unwrap_or(Value::Null),
        "conversationTools.grants.preview"=>{
            let tx=database.connection_mut().transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let output=grants::preview(&tx,root,client.id(),&value)?;tx.commit()?;output
        },
        _=>{
            let id=value["commandId"].as_str().context("conversation_tools.command_required")?.to_owned();
            value.as_object_mut().context("conversation_tools.input_invalid")?.remove("commandId");
            let execution=match method{
                "conversationTools.policy.set"=>{let mut p:policy::SetPolicy=serde_json::from_value(value)?;p.client=client.clone();policy::set(database,&envelope(&id,p))?},
                "conversationTools.questions.answer"=>{let mut p:user_commands::AnswerQuestion=serde_json::from_value(value)?;p.client=client.clone();user_commands::answer(database,&envelope(&id,p))?},
                "conversationTools.questions.decline"=>{let mut p:user_commands::DeclineQuestion=serde_json::from_value(value)?;p.client=client.clone();user_commands::decline(database,&envelope(&id,p))?},
                "conversationTools.grants.confirm"=>{let mut p:user_commands::ConfirmGrant=serde_json::from_value(value)?;p.client=client.clone();user_commands::confirm_grant(database,root,&envelope(&id,p))?},
                "conversationTools.grants.revoke"=>{let mut p:user_commands::RevokeGrant=serde_json::from_value(value)?;p.client=client.clone();user_commands::revoke_grant(database,&envelope(&id,p))?},
                "conversationTools.requests.resolve"=>{let mut p:user_commands::ResolveRequest=serde_json::from_value(value)?;p.client=client.clone();user_commands::resolve(database,&envelope(&id,p))?},
                "conversationTools.automationOwner.set"=>{let mut p:automation_owner::SetOwner=serde_json::from_value(value)?;p.client=client.clone();automation_owner::set(database,&envelope(&id,p))?},
                _=>anyhow::bail!("conversation_tools.operation_unknown"),
            };payload(execution)?
        }
    };
    validate_operation_output(method,&result)?;Ok(result)
}
pub fn automation_details(database:&crate::db::Database,id:&str,after:Option<&str>)->Result<Value>{
    let Some(view)=crate::automation::AutomationService::default().get(database,id)? else{return Ok(Value::Null)};
    let mut value=serde_json::to_value(view)?;
    if uuid::Uuid::parse_str(id).is_ok(){
        if let Some(owner)=automation_owner::get(database.connection(),id)?{
            value["conversationToolOwner"]=owner;
            value["queuedMissions"]=crate::automation::queued_missions(database.connection(),id,after)?;
        }
    }
    Ok(value)
}
pub fn maintain(database:&mut crate::db::Database,restarted:bool)->Result<usize>{
    let tx=database.connection_mut().transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if restarted{question_delivery::recover_after_restart(&tx)?;}
    let count=questions::close_unavailable(&tx)?;tx.commit()?;Ok(count)
}
