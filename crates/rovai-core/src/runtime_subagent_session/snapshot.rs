//! Complete readonly projection in signed UTF-8 windows. No arbitrary native
//! id/path input and no Runtime control. The caller reauthorizes the Core
//! reference before each window; this object only checks that exact scope.
use std::{
    io::Write,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    ObservedNativeSessionTarget,
    content::{NativeContent, model},
    legacy::{NativeAgentReadPlan, ObservedNativeAgentTarget},
};
use crate::{command::ActorRef, current_user::CURRENT_USER_ID};

pub(crate) const MAX_WINDOW_BYTES: usize = 32 * 1024;
static KEY: OnceLock<[u8; 32]> = OnceLock::new();

fn key() -> Result<ring::hmac::Key> {
    let bytes = if let Some(bytes) = KEY.get() {
        bytes
    } else {
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes)
            .map_err(|_| anyhow::anyhow!("native_child.cursor_unavailable"))?;
        KEY.get_or_init(|| bytes)
    };
    Ok(ring::hmac::Key::new(ring::hmac::HMAC_SHA256, bytes))
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Token {
    Snapshot {
        v: u8,
        id: String,
        scope: String,
        digest: String,
    },
    Window {
        v: u8,
        id: String,
        scope: String,
        digest: String,
        offset: u64,
    },
    Descendant {
        v: u8,
        id: String,
        scope: String,
        index: usize,
    },
}

fn encode(token: &Token) -> Result<String> {
    let payload = serde_json::to_vec(token)?;
    let tag = ring::hmac::sign(&key()?, &payload);
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload),
        URL_SAFE_NO_PAD.encode(tag.as_ref())
    ))
}

fn decode(value: &str) -> Result<Token> {
    ensure!(value.len() <= 4096, "native_child.cursor_invalid");
    let (body, tag) = value
        .split_once('.')
        .context("native_child.cursor_invalid")?;
    let body = URL_SAFE_NO_PAD
        .decode(body)
        .context("native_child.cursor_invalid")?;
    let tag = URL_SAFE_NO_PAD
        .decode(tag)
        .context("native_child.cursor_invalid")?;
    ring::hmac::verify(&key()?, &body, &tag)
        .map_err(|_| anyhow::anyhow!("native_child.cursor_invalid"))?;
    serde_json::from_slice(&body).context("native_child.cursor_invalid")
}

fn agent_scope(actor: &ActorRef, target: &ObservedNativeAgentTarget) -> Result<String> {
    ensure!(
        matches!(actor,ActorRef::User {user_id} if user_id==CURRENT_USER_ID),
        "native_child.user_required"
    );
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            "legacy-agent-snapshot-v1",
            CURRENT_USER_ID,
            &target.reference.thread_id,
            &target.reference.agent_run_id,
            &target.reference.evidence_id,
            &target.conversation_id,
            target.execution_epoch,
            target.adapter_kind.as_str(),
            &target.origin,
            &target.parent_tool_use_id,
            &target.input_digest,
            &target.agent_id
        ))?)
    ))
}

pub(super) fn scope(actor: &ActorRef, target: &ObservedNativeSessionTarget) -> Result<String> {
    ensure!(
        matches!(actor, ActorRef::User { user_id } if user_id == CURRENT_USER_ID),
        "native_child.user_required"
    );
    // Status/live can change while a frozen document is read. Identity, origin
    // and binding cannot; native addresses stay inside this one-way digest.
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            CURRENT_USER_ID,
            &target.thread_id,
            &target.conversation_id,
            &target.agent_run_id,
            target.execution_epoch,
            &target.evidence_id,
            &target.adapter_kind,
            &target.installation_id,
            &target.binding_id,
            target.binding_generation,
            &target.parent_native_session_id,
            &target.child_native_session_id,
            &target.recorded_home,
            &target.recorded_working_directory,
        ))?)
    ))
}

#[derive(Serialize)]
struct ChildReference {
    source_id: String,
    reference: String,
}

#[derive(Serialize)]
struct Notes {
    malformed_records: usize,
    incomplete_tail: bool,
    unavailable_children: usize,
    unavailable_advisories: usize,
}

#[derive(Serialize)]
struct Projection<'a> {
    schema_version: u32,
    turns: &'a [model::MessageTurn],
    context_tokens: Option<u64>,
    session_stats: Option<&'a model::SessionStats>,
    summary: Option<&'a model::NativeTranscriptSummary>,
    notes: Notes,
    child_references: Vec<ChildReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_stats: Option<&'a model::AgentExecutionStats>,
}

enum SnapshotTarget {
    Session(ObservedNativeSessionTarget),
    Agent(ObservedNativeAgentTarget),
}

