use super::*;

#[derive(Debug,Clone,Copy,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct FlagsV1{pub question:bool,pub session_info:bool,pub automations:bool,pub work_tasks:bool}
impl FlagsV1{
    pub(crate) fn intersect(self,other:Self)->Self{Self{question:self.question&&other.question,session_info:self.session_info&&other.session_info,automations:self.automations&&other.automations,work_tasks:self.work_tasks&&other.work_tasks}}
    pub(crate) fn any(self)->bool{self.question||self.session_info||self.automations||self.work_tasks}
    pub(crate) fn permits(self,name:&str)->bool{match name{"ask_user_question"=>self.question,"get_session_info"=>self.session_info,"create_automation"=>self.automations,"create_work_task"=>self.work_tasks,_=>false}}
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct MemberPolicyV1{pub agent_id:String,pub flags:FlagsV1}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PolicyV1{pub schema_version:u32,pub revision:i64,pub global_flags:FlagsV1,pub feedback_gate:Option<bool>,pub members:Vec<MemberPolicyV1>}
impl Default for PolicyV1{fn default()->Self{Self{schema_version:1,revision:0,global_flags:FlagsV1::default(),feedback_gate:None,members:Vec::new()}}}
impl PolicyV1{pub fn validate(&self)->Result<()>{validate("PolicyV1",&serde_json::to_value(self)?)}pub(crate) fn member_flags(&self,id:&str)->FlagsV1{self.global_flags.intersect(self.members.iter().find(|member|member.agent_id==id).map(|member|member.flags).unwrap_or_default())}}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ScopeKindV1{Public,SingleChat,DelegationChild}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeV1{pub thread_id:String,pub conversation_id:Option<String>,pub scope_kind:ScopeKindV1}
impl ScopeV1{pub fn validate(&self)->Result<()>{validate("ScopeV1",&serde_json::to_value(self)?)}}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct SourceGrantV1{
    pub grant_id:String,pub revision:i64,pub scope:ScopeV1,pub source_session_id:String,pub source_kind:String,
    pub source_digest:String,pub through_sequence:i64,pub expires_at:String,pub legacy_session_id:Option<i64>,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ProjectGrantV1{pub project_id:String,pub project_path:String,pub revision:i64,pub member_agent_ids:Vec<String>}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct FrozenConversationToolsV1{
    pub schema_version:u32,pub policy_revision:i64,pub policy_digest:String,pub scope:ScopeV1,pub agent_id:String,
    pub flags:FlagsV1,pub feedback_gate:Option<bool>,pub source_grants:Vec<SourceGrantV1>,pub project_grants:Vec<ProjectGrantV1>,pub catalog_digest:String,
}
impl FrozenConversationToolsV1{pub fn validate(&self)->Result<()>{validate("FrozenConversationToolsV1",&serde_json::to_value(self)?)}}
