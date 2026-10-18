//! User-only native-child read authority. A native id from the UI is never a grant.
//! The transcript reader consumes this non-deserializable target; it must not
//! create a Core Conversation, attach a native session, or start a model turn.
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    command::ActorRef, current_user::CURRENT_USER_ID, db::Database,
    execution_evidence::ExecutionEvidenceService, managed_blob::ManagedBlobStore,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeChildSessionReference {
    pub thread_id: String,
    pub agent_run_id: String,
    pub evidence_id: String,
    pub receiver_index: usize,
}

/// Core-correlated candidate, not a grant to read arbitrary native content.
/// A reported receiver is not necessarily a child: the transcript reader must
/// verify native parent/child lineage before returning any transcript content.
/// No Serialize/Deserialize: native addresses are not UI input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedNativeSessionTarget {
    pub(crate) thread_id: String,
    pub(crate) conversation_id: String,
    pub(crate) agent_run_id: String,
    pub(crate) execution_epoch: i64,
    pub(crate) evidence_id: String,
    pub(crate) adapter_kind: String,
    pub(crate) installation_id: String,
    pub(crate) binding_id: String,
    pub(crate) binding_generation: i64,
    pub(crate) parent_native_session_id: String,
    pub(crate) child_native_session_id: String,
    pub(crate) status: String,
    pub(crate) relationship: String,
    pub(crate) live: bool,
}

pub struct NativeChildSessionService;
impl NativeChildSessionService {
    pub fn resolve(
        database: &Database,
        blobs: &ManagedBlobStore,
        actor: &ActorRef,
        reference: &NativeChildSessionReference,
    ) -> Result<ObservedNativeSessionTarget> {
        ensure!(
            matches!(actor, ActorRef::User { user_id } if user_id == CURRENT_USER_ID),
            "native_child.user_required"
        );
        ensure!(
            reference.receiver_index < 64,
            "native_child.invalid_reference"
        );
        let row = database
            .connection()
            .query_row(
                "SELECT e.execution_epoch, r.execution_epoch, r.runtime_adapter_kind,
                    r.runtime_installation_id, r.status, r.cancel_requested_at,
                    c.id, c.native_session_id, c.native_binding_id,
                    c.native_binding_generation, c.native_adapter_installation_id,
                    r.runtime_binding_compatibility_digest, c.native_binding_compatibility_digest,
                    b.native_binding_id, b.native_binding_generation, b.conversation_id,
                    m.native_binding_generation
             FROM agent_run_execution_evidence e JOIN agent_run r ON r.id=e.agent_run_id
             LEFT JOIN camp_turn t ON t.id=r.camp_turn_id
             JOIN camp ON camp.id=COALESCE(r.camp_id,t.camp_id)
             JOIN conversation c ON c.id=r.conversation_id AND c.camp_id=camp.id
             JOIN context_manifest m ON m.agent_run_id=r.id
             JOIN native_session_bootstrap_evidence b ON b.id=m.bootstrap_evidence_id
             WHERE e.id=?1 AND r.id=?2 AND camp.id=?3 AND camp.deletion_operation_id IS NULL",
                params![
                    reference.evidence_id,
                    reference.agent_run_id,
                    reference.thread_id
                ],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<String>>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, i64>(9)?,
                        r.get::<_, Option<String>>(10)?,
                        r.get::<_, String>(11)?,
                        r.get::<_, Option<String>>(12)?,
                        r.get::<_, String>(13)?,
                        r.get::<_, i64>(14)?,
                        r.get::<_, String>(15)?,
                        r.get::<_, i64>(16)?,
                    ))
                },
            )
            .optional()?
            .context("native_child.reference_unavailable")?;
        ensure!(
            row.0 == row.1
                && row.0 >= 1
                && row.9 >= 1
                && row.8.as_deref() == Some(row.13.as_str())
                && row.9 == row.14
                && row.9 == row.16
                && row.6 == row.15
                && row.10.as_deref() == Some(row.3.as_str())
                && row.12.as_deref() == Some(row.11.as_str()),
            "native_child.binding_changed"
        );
        let parent = row
            .7
            .as_deref()
            .filter(|id| identity(id))
            .context("native_child.reference_unavailable")?;
        let payload = ExecutionEvidenceService.read_full_payload(
            database,
            blobs,
            &reference.thread_id,
            &reference.evidence_id,
        )?;
        let child = observed_child(&payload, &row.2, parent, reference.receiver_index)?;
        let live = matches!(row.4.as_str(), "queued" | "running" | "waiting")
            && row.5.is_none()
            && matches!(child.1.as_str(), "running" | "waiting");
        Ok(ObservedNativeSessionTarget {
            thread_id: reference.thread_id.clone(),
            conversation_id: row.6,
            agent_run_id: reference.agent_run_id.clone(),
            execution_epoch: row.0,
            evidence_id: reference.evidence_id.clone(),
            adapter_kind: row.2,
            installation_id: row.3,
            binding_id: row.13,
            binding_generation: row.14,
            parent_native_session_id: parent.into(),
            child_native_session_id: child.0,
            status: child.1,
            relationship: payload["nativeSubagents"]["relationship"]
                .as_str()
                .unwrap()
                .into(),
            live,
        })
    }
}

pub(crate) fn observed_child(
    payload: &Value,
    adapter: &str,
    parent: &str,
    index: usize,
) -> Result<(String, String)> {
    let observation = &payload["nativeSubagents"];
    ensure!(
        observation["schemaVersion"] == 1 && observation["adapterKind"] == adapter,
        "native_child.invalid_observation"
    );
    let source = observation["source"].as_str().unwrap_or_default();
    let relationship = observation["relationship"].as_str().unwrap_or_default();
    ensure!(
        matches!(
            (adapter, source, relationship),
            (
                "codex-cli",
                "codex-app-server.collabAgentToolCall",
                "runtime_reported_receiver"
            ) | (
                "codex-cli",
                "codex-app-server.subAgentActivity",
                "runtime_reported_child_activity"
            ) | (
                "grok-build",
                "grok-acp.subagent_notification",
                "runtime_reported_session_child"
            )
        ),
        "native_child.unsupported_source"
    );
    if adapter == "grok-build" {
        // Reapply the closed Grok projection, including the real parent and the
        // separation between subagent_id and an explicitly supplied Session id.
        ensure!(
            crate::runtime_subagent_observation::public_grok_observation(payload).is_some()
                && observation["parentNativeSessionId"] == parent,
            "native_child.parent_mismatch"
        );
    }
    let children = observation["children"]
        .as_array()
        .context("native_child.invalid_observation")?;
    ensure!(
        !children.is_empty() && children.len() <= 64 && index < children.len(),
        "native_child.invalid_reference"
    );
    let child = &children[index];
    let session = child["nativeSessionId"]
        .as_str()
        .filter(|id| identity(id) && *id != parent)
        .context("native_child.session_unavailable")?;
    let status = child["status"]
        .as_str()
        .context("native_child.invalid_observation")?;
    ensure!(
        matches!(
            status,
            "waiting"
                | "running"
                | "completed"
                | "failed"
                | "cancelled"
                | "interrupted"
                | "shutdown"
                | "not_found"
                | "unknown"
        ),
        "native_child.invalid_observation"
    );
    Ok((session.into(), status.into()))
}

fn identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
