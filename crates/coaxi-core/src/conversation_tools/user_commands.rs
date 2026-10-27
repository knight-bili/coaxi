//! 封闭的User命令；认证编辑器由Host注入，模型MCP不经过此入口。
use super::*;
use crate::{
    command::{
        ActorRef, CommandEnvelope, CommandExecution, CommandHandlerResult, DomainCommand,
        DomainCommandGateway, sealed,
    },
    draft_client::DraftClient,
};
use rusqlite::Connection;
use std::path::Path;

macro_rules! user_command {
    ($name:ident,$operation:literal) => {
        impl sealed::Sealed for $name {}
        impl DomainCommand for $name {
            const TYPE: &'static str = $operation;
            const EXACT_REPLAY: bool = true;
            fn draft_client(&self) -> Option<&DraftClient> {
                Some(&self.client)
            }
        }
    };
}
fn input<C: DomainCommand>(envelope: &CommandEnvelope<C>) -> Result<Value> {
    ensure!(
        matches!(&envelope.actor, ActorRef::User { .. }) && envelope.camp_id.is_none(),
        "conversation_tools.user_required"
    );
    let mut value = serde_json::to_value(&envelope.payload)?;
    value
        .as_object_mut()
        .context("conversation_tools.command_invalid")?
        .remove("client");
    value["commandId"] = json!(envelope.command_id);
    validate_operation(C::TYPE, &value)?;
    Ok(value)
}
fn applied(value: Value) -> CommandHandlerResult {
    CommandHandlerResult::applied("conversation_tools.saved", value, None)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuestionAnswer {
    pub question_id: String,
    pub labels: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnswerQuestion {
    #[serde(default, skip_deserializing)]
    pub client: DraftClient,
    pub request_id: String,
    pub expected_revision: i64,
    pub answers: Vec<QuestionAnswer>,
}
user_command!(AnswerQuestion, "conversationTools.questions.answer");
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclineQuestion {
    #[serde(default, skip_deserializing)]
    pub client: DraftClient,
    pub request_id: String,
    pub expected_revision: i64,
}
user_command!(DeclineQuestion, "conversationTools.questions.decline");
pub fn answer(
    database: &mut crate::db::Database,
    envelope: &CommandEnvelope<AnswerQuestion>,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(database, envelope, |tx| {
        let value = input(envelope)?;
        let p = &envelope.payload;
        let lease = scope::request_owner(tx, &p.request_id)?
            .context("conversation_tools.question_unavailable")?;
        let value = questions::answer_on(
            tx,
            &lease,
            p.client.id(),
            &envelope.command_id,
            &p.request_id,
            p.expected_revision,
            &value["answers"],
            false,
        )?;
        crate::collaboration::append_domain_event(
            tx,
            "conversationTools.questionChanged",
            Some(&lease.frozen.scope.thread_id),
            Some(("conversation_tool_request", &p.request_id)),
            &envelope.actor,
            None,
            &json!({"requestId":p.request_id,"state":value["state"]}),
        )?;
        Ok(applied(value))
    })
}
pub fn decline(
    database: &mut crate::db::Database,
    envelope: &CommandEnvelope<DeclineQuestion>,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(database, envelope, |tx| {
        input(envelope)?;
        let p = &envelope.payload;
        let lease = scope::request_owner(tx, &p.request_id)?
            .context("conversation_tools.question_unavailable")?;
        let value = questions::answer_on(
            tx,
            &lease,
            p.client.id(),
            &envelope.command_id,
            &p.request_id,
            p.expected_revision,
            &json!([]),
            true,
        )?;
        crate::collaboration::append_domain_event(
            tx,
            "conversationTools.questionChanged",
            Some(&lease.frozen.scope.thread_id),
            Some(("conversation_tool_request", &p.request_id)),
            &envelope.actor,
            None,
            &json!({"requestId":p.request_id,"state":value["state"]}),
        )?;
        Ok(applied(value))
    })
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmGrant {
    #[serde(default, skip_deserializing)]
    pub client: DraftClient,
    pub preview_id: String,
    pub preview_digest: String,
}
user_command!(ConfirmGrant, "conversationTools.grants.confirm");
pub fn confirm_grant(
    database: &mut crate::db::Database,
    root: &Path,
    envelope: &CommandEnvelope<ConfirmGrant>,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(database, envelope, |tx| {
        input(envelope)?;
        let p = &envelope.payload;
        let value = grants::confirm_on(tx, root, p.client.id(), &p.preview_id, &p.preview_digest)?;
        crate::collaboration::append_domain_event(
            tx,
            "conversationTools.grantChanged",
            Some(&value.scope.thread_id),
            Some(("conversation_tool_source_grant", &value.grant_id)),
            &envelope.actor,
            None,
            &json!({"grantId":value.grant_id,"revision":value.revision}),
        )?;
        Ok(applied(serde_json::to_value(value)?))
    })
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevokeGrant {
    #[serde(default, skip_deserializing)]
    pub client: DraftClient,
    pub grant_id: String,
    pub expected_revision: i64,
}
user_command!(RevokeGrant, "conversationTools.grants.revoke");
pub fn revoke_grant(
    database: &mut crate::db::Database,
    envelope: &CommandEnvelope<RevokeGrant>,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(database, envelope, |tx| {
        input(envelope)?;
        let p = &envelope.payload;
        let value = grants::revoke_on(tx, &p.grant_id, p.expected_revision)?;
        crate::collaboration::append_domain_event(
            tx,
            "conversationTools.grantChanged",
            None,
            Some(("conversation_tool_source_grant", &p.grant_id)),
            &envelope.actor,
            None,
            &json!({"grantId":p.grant_id,"revoked":true}),
        )?;
        Ok(applied(value))
    })
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    CancelRemaining,
    RevokeRun,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveRequest {
    #[serde(default, skip_deserializing)]
    pub client: DraftClient,
    pub action: Resolution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_execution_epoch: Option<i64>,
}
user_command!(ResolveRequest, "conversationTools.requests.resolve");
pub fn resolve(
    database: &mut crate::db::Database,
    envelope: &CommandEnvelope<ResolveRequest>,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(database,envelope,|tx|{
        input(envelope)?;let p=&envelope.payload;
        let value=match p.action{
            Resolution::CancelRemaining=>{
                let id=p.request_id.as_deref().context("conversation_tools.request_id_required")?;
                let lease=scope::request_owner(tx,id)?.context("conversation_tools.original_request_unavailable")?;
                requests::cancel_remaining(tx,id)?;requests::status(tx,&lease,id)?
            },
            Resolution::RevokeRun=>{
                let run=p.agent_run_id.as_deref().context("conversation_tools.run_required")?;
                let epoch=p.expected_execution_epoch.context("conversation_tools.epoch_required")?;
                let lease=scope::retained_run(tx,run,epoch)?;
                ensure!(p.scope.as_ref()==Some(&lease.frozen.scope),"conversation_tools.scope_changed");
                question_delivery::revoke(tx,&lease)?;
                json!({"commandId":envelope.command_id,"agentRunId":run,"executionEpoch":epoch,"revoked":true})
            }
        };
        crate::collaboration::append_domain_event(tx,"conversationTools.authorizationChanged",None,None,&envelope.actor,None,&json!({"action":p.action,"requestId":p.request_id,"agentRunId":p.agent_run_id}))?;Ok(applied(value))
    })
}
pub(crate) fn request_status(connection: &Connection, id: &str) -> Result<Value> {
    let Some(lease) = scope::request_owner(connection, id)? else {
        return Ok(json!({"request_id":id,"state":"not_seen"}));
    };
    requests::status(connection, &lease, id)
}
