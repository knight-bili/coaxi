//! User read authority comes from the Host actor and the durable Thread, never an active parent Run.
use super::*;
#[path = "delegation_user_tool_source.rs"]
pub(crate) mod tool_source;
use crate::{command::ActorRef, delegation::Identity};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ListParams {
    parent_conversation_id: crate::delegation_contract::NonEmpty,
    #[serde(default, deserialize_with = "crate::delegation_contract::present")]
    cursor: Option<crate::delegation_contract::NonEmpty>,
    #[serde(default, deserialize_with = "list_limit")]
    limit: Option<u32>,
}
fn list_limit<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<u32>, D::Error> {
    let value = u32::deserialize(d)?;
    if (1..=100).contains(&value) {
        Ok(Some(value))
    } else {
        Err(serde::de::Error::custom(
            "delegation page limit out of range",
        ))
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetParams {
    delegation_id: crate::delegation_contract::NonEmpty,
}
fn reader(actor: &ActorRef) -> Result<&str> {
    match actor {
        ActorRef::User { user_id } => Ok(user_id),
        _ => anyhow::bail!("delegation.user_required"),
    }
}
fn require_installed(conn: &Connection) -> Result<()> {
    ensure!(conn.query_row(
        "SELECT count(*)=3 FROM sqlite_schema WHERE type='table' AND name IN ('delegation','delegation_control','delegation_attempt')",
        [], |r| r.get::<_, bool>(0),
    )?, "delegation.unavailable");
    Ok(())
}
fn parent_camp(conn: &Connection, parent: &str) -> Result<String> {
    conn.query_row(
        "SELECT c.camp_id FROM conversation c JOIN camp ON camp.id=c.camp_id
         WHERE c.id=?1 AND camp.deletion_operation_id IS NULL",
        [parent],
        |r| r.get(0),
    )
    .optional()?
    .context("delegation.not_found")
}
fn identity(conn: &Connection, task: &str) -> Result<Option<Identity>> {
    let parent: Option<String> = conn.query_row(
        "SELECT d.parent_conversation_id FROM delegation d JOIN conversation c ON c.id=d.child_conversation_id
         JOIN camp ON camp.id=c.camp_id WHERE d.id=?1 AND c.kind='delegation_child'
           AND d.camp_id=c.camp_id AND d.owner_agent_id=c.agent_id AND camp.deletion_operation_id IS NULL",
        [task], |r| r.get(0),
    ).optional()?;
    parent
        .map(|parent| crate::delegation::find_for_parent(conn, task, &parent))
        .transpose()
        .map(Option::flatten)
}
fn user_scope(conn: &Connection, actor: &ActorRef, task: &str) -> Result<Option<ReadScope>> {
    let reader = reader(actor)?;
    require_installed(conn)?;
    let Some(identity) = identity(conn, task)? else {
        return Ok(None);
    };
    let through = conn.query_row(
        "SELECT last_message_sequence FROM conversation WHERE id=?1",
        [&identity.child_conversation_id],
        |r| r.get(0),
    )?;
    Ok(Some(ReadScope {
        key: ScopeKey {
            camp: identity.camp_id,
            reader: reader.into(),
            task: task.into(),
            child: identity.child_conversation_id,
            own: false,
            user: true,
        },
        through,
    }))
}
pub(crate) fn read(db: &Database, actor: &ActorRef, input: &ReadParams) -> Result<Value> {
    let tx = db.connection().unchecked_transaction()?;
    let task = input.task_id.as_str();
    let Some(scope) = user_scope(&tx, actor, task)? else {
        return Ok(json!({"task_id":task,"status":"unknown"}));
    };
    let value = read_scoped(&tx, input, scope)?;
    tx.commit()?;
    Ok(value)
}
pub(crate) fn history(db: &Database, actor: &ActorRef, input: &HistoryParams) -> Result<Value> {
    let tx = db.connection().unchecked_transaction()?;
    let task = input.task_id.as_str();
    let Some(scope) = user_scope(&tx, actor, task)? else {
        return Ok(json!({"task_id":task,"status":"unknown"}));
    };
    let value = history_scoped(&tx, input, scope)?;
    tx.commit()?;
    Ok(value)
}
fn detail(conn: &Connection, id: &Identity) -> Result<Value> {
    let (attempt, version, taken_over, run, epoch, status, runtime, created, started, ended):
        (i64,i64,bool,String,i64,String,String,String,Option<String>,Option<String>) = conn.query_row(
        "SELECT control.current_attempt_no,control.control_version,control.takeover_at IS NOT NULL,
            r.id,r.execution_epoch,r.status,d.requested_runtime,d.created_at,r.started_at,r.ended_at
         FROM delegation d JOIN delegation_control control ON control.delegation_id=d.id
         JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=control.current_attempt_no
         JOIN agent_run r ON r.id=a.agent_run_id WHERE d.id=?1", [&id.id], |r|
            Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?)),
    )?;
    let mut value = json!({"delegationId":id.id,"threadId":id.camp_id,"ownerAgentId":id.owner_agent_id,
        "rootConversationId":id.root_conversation_id,"parentConversationId":id.parent_conversation_id,
        "parentDelegationId":id.parent_delegation_id,"conversationId":id.child_conversation_id,"depth":id.depth,
        "currentAttemptNo":attempt,"controlVersion":version,"takenOver":taken_over,
        "agentRunId":run,"executionEpoch":epoch,"status":status,"agentType":runtime,
        "createdAt":created,"startedAt":started,"endedAt":ended,
        "cleanupPending":(taken_over || status == "cancelled") && crate::delegation_control::cleanup_pending(conn,&id.id)?});
    if matches!(status.as_str(), "queued" | "running" | "waiting") {
        if let Some(blocked) = crate::delegation_model_command::blocked_summary(conn, &run, epoch)?
        {
            value["blockedOn"] = blocked;
        }
    }
    Ok(value)
}
pub(crate) fn get(db: &Database, actor: &ActorRef, input: &GetParams) -> Result<Value> {
    reader(actor)?;
    let tx = db.connection().unchecked_transaction()?;
    require_installed(&tx)?;
    let id = identity(&tx, input.delegation_id.as_str())?.context("delegation.not_found")?;
    let mut value = detail(&tx, &id)?;
    value["activity"] = activity(db, &tx, &id)?;
    tx.commit()?;
    Ok(value)
}

