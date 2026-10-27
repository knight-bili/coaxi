//! Read-only model and User views. A signed cursor fixes position/content, never authority.
use std::sync::{Arc, OnceLock};

use crate::{
    db::Database,
    delegation_admission::{OperationAdmission, authenticate_operation},
    delegation_contract::{HistoryParams, ReadParams, StatusParams},
    delegation_model_command::{FIXED_FINAL, Invocation},
    delegation_policy::Role,
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[path = "delegation_user_query.rs"]
pub(crate) mod user;

static CURSOR_KEY: OnceLock<[u8; 32]> = OnceLock::new();
fn key() -> Result<ring::hmac::Key> {
    let bytes = if let Some(bytes) = CURSOR_KEY.get() {
        bytes
    } else {
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes)
            .map_err(|_| anyhow::anyhow!("delegation.cursor_unavailable"))?;
        CURSOR_KEY.get_or_init(|| bytes)
    };
    Ok(ring::hmac::Key::new(ring::hmac::HMAC_SHA256, bytes))
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeKey {
    camp: String,
    reader: String,
    task: String,
    child: String,
    own: bool,
    #[serde(default, skip_serializing_if = "is_model")]
    user: bool,
}
fn is_model(user: &bool) -> bool {
    !user
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Cursor {
    Read {
        v: u8,
        scope: ScopeKey,
        run: String,
        message: String,
        version: String,
        offset: usize,
    },
    History {
        v: u8,
        scope: ScopeKey,
        through: i64,
        before: i64,
    },
    UserList {
        v: u8,
        reader: String,
        parent: String,
        camp: String,
        upper: (String, String),
        before: (String, String),
    },
    Status {
        v: u8,
        camp: String,
        reader: String,
        ids_digest: String,
        marks: Vec<Option<String>>,
    },
}
fn encode(cursor: &Cursor) -> Result<String> {
    let payload = serde_json::to_vec(cursor)?;
    let tag = ring::hmac::sign(&key()?, &payload);
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload),
        URL_SAFE_NO_PAD.encode(tag.as_ref())
    ))
}
fn decode(value: &str, max: usize) -> Result<Cursor> {
    ensure!(value.len() <= max, "delegation.cursor_invalid");
    let (body, tag) = value.split_once('.').context("delegation.cursor_invalid")?;
    let body = URL_SAFE_NO_PAD
        .decode(body)
        .context("delegation.cursor_invalid")?;
    let tag = URL_SAFE_NO_PAD
        .decode(tag)
        .context("delegation.cursor_invalid")?;
    ring::hmac::verify(&key()?, &body, &tag)
        .map_err(|_| anyhow::anyhow!("delegation.cursor_invalid"))?;
    serde_json::from_slice(&body).context("delegation.cursor_invalid")
}
struct ReadScope {
    key: ScopeKey,
    through: i64,
}
fn scope(
    connection: &Connection,
    admission: &OperationAdmission,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    task: &str,
) -> Result<Option<ReadScope>> {
    if admission.policy.role == Role::Child {
        let own:Option<(String,i64)>=connection.query_row(
            "SELECT d.child_conversation_id,m.sequence-1 FROM delegation d JOIN agent_run r ON r.conversation_id=d.child_conversation_id
             JOIN conversation_message m ON m.id=r.trigger_conversation_message_id AND m.conversation_id=r.conversation_id
             WHERE d.id=?1 AND d.child_conversation_id=?2 AND r.id=?3 AND r.execution_epoch=?4
               AND m.sequence=r.initial_conversation_context_through_sequence",
            params![task,admission.conversation,source.agent_run_id,source.execution_epoch],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((child, through)) = own {
            return Ok(Some(ReadScope {
                key: ScopeKey {
                    camp: source.camp_id.clone(),
                    reader: admission.conversation.clone(),
                    task: task.into(),
                    child,
                    own: true,
                    user: false,
                },
                through,
            }));
        }
    }
    let direct:Option<(String,i64)>=connection.query_row(
        "SELECT d.child_conversation_id,MIN(c.last_message_sequence,COALESCE(control.takeover_message_sequence,c.last_message_sequence))
         FROM delegation d JOIN conversation c ON c.id=d.child_conversation_id JOIN delegation_control control ON control.delegation_id=d.id
         WHERE d.id=?1 AND d.parent_conversation_id=?2 AND d.camp_id=?3",
        params![task,admission.conversation,source.camp_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    Ok(direct.map(|(child, through)| ReadScope {
        key: ScopeKey {
            camp: source.camp_id.clone(),
            reader: admission.conversation.clone(),
            task: task.into(),
            child,
            own: false,
            user: false,
        },
        through,
    }))
}

pub(crate) fn read(db: &Database, invocation: &Invocation<'_, ReadParams>) -> Result<Value> {
    let tx = db.connection().unchecked_transaction()?;
    let admission = authenticate_operation(
        &tx,
        invocation.binding,
        invocation.source,
        invocation.tool_call,
        "delegation.read",
    )?;
    let task = invocation.input.task_id.as_str();
    let Some(scope) = scope(&tx, &admission, invocation.source, task)? else {
        return Ok(json!({"task_id":task,"status":"unknown"}));
    };
    let page = read_scoped(&tx, invocation.input, scope)?;
    tx.commit()?;
    Ok(page)
}

fn read_scoped(tx: &Connection, input: &ReadParams, scope: ReadScope) -> Result<Value> {
    let task = input.task_id.as_str();
    let cursor = input
        .cursor
        .as_ref()
        .map(|c| decode(c.as_str(), 4096))
        .transpose()?;
    let (run, message, version, offset) = if let Some(cursor) = cursor {
        let Cursor::Read {
            v: 1,
            scope: before,
            run,
            message,
            version,
            offset,
        } = cursor
        else {
            anyhow::bail!("delegation.cursor_invalid");
        };
        ensure!(before == scope.key, "delegation.cursor_invalid");
        (run, Some(message), Some(version), offset)
    } else {
        let run: Option<String> = if scope.key.own {
            tx.query_row(&format!("SELECT r.id FROM agent_run r JOIN conversation_message m ON m.id=r.final_conversation_message_id
                WHERE m.conversation_id=?1 AND m.sequence<=?2 AND {FIXED_FINAL} ORDER BY m.sequence DESC LIMIT 1"),
                params![scope.key.child,scope.through],|r|r.get(0)).optional()?
        } else {
            tx.query_row("SELECT a.agent_run_id FROM delegation_control c JOIN delegation_attempt a ON a.delegation_id=c.delegation_id AND a.attempt_no=c.current_attempt_no
                WHERE c.delegation_id=?1",[task],|r|r.get(0)).optional()?
        };
        (run.context("delegation.result_unavailable")?, None, None, 0)
    };
    if !scope.key.own {
        ensure!(tx.query_row("SELECT EXISTS(SELECT 1 FROM delegation_attempt WHERE delegation_id=?1 AND agent_run_id=?2)",params![task,run],|r|r.get::<_,bool>(0))?,"delegation.cursor_invalid");
    }
    let result =
        crate::delegation_model_command::fixed_final(tx, &run, &scope.key.child, scope.through)?
            .context("delegation.result_unavailable")?;
    ensure!(
        message.as_ref().is_none_or(|id| id == &result.message)
            && version.as_ref().is_none_or(|v| v == &result.version),
        "delegation.result_changed"
    );
    ensure!(
        offset <= result.body.len() && result.body.is_char_boundary(offset),
        "delegation.cursor_invalid"
    );
    let mut end = result.body.len().min(offset + input.page_bytes() as usize);
    while !result.body.is_char_boundary(end) {
        end -= 1;
    }
    ensure!(
        end > offset || end == result.body.len(),
        "delegation.page_limit_too_small"
    );
    let mut page = json!({"task_id":task,"result_ref":{"conversation_id":scope.key.child,"message_id":result.message},"text":&result.body[offset..end]});
    if end < result.body.len() {
        page["next_cursor"] = json!(encode(&Cursor::Read {
            v: 1,
            scope: scope.key,
            run,
            message: result.message,
            version: result.version,
            offset: end
        })?);
    }
    Ok(page)
}

pub(crate) fn history(db: &Database, invocation: &Invocation<'_, HistoryParams>) -> Result<Value> {
    let tx = db.connection().unchecked_transaction()?;
    let admission = authenticate_operation(
        &tx,
        invocation.binding,
        invocation.source,
        invocation.tool_call,
        "delegation.history",
    )?;
    let task = invocation.input.task_id.as_str();
    let Some(scope) = scope(&tx, &admission, invocation.source, task)? else {
        return Ok(json!({"task_id":task,"status":"unknown"}));
    };
    let page = history_scoped(&tx, invocation.input, scope)?;
    tx.commit()?;
    Ok(page)
}

fn history_scoped(tx: &Connection, input: &HistoryParams, scope: ReadScope) -> Result<Value> {
    let task = input.task_id.as_str();
    let (through, before) = if let Some(cursor) = input.before_cursor.as_ref() {
        let Cursor::History {
            v: 1,
            scope: previous,
            through,
            before,
        } = decode(cursor.as_str(), 4096)?
        else {
            anyhow::bail!("delegation.cursor_invalid");
        };
        ensure!(
            previous == scope.key
                && through >= 0
                && through <= scope.through
                && before > 0
                && before <= through + 1,
            "delegation.cursor_invalid"
        );
        (through, before)
    } else {
        (scope.through, scope.through + 1)
    };
    let mut rows=tx.prepare(&format!("SELECT m.id,m.sequence,m.author_type,m.body,m.created_at,m.source_attachments_json
        FROM conversation_message m WHERE m.conversation_id=?1 AND m.sequence<=?2 AND m.sequence<?3
          AND ((m.author_type='user' AND m.source_agent_run_id IS NULL)
            OR (m.author_type='system' AND m.author_id='delegation-parent' AND m.source_agent_run_id IS NULL
              AND EXISTS(SELECT 1 FROM delegation_attempt a JOIN agent_run input ON input.id=a.agent_run_id
                WHERE a.delegation_id=?5 AND input.conversation_id=m.conversation_id AND input.trigger_conversation_message_id=m.id
                  AND (m.agent_run_id IS NULL OR m.agent_run_id=input.id)
                  AND input.invocation_kind='delegation' AND input.operation_policy='delegation_child_v1' AND input.operation_policy_version=1))
            OR EXISTS(SELECT 1 FROM agent_run r WHERE {FIXED_FINAL}))
        ORDER BY m.sequence DESC,m.id DESC LIMIT ?4"))?.query_map(params![scope.key.child,through,before,i64::from(input.page_size())+1,task],|r|
            Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more = rows.len() > input.page_size() as usize;
    rows.truncate(input.page_size() as usize);
    rows.reverse();
    let next = has_more.then(|| rows.first().map(|r| r.1)).flatten();
    let messages=rows.into_iter().map(|(id,sequence,author,body,created,attachments)|->Result<Value> {
        let mut message=json!({"id":id,"sequence":sequence,"author_type":match author.as_str(){"agent"=>"assistant","system"=>"delegation_parent",other=>other},"body":body,"created_at":created});
        let refs=crate::local_attachment_source::parse_source_attachments(&attachments)?;
        if !refs.is_empty() {
            message["attachments"]=json!(refs.into_iter().map(|source| {
                let mut view=json!({"id":source.id,"name":source.display_name,"kind":source.kind.as_str()});
                if let Some(size)=source.observed_byte_size {view["size_bytes"]=json!(size);} view
            }).collect::<Vec<_>>());
        }
        Ok(message)
    }).collect::<Result<Vec<_>>>()?;
    let mut page = json!({"task_id":task,"conversation_id":scope.key.child,"messages":messages});
    if let Some(before) = next {
        page["next_cursor"] = json!(encode(&Cursor::History {
            v: 1,
            scope: scope.key,
            through,
            before
        })?);
    }
    Ok(page)
}

pub(crate) struct StatusSnapshot {
    payload: Value,
    newly_observable: bool,
}
pub(crate) fn status_snapshot(
    db: &Database,
    invocation: &Invocation<'_, StatusParams>,
) -> Result<StatusSnapshot> {
    let tx = db.connection().unchecked_transaction()?;
    let admission = authenticate_operation(
        &tx,
        invocation.binding,
        invocation.source,
        invocation.tool_call,
        "delegation.status",
    )?;
    let ids = invocation
        .input
        .task_ids
        .iter()
        .map(|id| id.as_str())
        .collect::<Vec<_>>();
    let ids_digest = crate::command::canonical_json_digest(&json!(ids))?;
    let previous = if let Some(cursor) = invocation.input.after_cursor.as_ref() {
        // The approved task list has no arbitrary count limit. Its compact
        // observation cursor is bounded by that actual list, not by a new cap.
        let max = ids
            .len()
            .checked_mul(100)
            .and_then(|size| size.checked_add(8192))
            .context("delegation.cursor_invalid")?;
        let Cursor::Status {
            v: 1,
            camp,
            reader,
            ids_digest: before,
            marks,
        } = decode(cursor.as_str(), max)?
        else {
            anyhow::bail!("delegation.cursor_invalid");
        };
        ensure!(
            camp == invocation.source.camp_id
                && reader == admission.conversation
                && before == ids_digest
                && marks.len() == ids.len(),
            "delegation.cursor_invalid"
        );
        Some(marks)
    } else {
        None
    };
    let tasks = ids
        .into_iter()
        .map(|id| crate::delegation_model_command::task_report(&tx, id, &admission.conversation))
        .collect::<Result<Vec<_>>>()?;
    let marks = tasks
        .iter()
        .map(|task| -> Result<Option<String>> {
            if task["status"] == "running" && task.get("blocked_on").is_none() {
                return Ok(None);
            }
            let attempt:Option<i64>=tx.query_row("SELECT c.current_attempt_no FROM delegation d JOIN delegation_control c ON c.delegation_id=d.id
                WHERE d.id=?1 AND d.parent_conversation_id=?2",params![task["task_id"].as_str(),admission.conversation],|r|r.get(0)).optional()?;
            let blocked=task.get("blocked_on").map(|request|json!({"kind":request["kind"],"request_id":request["request_id"]}));
            Ok(Some(crate::command::canonical_json_digest(
                &json!({"status":task["status"],
            "attempt":attempt,"blocked":blocked,"cleanup":task.get("cleanup_pending")}),
            )?))
        })
        .collect::<Result<Vec<_>>>()?;
    let newly_observable = marks.iter().enumerate().any(|(index, mark)| {
        mark.is_some() && previous.as_ref().is_none_or(|old| old[index] != *mark)
    });
    let cursor = encode(&Cursor::Status {
        v: 1,
        camp: invocation.source.camp_id.clone(),
        reader: admission.conversation,
        ids_digest,
        marks,
    })?;
    tx.commit()?;
    Ok(StatusSnapshot {
        payload: json!({"tasks":tasks,"cursor":cursor}),
        newly_observable,
    })
}

pub(crate) async fn status<D: std::future::Future<Output = ()>>(
    database: &tokio::sync::Mutex<Database>,
    invocation: &Invocation<'_, StatusParams>,
    disconnected: D,
) -> Result<Value> {
    let notify = database.lock().await.execution_wake.delegation.clone();
    wait_for_status(
        notify,
        invocation.input,
        || async { status_snapshot(&*database.lock().await, invocation) },
        disconnected,
    )
    .await
}

pub(crate) async fn wait_for_status<F, Fut, D>(
    notify: Arc<tokio::sync::Notify>,
    input: &StatusParams,
    mut snapshot: F,
    disconnected: D,
) -> Result<Value>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<StatusSnapshot>>,
    D: std::future::Future<Output = ()>,
{
    let deadline = input
        .bounded_wait_ms()
        .filter(|wait| *wait > 0)
        .map(|wait| tokio::time::Instant::now() + std::time::Duration::from_millis(wait));
    tokio::pin!(disconnected);
    loop {
        // Register every waiter before taking its durable snapshot. No driver
        // notification is consumed and no DB lock survives this await boundary.
        let changed = notify.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        let current = snapshot().await?;
        if input.wait_ms.is_none() || current.newly_observable {
            return Ok(current.payload);
        }
        tokio::select! {
            biased;
            _=&mut disconnected=>anyhow::bail!("delegation.status_disconnected"),
            _=async {if let Some(deadline)=deadline {tokio::time::sleep_until(deadline).await;} else {std::future::pending::<()>().await;}}=>return Ok(snapshot().await?.payload),
            _=&mut changed=>{},
        }
    }
}
