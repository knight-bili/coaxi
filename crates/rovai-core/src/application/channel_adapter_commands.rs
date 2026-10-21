//! Source channel command vocabulary, projected from authorized Core Threads.
//! Read commands never start a provider turn or widen a channel's bindings.
use super::*;
use super::channel_adapters::{Adapter,Binding};
use super::channel_adapter_messages::{Inbound,source_id};
use anyhow::ensure;
use rusqlite::OptionalExtension;
use serde::{Deserialize,Serialize};

#[derive(Clone,Deserialize,Serialize)]
#[serde(transparent)]
struct ChannelCommand(Inbound);
impl crate::command::sealed::Sealed for ChannelCommand {}
impl crate::command::DomainCommand for ChannelCommand { const TYPE: &'static str = "channel.adapter.command"; }
fn help()->String { "/search 关键词：搜索已绑定对话\n/today：今日活动\n/status：当前对话运行状态\n/folder：可用工程\n/agent：可用队员\n/sessions：可用对话\n/task 内容 或 /do 内容：向当前对话发起任务\n/resume、/cancel、/approve、/deny：会话控制\n/help：命令帮助".into() }
impl Core {
    pub(super) async fn channel_adapter_command(&self,input:&Inbound,config:&Adapter,binding:&Binding,command_id:&str)->Result<Option<Value>> {
        let text=input.text.trim();
        let command=if let Some(callback)=input.callback_data.as_deref(){
            match callback { "rv:today"=>"today", "rv:status"=>"status", "rv:sessions"=>"sessions", "rv:help"=>"help", _=>"expired_callback" }
        }else if let Some(value)=text.strip_prefix('/') { value.split_whitespace().next().unwrap_or("help").split('@').next().unwrap_or("help") } else { return Ok(None) };
        let args=text.split_once(char::is_whitespace).map(|(_,value)|value.trim()).unwrap_or("");
        if matches!(command,"task"|"do") && !args.is_empty() { return Ok(None) }
        let envelope=CommandEnvelope{command_id:command_id.into(),actor:ActorRef::System{component_id:"channel-adapter".into()},camp_id:Some(binding.thread_id.clone()),expected_versions:vec![],execution_epoch:None,payload:ChannelCommand(input.clone())};
        let mut database=self.database.lock().await;
        let execution=DomainCommandGateway.execute(&mut database,&envelope,|transaction|{
            let mut lines=Vec::<String>::new();
            match command {
                "help"|"start"=>lines.push(help()),
                "task"|"do"=>lines.push("用法：/task 任务内容；任务在当前已绑定对话执行。".into()),
                "expired_callback"=>lines.push("按钮已失效或不属于当前协议，请重新输入 /help；没有执行操作。".into()),
                "sessions"|"search"|"folder"=>{
                    if command=="search"&&args.is_empty(){lines.push("用法：/search 关键词".into())}
                    else {let mut ids=std::collections::HashSet::new();for candidate in &config.bindings{
                        if !ids.insert(candidate.thread_id.clone()){continue}
                        let row=transaction.query_row("SELECT COALESCE(title,''),project_path FROM camp WHERE id=?1 AND deletion_operation_id IS NULL",[&candidate.thread_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).optional()?;
                        if let Some((title,path))=row{if command=="search"&&!title.to_lowercase().contains(&args.to_lowercase()){continue}lines.push(if command=="folder"{format!("{} · {}\n{}",candidate.thread_id,title,path)}else{format!("{} · {}{}",candidate.thread_id,title,if candidate.thread_id==binding.thread_id{" [当前]"}else{""})});if lines.len()>=20{break}}
                    }if lines.is_empty(){lines.push("没有匹配的已授权对话。".into())}}
                },
                "agent"=>{for id in &binding.agent_ids {let name=transaction.query_row("SELECT display_name FROM agent_profile WHERE id=?1",[id],|row|row.get::<_,String>(0)).optional()?.unwrap_or_else(||id.clone());lines.push(format!("{id} · {name}"));}},
                "today"=>{
                    let start=chrono::Utc::now().date_naive().and_hms_opt(0,0,0).context("channel_adapter.date_unavailable")?.and_utc().to_rfc3339();let mut messages=0i64;let mut runs=0i64;let mut seen=std::collections::HashSet::new();
                    for candidate in &config.bindings {if !seen.insert(&candidate.thread_id){continue}messages+=transaction.query_row("SELECT COUNT(*) FROM camp_message WHERE camp_id=?1 AND created_at>=?2 AND tombstoned_at IS NULL",rusqlite::params![candidate.thread_id,start],|row|row.get::<_,i64>(0))?;runs+=transaction.query_row("SELECT COUNT(*) FROM agent_run WHERE camp_id=?1 AND created_at>=?2",rusqlite::params![candidate.thread_id,start],|row|row.get::<_,i64>(0))?;}
                    lines.push(format!("今日活动（UTC）\n已授权对话 {} 个，消息 {messages} 条，队员执行 {runs} 次。",seen.len()));
                },
                "status"=>{let mut statement=transaction.prepare("SELECT id,status FROM agent_run WHERE camp_id=?1 ORDER BY created_at DESC,id DESC LIMIT 20")?;let rows=statement.query_map([&binding.thread_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;lines.push(format!("当前对话：{}",binding.thread_id));for(id,status)in rows{lines.push(format!("{id} · {status}"));}},
                _=>lines.push(format!("该命令尚未执行。\n{}",help())),
            }
            let body=lines.join("\n");ensure!(body.len()<=256*1024,"channel_adapter.response_too_large");
            let response=json!({"body":body,"buttons":[{"id":"rv:sessions","label":"对话"},{"id":"rv:today","label":"今日"},{"id":"rv:status","label":"状态"},{"id":"rv:help","label":"帮助"}]});
            Ok(crate::command::CommandHandlerResult::applied("channel.adapter.command.completed",json!({"adapterId":config.id,"threadId":binding.thread_id,"senderId":input.sender_id,"externalId":input.external_id,"target":input.target,"response":response}),None))
        })?;
        Ok(Some(serde_json::to_value(execution.result)?))
    }
}
