use super::*;
use crate::command::{
    ActorRef, CommandEnvelope, CommandExecution, CommandHandlerResult, DomainCommand,
    DomainCommandGateway, EntityReference, sealed,
};
use crate::draft_client::DraftClient;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

pub fn get(connection: &Connection) -> Result<PolicyV1> {
    if storage::absent(connection)? {
        return Ok(PolicyV1::default());
    }
    let raw:Option<String>=connection.query_row("SELECT r.value_json FROM conversation_tool_policy_head h JOIN conversation_tool_policy_revision r ON r.revision=h.revision WHERE h.singleton=1",[],|row|row.get(0)).optional()?;
    let value = raw
        .map(|raw| serde_json::from_str::<PolicyV1>(&raw))
        .transpose()?
        .unwrap_or_default();
    value.validate()?;
    Ok(value)
}
pub(crate) fn replace_on(
    tx: &Transaction<'_>,
    expected: i64,
    global_flags: FlagsV1,
    feedback_gate: Option<bool>,
    members: Vec<MemberPolicyV1>,
) -> Result<PolicyV1> {
    let previous = get(tx)?;
    ensure!(
        previous.revision == expected,
        "conversation_tools.policy_changed"
    );
    let revision = expected
        .checked_add(1)
        .context("conversation_tools.revision_exhausted")?;
    let value = PolicyV1 {
        schema_version: 1,
        revision,
        global_flags,
        feedback_gate,
        members,
    };
    value.validate()?;
    let mut unique = std::collections::HashSet::new();
    for member in &value.members {
        ensure!(
            unique.insert(member.agent_id.as_str()),
            "conversation_tools.member_duplicate"
        );
        let present: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_profile WHERE id=?1 AND profile_status='present')",
            [&member.agent_id],
            |row| row.get(0),
        )?;
        ensure!(present, "conversation_tools.member_unavailable");
    }
    let json = serde_json::to_value(&value)?;
    tx.execute("INSERT INTO conversation_tool_policy_revision(revision,digest,value_json,created_at) VALUES(?1,?2,?3,?4)",params![revision,digest(&json)?,serde_json::to_string(&value)?,chrono::Utc::now().to_rfc3339()])?;
    tx.execute("INSERT INTO conversation_tool_policy_head(singleton,revision) VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET revision=excluded.revision",[revision])?;
    Ok(value)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetPolicy {
    #[serde(default, skip_deserializing)]
    pub client: DraftClient,
    pub expected_revision: i64,
    pub global_flags: FlagsV1,
    pub feedback_gate: Option<bool>,
    pub members: Vec<MemberPolicyV1>,
}
impl sealed::Sealed for SetPolicy {}
impl DomainCommand for SetPolicy {
    const TYPE: &'static str = "conversationTools.policy.set";
    const EXACT_REPLAY: bool = true;
    fn draft_client(&self) -> Option<&DraftClient> {
        Some(&self.client)
    }
}
pub fn set(
    database: &mut crate::db::Database,
    envelope: &CommandEnvelope<SetPolicy>,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(database,envelope,|tx|{
        ensure!(matches!(&envelope.actor,ActorRef::User{..})&&envelope.camp_id.is_none(),"conversation_tools.user_required");
        let input=&envelope.payload;
        validate_operation(SetPolicy::TYPE,&json!({"commandId":envelope.command_id,"expectedRevision":input.expected_revision,"globalFlags":input.global_flags,"feedbackGate":input.feedback_gate,"members":input.members}))?;
        if get(tx)?.revision!=input.expected_revision{return Ok(CommandHandlerResult::rejected("conversation_tools.policy_changed",json!({})))}
        let value=replace_on(tx,input.expected_revision,input.global_flags,input.feedback_gate,input.members.clone())?;
        crate::collaboration::append_domain_event(tx,"conversationTools.policyChanged",None,Some(("conversation_tool_policy","singleton")),&envelope.actor,None,&json!({"revision":value.revision}))?;
        Ok(CommandHandlerResult::applied("conversation_tools.policy_saved",serde_json::to_value(value)?,Some(EntityReference{entity_type:"conversation_tool_policy".into(),entity_id:"singleton".into()})))
    })
}
