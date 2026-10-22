use super::*;

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum NativeContinuationMode{Continue,ForkTail,ForkPoint}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NativeSourceReferenceV1{
    pub schema_version:u32,pub import_id:String,pub import_revision:String,pub provider:String,
    pub native_session_id:String,pub source_handle_id:String,pub source_revision:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NativeForkPointV1{
    pub selected_turn_id:String,pub message_id:String,pub message_fingerprint:Option<String>,pub message_occurrence:Option<u32>,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NativeSessionOriginV1{
    pub schema_version:u32,pub origin_id:String,pub command_id:String,pub source:NativeSourceReferenceV1,
    pub mode:NativeContinuationMode,pub point:Option<NativeForkPointV1>,pub workspace_digest:String,
}
impl NativeSessionOriginV1{
    pub fn validate(&self)->Result<()>{
        validate_shape("NativeSessionOriginV1",&serde_json::to_value(self)?)?;
        ensure!((self.mode==NativeContinuationMode::ForkPoint)==self.point.is_some(),"native_lifecycle.fork_point_mode_mismatch");
        if let Some(point)=&self.point{ensure!(point.message_fingerprint.is_some()==point.message_occurrence.is_some(),"native_lifecycle.fingerprint_occurrence_required");}Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct FrozenNativeSteerV1{pub schema_version:u32,pub policy_revision:i64,pub enabled:bool}
impl FrozenNativeSteerV1{pub fn validate(&self)->Result<()>{validate_shape("FrozenNativeSteerV1",&serde_json::to_value(self)?)}}

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NativeSteerScopeV1{pub thread_id:String,pub conversation_id:Option<String>,pub scope_kind:String}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NativeSteerTargetV1{
    pub scope:NativeSteerScopeV1,pub agent_id:String,pub agent_run_id:String,pub execution_epoch:i64,
    pub native_binding_id:String,pub native_binding_generation:i64,pub binding_compatibility_digest:String,pub active_turn_id:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NativeSteerMaterialV1{
    pub source_id:String,pub snapshot_digest:String,pub display_name:String,pub kind:String,pub byte_size:u64,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NativeSteerInputV1{pub text:String,pub materials:Vec<NativeSteerMaterialV1>}
impl NativeSteerInputV1{
    pub fn validate(&self)->Result<()>{
        validate_shape("NativeSteerInputV1",&serde_json::to_value(self)?)?;
        ensure!(!self.text.trim().is_empty()||!self.materials.is_empty(),"native_lifecycle.empty_input");
        let mut ids=std::collections::HashSet::new();for material in &self.materials{ensure!(ids.insert(&material.source_id),"native_lifecycle.duplicate_material");}Ok(())
    }
    pub fn image_bytes(&self)->Result<u64>{self.validate()?;self.materials.iter().filter(|item|item.kind=="image").try_fold(0u64,|total,item|total.checked_add(item.byte_size).context("native_lifecycle.image_budget_exceeded"))}
}
