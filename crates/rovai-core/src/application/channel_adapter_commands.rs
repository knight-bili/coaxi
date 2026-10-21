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
impl crate::command::DomainCommand for ChannelCommand { const TYPE: &'static str = "channel.adapter.command"; const EXACT_REPLAY:bool=true; }
pub(super) fn sender_scope(input:&Inbound,config:&Adapter)->Result<String>{source_id(&json!({"adapterId":config.id,"senderId":input.sender_id,"chatId":input.target.chat_id.as_deref().unwrap_or(&config.chat_id),"threadKey":input.target.thread_key}))}
fn target_matches(input:&Inbound,config:&Adapter,binding:&Binding)->bool{binding.chat_id==input.target.chat_id.as_deref().unwrap_or(&config.chat_id)&&binding.thread_key==input.target.thread_key}
impl Core{
    pub(super) async fn channel_adapter_command_replay(&self,input:&Inbound,id:&str,camp_id:String)->Result<Value>{
        let database=self.database.lock().await;let envelope=CommandEnvelope{command_id:id.into(),actor:ActorRef::System{component_id:"channel-adapter".into()},camp_id:Some(camp_id),expected_versions:vec![],execution_epoch:None,payload:ChannelCommand(input.clone())};
        let execution=DomainCommandGateway.replay_if_recorded(&database,&envelope)?.context("channel_adapter.receipt_missing")?;Ok(serde_json::to_value(execution.result)?)
    }
    pub(super) async fn channel_adapter_selected_binding(&self,input:&Inbound,config:&Adapter,default:&Binding)->Result<(Binding,bool)>{
        let scope=sender_scope(input,config)?;let database=self.database.lock().await;
        let raw=database.connection().query_row("SELECT result_payload_json FROM event_log WHERE command_type='channel.adapter.command' AND result_status='applied' AND json_extract(result_payload_json,'$.senderScope')=?1 AND json_type(result_payload_json,'$.selection')='object' ORDER BY global_sequence DESC LIMIT 1",[scope],|row|row.get::<_,String>(0)).optional()?;
        let Some(raw)=raw else{return Ok((default.clone(),false))};let value:Value=serde_json::from_str(&raw)?;let selection=&value["selection"];
        if value["revision"]!=input.expected_revision{return Ok((default.clone(),true))}
        let Some(binding)=config.bindings.iter().find(|candidate|target_matches(input,config,candidate)&&selection["threadId"]==candidate.thread_id)else{return Ok((default.clone(),true))};
        let ids=selection["agentIds"].as_array().context("channel_adapter.invalid_selection")?.iter().map(|id|id.as_str().map(str::to_owned).context("channel_adapter.invalid_selection")).collect::<Result<Vec<_>>>()?;
        if ids.is_empty()||!ids.iter().all(|id|binding.agent_ids.contains(id)){return Ok((default.clone(),true))}
        Ok((Binding{agent_ids:ids,..binding.clone()},false))
    }
}
fn help()->String { "/search 关键词：搜索已绑定对话\n/today：今日活动\n/status：当前对话运行状态\n/folder：可用工程\n/agent：可用队员\n/sessions：可用对话\n/task 内容 或 /do 内容：向当前对话发起任务\n/resume、/cancel、/approve、/deny：会话控制\n/help：命令帮助".into() }
impl Core {
    pub(super) async fn channel_adapter_command(&self,input:&Inbound,config:&Adapter,binding:&Binding,command_id:&str,selection_expired:bool)->Result<Option<Value>> {
        let text=input.text.trim();
        let command=if let Some(callback)=input.callback_data.as_deref(){
            match callback { "rv:today"=>"today", "rv:status"=>"status", "rv:sessions"=>"sessions", "rv:help"=>"help", value if value.starts_with("rv:choose:")=>"choose", _=>"expired_callback" }
        }else if let Some(value)=text.strip_prefix('/') { value.split_whitespace().next().unwrap_or("help").split('@').next().unwrap_or("help") } else if selection_expired { "selection_expired" } else { return Ok(None) };
        let args=text.split_once(char::is_whitespace).map(|(_,value)|value.trim()).unwrap_or("");
        if matches!(command,"task"|"do") && !args.is_empty() && !selection_expired { return Ok(None) }
        let envelope=CommandEnvelope{command_id:command_id.into(),actor:ActorRef::System{component_id:"channel-adapter".into()},camp_id:Some(binding.thread_id.clone()),expected_versions:vec![],execution_epoch:None,payload:ChannelCommand(input.clone())};
        let sender_scope=sender_scope(input,config)?;
        let mut database=self.database.lock().await;
        let execution=DomainCommandGateway.execute(&mut database,&envelope,|transaction|{
            let mut lines=Vec::<String>::new();let mut selection=Value::Null;let mut choices=Vec::<Value>::new();
            let candidates=config.bindings.iter().filter(|candidate|target_matches(input,config,candidate)).collect::<Vec<_>>();
            match command {
                "help"|"start"=>lines.push(help()),
                "task"|"do"|"selection_expired"=>lines.push("对话选择已随渠道配置变化失效，请先用 /sessions 或 /resume 对话编号重新选择。用法：/task 任务内容。".into()),
                "expired_callback"=>lines.push("按钮已失效或不属于当前协议，请重新输入 /help；没有执行操作。".into()),
                "choose"=>{
                    let parts=input.callback_data.as_deref().unwrap_or_default().split(':').collect::<Vec<_>>();
                    let chosen=(||->Result<Value>{ensure!(parts.len()==4,"channel_adapter.invalid_callback");uuid::Uuid::parse_str(parts[2])?;let index:usize=parts[3].parse()?;
                        let raw:String=transaction.query_row("SELECT result_payload_json FROM event_log WHERE command_id=?1 AND command_type='channel.adapter.command' AND result_status='applied'",[parts[2]],|row|row.get(0))?;let previous:Value=serde_json::from_str(&raw)?;
                        ensure!(previous["senderScope"]==sender_scope&&previous["revision"]==input.expected_revision,"channel_adapter.callback_scope_changed");let choice=previous["choices"].as_array().and_then(|items|items.get(index)).context("channel_adapter.callback_expired")?.clone();
                        let target=candidates.iter().find(|candidate|choice["threadId"]==candidate.thread_id).context("channel_adapter.target_not_bound")?;let ids=choice["agentIds"].as_array().context("channel_adapter.invalid_selection")?;ensure!(!ids.is_empty()&&ids.iter().all(|id|id.as_str().is_some_and(|id|target.agent_ids.iter().any(|agent|agent==id))),"channel_adapter.member_not_bound");Ok(choice)})();
                    match chosen{Ok(value)=>{lines.push(format!("已选择对话 {}",value["threadId"].as_str().unwrap_or_default()));selection=value},Err(_)=>lines.push("按钮已失效或不属于当前发送人，请重新获取列表；没有更改选择。".into())}
                },
                "resume"=>{
                    if let Some(candidate)=candidates.iter().find(|candidate|candidate.thread_id==args){selection=json!({"threadId":candidate.thread_id,"agentIds":candidate.agent_ids});lines.push(format!("已选择对话 {}，后续输入会进入该对话。",candidate.thread_id))}else{lines.push("请使用 /resume 已绑定对话编号，或通过 /sessions 重新选择。".into())}
                },
                "sessions"|"search"|"folder"=>{
                    if command=="search"&&args.is_empty(){lines.push("用法：/search 关键词".into())}
                    else {let mut ids=std::collections::HashSet::new();for candidate in &candidates{
                        if !ids.insert(candidate.thread_id.clone()){continue}
                        let row=transaction.query_row("SELECT COALESCE(title,''),project_path FROM camp WHERE id=?1 AND deletion_operation_id IS NULL",[&candidate.thread_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).optional()?;
                        if let Some((title,path))=row{if command=="folder"&&!args.is_empty(){if args==path||args==candidate.thread_id{selection=json!({"threadId":candidate.thread_id,"agentIds":candidate.agent_ids});lines.push(format!("已选择工程 {path}"));break}else{continue}}if command=="search"&&!title.to_lowercase().contains(&args.to_lowercase()){continue}lines.push(if command=="folder"{format!("{} · {}\n{}",candidate.thread_id,title,path)}else{format!("{} · {}{}",candidate.thread_id,title,if candidate.thread_id==binding.thread_id{" [当前]"}else{""})});choices.push(json!({"threadId":candidate.thread_id,"agentIds":candidate.agent_ids}));if lines.len()>=20{break}}
                    }if lines.is_empty(){lines.push("没有匹配的已授权对话。".into())}}
                },
                "agent"=>{let allowed=candidates.iter().find(|candidate|candidate.thread_id==binding.thread_id).context("channel_adapter.target_not_bound")?;for id in &allowed.agent_ids {let name=transaction.query_row("SELECT display_name FROM agent_profile WHERE id=?1",[id],|row|row.get::<_,String>(0)).optional()?.unwrap_or_else(||id.clone());if args.is_empty(){lines.push(format!("{id} · {name}"));choices.push(json!({"threadId":binding.thread_id,"agentIds":[id]}));}else if args==id||args.eq_ignore_ascii_case(&name){selection=json!({"threadId":binding.thread_id,"agentIds":[id]});lines.push(format!("后续输入交给 {name}"));break}}if lines.is_empty(){lines.push("未找到已授权队员，请输入 /agent 重新选择。".into())}},
                "today"=>{
                    let start=chrono::Utc::now().date_naive().and_hms_opt(0,0,0).context("channel_adapter.date_unavailable")?.and_utc().to_rfc3339();let mut messages=0i64;let mut runs=0i64;let mut seen=std::collections::HashSet::new();
                    for candidate in &config.bindings {if !seen.insert(&candidate.thread_id){continue}messages+=transaction.query_row("SELECT COUNT(*) FROM camp_message WHERE camp_id=?1 AND created_at>=?2 AND tombstoned_at IS NULL",rusqlite::params![candidate.thread_id,start],|row|row.get::<_,i64>(0))?;runs+=transaction.query_row("SELECT COUNT(*) FROM agent_run WHERE camp_id=?1 AND created_at>=?2",rusqlite::params![candidate.thread_id,start],|row|row.get::<_,i64>(0))?;}
                    lines.push(format!("今日活动（UTC）\n已授权对话 {} 个，消息 {messages} 条，队员执行 {runs} 次。",seen.len()));
                },
                "status"=>{let mut statement=transaction.prepare("SELECT id,status FROM agent_run WHERE camp_id=?1 ORDER BY created_at DESC,id DESC LIMIT 20")?;let rows=statement.query_map([&binding.thread_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;lines.push(format!("当前对话：{}",binding.thread_id));for(id,status)in rows{lines.push(format!("{id} · {status}"));}},
                _=>lines.push(format!("该命令尚未执行。\n{}",help())),
            }
            let body=lines.join("\n");ensure!(body.len()<=256*1024,"channel_adapter.response_too_large");
            let mut buttons=choices.iter().enumerate().take(20).map(|(index,choice)|json!({"id":format!("rv:choose:{command_id}:{index}"),"label":format!("选择 {}",choice["threadId"].as_str().unwrap_or_default()).chars().take(32).collect::<String>()})).collect::<Vec<_>>();if buttons.is_empty(){buttons=vec![json!({"id":"rv:sessions","label":"对话"}),json!({"id":"rv:today","label":"今日"}),json!({"id":"rv:status","label":"状态"}),json!({"id":"rv:help","label":"帮助"})]}
            let response=json!({"body":body,"buttons":buttons});
            Ok(crate::command::CommandHandlerResult::applied("channel.adapter.command.completed",json!({"adapterId":config.id,"threadId":binding.thread_id,"senderId":input.sender_id,"externalId":input.external_id,"target":input.target,"response":response,"senderScope":sender_scope,"revision":input.expected_revision,"selection":selection,"choices":choices}),None))
        })?;
        Ok(Some(serde_json::to_value(execution.result)?))
    }
}
