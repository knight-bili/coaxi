//! Independent authority for legacy agent files. An agent is not a Session.
//! Preparation is DB/Blob-only; filesystem work belongs on the existing native
//! reader worker, followed by Core revalidation before any publication.
use std::{
    fmt,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;

use super::{
    origin::{self, OriginRecord},
    source::{self, NativeDocuments},
};
use crate::{
    agent_profile::AdapterKind, command::ActorRef, current_user::CURRENT_USER_ID, db::Database,
    execution_evidence::ExecutionEvidenceService, managed_blob::ManagedBlobStore,
};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeAgentReference {
    pub thread_id: String,
    pub agent_run_id: String,
    pub evidence_id: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct NativeAgentReadPlan {
    reference: NativeAgentReference,
    conversation_id: String,
    epoch: i64,
    adapter: AdapterKind,
    tool_id: String,
    input_digest: String,
    origins: Vec<OriginRecord>,
    current_binding: Option<String>,
    current_generation: i64,
    current_parent: Option<String>,
    current_epoch: i64,
    current_compatible: bool,
    active: bool,
}

impl fmt::Debug for NativeAgentReadPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeAgentReadPlan")
            .field("agent_run_id", &self.reference.agent_run_id)
            .field("epoch", &self.epoch)
            .field("source_count", &self.origins.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ObservedNativeAgentTarget {
    pub(crate) reference: NativeAgentReference,
    pub(crate) conversation_id: String,
    pub(crate) execution_epoch: i64,
    pub(crate) adapter_kind: AdapterKind,
    pub(crate) origin: OriginRecord,
    pub(crate) parent_tool_use_id: String,
    pub(crate) input_digest: String,
    pub(crate) agent_id: String,
    pub(crate) live: bool,
}

impl fmt::Debug for ObservedNativeAgentTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservedNativeAgentTarget")
            .field("agent_run_id", &self.reference.agent_run_id)
            .field("epoch", &self.execution_epoch)
            .field("live", &self.live)
            .finish_non_exhaustive()
    }
}

pub struct NativeAgentService;
impl NativeAgentService {
    pub fn prepare(
        database: &Database,
        blobs: &ManagedBlobStore,
        actor: &ActorRef,
        reference: &NativeAgentReference,
    ) -> Result<NativeAgentReadPlan> {
        ensure!(
            matches!(actor,ActorRef::User {user_id} if user_id==CURRENT_USER_ID),
            "native_child.user_required"
        );
        let row=database.connection().query_row(
            "SELECT e.execution_epoch,r.execution_epoch,r.runtime_adapter_kind,r.runtime_installation_id,
                r.runtime_binding_compatibility_digest,r.status,r.cancel_requested_at,c.id,
                c.native_binding_id,c.native_binding_generation,c.native_session_id,
                c.native_adapter_installation_id,c.native_binding_compatibility_digest,
                b.conversation_id,b.native_binding_generation,m.native_binding_generation,e.event_type
             FROM agent_run_execution_evidence e JOIN agent_run r ON r.id=e.agent_run_id
             LEFT JOIN camp_turn t ON t.id=r.camp_turn_id JOIN camp ON camp.id=COALESCE(r.camp_id,t.camp_id)
             JOIN conversation c ON c.id=r.conversation_id AND c.camp_id=camp.id
             JOIN context_manifest m ON m.agent_run_id=r.id
             JOIN native_session_bootstrap_evidence b ON b.id=m.bootstrap_evidence_id
             WHERE e.id=?1 AND r.id=?2 AND camp.id=?3 AND camp.deletion_operation_id IS NULL",
            params![reference.evidence_id,reference.agent_run_id,reference.thread_id],|r|Ok((
                r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,
                r.get::<_,String>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,String>(7)?,r.get::<_,Option<String>>(8)?,
                r.get::<_,i64>(9)?,r.get::<_,Option<String>>(10)?,r.get::<_,Option<String>>(11)?,r.get::<_,Option<String>>(12)?,
                r.get::<_,String>(13)?,r.get::<_,i64>(14)?,r.get::<_,i64>(15)?,r.get::<_,String>(16)?)))
            .optional()?.context("native_child.reference_unavailable")?;
        let adapter = if row.2 == AdapterKind::ClaudeCodeCli.as_str() {
            AdapterKind::ClaudeCodeCli
        } else if row.2 == AdapterKind::QoderCli.as_str() {
            AdapterKind::QoderCli
        } else {
            anyhow::bail!("native_agent.unsupported_source")
        };
        ensure!(
            row.0 >= 1 && row.7 == row.13 && row.14 == row.15,
            "native_child.binding_changed"
        );
        ensure!(
            row.16 == "runtime.action",
            "native_agent.invalid_observation"
        );
        let payload = ExecutionEvidenceService.read_full_payload(
            database,
            blobs,
            &reference.thread_id,
            &reference.evidence_id,
        )?;
        ensure!(
            payload["sourceAuthority"] != "core" && payload["canonicalTool"].is_null(),
            "native_agent.invalid_observation"
        );
        let tool_id = payload["toolCallId"]
            .as_str()
            .filter(|id| super::identity(id))
            .context("native_agent.tool_unavailable")?
            .to_owned();
        let input_digest = payload["rawInputDigest"]
            .as_str()
            .filter(|d| {
                d.len() == 64
                    && d.bytes()
                        .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
            })
            .context("native_agent.input_unavailable")?
            .to_owned();
        let origins = origin::for_run(
            database,
            &reference.thread_id,
            &row.7,
            &reference.agent_run_id,
            row.0,
            &row.2,
        )?;
        ensure!(!origins.is_empty(), "native_child.origin_unavailable");
        for origin in &origins {
            ensure!(
                origin.installation_id == row.3
                    && origin.compatibility_digest == row.4
                    && origin.home_path.len() <= 32768
                    && !origin.home_path.contains('\0'),
                "native_child.binding_changed"
            );
        }
        Ok(NativeAgentReadPlan {
            reference: reference.clone(),
            conversation_id: row.7,
            epoch: row.0,
            adapter,
            tool_id,
            input_digest,
            origins,
            current_binding: row.8,
            current_generation: row.9,
            current_parent: row.10,
            current_epoch: row.1,
            current_compatible: row.11.as_deref() == Some(row.3.as_str())
                && row.12.as_deref() == Some(row.4.as_str()),
            active: matches!(row.5.as_str(), "queued" | "running" | "waiting") && row.6.is_none(),
        })
    }

    pub fn revalidate(
        database: &Database,
        blobs: &ManagedBlobStore,
        actor: &ActorRef,
        target: &ObservedNativeAgentTarget,
    ) -> Result<()> {
        let plan = Self::prepare(database, blobs, actor, &target.reference)?;
        ensure!(
            plan.conversation_id == target.conversation_id
                && plan.epoch == target.execution_epoch
                && plan.adapter == target.adapter_kind
                && plan.tool_id == target.parent_tool_use_id
                && plan.input_digest == target.input_digest
                && plan.origins.contains(&target.origin),
            "native_child.binding_changed"
        );
        Ok(())
    }
}

impl NativeAgentReadPlan {
    pub(crate) fn open(
        &self,
        stop: &AtomicBool,
    ) -> Result<(ObservedNativeAgentTarget, NativeDocuments)> {
        let mut selected = None;
        for origin in &self.origins {
            ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
            if let Some(found) = source::legacy::open_observed(
                self.adapter,
                &PathBuf::from(&origin.home_path),
                &origin.parent_native_session_id,
                &self.tool_id,
                &self.input_digest,
                stop,
            )? {
                ensure!(selected.is_none(), "native_child.ambiguous_origin");
                selected = Some((origin.clone(), found));
            }
        }
        let (origin, found) = selected.context("native_child.agent_link_unavailable")?;
        let live = self.active
            && self.current_compatible
            && self.current_epoch == self.epoch
            && self.current_binding.as_deref() == Some(origin.binding_id.as_str())
            && self.current_generation == origin.binding_generation
            && self.current_parent.as_deref() == Some(origin.parent_native_session_id.as_str());
        Ok((
            ObservedNativeAgentTarget {
                reference: self.reference.clone(),
                conversation_id: self.conversation_id.clone(),
                execution_epoch: self.epoch,
                adapter_kind: self.adapter,
                origin,
                parent_tool_use_id: self.tool_id.clone(),
                input_digest: self.input_digest.clone(),
                agent_id: found.agent_id,
                live,
            },
            found.documents,
        ))
    }
}