pub(crate) struct NativeSnapshot {
    id: String,
    scope: String,
    target: SnapshotTarget,
    projection: String,
    digest: String,
    source_digest: String,
    children: Vec<String>,
    agent_status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContentWindow {
    pub snapshot_reference: String,
    pub projection_digest: String,
    pub source_digest: String,
    pub offset: u64,
    pub total_bytes: u64,
    pub fragment: String,
    pub next_cursor: Option<String>,
    pub complete: bool,
    pub reported_status: String,
    pub live: bool,
}

impl NativeSnapshot {
    pub(crate) fn new(
        actor: &ActorRef,
        target: &ObservedNativeSessionTarget,
        content: NativeContent,
        stop: &AtomicBool,
    ) -> Result<Self> {
        Self::build(
            actor,
            SnapshotTarget::Session(target.clone()),
            content,
            None,
            stop,
        )
    }

    pub(crate) fn new_agent(
        actor: &ActorRef,
        target: ObservedNativeAgentTarget,
        content: NativeContent,
        stats: model::AgentExecutionStats,
        stop: &AtomicBool,
    ) -> Result<Self> {
        // Legacy agent ids must never enter the Session descendant-token path.
        ensure!(
            content.verified_children.is_empty(),
            "native_agent.invalid_descendant"
        );
        Self::build(
            actor,
            SnapshotTarget::Agent(target),
            content,
            Some(stats),
            stop,
        )
    }

