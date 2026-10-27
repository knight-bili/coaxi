use super::*;
use crate::delegation_policy::Role;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ChildContext {
    pub context: Value,
    pub mode: &'static str,
    pub task_sha: String,
    pub task_blob_id: String,
    pub interruption: Option<String>,
    pub takeover_sequence: Option<i64>,
}

pub(super) fn validate_skill_exposure(
    snapshot: &RunSnapshot,
    exposure: &SkillExposureSnapshot,
) -> Result<()> {
    if frozen_delegation_policy(snapshot)?.is_some_and(|policy| policy.role == Role::Child) {
        anyhow::ensure!(
            exposure == &SkillExposureSnapshot::default(),
            "Child Run cannot inherit a legacy project Skill exposure"
        );
    }
    Ok(())
}

pub(super) fn load<R: ContextReadConnection>(
    database: &R,
    snapshot: &RunSnapshot,
) -> Result<Option<ChildContext>> {
    if !frozen_delegation_policy(snapshot)?.is_some_and(|policy| policy.role == Role::Child) {
        return Ok(None);
    }
    let (id, parent, depth, task_blob_id, task_sha, takeover, attempt, reason, current_attempt, run_attempt):
        (String, String, i64, String, String, Option<i64>, Option<String>, Option<String>, i64, Option<i64>) = database.context_connection().query_row(
        "SELECT d.id,d.parent_conversation_id,d.depth,d.task_blob_id,b.sha256,control.takeover_message_sequence,a.kind,a.interruption_reason,control.current_attempt_no,a.attempt_no
         FROM delegation d JOIN delegation_control control ON control.delegation_id=d.id
         JOIN managed_blob b ON b.id=d.task_blob_id JOIN conversation c ON c.id=d.child_conversation_id
         JOIN agent_run r ON r.conversation_id=c.id
         LEFT JOIN delegation_attempt a ON a.delegation_id=d.id AND a.agent_run_id=r.id
         WHERE r.id=?1 AND c.id=?2 AND c.camp_id=?3 AND c.agent_id=?4
           AND d.camp_id=c.camp_id AND d.owner_agent_id=c.agent_id AND c.kind='delegation_child'
           AND r.operation_policy='delegation_child_v1' AND r.operation_policy_version=1
           AND r.response_delivery='conversation_message' AND r.destination_conversation_id=c.id
           AND r.task_id IS NULL",
        params![snapshot.agent_run_id,snapshot.conversation_id,snapshot.camp_id,snapshot.agent_id],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?)),
    ).optional()?.context("Delegation Context has no owned child Run")?;
    anyhow::ensure!(
        (1..=8).contains(&depth),
        "Invalid persisted delegation depth"
    );
    let mode = match snapshot.invocation_kind.as_str() {
        "delegation" => {
            anyhow::ensure!(
                takeover.is_none() && run_attempt == Some(current_attempt),
                "Delegation attempt no longer has parent control"
            );
            match attempt.as_deref() {
                Some("initial") => "initial",
                Some("resume") => "resume",
                _ => anyhow::bail!("Missing delegation attempt kind"),
            }
        }
        "delegation_user" => {
            anyhow::ensure!(
                takeover.is_some() && attempt.is_none(),
                "User follow-up requires explicit takeover and its own Run"
            );
            "user_followup"
        }
        _ => anyhow::bail!("Invalid child invocation"),
    };
    Ok(Some(ChildContext {
        context: json!({"schemaVersion":1,"delegationId":id,"parentConversationId":parent,"depth":depth,"control":if mode=="user_followup" {"user"} else {"parent"},"mode":mode}),
        mode,
        task_sha,
        task_blob_id,
        interruption: reason,
        takeover_sequence: takeover,
    }))
}

