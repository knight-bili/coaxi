//! 仅可信 Desktop Main 领取原 Store 的文件计划；Renderer、网络和模型没有此入口。
use super::*;
use rusqlite::{Connection,OptionalExtension,params};
use std::path::Path;

#[derive(Deserialize,Serialize,Clone)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct Cursor{command_id:String,domain:String,target_id:String}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct Pending{after:Option<Cursor>,limit:u32}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct Acknowledge{command_id:String,domain:String,target_id:String,plan_digest:String,state:String}

fn owned(target:&str)->bool{matches!(target,"appearance"|"general"|"windowClose"|"logging")}
fn plan(root:&Path,command:&str,domain:&str,target:&str,expected:&str,raw:&str)->Result<Value>{
    ensure!(owned(target),"configuration.owner_target_invalid");
    let value:Value=serde_json::from_str(raw)?;let owner=files::path(root,target)?;
    ensure!(value["ownerPath"]==serde_json::to_value(&owner)?,"configuration.owner_path_changed");
    let mut result=json!({"commandId":command,"domain":domain,"targetId":target,"ownerPath":owner,"expectedDigest":expected,"before":value["before"],"after":value["after"]});
    result["planDigest"]=json!(digest(&result)?);Ok(result)
}
pub(crate) fn pending(connection:&Connection,root:&Path,input:Pending)->Result<Value>{
    ensure!((1..=4).contains(&input.limit),"configuration.owner_page_invalid");
    let after=input.after.unwrap_or(Cursor{command_id:String::new(),domain:String::new(),target_id:String::new()});
    ensure!(after.command_id.len()<=128&&after.domain.len()<=128&&after.target_id.len()<=128,"configuration.owner_cursor_invalid");
    let mut query=connection.prepare("SELECT command_id,domain,target_id,expected_digest,value_json FROM configuration_file_outbox WHERE state='pending' AND target_id IN ('appearance','general','windowClose','logging') AND (command_id,domain,target_id)>(?1,?2,?3) ORDER BY command_id,domain,target_id LIMIT ?4")?;
    let rows=query.query_map(params![after.command_id,after.domain,after.target_id,input.limit+1],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;drop(query);
    let more=rows.len()>input.limit as usize;let mut items=Vec::new();let mut cursor=None;
    for(command,domain,target,expected,raw)in rows.into_iter().take(input.limit as usize){
        cursor=Some(Cursor{command_id:command.clone(),domain:domain.clone(),target_id:target.clone()});
        match plan(root,&command,&domain,&target,&expected,&raw){
            Ok(value)=>items.push(value),
            Err(_)=>{connection.execute("UPDATE configuration_file_outbox SET state='conflict',error_code='configuration.owner_path_changed',updated_at=?4 WHERE command_id=?1 AND domain=?2 AND target_id=?3 AND state='pending'",params![command,domain,target,chrono::Utc::now().to_rfc3339()])?;},
        }
    }
    Ok(json!({"items":items,"nextAfter":if more{serde_json::to_value(cursor)?}else{Value::Null}}))
}
pub(crate) fn acknowledge(connection:&Connection,root:&Path,input:Acknowledge)->Result<Value>{
    ensure!(owned(&input.target_id)&&matches!(input.state.as_str(),"applied"|"conflict"|"failed"),"configuration.owner_ack_invalid");
    let row:Option<(String,String,String)>=connection.query_row("SELECT state,expected_digest,value_json FROM configuration_file_outbox WHERE command_id=?1 AND domain=?2 AND target_id=?3",params![input.command_id,input.domain,input.target_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    let(state,expected,raw)=row.context("configuration.owner_plan_missing")?;
    let value=plan(root,&input.command_id,&input.domain,&input.target_id,&expected,&raw)?;
    ensure!(value["planDigest"].as_str()==Some(input.plan_digest.as_str()),"configuration.owner_plan_changed");
    if state=="pending"{
        let outcome=if input.state=="applied"&&files::read(root,&input.target_id)?!=value["after"]{"conflict"}else{input.state.as_str()};
        let error=match outcome{"applied"=>None,"conflict"=>Some("configuration.file_changed"),_=>Some("configuration.file_write_failed")};
        connection.execute("UPDATE configuration_file_outbox SET state=?4,error_code=?5,updated_at=?6 WHERE command_id=?1 AND domain=?2 AND target_id=?3 AND state='pending'",params![input.command_id,input.domain,input.target_id,outcome,error,chrono::Utc::now().to_rfc3339()])?;
    }
    Ok(json!({"commandId":input.command_id,"files":files::statuses(connection,&input.command_id)?}))
}
