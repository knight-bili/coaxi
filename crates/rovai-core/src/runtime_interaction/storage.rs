//! Durable native interactions. Only the trusted native coordinator may capture/send;
//! User commands never supply a Native Session, RPC id, Run or original question.
use super::{Answer, Request, RequestBody};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    command::{
        ActorRef, CommandEnvelope, CommandExecution, CommandHandlerResult, DomainCommand,
        DomainCommandGateway, canonical_json_digest, sealed,
    },
    current_user::CURRENT_USER_ID,
    db::Database,
};

/// Constructed by Core from its live native reader, never deserialized from User input.
/// This value alone is not transport attestation; the coordinator must own that reader.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct NativeFrame {
    pub run: String,
    pub epoch: i64,
    pub input_delivery: String,
    pub core_generation: String,
    pub host_instance: String,
    pub sequence: i64,
    pub rpc_id: Value,
    pub method: String,
    pub session: String,
    pub binding: String,
    pub binding_generation: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveInteractionCommand {
    pub interaction_id: String,
    pub expected_version: i64,
    pub answer: Answer,
}
impl sealed::Sealed for ResolveInteractionCommand {}
impl DomainCommand for ResolveInteractionCommand {
    const TYPE: &'static str = "runtime_request.interaction.resolve";
    const EXACT_REPLAY: bool = true;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInteraction {
    pub id: String,
    pub conversation_id: String,
    pub kind: String,
    pub status: String,
    pub version: i64,
    /// Explicit UI projection: no native ids, original tool id, Host or Run credentials.
    pub body: Value,
    pub answer: Option<Answer>,
    pub close_reason: Option<String>,
}

fn user(actor: &ActorRef) -> bool {
    matches!(actor, ActorRef::User { user_id } if user_id == CURRENT_USER_ID)
}

// All decision/send fences consult the frozen owner, not caller-supplied scope.
fn live(connection: &Connection, id: &str, generation: &str, host: Option<&str>) -> Result<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM runtime_user_interaction i
         JOIN agent_run r ON r.id=i.agent_run_id JOIN conversation c ON c.id=r.conversation_id
         JOIN camp ON camp.id=c.camp_id JOIN runtime_input_delivery d ON d.id=i.input_delivery_id
         WHERE i.id=?1 AND i.core_generation=?2 AND (?3 IS NULL OR i.host_instance_id=?3)
         AND r.execution_epoch=i.execution_epoch AND r.status IN ('running','waiting')
         AND r.cancel_requested_at IS NULL AND r.runtime_recovery_required=0
         AND c.id=i.conversation_id AND c.camp_id=i.camp_id AND c.ended_at IS NULL
         AND camp.deletion_operation_id IS NULL AND camp.activation_state='active'
         AND c.native_session_id=i.native_session_id AND c.native_binding_id=i.native_binding_id
         AND c.native_binding_generation=i.native_binding_generation
         AND d.agent_run_id=r.id AND d.execution_epoch=r.execution_epoch
         AND d.native_binding_id=i.native_binding_id AND d.native_binding_generation=i.native_binding_generation
         AND (d.status='accepted' OR (d.status='prepared' AND d.dispatch_started_at IS NOT NULL)))",
        params![id,generation,host], |r|r.get(0),
    )?)
}

