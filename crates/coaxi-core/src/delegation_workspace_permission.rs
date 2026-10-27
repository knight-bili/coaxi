//! Core-owned pre-execution directory grants. No invented native Runtime request,
//! second approval state machine, filesystem I/O or process launch lives here.
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    action::{ResolveActionApprovalCommand, RuntimePermissionOption},
    command::{
        ActorRef, CommandEnvelope, CommandHandlerResult, EntityReference, canonical_json_digest,
    },
};

const POLICY: &str = "delegation-workspace-v1";

pub(crate) fn exists(connection: &Connection, delegation: &str) -> Result<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM delegation_workspace_permission WHERE delegation_id=?1)",
        [delegation],
        |r| r.get(0),
    )?)
}

fn input(connection: &Connection, delegation: &str) -> Result<Value> {
    let (scope, workspace): (String, String) = connection.query_row(
        "SELECT json_object('delegationId',d.id,'childConversationId',d.child_conversation_id,
          'parentRunId',d.origin_parent_run_id,'parentEpoch',d.origin_parent_epoch,
          'bindingId',d.origin_binding_id,'bindingGeneration',d.origin_binding_generation,
          'toolCallId',d.origin_tool_call_id,'taskSha256',b.sha256,'runtime',d.requested_runtime,
          'requestedDirectory',d.requested_working_dir,'runtimePreset',json(d.runtime_preset_json),
          'initialRunId',r.id,'configDigest',COALESCE((SELECT initial_config_digest
             FROM delegation_workspace_permission WHERE delegation_id=d.id),
             json_extract(r.effective_config_json,'$.configDigest'))),
          d.resolved_workspace_json
         FROM delegation d JOIN managed_blob b ON b.id=d.task_blob_id
         JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=1
         JOIN agent_run r ON r.id=a.agent_run_id WHERE d.id=?1",
        [delegation],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let scope: Value = serde_json::from_str(&scope)?;
    let workspace: Value = serde_json::from_str(&workspace)?;
    Ok(
        json!({"kind":"runtime_permission_grant","cwd":workspace["executionRoot"],
        "permissions":{"workspace":workspace},"request_digest":canonical_json_digest(&scope)?}),
    )
}

fn options() -> Result<Vec<RuntimePermissionOption>> {
    Ok(vec![
        RuntimePermissionOption::from_native(
            "core.deny",
            "deny",
            "拒绝",
            "不启动本次独立帮手。",
            Value::Null,
            false,
        )?,
        RuntimePermissionOption::from_native(
            "core.allow_once",
            "allow_once",
            "允许本次目录",
            "仅允许本次帮手使用显示的目录和访问范围，不提升目标 Runtime 权限。",
            Value::Null,
            true,
        )?,
    ])
}

pub(crate) fn prepare(tx: &Transaction<'_>, delegation: &str, at: &str) -> Result<()> {
    let canonical = input(tx, delegation)?;
    let digest = canonical_json_digest(&canonical)?;
    let action = Uuid::new_v4().to_string();
    let approval = Uuid::new_v4().to_string();
    let (run, camp, config_digest): (String, String, String) = tx.query_row(
        "SELECT r.id,d.camp_id,json_extract(r.effective_config_json,'$.configDigest')
         FROM delegation d JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=1
         JOIN agent_run r ON r.id=a.agent_run_id WHERE d.id=?1",
        [delegation],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let summary = format!(
        "独立帮手申请使用目录 {}（{}）",
        canonical["cwd"].as_str().context("Missing directory")?,
        canonical["permissions"]["workspace"]["access"]
            .as_str()
            .context("Missing access")?
    );
    tx.execute(
        "INSERT INTO action_execution(id,agent_run_id,action_kind,action_schema_version,
          action_digest,digest_algorithm,canonicalization_version,canonical_input_json,input_completeness,
          action_summary,execution_authority,control_mode,source_agent_run_execution_epoch,
          policy_decision,policy_version,matched_policy_rule_ids_json,status,version,created_at,updated_at)
         VALUES(?1,?2,'runtime_permission_grant','1',?3,'sha256','canonical-json-v1',?4,'complete',
          ?5,'core','mediated',0,'ask',?6,'[]','prepared',1,?7,?7)",
        params![action,run,digest,serde_json::to_string(&canonical)?,summary,POLICY,at],
    )?;
    tx.execute(
        "INSERT INTO approval(id,approval_type,reason,request_json,action_id,action_kind,action_digest,
          digest_algorithm,canonicalization_version,action_summary,requested_for_user_id,request_policy_version,
          status,version,requested_at,updated_at,native_options_json)
         VALUES(?1,'action',?2,?3,?4,'runtime_permission_grant',?5,'sha256','canonical-json-v1',?2,
          ?6,?7,'pending',1,?8,?8,?9)",
        params![approval,summary,serde_json::to_string(&canonical)?,action,digest,
            crate::current_user::CURRENT_USER_ID,POLICY,at,serde_json::to_string(&options()?)?],
    )?;
    tx.execute(
        "INSERT INTO delegation_workspace_permission(delegation_id,action_id,initial_config_digest,created_at) VALUES(?1,?2,?3,?4)",
        params![delegation,action,config_digest,at],
    )?;
    crate::collaboration::append_domain_event(
        tx,
        "approval.requested",
        Some(&camp),
        Some(("approval", &approval)),
        &ActorRef::System {
            component_id: "delegation-admission".into(),
        },
        None,
        &json!({"approvalId":approval,"actionId":action,"delegationId":delegation,
            "agentRunId":run,"actionKind":"runtime_permission_grant"}),
    )?;
    Ok(())
}

