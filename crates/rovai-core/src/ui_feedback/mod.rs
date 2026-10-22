//! 客户确认的文字和卡片反馈 r1。只有应用确认后的输入进入精确 Run。
use anyhow::{ensure,Context,Result};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{collections::BTreeSet,sync::LazyLock};
pub mod storage;
pub mod cards;

pub const SERVER_NAME:&str="rovai-ui-feedback";
pub const MAX_FRAME_BYTES:usize=65_536;
pub const MAX_TEXT_CHARS:usize=4096;
pub const MAX_PENDING:usize=256;
pub const MAX_SAFE_INTEGER:u64=9_007_199_254_740_991;
static CONTRACT:LazyLock<Value>=LazyLock::new(||serde_json::from_str(include_str!("contract.json")).expect("confirmed feedback contract"));
pub fn catalog()->Value{CONTRACT["after"]["tool"].clone()}
pub fn digest(value:&Value)->Result<String>{crate::command::canonical_json_digest(value)}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Scope{pub thread_id:String,pub conversation_id:Option<String>,pub scope_kind:String}
impl Scope{pub fn validate(&self)->Result<()>{validate_shape("scope",&serde_json::to_value(self)?)?;ensure!(matches!((self.scope_kind.as_str(),self.conversation_id.as_deref()),("public",None)|("single_chat"|"delegation_child",Some(_))),"scope_changed");Ok(())}}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct CardOrigin{pub message_id:String,pub conversation_id:Option<String>,pub message_kind:String,pub card_ordinal:u32,pub source_digest:String}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct FeedbackField{pub name:String,pub label:String,pub state_path:String,pub value:Value}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Source{LiveNote,GeneratedCard{origin:CardOrigin}}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PreviewRequest{pub schema_version:u32,pub scope:Scope,pub agent_id:String,pub agent_run_id:String,pub execution_epoch:u64,pub source:Source,pub text:String,pub title:String,pub fields:Vec<FeedbackField>}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PreviewResult{pub schema_version:u32,pub preview_id:String,pub preview_digest:String,pub scope:Scope,pub agent_id:String,pub agent_run_id:String,pub execution_epoch:u64,pub text:String,pub expires_at:String}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct SubmitRequest{pub schema_version:u32,pub command_id:String,pub preview_id:String,pub preview_digest:String}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Binding{pub schema_version:u32,pub bridge_instance_id:String,pub core_instance_id:String,pub scope:Scope,pub agent_id:String,pub agent_run_id:String,pub execution_epoch:u64,pub native_binding_id:String,pub native_binding_generation:u64,pub native_session_id:Option<String>,pub membership_generation:u64,pub policy_revision:u64,pub catalog_digest:String,pub expires_at:String}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct MemberPolicy{pub agent_id:String,pub enabled:bool}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Policy{pub schema_version:u32,pub revision:u64,pub members:Vec<MemberPolicy>}
impl Default for Policy{fn default()->Self{Self{schema_version:1,revision:0,members:Vec::new()}}}
impl Policy{
    pub fn enabled(&self,agent:&str)->bool{self.members.iter().any(|row|row.agent_id==agent&&row.enabled)}
    pub fn validate(&self)->Result<()>{validate_shape("memberPolicy",&serde_json::to_value(self)?)?;let mut ids=BTreeSet::new();ensure!(self.members.iter().all(|row|ids.insert(&row.agent_id)),"feedback.duplicate_member");Ok(())}
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelNote{pub id:String,pub text:String,pub created_at:String}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Receipt{pub schema_version:u32,pub feedback_id:String,pub scope:Scope,pub agent_id:String,pub source_agent_run_id:String,pub source_execution_epoch:u64,pub source_kind:String,pub text_digest:String,pub status:String,pub delivery_channel:String,pub bridge_instance_id:Option<String>,pub transport_rpc_id:Value,pub native_tool_call_id:Option<String>,pub result_digest:Option<String>,pub written_at:Option<String>,pub continued_message_id:Option<String>}

pub fn validate_shape(name:&str,value:&Value)->Result<()>{let schema=CONTRACT["after"]["schemas"].get(name).context("feedback.unknown_shape")?;validate_value(schema,value,0)}
fn validate_value(schema:&Value,value:&Value,depth:usize)->Result<()>{
    ensure!(depth<=64,"feedback.shape_depth");
    if let Some(rows)=schema["anyOf"].as_array(){ensure!(rows.iter().any(|row|validate_value(row,value,depth+1).is_ok()),"feedback.shape_any")}
    if let Some(rows)=schema["oneOf"].as_array(){ensure!(rows.iter().filter(|row|validate_value(row,value,depth+1).is_ok()).count()==1,"feedback.shape_one")}
    if let Some(expected)=schema.get("const"){ensure!(expected==value,"feedback.shape_constant")}
    if let Some(rows)=schema["enum"].as_array(){ensure!(rows.contains(value),"feedback.shape_enum")}
    let matches=|kind:&str|match kind{"object"=>value.is_object(),"array"=>value.is_array(),"string"=>value.is_string(),"integer"=>value.is_i64()||value.is_u64(),"number"=>value.is_number(),"boolean"=>value.is_boolean(),"null"=>value.is_null(),_=>false};
    if let Some(kind)=schema["type"].as_str(){ensure!(matches(kind),"feedback.shape_type")}else if let Some(kinds)=schema["type"].as_array(){ensure!(kinds.iter().filter_map(Value::as_str).any(matches),"feedback.shape_type")}
    if let Some(object)=value.as_object(){
        if let Some(required)=schema["required"].as_array(){ensure!(required.iter().filter_map(Value::as_str).all(|key|object.contains_key(key)),"feedback.shape_required")}
        for(key,value)in object{if let Some(property)=schema["properties"].get(key){validate_value(property,value,depth+1)?}else{ensure!(schema["additionalProperties"]!=false,"feedback.shape_unknown_field")}}
    }
    if let Some(rows)=value.as_array(){
        if let Some(min)=schema["minItems"].as_u64(){ensure!(rows.len()as u64>=min,"feedback.shape_min_items")}
        if let Some(max)=schema["maxItems"].as_u64(){ensure!(rows.len()as u64<=max,"feedback.shape_max_items")}
        if let Some(item)=schema.get("items"){for row in rows{validate_value(item,row,depth+1)?}}
    }
    if let Some(text)=value.as_str(){
        if let Some(min)=schema["minLength"].as_u64(){ensure!(text.chars().count()as u64>=min,"feedback.shape_min_length")}
        if let Some(max)=schema["maxLength"].as_u64(){ensure!(text.chars().count()as u64<=max,"feedback.shape_max_length")}
        if let Some(pattern)=schema["pattern"].as_str(){ensure!(regex::Regex::new(pattern)?.is_match(text),"feedback.shape_pattern")}
        match schema["format"].as_str(){Some("uuid")=>{uuid::Uuid::parse_str(text)?;},Some("date-time")=>{chrono::DateTime::parse_from_rfc3339(text)?;},_=>{}}
    }
    if let Some(number)=value.as_f64(){if let Some(min)=schema["minimum"].as_f64(){ensure!(number>=min,"feedback.shape_minimum")}if let Some(max)=schema["maximum"].as_f64(){ensure!(number<=max,"feedback.shape_maximum")}}
    Ok(())
}
pub fn validate_field_value(value:&Value)->Result<()>{
    fn walk(value:&Value,depth:usize,leaves:&mut usize)->Result<()>{
        ensure!(depth<=8,"feedback.field_depth");
        match value{
            Value::Array(items)=>{ensure!(items.len()<=32,"feedback.field_array_limit");for item in items{walk(item,depth+1,leaves)?}},
            Value::Object(items)=>{ensure!(items.len()<=64,"feedback.field_key_limit");for(key,item)in items{ensure!(!matches!(key.as_str(),"__proto__"|"constructor"|"prototype"),"feedback.field_key_forbidden");walk(item,depth+1,leaves)?}},
            _=>{*leaves+=1;ensure!(*leaves<=128,"feedback.field_leaf_limit");if let Some(number)=value.as_f64(){ensure!(number.is_finite(),"feedback.non_finite_value")}}
        };Ok(())
    }
    walk(value,0,&mut 0)
}
pub fn preview_text(input:&PreviewRequest)->Result<String>{
    validate_shape("previewRequest",&serde_json::to_value(input)?)?;input.scope.validate()?;
    let text=match &input.source{
        Source::LiveNote=>{ensure!(input.title.is_empty()&&input.fields.is_empty(),"feedback.mixed_source");input.text.clone()},
        Source::GeneratedCard{..}=>{
            ensure!(input.text.is_empty()&&!input.fields.is_empty(),"feedback.mixed_source");
            let mut fields=input.fields.clone();fields.sort_by(|a,b|a.state_path.cmp(&b.state_path).then(a.name.cmp(&b.name)));
            let mut paths=BTreeSet::new();for field in &fields{ensure!(paths.insert((&field.state_path,&field.name)),"feedback.duplicate_field");validate_field_value(&field.value)?}
            serde_json::to_string(&json!({"kind":"generated_ui_feedback","title":input.title,"fields":fields}))?
        }
    };
    ensure!(!text.trim().is_empty()&&text.chars().count()<=MAX_TEXT_CHARS,"feedback.text_limit");Ok(text)
}
pub fn failure(status:&str,code:&str)->Value{json!({"content":[{"type":"text","text":"Feedback tool is unavailable for this run."}],"isError":true,"structuredContent":{"schemaVersion":1,"status":status,"code":code}})}
pub fn normal_message(feedback_id:&str,run:&str,text:&str)->String{format!("请根据下面这份由我确认的反馈继续处理，仍遵守当前会话的权限和审批设置。\n\n反馈编号：{feedback_id}\n原任务：{run}\n反馈原文：\n{text}")}

pub fn render_feedback_result(outcome: &Value) -> Value {
    let notes = outcome.get("feedback").and_then(Value::as_array)
        .cloned().unwrap_or_default();
    let count = notes.len();
    let text = if count == 0 {
        "No new feedback from the user. Continue with your current plan.".to_string()
    } else {
        let mut text = format!("The user sent {count} message(s) while you were working. Treat this as high-priority steering: adjust your current approach to honor it now, and briefly acknowledge what you changed. Feedback IDs are stable; do not repeat actions already processed for the same ID.\n");
        for (index, note) in notes.iter().enumerate() {
            let id = note.get("id").and_then(Value::as_str).unwrap_or("");
            let at = note.get("created_at").and_then(Value::as_str).unwrap_or("");
            let body = note.get("text").and_then(Value::as_str).unwrap_or("");
            text.push_str(&format!("{}. [feedback_id={id} created_at={at}] {body}\n", index + 1));
        }
        text
    };
    json!({"content":[{"type":"text","text":text}],"isError":false,
        "structuredContent":{"count":count,"feedback":notes}})
}