pub(crate) fn capture(
    database: &mut Database,
    frame: &NativeFrame,
    native_params: &Value,
) -> Result<String> {
    ensure!(
        frame.epoch > 0
            && frame.sequence >= 0
            && frame.binding_generation > 0
            && !frame.core_generation.trim().is_empty()
            && !frame.host_instance.trim().is_empty(),
        "interaction.invalid_frame"
    );
    ensure!(
        match &frame.rpc_id {
            Value::String(s) => s.chars().count() <= 4096,
            Value::Number(n) => n.as_i64().is_some(),
            _ => false,
        },
        "interaction.invalid_rpc_id"
    );
    let tx = database.connection_mut().transaction()?;
    let (camp,conversation,adapter):(String,String,String)=tx.query_row(
        "SELECT c.camp_id,c.id,r.runtime_adapter_kind FROM agent_run r JOIN conversation c ON c.id=r.conversation_id WHERE r.id=?1",
        [&frame.run],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    let request = Request::from_acp(adapter.parse()?, &frame.method, native_params)?
        .context("interaction.unsupported_method")?;
    let digest = canonical_json_digest(&json!({"frame":frame,"request":request}))?;
    let existing: Option<(String, String)> = tx
        .query_row(
            "SELECT id,request_digest FROM runtime_user_interaction WHERE core_generation=?1
         AND host_instance_id=?2 AND agent_run_id=?3 AND execution_epoch=?4 AND native_sequence=?5",
            params![
                frame.core_generation,
                frame.host_instance,
                frame.run,
                frame.epoch,
                frame.sequence
            ],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((id, stored)) = existing {
        ensure!(stored == digest, "interaction.frame_conflict");
        return Ok(id); // Read-only replay, even after cancellation or restart.
    }
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute("INSERT INTO runtime_user_interaction(id,camp_id,conversation_id,agent_run_id,execution_epoch,
        input_delivery_id,core_generation,host_instance_id,native_sequence,native_request_id_json,native_method,
        native_session_id,native_binding_id,native_binding_generation,request_json,request_digest,kind,status,version,
        followup_command_id,created_at,updated_at)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,'pending',1,?18,?19,?19)",
        params![id,camp,conversation,frame.run,frame.epoch,frame.input_delivery,frame.core_generation,
        frame.host_instance,frame.sequence,serde_json::to_string(&frame.rpc_id)?,frame.method,frame.session,
        frame.binding,frame.binding_generation,serde_json::to_string(&request)?,digest,request.kind(),Uuid::new_v4().to_string(),now])?;
    ensure!(
        live(&tx, &id, &frame.core_generation, Some(&frame.host_instance))?,
        "interaction.scope_not_live"
    );
    tx.commit()?;
    database.execution_wake.delegation.notify_waiters();
    Ok(id)
}

