use super::*;
use crate::{
    command::{
        ActorRef, CommandEnvelope, CommandExecution, CommandHandlerResult, DomainCommand,
        DomainCommandGateway, sealed,
    },
    draft_client::DraftClient,
};
use rusqlite::{Connection, OptionalExtension, params};

pub fn get(connection: &Connection, id: &str) -> Result<Option<Value>> {
    uuid::Uuid::parse_str(id)?;
    let row:Option<(i64,i64,String,String,String)>=connection.query_row("SELECT o.revision,a.version,o.action,o.timezone,o.source_request_id FROM conversation_tool_automation_owner o JOIN automation a ON a.id=o.automation_id WHERE o.automation_id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).optional()?;
    row.map(|(revision,version,action,timezone,request)|{let value=json!({"automationId":id,"revision":revision,"automationVersion":version,"action":action,"timezone":timezone,"sourceRequestId":request});validate("AutomationOwnerV1",&value)?;Ok(value)}).transpose()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetOwner {
    #[serde(default, skip_deserializing)]
    pub client: DraftClient,
    pub automation_id: String,
    pub expected_automation_version: i64,
    pub expected_owner_revision: i64,
    pub action: String,
    pub timezone: String,
}
impl sealed::Sealed for SetOwner {}
impl DomainCommand for SetOwner {
    const TYPE: &'static str = "conversationTools.automationOwner.set";
    const EXACT_REPLAY: bool = true;
    fn draft_client(&self) -> Option<&DraftClient> {
        Some(&self.client)
    }
}
pub fn set(
    database: &mut crate::db::Database,
    envelope: &CommandEnvelope<SetOwner>,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(database,envelope,|tx|{
        ensure!(matches!(&envelope.actor,ActorRef::User{..})&&envelope.camp_id.is_none(),"conversation_tools.user_required");let p=&envelope.payload;
        let mut value=serde_json::to_value(p)?;value.as_object_mut().context("conversation_tools.command_invalid")?.remove("client");value["commandId"]=json!(envelope.command_id);validate_operation(SetOwner::TYPE,&value)?;
        let current=get(tx,&p.automation_id)?.context("conversation_tools.automation_owner_unavailable")?;
        if current["revision"]!=p.expected_owner_revision||current["automationVersion"]!=p.expected_automation_version{return Ok(CommandHandlerResult::rejected("conversation_tools.automation_owner_changed",json!({})))}
        let timezone=calendar::zone(&p.timezone)?.name();
        let(raw,enabled):(String,bool)=tx.query_row("SELECT schedule_json,enabled FROM automation WHERE id=?1",[&p.automation_id],|row|Ok((row.get(0)?,row.get(1)?)))?;
        let schedule:crate::automation::AutomationSchedule=serde_json::from_str(&raw)?;
        let next=if enabled{calendar::next(&schedule,timezone,chrono::Utc::now())?.map(|value|value.to_rfc3339_opts(chrono::SecondsFormat::Secs,true))}else{None};let now=chrono::Utc::now().to_rfc3339();
        ensure!(tx.execute("UPDATE automation SET version=version+1,next_run_at=?3,updated_at=?4 WHERE id=?1 AND version=?2",params![p.automation_id,p.expected_automation_version,next,now])?==1,"conversation_tools.automation_owner_changed");
        ensure!(tx.execute("UPDATE conversation_tool_automation_owner SET revision=revision+1,action=?3,timezone=?4,updated_at=?5 WHERE automation_id=?1 AND revision=?2",params![p.automation_id,p.expected_owner_revision,p.action,timezone,now])?==1,"conversation_tools.automation_owner_changed");
        crate::collaboration::append_domain_event(tx,"conversationTools.automationOwnerChanged",None,Some(("automation",&p.automation_id)),&envelope.actor,None,&json!({"automationId":p.automation_id,"revision":p.expected_owner_revision+1}))?;
        Ok(CommandHandlerResult::applied("conversation_tools.automation_owner_saved",get(tx,&p.automation_id)?.context("conversation_tools.automation_owner_unavailable")?,None))
    })
}