pub(super) fn current_input<R: ContextReadConnection>(
    database: &R,
    snapshot: &RunSnapshot,
    child: &ChildContext,
) -> Result<CurrentInput> {
    anyhow::ensure!(
        snapshot.trigger_camp_message_id.is_none()
            && snapshot.trigger_message_delivery_id.is_none()
            && snapshot.a2a_parent_agent_run_id.is_none()
            && snapshot.a2a_root_agent_run_id.is_none()
            && snapshot.a2a_depth == 0,
        "Child input cannot inherit public or A2A lineage"
    );
    let message_id = snapshot
        .trigger_conversation_message_id
        .as_deref()
        .context("Missing child input message")?;
    let (body, author, author_id, source_run, sequence): (String,String,String,Option<String>,i64) = database.context_connection().query_row(
        "SELECT body,author_type,author_id,source_agent_run_id,sequence FROM conversation_message
         WHERE id=?1 AND conversation_id=?2 AND sequence<=?3",
        params![message_id,snapshot.conversation_id,snapshot.conversation_message_boundary_sequence],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)),
    ).optional()?.context("Child input message is outside its own Conversation boundary")?;
    anyhow::ensure!(
        !author_id.is_empty() && source_run.is_none(),
        "Invalid child input author or body"
    );
    let message = if child.mode == "user_followup" {
        anyhow::ensure!(
            author == "user"
                && sequence
                    > child
                        .takeover_sequence
                        .context("Missing takeover boundary")?,
            "Child User input predates takeover"
        );
        if body.is_empty() {
            let sources:String=database.context_connection().query_row(
                "SELECT source_attachments_json FROM conversation_message WHERE id=?1 AND conversation_id=?2",
                params![message_id,snapshot.conversation_id],|r|r.get(0))?;
            anyhow::ensure!(
                !crate::local_attachment_source::parse_source_attachments(&sources)?.is_empty(),
                "Empty child User input"
            );
        }
        body.clone()
    } else {
        anyhow::ensure!(!body.is_empty(), "Empty delegated task or resume message");
        anyhow::ensure!(author == "system", "Delegated input must be Core-owned");
        if child.mode == "initial" {
            anyhow::ensure!(
                format!("{:x}", Sha256::digest(body.as_bytes())) == child.task_sha,
                "Initial child input differs from the complete original task"
            );
        } else {
            anyhow::ensure!(
                body == include_str!("../../resources/delegation-resume.txt").trim(),
                "Resume must use the fixed continuation message, never resend the task"
            );
        }
        body.clone()
    };
    let mut payload = json!({"id":message_id,"message":message,"source":if child.mode=="user_followup" {"user"} else {"delegation_parent"}});
    if child.mode == "resume"
        && let Some(reason) = &child.interruption
    {
        anyhow::ensure!(
            !reason.is_empty() && reason.len() <= 2048,
            "Invalid resume interruption context"
        );
        payload["interruptionContext"] = json!(reason);
    }
    Ok(CurrentInput {
        quotes: Vec::new(),
        id: message_id.to_string(),
        payload,
        source_camp_message_id: None,
        source_conversation_message_id: Some(message_id.to_string()),
        source_content_digest: sha256_text(&body),
        mentions_current_user: false,
    })
}

pub(super) fn facts(snapshot: &RunSnapshot, child: &ChildContext) -> Result<RenderedRunFacts> {
    let root = snapshot.workspace["executionRoot"]
        .as_str()
        .filter(|value| !value.is_empty())
        .context("Missing child execution root")?;
    let access = snapshot.workspace["access"]
        .as_str()
        .context("Missing child workspace access")?;
    let isolation = snapshot.workspace["isolation"]
        .as_str()
        .context("Missing child workspace isolation")?;
    anyhow::ensure!(
        matches!(access, "read_only" | "write") && matches!(isolation, "shared" | "git_worktree"),
        "Invalid child workspace facts"
    );
    Ok(RenderedRunFacts {
        references: vec![RunFactRef {
            fact: "delegation_context",
            task_id: None,
            mission_id: None,
        }],
        payload_json: serde_json::to_string(
            &json!({"schemaVersion":10,"threadId":snapshot.camp_id,"ownerAgentId":snapshot.agent_id,"conversationId":snapshot.conversation_id,"delegationId":child.context["delegationId"],"executionRoot":root,"workspaceAccess":access,"workspaceIsolation":isolation}),
        )?,
    })
}

pub(super) fn evidence(child: &ChildContext) -> Value {
    json!({"context":child.context,"originalTaskBlobId":child.task_blob_id,"originalTaskSha256":child.task_sha})
}