pub fn read(
    database: &Database,
    actor: &ActorRef,
    camp: &str,
    id: &str,
) -> Result<Option<UserInteraction>> {
    ensure!(user(actor), "interaction.user_required");
    let row: Option<(
        String,
        String,
        String,
        i64,
        String,
        Option<String>,
        Option<String>,
    )> = database
        .connection()
        .query_row(
            "SELECT conversation_id,kind,status,version,request_json,answer_json,close_reason
         FROM runtime_user_interaction WHERE id=?1 AND camp_id=?2",
            params![id, camp],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .optional()?;
    row.map(|(conversation_id,kind,status,version,request,answer,close_reason)|{
        let request:Request=serde_json::from_str(&request)?;
        request.validate()?;
        let body=match request.body {
            RequestBody::Question{questions}=>json!({"kind":"question","questions":questions.into_iter().map(|q|
                json!({"id":q.id,"question":q.question,"header":q.header,"multiSelect":q.multi_select,
                    "options":q.options.into_iter().map(|o|json!({"label":o.label,"description":o.description})).collect::<Vec<_>>()})).collect::<Vec<_>>()}),
            RequestBody::PlanApproval{plan_markdown}=>json!({"kind":"plan_approval","planMarkdown":plan_markdown}),
        };
        Ok(UserInteraction{id:id.into(),conversation_id,kind,status,version,body,
            answer:answer.map(|s|serde_json::from_str(&s)).transpose()?,close_reason})
    }).transpose()
}

pub fn resolve(
    database: &mut Database,
    envelope: &CommandEnvelope<ResolveInteractionCommand>,
    current_generation: &str,
) -> Result<CommandExecution> {
    // Gateway validates exact actor/camp/payload identity before returning an old receipt.
    DomainCommandGateway.execute(database,envelope,|tx|{
        let reject=|code|Ok(CommandHandlerResult::rejected(code,json!({"interactionId":envelope.payload.interaction_id})));
        if !user(&envelope.actor) {return reject("interaction.user_required");}
        let row:Option<(String,String,i64,String)>=tx.query_row(
            "SELECT camp_id,status,version,request_json FROM runtime_user_interaction WHERE id=?1",
            [&envelope.payload.interaction_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let Some((camp,status,version,request))=row else{return reject("interaction.not_found");};
        if envelope.camp_id.as_deref()!=Some(&camp) {return reject("interaction.not_found");}
        if status!="pending" || version!=envelope.payload.expected_version {return reject("interaction.stale");}
        if !live(tx,&envelope.payload.interaction_id,current_generation,None)? {return reject("interaction.scope_not_live");}
        let request:Request=serde_json::from_str(&request)?;
        let reply=match request.reply(&envelope.payload.answer) {Ok(reply)=>reply,Err(_)=>return reject("interaction.invalid_answer")};
        let now=chrono::Utc::now().to_rfc3339();
        tx.execute("UPDATE runtime_user_interaction SET status='resolved',version=version+1,
            answer_json=?2,response_json=?3,followup_feedback=?4,resolved_command_id=?5,resolved_at=?6,updated_at=?6 WHERE id=?1",
            params![envelope.payload.interaction_id,serde_json::to_string(&envelope.payload.answer)?,
                serde_json::to_string(&reply.native_response)?,reply.followup_feedback,envelope.command_id,now])?;
        Ok(CommandHandlerResult::applied("interaction.resolved",json!({"interactionId":envelope.payload.interaction_id,"version":version+1}),None))
    })
}

/// Obtained once, after a durable dispatch marker. It cannot be recovered for resend.
#[derive(Debug)]
pub(crate) struct NativeReply {
    pub token: String,
    pub rpc_id: Value,
    pub response: Value,
}
pub(crate) fn begin_dispatch(
    database: &mut Database,
    id: &str,
    expected_version: i64,
    generation: &str,
    host: &str,
) -> Result<NativeReply> {
    let tx = database.connection_mut().transaction()?;
    ensure!(
        live(&tx, id, generation, Some(host))?,
        "interaction.scope_not_live"
    );
    let (status,version,rpc,response,feedback):(String,i64,String,Option<String>,Option<String>)=tx.query_row(
        "SELECT status,version,native_request_id_json,response_json,followup_feedback FROM runtime_user_interaction WHERE id=?1",
        [id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
    ensure!(
        status == "resolved" && version == expected_version,
        "interaction.stale"
    );
    // Grok's keep_planning drops feedback: never send before original Core User-queue
    // integration can prove those exact words were registered. No arbitrary setter.
    ensure!(
        feedback.is_none(),
        "interaction.followup_registration_required"
    );
    let token = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE runtime_user_interaction SET status='dispatching',version=version+1,
        dispatch_token=?2,dispatch_started_at=?3,updated_at=?3 WHERE id=?1",
        params![id, token, now],
    )?;
    let reply = NativeReply {
        token,
        rpc_id: serde_json::from_str(&rpc)?,
        response: serde_json::from_str(&response.context("interaction.response_missing")?)?,
    };
    tx.commit()?;
    Ok(reply)
}

pub(crate) fn finish_dispatch(
    database: &mut Database,
    id: &str,
    token: &str,
    sent: bool,
) -> Result<bool> {
    let now = chrono::Utc::now().to_rfc3339();
    let changed=database.connection().execute("UPDATE runtime_user_interaction SET status=?3,
        close_reason=?4,finished_at=?5,updated_at=?5,version=version+1 WHERE id=?1 AND dispatch_token=?2 AND status='dispatching'",
        params![id,token,if sent {"sent"} else {"delivery_unknown"},if sent {None} else {Some("native_write_unconfirmed")},now])?;
    if changed > 0 {
        database.execution_wake.delegation.notify_waiters();
    }
    Ok(changed > 0)
}

/// Called in startup recovery's original transaction, before any new native reader.
pub(crate) fn close_on_restart(tx: &Transaction<'_>, now: &str) -> Result<usize> {
    let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='runtime_user_interaction')",[],|r|r.get(0))?;
    if !exists {
        return Ok(0);
    }
    Ok(tx.execute("UPDATE runtime_user_interaction SET status=CASE WHEN status='dispatching' THEN 'delivery_unknown' ELSE 'closed' END,
        close_reason='core_restarted',finished_at=?1,updated_at=?1,version=version+1 WHERE status IN ('pending','resolved','dispatching')",[now])?)
}
