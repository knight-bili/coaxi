use super::*;
use crate::{
    command::{
        ActorRef, CommandEnvelope, CommandExecution, CommandHandlerResult, DomainCommand,
        DomainCommandGateway, EntityReference, sealed,
    },
    db::Database,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetPolicy {
    pub schema_version: u32,
    pub expected_revision: i64,
    pub members: Value,
}
impl sealed::Sealed for SetPolicy {}
impl DomainCommand for SetPolicy {
    const TYPE: &'static str = "runtime.steer.policy.set";
    const EXACT_REPLAY: bool = true;
}
pub fn set(
    database: &mut Database,
    envelope: &CommandEnvelope<SetPolicy>,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(database,envelope,|tx|{
        if !matches!(&envelope.actor,ActorRef::User{..}){return Ok(CommandHandlerResult::rejected("native_lifecycle.user_required",json!({})))}
        let input=&envelope.payload;
        validate_operation("runtime.steer.policy.set",&json!({"schemaVersion":input.schema_version,"commandId":envelope.command_id,"expectedRevision":input.expected_revision,"members":input.members}))?;
        if storage::policy(tx)?["revision"].as_i64()!=Some(input.expected_revision){return Ok(CommandHandlerResult::rejected("native_lifecycle.policy_changed",json!({})))}
        let value=storage::set_policy(tx,input.expected_revision,&input.members)?;
        Ok(CommandHandlerResult::applied("native_lifecycle.policy_saved",value,Some(EntityReference{entity_type:"native_steer_policy".into(),entity_id:"singleton".into()})))
    })
}
