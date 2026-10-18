//! Process-local, readonly native transcript jobs. Core references are resolved
//! on the caller thread for every observation; no SQLite enters an FS worker.
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use super::{
    NativeChildSessionReference, ObservedNativeSessionTarget, content,
    snapshot::{self, ContentWindow, NativeSnapshot},
};
use crate::{command::ActorRef, current_user::CURRENT_USER_ID};

const MAX_READERS: usize = 16;
const MAX_WORKERS: usize = 4;
const MAX_CLOSED: usize = 1024;
const IDLE_LIMIT: Duration = Duration::from_secs(300);

#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DescendantRead {
    pub parent_read_id: String,
    pub descendant_reference: String,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StartRead {
    pub read_id: String,
    pub reference: NativeChildSessionReference,
    pub descendant: Option<DescendantRead>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReadId {
    pub read_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PageRead {
    pub read_id: String,
    pub snapshot_reference: String,
    pub cursor: Option<String>,
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadView {
    pub read_id: String,
    pub core_generation: String,
    pub state: &'static str,
    pub snapshot_reference: Option<String>,
    pub code: Option<&'static str>,
}

enum State {
    Reading,
    Ready(Arc<NativeSnapshot>),
    Failed(&'static str),
}
struct Data {
    state: State,
    last_used: Instant,
}
struct Entry {
    request: StartRead,
    root_scope: String,
    target: ObservedNativeSessionTarget,
    stop: Arc<AtomicBool>,
    data: Mutex<Data>,
}

impl Entry {
    fn data(&self) -> Result<MutexGuard<'_, Data>> {
        self.data
            .lock()
            .map_err(|_| anyhow::anyhow!("native_child.reader_poisoned"))
    }
    fn target(
        &self,
        actor: &ActorRef,
        root: &ObservedNativeSessionTarget,
    ) -> Result<ObservedNativeSessionTarget> {
        ensure!(
            snapshot::scope(actor, root)? == self.root_scope,
            "native_child.scope_changed"
        );
        if self.request.descendant.is_none() {
            Ok(root.clone())
        } else {
            Ok(self.target.clone())
        }
    }
}

pub(crate) struct Registry {
    entries: Mutex<HashMap<String, Arc<Entry>>>,
    closed: Mutex<HashMap<String, Instant>>,
    workers: Arc<Semaphore>,
    closing: AtomicBool,
    generation: String,
}

fn user(actor: &ActorRef) -> Result<()> {
    ensure!(
        matches!(actor, ActorRef::User { user_id } if user_id == CURRENT_USER_ID),
        "native_child.user_required"
    );
    Ok(())
}
fn read_id(id: &str) -> Result<()> {
    ensure!(
        id.len() == 36 && uuid::Uuid::parse_str(id).is_ok_and(|v| v.to_string() == id),
        "native_child.invalid_read_id"
    );
    Ok(())
}
fn failure(error: &anyhow::Error) -> &'static str {
    // Never send filesystem paths, raw parser errors, launch data or tool text.
    for code in [
        "native_child.read_cancelled",
        "native_child.source_changed",
        "native_child.source_unavailable",
        "native_child.origin_unavailable",
        "native_child.parent_mismatch",
        "native_child.ambiguous_source",
        "native_child.history_cycle",
        "native_child.history_unavailable",
    ] {
        if error.chain().any(|cause| cause.to_string() == code) {
            return code;
        }
    }
    "native_child.read_failed"
}

impl Registry {
    pub(crate) fn new(generation: String) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            closed: Mutex::new(HashMap::new()),
            workers: Arc::new(Semaphore::new(MAX_WORKERS)),
            closing: AtomicBool::new(false),
            generation,
        }
    }
    fn entries(&self) -> Result<MutexGuard<'_, HashMap<String, Arc<Entry>>>> {
        self.entries
            .lock()
            .map_err(|_| anyhow::anyhow!("native_child.reader_poisoned"))
    }
    fn reap(&self, entries: &mut HashMap<String, Arc<Entry>>) -> Result<()> {
        let mut closed = self
            .closed
            .lock()
            .map_err(|_| anyhow::anyhow!("native_child.reader_poisoned"))?;
        closed.retain(|_, at| at.elapsed() < IDLE_LIMIT);
        entries.retain(|id, entry| {
            let keep = entry
                .data
                .lock()
                .is_ok_and(|data| data.last_used.elapsed() < IDLE_LIMIT);
            if !keep {
                entry.stop.store(true, Ordering::Release);
                closed.insert(id.clone(), Instant::now());
            }
            keep
        });
        Ok(())
    }
    fn view(&self, id: &str, entry: &Entry) -> Result<ReadView> {
        let mut data = entry.data()?;
        data.last_used = Instant::now();
        let (state, snapshot_reference, code) = match &data.state {
            State::Reading => ("reading", None, None),
            State::Ready(value) => ("ready", Some(value.reference()?), None),
            State::Failed(code) => ("failed", None, Some(*code)),
        };
        Ok(ReadView {
            read_id: id.into(),
            core_generation: self.generation.clone(),
            state,
            snapshot_reference,
            code,
        })
    }
    /// Only returns the original Core reference. The caller must resolve it
    /// again before status/page; this is never a cached authorization decision.
    pub(crate) fn reference(
        &self,
        actor: &ActorRef,
        id: &str,
    ) -> Result<NativeChildSessionReference> {
        user(actor)?;
        read_id(id)?;
        let mut entries = self.entries()?;
        self.reap(&mut entries)?;
        Ok(entries
            .get(id)
            .context("native_child.read_missing")?
            .request
            .reference
            .clone())
    }
    pub(crate) fn start(
        &self,
        actor: &ActorRef,
        request: StartRead,
        root: &ObservedNativeSessionTarget,
        fallback: chrono::DateTime<chrono::Utc>,
    ) -> Result<ReadView> {
        let worker_actor = actor.clone();
        self.start_with(request, root, actor, move |target, stop| {
            let content = content::read(&target, fallback, &stop)?;
            NativeSnapshot::new(&worker_actor, &target, content, &stop)
        })
    }
    fn start_with<F>(
        &self,
        request: StartRead,
        root: &ObservedNativeSessionTarget,
        actor: &ActorRef,
        work: F,
    ) -> Result<ReadView>
    where
        F: FnOnce(ObservedNativeSessionTarget, Arc<AtomicBool>) -> Result<NativeSnapshot>
            + Send
            + 'static,
    {
        user(actor)?;
        read_id(&request.read_id)?;
        let root_scope = snapshot::scope(actor, root)?;
        let mut entries = self.entries()?;
        self.reap(&mut entries)?;
        ensure!(
            !self.closing.load(Ordering::Acquire),
            "native_child.reader_closing"
        );
        {
            let closed = self
                .closed
                .lock()
                .map_err(|_| anyhow::anyhow!("native_child.reader_poisoned"))?;
            ensure!(
                !closed.contains_key(&request.read_id),
                "native_child.read_closed"
            );
        }
        if let Some(entry) = entries.get(&request.read_id) {
            ensure!(
                entry.request == request && entry.root_scope == root_scope,
                "native_child.read_id_conflict"
            );
            return self.view(&request.read_id, entry);
        }
        ensure!(
            self.closed
                .lock()
                .map_err(|_| anyhow::anyhow!("native_child.reader_poisoned"))?
                .len()
                < MAX_CLOSED,
            "native_child.reader_busy"
        );
        ensure!(entries.len() < MAX_READERS, "native_child.reader_limit");
        let target = if let Some(descendant) = &request.descendant {
            read_id(&descendant.parent_read_id)?;
            let parent = entries
                .get(&descendant.parent_read_id)
                .context("native_child.read_missing")?;
            ensure!(
                parent.request.reference == request.reference,
                "native_child.scope_changed"
            );
            let parent_target = parent.target(actor, root)?;
            let data = parent.data()?;
            let State::Ready(value) = &data.state else {
                anyhow::bail!("native_child.read_not_ready")
            };
            value.descendant(actor, &parent_target, &descendant.descendant_reference)?
        } else {
            root.clone()
        };
        // The worker, not the UI entry, owns this permit. Closing/removing a job
        // cannot make a stuck FS call invisible to the global concurrency bound.
        let slot = self
            .workers
            .clone()
            .try_acquire_owned()
            .context("native_child.reader_busy")?;
        let id = request.read_id.clone();
        let entry = Arc::new(Entry {
            request,
            root_scope,
            target: target.clone(),
            stop: Arc::new(AtomicBool::new(false)),
            data: Mutex::new(Data {
                state: State::Reading,
                last_used: Instant::now(),
            }),
        });
        entries.insert(id.clone(), entry.clone());
        let worker = entry.clone();
        let spawned = std::thread::Builder::new()
            .name("native-child-reader".into())
            .spawn(move || {
                let _slot = slot;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    work(target, worker.stop.clone())
                }));
                if let Ok(mut data) = worker.data.lock() {
                    if !worker.stop.load(Ordering::Acquire) {
                        data.state = match result {
                            Ok(Ok(snapshot)) => State::Ready(Arc::new(snapshot)),
                            Ok(Err(error)) => State::Failed(failure(&error)),
                            Err(_) => State::Failed("native_child.read_failed"),
                        };
                    }
                }
            });
        if spawned.is_err() {
            entries.remove(&id);
            entry.stop.store(true, Ordering::Release);
            anyhow::bail!("native_child.reader_unavailable");
        }
        self.view(&id, &entry)
    }
    pub(crate) fn poll(
        &self,
        actor: &ActorRef,
        id: &str,
        root: &ObservedNativeSessionTarget,
    ) -> Result<ReadView> {
        user(actor)?;
        read_id(id)?;
        let mut entries = self.entries()?;
        self.reap(&mut entries)?;
        let entry = entries.get(id).context("native_child.read_missing")?;
        entry.target(actor, root)?;
        self.view(id, entry)
    }
    pub(crate) fn page(
        &self,
        actor: &ActorRef,
        request: PageRead,
        root: &ObservedNativeSessionTarget,
    ) -> Result<ContentWindow> {
        user(actor)?;
        read_id(&request.read_id)?;
        let mut entries = self.entries()?;
        self.reap(&mut entries)?;
        let entry = entries
            .get(&request.read_id)
            .context("native_child.read_missing")?;
        let target = entry.target(actor, root)?;
        let mut data = entry.data()?;
        data.last_used = Instant::now();
        let State::Ready(value) = &data.state else {
            anyhow::bail!("native_child.read_not_ready")
        };
        value.window(
            actor,
            &target,
            &request.snapshot_reference,
            request.cursor.as_deref(),
            request.max_bytes.unwrap_or(snapshot::MAX_WINDOW_BYTES),
        )
    }
    /// File actions share the active, lineage-verified read and its captured
    /// Host directory; today's Camp and transcript text cannot mint a root.
    pub(crate) fn file_target(
        &self,
        actor: &ActorRef,
        id: &str,
        root: &ObservedNativeSessionTarget,
    ) -> Result<ObservedNativeSessionTarget> {
        user(actor)?;
        read_id(id)?;
        let mut entries = self.entries()?;
        self.reap(&mut entries)?;
        let entry = entries.get(id).context("native_child.read_missing")?;
        let target = entry.target(actor, root)?;
        let mut data = entry.data()?;
        ensure!(
            matches!(&data.state, State::Ready(_)),
            "native_child.read_not_ready"
        );
        ensure!(
            target
                .recorded_working_directory
                .as_ref()
                .is_some_and(|path| path.is_absolute()),
            "native_child.source_unavailable"
        );
        data.last_used = Instant::now();
        Ok(target)
    }

    /// Cleanup is Current User-only and idempotent even after source deletion.
    /// It grants no content and never sends cancellation to the agent Runtime.
    pub(crate) fn close(&self, actor: &ActorRef, id: &str) -> Result<()> {
        user(actor)?;
        read_id(id)?;
        let mut entries = self.entries()?;
        self.reap(&mut entries)?;
        let removed = entries.remove(id);
        if let Some(entry) = &removed {
            entry.stop.store(true, Ordering::Release);
        }
        let mut closed = self
            .closed
            .lock()
            .map_err(|_| anyhow::anyhow!("native_child.reader_poisoned"))?;
        // At saturation all new starts fail closed. Existing jobs can still be
        // cancelled; their at-most MAX_READERS tombstones may exceed the soft cap.
        if removed.is_some() || closed.len() < MAX_CLOSED {
            closed.insert(id.into(), Instant::now());
        }
        Ok(())
    }
    pub(crate) fn begin_shutdown(&self) {
        self.closing.store(true, Ordering::Release);
        self.workers.close();
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        for (_, entry) in entries.drain() {
            entry.stop.store(true, Ordering::Release);
        }
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        self.begin_shutdown();
    }
}

#[cfg(all(test, feature = "extended-tests"))]
#[path = "reader_tests.rs"]
pub(super) mod tests;