    fn build(
        actor: &ActorRef,
        target: SnapshotTarget,
        content: NativeContent,
        agent_stats: Option<model::AgentExecutionStats>,
        stop: &AtomicBool,
    ) -> Result<Self> {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        let scope = match &target {
            SnapshotTarget::Session(t) => scope(actor, t)?,
            SnapshotTarget::Agent(t) => agent_scope(actor, t)?,
        };
        let id = uuid::Uuid::new_v4().to_string();
        let mut children = content.verified_children.clone();
        children.sort();
        children.dedup();
        ensure!(
            children
                .iter()
                .all(|id| super::identity(id) && matches!(&target,SnapshotTarget::Session(t) if id != &t.child_native_session_id)),
            "native_child.invalid_reference"
        );
        let child_references = children
            .iter()
            .enumerate()
            .map(|(index, source_id)| {
                ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
                Ok(ChildReference {
                    source_id: source_id.clone(),
                    reference: encode(&Token::Descendant {
                        v: 1,
                        id: id.clone(),
                        scope: scope.clone(),
                        index,
                    })?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut writer = ProjectionWriter {
            bytes: Vec::new(),
            digest: Sha256::new(),
            stop,
        };
        serde_json::to_writer(
            &mut writer,
            &Projection {
                schema_version: 1,
                turns: &content.turns,
                context_tokens: content.context_tokens,
                session_stats: content.session_stats.as_ref(),
                summary: content.summary.as_ref(),
                notes: Notes {
                    malformed_records: content.notes.malformed_records,
                    incomplete_tail: content.notes.incomplete_tail,
                    unavailable_children: content.notes.unavailable_children,
                    unavailable_advisories: content.notes.unavailable_advisories,
                },
                child_references,
                agent_stats: agent_stats.as_ref(),
            },
        )?;
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        let projection = String::from_utf8(writer.bytes)?;
        let digest = format!("{:x}", writer.digest.finalize());
        Ok(Self {
            id,
            scope,
            target,
            projection,
            digest,
            source_digest: content.snapshot_digest,
            children,
            agent_status: agent_stats
                .and_then(|s| s.status)
                .unwrap_or_else(|| "unknown".into()),
        })
    }

    fn authorize(&self, actor: &ActorRef, expected: &ObservedNativeSessionTarget) -> Result<()> {
        ensure!(
            matches!(&self.target, SnapshotTarget::Session(_)),
            "native_child.scope_changed"
        );
        ensure!(
            scope(actor, expected)? == self.scope,
            "native_child.scope_changed"
        );
        Ok(())
    }

    pub(crate) fn reference(&self) -> Result<String> {
        encode(&Token::Snapshot {
            v: 1,
            id: self.id.clone(),
            scope: self.scope.clone(),
            digest: self.digest.clone(),
        })
    }

    pub(crate) fn window(
        &self,
        actor: &ActorRef,
        expected: &ObservedNativeSessionTarget,
        reference: &str,
        cursor: Option<&str>,
        max_bytes: usize,
    ) -> Result<ContentWindow> {
        self.authorize(actor, expected)?;
        self.window_authorized(
            reference,
            cursor,
            max_bytes,
            expected.status.clone(),
            expected.live,
        )
    }

    pub(crate) fn authorize_agent(
        &self,
        actor: &ActorRef,
        plan: &NativeAgentReadPlan,
    ) -> Result<()> {
        let SnapshotTarget::Agent(target) = &self.target else {
            anyhow::bail!("native_child.scope_changed")
        };
        plan.authorize(actor, target)?;
        ensure!(
            agent_scope(actor, target)? == self.scope,
            "native_child.scope_changed"
        );
        Ok(())
    }

    pub(crate) fn agent_window(
        &self,
        actor: &ActorRef,
        plan: &NativeAgentReadPlan,
        reference: &str,
        cursor: Option<&str>,
        max_bytes: usize,
    ) -> Result<ContentWindow> {
        self.authorize_agent(actor, plan)?;
        let SnapshotTarget::Agent(target) = &self.target else {
            anyhow::bail!("native_child.scope_changed")
        };
        self.window_authorized(
            reference,
            cursor,
            max_bytes,
            self.agent_status.clone(),
            plan.live(target),
        )
    }

    fn window_authorized(
        &self,
        reference: &str,
        cursor: Option<&str>,
        max_bytes: usize,
        status: String,
        live: bool,
    ) -> Result<ContentWindow> {
        ensure!(
            (4..=MAX_WINDOW_BYTES).contains(&max_bytes),
            "native_child.window_limit"
        );
        let Token::Snapshot {
            v: 1,
            id,
            scope,
            digest,
        } = decode(reference)?
        else {
            anyhow::bail!("native_child.cursor_invalid")
        };
        ensure!(
            id == self.id && scope == self.scope && digest == self.digest,
            "native_child.cursor_invalid"
        );
        let offset = if let Some(cursor) = cursor {
            let Token::Window {
                v: 1,
                id,
                scope,
                digest,
                offset,
            } = decode(cursor)?
            else {
                anyhow::bail!("native_child.cursor_invalid")
            };
            ensure!(
                id == self.id && scope == self.scope && digest == self.digest,
                "native_child.cursor_invalid"
            );
            usize::try_from(offset).context("native_child.cursor_invalid")?
        } else {
            0
        };
        ensure!(
            offset <= self.projection.len() && self.projection.is_char_boundary(offset),
            "native_child.cursor_invalid"
        );
        let mut end = offset.saturating_add(max_bytes).min(self.projection.len());
        while !self.projection.is_char_boundary(end) {
            end -= 1;
        }
        let complete = end == self.projection.len();
        let next_cursor = if complete {
            None
        } else {
            Some(encode(&Token::Window {
                v: 1,
                id: self.id.clone(),
                scope: self.scope.clone(),
                digest: self.digest.clone(),
                offset: end as u64,
            })?)
        };
        Ok(ContentWindow {
            snapshot_reference: reference.into(),
            projection_digest: self.digest.clone(),
            source_digest: self.source_digest.clone(),
            offset: offset as u64,
            total_bytes: self.projection.len() as u64,
            fragment: self.projection[offset..end].to_owned(),
            next_cursor,
            complete,
            reported_status: status,
            live,
        })
    }

    pub(crate) fn descendant(
        &self,
        actor: &ActorRef,
        expected: &ObservedNativeSessionTarget,
        reference: &str,
    ) -> Result<ObservedNativeSessionTarget> {
        self.authorize(actor, expected)?;
        let Token::Descendant {
            v: 1,
            id,
            scope,
            index,
        } = decode(reference)?
        else {
            anyhow::bail!("native_child.cursor_invalid")
        };
        ensure!(
            id == self.id && scope == self.scope,
            "native_child.cursor_invalid"
        );
        let child = self
            .children
            .get(index)
            .context("native_child.cursor_invalid")?;
        let SnapshotTarget::Session(target) = &self.target else {
            anyhow::bail!("native_child.scope_changed")
        };
        let mut target = target.clone();
        target.parent_native_session_id = target.child_native_session_id;
        target.child_native_session_id = child.clone();
        target.status = "unknown".into();
        target.relationship = "source_verified_descendant".into();
        target.live = false;
        Ok(target)
    }
}

// Serialization runs on the same cancellable read worker. A large raw tool
// value must not postpone stop checks until after the full JSON is copied/hashed.
struct ProjectionWriter<'a> {
    bytes: Vec<u8>,
    digest: Sha256,
    stop: &'a AtomicBool,
}

impl Write for ProjectionWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        for chunk in bytes.chunks(MAX_WINDOW_BYTES) {
            if self.stop.load(Ordering::Acquire) {
                // write_all retries Interrupted forever. Cancellation must be
                // terminal, otherwise stopping during serialization would hang.
                return Err(std::io::Error::other("native_child.read_cancelled"));
            }
            self.bytes.extend_from_slice(chunk);
            self.digest.update(chunk);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(all(test, feature = "extended-tests"))]
#[path = "snapshot_tests.rs"]
pub(super) mod tests;