// User-only viewer projection. List/model status stay metadata-only; the child
// identity was authorized above, and every activity comes from that exact child.
fn activity(db: &Database, conn: &Connection, id: &Identity) -> Result<Value> {
    let runs = {
        let mut statement = conn.prepare(
            "SELECT r.id,r.execution_epoch,r.status,r.created_at,r.started_at,r.ended_at,
                r.camp_turn_id,r.trigger_conversation_message_id,r.version,r.cancel_requested_at,
                r.last_error_code,r.final_conversation_message_id,r.execution_evidence_change_sequence,
                (SELECT count(*) FROM agent_run_execution_evidence e WHERE e.agent_run_id=r.id)
             FROM agent_run r WHERE r.conversation_id=?1
               AND r.invocation_kind IN ('delegation','delegation_user') ORDER BY r.created_at,r.id",
        )?;
        let rows = statement.query_map([&id.child_conversation_id], |row| {
            Ok(json!({"id":row.get::<_,String>(0)?,"executionEpoch":row.get::<_,i64>(1)?,
                "status":row.get::<_,String>(2)?,"createdAt":row.get::<_,String>(3)?,
                "startedAt":row.get::<_,Option<String>>(4)?,"endedAt":row.get::<_,Option<String>>(5)?,
                "threadTurnId":row.get::<_,String>(6)?,"triggerConversationMessageId":row.get::<_,String>(7)?,
                "version":row.get::<_,i64>(8)?,"cancelRequestedAt":row.get::<_,Option<String>>(9)?,
                "lastErrorCode":row.get::<_,Option<String>>(10)?,"finalConversationMessageId":row.get::<_,Option<String>>(11)?,
                "executionEvidenceChangeSequence":row.get::<_,i64>(12)?,"executionEvidenceCount":row.get::<_,i64>(13)?}))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut evidence = Vec::new();
    for run in &runs {
        evidence.extend(crate::read_model::public_execution_evidence_for_agent_run(
            conn,
            run["id"].as_str().context("delegation.activity_invalid")?,
        )?);
    }
    crate::execution_text::overlay(db, &mut evidence)?;
    let approvals = crate::read_model::load_conversation_approvals(
        conn,
        &id.camp_id,
        Some(&id.child_conversation_id),
        true,
        None,
    )?;
    Ok(json!({"runs":runs,"executionEvidence":evidence,"approvals":approvals}))
}
pub(crate) fn list(db: &Database, actor: &ActorRef, input: &ListParams) -> Result<Value> {
    let reader = reader(actor)?;
    let tx = db.connection().unchecked_transaction()?;
    require_installed(&tx)?;
    let parent = input.parent_conversation_id.as_str();
    let camp = parent_camp(&tx, parent)?;
    let (upper, before) = if let Some(cursor) = &input.cursor {
        let Cursor::UserList {
            v: 1,
            reader: old_reader,
            parent: old_parent,
            camp: old_camp,
            upper,
            before,
        } = decode(cursor.as_str(), 4096)?
        else {
            anyhow::bail!("delegation.cursor_invalid");
        };
        ensure!(
            old_reader == reader
                && old_parent == parent
                && old_camp == camp
                && before <= upper
                && !upper.0.is_empty()
                && !upper.1.is_empty()
                && !before.0.is_empty()
                && !before.1.is_empty(),
            "delegation.cursor_invalid"
        );
        (Some(upper), Some(before))
    } else {
        (None, None)
    };
    let limit = input.limit.unwrap_or(20) as usize;
    let mut rows = tx.prepare(
        "SELECT d.id,d.created_at FROM delegation d JOIN conversation c ON c.id=d.child_conversation_id
         WHERE d.parent_conversation_id=?1 AND d.camp_id=?2 AND c.camp_id=d.camp_id
           AND c.agent_id=d.owner_agent_id AND c.kind='delegation_child'
           AND (?3 IS NULL OR (d.created_at,d.id)<=(?3,?4))
           AND (?5 IS NULL OR (d.created_at,d.id)<(?5,?6))
         ORDER BY d.created_at DESC,d.id DESC LIMIT ?7",
    )?.query_map(params![parent,camp,upper.as_ref().map(|r|&r.0),upper.as_ref().map(|r|&r.1),
        before.as_ref().map(|r|&r.0),before.as_ref().map(|r|&r.1),(limit+1) as i64], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let upper = upper.or_else(|| rows.first().map(|(id, at)| (at.clone(), id.clone())));
    let more = rows.len() > limit;
    rows.truncate(limit);
    let next = more
        .then(|| rows.last().map(|(id, at)| (at.clone(), id.clone())))
        .flatten();
    let items = rows
        .iter()
        .map(|(id, _)| {
            let id = identity(&tx, id)?.context("delegation.not_found")?;
            detail(&tx, &id)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut page = json!({"parentConversationId":parent,"items":items});
    if let (Some(upper), Some(before)) = (upper, next) {
        page["nextCursor"] = json!(encode(&Cursor::UserList {
            v: 1,
            reader: reader.into(),
            parent: parent.into(),
            camp,
            upper,
            before
        })?);
    }
    tx.commit()?;
    Ok(page)
}