/// Called by the original User approval command before its native Runtime branch.
/// Only the reserved Core policy enters this path; legacy approvals are untouched.
pub(crate) fn resolve(
    tx: &Transaction<'_>,
    envelope: &CommandEnvelope<ResolveActionApprovalCommand>,
) -> Result<Option<CommandHandlerResult>> {
    let policy: Option<String> = tx.query_row(
        "SELECT x.policy_version FROM approval p JOIN action_execution x ON x.id=p.action_id WHERE p.id=?1",
        [&envelope.payload.approval_id], |r| r.get(0),
    ).optional()?;
    if policy.as_deref() != Some(POLICY) {
        return Ok(None);
    }
    let reject = |code: &str| {
        Some(CommandHandlerResult::rejected(
            code,
            json!({"message":code}),
        ))
    };
    let ActorRef::User { user_id } = &envelope.actor else {
        return Ok(reject("approval.user_required"));
    };
    let (
        delegation,
        action,
        run,
        camp,
        target,
        status,
        version,
        action_status,
        epoch,
        run_epoch,
        run_status,
        cancelled,
        stored,
        digest,
        stored_options,
    ): (
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        String,
        i64,
        i64,
        String,
        Option<String>,
        String,
        String,
        String,
    ) = tx
        .query_row(
            "SELECT d.id,x.id,r.id,d.camp_id,p.requested_for_user_id,p.status,p.version,x.status,
           x.source_agent_run_execution_epoch,r.execution_epoch,r.status,r.cancel_requested_at,
           x.canonical_input_json,x.action_digest,p.native_options_json
         FROM approval p JOIN action_execution x ON x.id=p.action_id
         JOIN delegation_workspace_permission permission ON permission.action_id=x.id
         JOIN delegation d ON d.id=permission.delegation_id
         JOIN agent_run r ON r.id=x.agent_run_id WHERE p.id=?1",
            [&envelope.payload.approval_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                    r.get(13)?,
                    r.get(14)?,
                ))
            },
        )
        .optional()?
        .context("delegation.workspace_permission_scope_missing")?;
    if envelope.camp_id.as_deref() != Some(&camp) {
        return Ok(reject("approval.camp_mismatch"));
    }
    if user_id != &target || user_id != crate::current_user::CURRENT_USER_ID {
        return Ok(reject("approval.wrong_user"));
    }
    if status != "pending" {
        return Ok(reject("approval.not_pending"));
    }
    if version != envelope.payload.expected_version {
        return Ok(reject("approval.version_conflict"));
    }
    if action_status != "prepared"
        || epoch != 0
        || run_epoch != epoch
        || run_status != "queued"
        || cancelled.is_some()
    {
        return Ok(reject("approval.runtime_request_fenced"));
    }
    let canonical = input(tx, &delegation)?;
    if serde_json::from_str::<Value>(&stored)? != canonical
        || digest != canonical_json_digest(&canonical)?
        || serde_json::from_str::<Value>(&stored_options)? != serde_json::to_value(options()?)?
    {
        return Ok(reject("approval.workspace_request_changed"));
    }
    let valid: bool=tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM delegation d JOIN conversation c ON c.id=d.child_conversation_id
         JOIN conversation parent ON parent.id=d.parent_conversation_id
         JOIN agent_run source ON source.id=d.origin_parent_run_id JOIN camp ON camp.id=d.camp_id
         JOIN delegation_control control ON control.delegation_id=d.id
         JOIN camp_member member ON member.camp_id=d.camp_id AND member.agent_id=d.owner_agent_id
         JOIN agent_profile profile ON profile.id=d.owner_agent_id
         JOIN agent_run child ON child.conversation_id=c.id AND child.id=?2
         WHERE d.id=?1 AND c.ended_at IS NULL AND parent.ended_at IS NULL
           AND camp.activation_state='active' AND camp.deletion_operation_id IS NULL
           AND member.status='active' AND member.leave_requested_at IS NULL AND profile.profile_status='present'
           AND json_extract(child.effective_config_json,'$.campMemberVersion')=member.version
           AND parent.native_binding_id=d.origin_binding_id AND parent.native_binding_generation=d.origin_binding_generation
           AND source.execution_epoch=d.origin_parent_epoch AND source.cancel_requested_at IS NULL
           AND source.status IN ('running','waiting','succeeded') AND control.takeover_at IS NULL
           AND control.current_attempt_no=1)",params![delegation,run], |r|r.get(0),
    )?;
    if !valid {
        return Ok(reject("approval.runtime_request_fenced"));
    }
    let allowed = match envelope.payload.option_id.as_str() {
        "core.allow_once" => true,
        "core.deny" => false,
        _ => return Ok(reject("approval.option_not_found")),
    };
    let at = chrono::Utc::now().to_rfc3339();
    let decision = if allowed { "approved" } else { "denied" };
    tx.execute(
        "UPDATE approval SET status=?2,decision_json=?3,resolved_by_type='user',resolved_by_id=?4,
        resolution_code=?5,resolution_reason=?6,version=version+1,resolved_at=?7,updated_at=?7
        WHERE id=?1 AND status='pending' AND version=?8",
        params![
            envelope.payload.approval_id,
            decision,
            serde_json::to_string(
                &json!({"decision":decision,"optionId":envelope.payload.option_id})
            )?,
            user_id,
            if allowed {
                "user_selected_allow"
            } else {
                "user_selected_deny"
            },
            envelope.payload.reason,
            at,
            version
        ],
    )?;
    if allowed {
        // This action is solely the directory grant, not an executor operation.
        // Completing it does not execute a tool, start a process or relax native permissions.
        tx.execute("UPDATE action_execution SET status='succeeded',effect_disposition='complete',
            result_code='delegation.workspace_authorized',result_summary='User authorized this child workspace',
            resolution_source='user',ended_at=?2,version=version+1,updated_at=?2 WHERE id=?1 AND status='prepared'",
            params![action,at])?;
    } else {
        let settled = crate::runtime::settle_abortive_agent_run_in_tx(
            tx,
            &run,
            "workspace_permission_denied",
            &envelope.actor,
            &at,
        )?;
        crate::runtime::recompute_camp_turn(
            tx,
            &camp,
            &settled.camp_turn_id,
            &envelope.actor,
            None,
            &at,
        )?;
    }
    crate::collaboration::append_domain_event(
        tx,
        "approval.resolved",
        Some(&camp),
        Some(("approval", &envelope.payload.approval_id)),
        &envelope.actor,
        None,
        &json!({"actionId":action,"decision":decision,"optionId":envelope.payload.option_id,
            "actionStatus":if allowed {"succeeded"} else {"not_executed"}}),
    )?;
    Ok(Some(CommandHandlerResult::applied(
        "approval.resolved",
        json!({"approvalId":envelope.payload.approval_id,"actionId":action,"decision":decision,"optionId":envelope.payload.option_id}),
        Some(EntityReference {
            entity_type: "approval".into(),
            entity_id: envelope.payload.approval_id.clone(),
        }),
    )))
}

/// Absence means the trusted factory selected the original scope. External
/// admission always stores its requirement atomically before the child is visible.
pub(crate) fn covers_run(connection: &Connection, run: &str) -> Result<bool> {
    let row:Option<(String,String,String,String,String,Option<String>,Option<String>)>=connection.query_row(
        "SELECT d.id,x.canonical_input_json,x.action_digest,x.status,p.status,p.resolved_by_type,p.resolved_by_id
         FROM agent_run r JOIN delegation d ON d.child_conversation_id=r.conversation_id
         JOIN delegation_workspace_permission permission ON permission.delegation_id=d.id
         LEFT JOIN action_execution x ON x.id=permission.action_id LEFT JOIN approval p ON p.action_id=x.id WHERE r.id=?1",
        [run], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)),
    ).optional()?;
    let Some((delegation, stored, digest, action, status, resolver, user)) = row else {
        return Ok(true);
    };
    let expected = input(connection, &delegation)?;
    Ok(action == "succeeded"
        && status == "approved"
        && resolver.as_deref() == Some("user")
        && user.as_deref() == Some(crate::current_user::CURRENT_USER_ID)
        && serde_json::from_str::<Value>(&stored)? == expected
        && digest == canonical_json_digest(&expected)?)
}
