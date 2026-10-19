//! Internal source files, never a public transcript API. The caller must supply
//! the home captured for the original binding, not today's settings or UI input.
//! Native content still requires the Core turn/content projection before UI use.
//! Lineage/rollout rules adapted from pinned Codeg aff1e6c, Apache-2.0;
//! see integration/inventory/native-subagent-viewer-reference.md and licenses.
use std::{
    collections::HashSet,
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{ObservedNativeSessionTarget, identity};

pub(super) mod claude;
pub(super) mod legacy;
pub(super) mod qoder;

pub(crate) const CHUNK_BYTES: usize = 64 * 1024;
const HEADER_BYTES: u64 = 1024 * 1024;
const DISCOVERY_ENTRIES: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocumentKind {
    CodexRollout,
    GrokUpdates,
    GrokChatHistory,
    GrokSummary,
    GrokSubagentMetadata,
    GrokModelsCache,
    GrokConfig,
    ProjectPatch,
    ClaudeParent,
    ClaudeAgent,
    QoderParent,
    QoderAgent,
    QoderAgentMetadata,
    QoderTask,
}

/// A Codex link names an exact rollout, not whichever newer file has that thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoryLink {
    pub(crate) document_index: usize,
    pub(crate) end_ordinal_exclusive: u64,
}

/// Raw native documents stay private to Core; they may contain startup metadata.
/// No Serialize/Deserialize: a future API must return the content projection.
pub(crate) struct NativeDocuments {
    pub(crate) documents: Vec<NativeDocument>,
    pub(crate) unavailable_advisories: usize,
}

pub(crate) struct NativeDocument {
    pub(crate) kind: DocumentKind,
    pub(crate) history_base: Option<HistoryLink>,
    pub(crate) seed_start_ordinal: Option<u64>,
    pub(crate) own_header: Option<Value>,
    pub(crate) snapshot_bytes: u64,
    pub(crate) snapshot_digest: String,
    pub(crate) source_label: Option<String>,
    file: File,
    chunk_digests: Vec<[u8; 32]>,
}

impl NativeDocuments {
    pub(crate) fn open_recorded(target: &ObservedNativeSessionTarget) -> Result<Self> {
        Self::open(target.recorded_home.as_deref().context("native_child.origin_unavailable")?, target)
    }
    /// Filesystem seam only. Production launch capture/historical binding lookup
    /// must establish `binding_home`; this is not a grant to select arbitrary roots.
    pub(crate) fn open(binding_home: &Path, target: &ObservedNativeSessionTarget) -> Result<Self> {
        Self::open_cancellable(binding_home, target, &std::sync::atomic::AtomicBool::new(false))
    }

    pub(crate) fn open_recorded_cancellable(target: &ObservedNativeSessionTarget, stop: &std::sync::atomic::AtomicBool) -> Result<Self> {
        Self::open_cancellable(target.recorded_home.as_deref().context("native_child.origin_unavailable")?, target, stop)
    }

    pub(crate) fn open_cancellable(binding_home: &Path, target: &ObservedNativeSessionTarget, stop: &std::sync::atomic::AtomicBool) -> Result<Self> {
        ensure!(!stop.load(std::sync::atomic::Ordering::Acquire), "native_child.read_cancelled");
        ensure!(
            binding_home.is_absolute(),
            "native_child.invalid_source_root"
        );
        ensure!(
            identity(&target.parent_native_session_id)
                && identity(&target.child_native_session_id)
                && target.parent_native_session_id != target.child_native_session_id,
            "native_child.invalid_reference"
        );
        let home = fs::canonicalize(binding_home).context("native_child.source_unavailable")?;
        let (documents, unavailable_advisories) = match target.adapter_kind.as_str() {
            "codex-cli" => (codex_documents(&home, target, stop)?, 0),
            "grok-build" => grok_documents(&home, target, stop)?,
            _ => bail!("native_child.unsupported_source"),
        };
        Ok(Self { documents, unavailable_advisories })
    }
}

impl NativeDocument {
    pub(super) fn open_project(
        home: &Path,
        path: &Path,
        stop: &std::sync::atomic::AtomicBool,
    ) -> Result<Self> {
        Self::open_with_cancel(home, path, DocumentKind::ProjectPatch, Some(stop))
    }

    fn open_with_cancel(
        home: &Path,
        path: &Path,
        kind: DocumentKind,
        stop: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<Self> {
        ensure!(
            !stop.is_some_and(|stop| stop.load(std::sync::atomic::Ordering::Acquire)),
            "native_child.read_cancelled"
        );
        let mut file = open_contained(home, path)?;
        let before = file.metadata()?;
        let length = before.len();
        let mut remaining = length;
        let mut buffer = vec![0; CHUNK_BYTES];
        let mut chunks = Vec::new();
        let mut digest = Sha256::new();
        while remaining > 0 {
            ensure!(
                !stop.is_some_and(|stop| stop.load(std::sync::atomic::Ordering::Acquire)),
                "native_child.read_cancelled"
            );
            let count = remaining.min(CHUNK_BYTES as u64) as usize;
            file.read_exact(&mut buffer[..count])
                .context("native_child.source_changed")?;
            digest.update(&buffer[..count]);
            chunks.push(Sha256::digest(&buffer[..count]).into());
            remaining -= count as u64;
        }
        let after = file.metadata()?;
        ensure!(
            !stop.is_some_and(|stop| stop.load(std::sync::atomic::Ordering::Acquire)),
            "native_child.read_cancelled"
        );
        ensure!(
            before.len() == after.len() && before.modified()? == after.modified()?,
            "native_child.source_changed"
        );
        Ok(Self {
            kind,
            history_base: None,
            seed_start_ordinal: None,
            own_header: None,
            snapshot_bytes: length,
            snapshot_digest: format!("{:x}", digest.finalize()),
            source_label: None,
            file,
            chunk_digests: chunks,
        })
    }

    /// Each returned byte belongs to the captured snapshot. Appends do not shift
    /// its pages; rewriting any requested chunk fails, even at identical length.
    /// The later client cursor must bind document index/digest to the Core target.
    pub(crate) fn read_chunk(&mut self, index: usize) -> Result<Vec<u8>> {
        let expected = self
            .chunk_digests
            .get(index)
            .context("native_child.invalid_chunk")?;
        let offset = (index as u64)
            .checked_mul(CHUNK_BYTES as u64)
            .context("native_child.invalid_chunk")?;
        let count = (self.snapshot_bytes - offset).min(CHUNK_BYTES as u64) as usize;
        let mut bytes = vec![0; count];
        self.file.seek(SeekFrom::Start(offset))?;
        self.file
            .read_exact(&mut bytes)
            .context("native_child.source_changed")?;
        ensure!(
            <[u8; 32]>::from(Sha256::digest(&bytes)) == *expected,
            "native_child.source_changed"
        );
        Ok(bytes)
    }

    pub(crate) fn chunk_count(&self) -> usize {
        self.chunk_digests.len()
    }

    fn header(&mut self, stop: &std::sync::atomic::AtomicBool) -> Result<Value> {
        let mut bytes = Vec::new();
        for index in 0..self.chunk_count().min(HEADER_BYTES as usize / CHUNK_BYTES) {
            ensure!(!stop.load(std::sync::atomic::Ordering::Acquire), "native_child.read_cancelled");
            bytes.extend(self.read_chunk(index)?);
            for line in bytes.split(|byte| *byte == b'\n').take(4) {
                if let Ok(value) = serde_json::from_slice::<Value>(line)
                    && value["type"] == "session_meta"
                {
                    return Ok(value);
                }
            }
            if bytes.iter().filter(|byte| **byte == b'\n').take(4).count() == 4 {
                break;
            }
        }
        bail!("native_child.header_unavailable")
    }
}

fn is_link(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    false
}

fn file_identity(file: &File) -> Result<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata()?;
        Ok((metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: the borrowed file handle stays open and info has the exact
        // layout required by GetFileInformationByHandle.
        let result = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
        ensure!(result != 0, "native_child.source_identity_unavailable");
        Ok((
            info.dwVolumeSerialNumber as u64,
            ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
        ))
    }
}

fn open_contained(home: &Path, path: &Path) -> Result<File> {
    let relative = path
        .strip_prefix(home)
        .context("native_child.source_outside_root")?;
    let mut current = home.to_path_buf();
    for component in relative.components() {
        ensure!(
            matches!(component, Component::Normal(_)),
            "native_child.invalid_source_path"
        );
        current.push(component);
        ensure!(
            !is_link(&fs::symlink_metadata(&current)?),
            "native_child.source_link_rejected"
        );
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(path)?;
    ensure!(
        file.metadata()?.is_file() && !is_link(&file.metadata()?),
        "native_child.source_not_regular"
    );
    let resolved = fs::canonicalize(path)?;
    ensure!(
        resolved.starts_with(home),
        "native_child.source_outside_root"
    );
    let repeated = options.open(&resolved)?;
    ensure!(
        !is_link(&repeated.metadata()?) && file_identity(&file)? == file_identity(&repeated)?,
        "native_child.source_changed"
    );
    Ok(file)
}

fn entries_with_stop(
    home: &Path,
    root: &Path,
    max_depth: usize,
    stop: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<PathBuf>> {
    ensure!(
        !stop.is_some_and(|stop| stop.load(std::sync::atomic::Ordering::Acquire)),
        "native_child.read_cancelled"
    );
    if !root.try_exists()? {
        return Ok(Vec::new());
    }
    ensure!(
        root.starts_with(home) && !is_link(&fs::symlink_metadata(root)?),
        "native_child.source_link_rejected"
    );
    let mut pending = vec![(root.to_path_buf(), 0)];
    let mut found = Vec::new();
    let mut count = 0;
    while let Some((directory, depth)) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            ensure!(
                !stop.is_some_and(|stop| stop.load(std::sync::atomic::Ordering::Acquire)),
                "native_child.read_cancelled"
            );
            count += 1;
            ensure!(count <= DISCOVERY_ENTRIES, "native_child.discovery_limit");
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            if is_link(&metadata) {
                continue;
            }
            if metadata.is_dir() && depth < max_depth {
                pending.push((path, depth + 1));
            } else if metadata.is_file() {
                found.push(path);
            }
        }
    }
    Ok(found)
}

#[derive(Clone)]
struct Rollout {
    path: PathBuf,
    timestamp: String,
    thread: String,
    rollout: String,
}

impl Rollout {
    fn from_path(path: PathBuf) -> Option<Self> {
        let stem = path
            .file_name()?
            .to_str()?
            .strip_prefix("rollout-")?
            .strip_suffix(".jsonl")?;
        let timestamp = stem.get(..19)?;
        chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%dT%H-%M-%S").ok()?;
        let ids = stem.get(19..)?.strip_prefix('-')?;
        let (thread, rollout) = ids.split_once('_').unwrap_or((ids, ids));
        if !identity(thread) || !identity(rollout) {
            return None;
        }
        Some(Self {
            timestamp: timestamp.into(),
            thread: thread.into(),
            rollout: rollout.into(),
            path,
        })
    }
}

fn codex_parent(payload: &Value) -> Option<&str> {
    payload["parent_thread_id"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .or_else(|| {
            payload
                .pointer("/source/subagent/thread_spawn/parent_thread_id")?
                .as_str()
                .map(str::trim)
                .filter(|v| !v.is_empty())
        })
}

fn history_pointer(payload: &Value) -> Result<Option<(String, u64)>> {
    let pointer = if let Some(base) = payload.get("history_base").filter(|v| !v.is_null()) {
        Some((&base["thread_id"], &base["end_ordinal_exclusive"]))
    } else if let Some(id) = payload.get("forked_from_id").filter(|v| !v.is_null()) {
        Some((id, &payload["forked_from_ordinal_exclusive"]))
    } else {
        None
    };
    pointer
        .map(|(id, end)| {
            let id = id
                .as_str()
                .filter(|id| identity(id))
                .context("native_child.invalid_history_link")?;
            let end = end.as_u64().context("native_child.invalid_history_link")?;
            Ok((id.into(), end))
        })
        .transpose()
}

fn codex_documents(
    home: &Path,
    target: &ObservedNativeSessionTarget,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<Vec<NativeDocument>> {
    let mut paths = entries_with_stop(home, &home.join("sessions"), 4, Some(stop))?;
    paths.extend(entries_with_stop(home, &home.join("archived_sessions"), 1, Some(stop))?);
    let rollouts: Vec<_> = paths.into_iter().filter_map(Rollout::from_path).collect();
    let mut current = rollouts
        .iter()
        .filter(|r| r.thread == target.child_native_session_id)
        .max_by(|a, b| (&a.timestamp, &a.rollout).cmp(&(&b.timestamp, &b.rollout)))
        .context("native_child.source_unavailable")?
        .clone();
    ensure!(
        rollouts
            .iter()
            .filter(|r| r.rollout == current.rollout)
            .count()
            == 1,
        "native_child.ambiguous_source"
    );
    let mut documents = Vec::<NativeDocument>::new();
    let mut visited = HashSet::new();
    loop {
        ensure!(
            visited.insert(current.rollout.clone()),
            "native_child.history_cycle"
        );
        let mut document = NativeDocument::open_with_cancel(home, &current.path, DocumentKind::CodexRollout, Some(stop))?;
        let header = document.header(stop)?;
        let payload = &header["payload"];
        ensure!(
            payload["id"] == current.thread,
            "native_child.session_mismatch"
        );
        if documents.is_empty() {
            ensure!(
                codex_parent(payload) == Some(target.parent_native_session_id.as_str()),
                "native_child.parent_mismatch"
            );
        }
        document.seed_start_ordinal =
            codex_parent(payload).and_then(|_| payload["subagent_history_start_ordinal"].as_u64());
        let pointer = history_pointer(payload)?;
        document.own_header = Some(header);
        if let Some((_, end)) = &pointer {
            document.history_base = Some(HistoryLink {
                document_index: documents.len() + 1,
                end_ordinal_exclusive: *end,
            });
        }
        documents.push(document);
        let Some((base, _)) = pointer else { break };
        let matches: Vec<_> = rollouts.iter().filter(|r| r.rollout == base).collect();
        ensure!(
            matches.len() == 1,
            "native_child.history_source_unavailable"
        );
        current = matches[0].clone();
    }
    Ok(documents)
}

fn grok_session(home: &Path, paths: &[PathBuf], id: &str) -> Result<PathBuf> {
    let matches: Vec<_> = paths
        .iter()
        .filter(|p| {
            p.file_name().is_some_and(|name| name == "updates.jsonl")
                && p.parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == id)
        })
        .collect();
    ensure!(matches.len() == 1, "native_child.source_unavailable");
    let path = matches[0].parent().unwrap().to_path_buf();
    ensure!(path.starts_with(home), "native_child.source_outside_root");
    Ok(path)
}

fn grok_link(home: &Path, parent: &Path, target: &ObservedNativeSessionTarget, stop: &std::sync::atomic::AtomicBool) -> Result<bool> {
    use super::content::records::{JsonRecords, ParseNotes, json_document};
    let mut parent_updates = NativeDocument::open_with_cancel(home, &parent.join("updates.jsonl"), DocumentKind::GrokUpdates, Some(stop))?;
    let mut notes = ParseNotes::default();
    for record in JsonRecords::new(&mut parent_updates, stop, &mut notes) {
        let value = record?;
        let update = &value["params"]["update"];
        if matches!(update["sessionUpdate"].as_str(), Some("subagent_spawned" | "subagent_finished"))
            && update["child_session_id"] == target.child_native_session_id
            && update.get("parent_session_id").is_none_or(|v| v == &target.parent_native_session_id)
        {
            return Ok(true);
        }
    }
    for path in entries_with_stop(home, &parent.join("subagents"), 1, Some(stop))? {
        if path.file_name().is_none_or(|name| name != "meta.json") { continue; }
        let mut document = NativeDocument::open_with_cancel(home, &path, DocumentKind::GrokSubagentMetadata, Some(stop))?;
        let value = match json_document(&mut document, stop) {
            Ok(value) => value,
            Err(error) if error.is::<serde_json::Error>() => continue,
            Err(error) => return Err(error),
        };
        if value["child_session_id"] == target.child_native_session_id
            && value.get("parent_session_id").is_none_or(|v| v == &target.parent_native_session_id)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn grok_documents(
    home: &Path,
    target: &ObservedNativeSessionTarget,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<(Vec<NativeDocument>, usize)> {
    let paths = entries_with_stop(home, &home.join("sessions"), 2, Some(stop))?;
    let parent = grok_session(home, &paths, &target.parent_native_session_id)?;
    let child = grok_session(home, &paths, &target.child_native_session_id)?;
    ensure!(
        grok_link(home, &parent, target, stop)?,
        "native_child.parent_mismatch"
    );
    let mut documents = vec![NativeDocument::open_with_cancel(
        home,
        &child.join("updates.jsonl"),
        DocumentKind::GrokUpdates,
        Some(stop),
    )?];
    let answers = child.join("chat_history.jsonl");
    if answers.try_exists()? {
        documents.push(NativeDocument::open_with_cancel(
            home,
            &answers,
            DocumentKind::GrokChatHistory,
            Some(stop),
        )?);
    }
    let summary = child.join("summary.json");
    if summary.try_exists()? {
        documents.push(NativeDocument::open_with_cancel(home, &summary, DocumentKind::GrokSummary, Some(stop))?);
    }
    for path in entries_with_stop(home, &child.join("subagents"), 1, Some(stop))? {
        if path.file_name().is_none_or(|name| name != "meta.json") { continue; }
        let Some(id) = path.parent().and_then(Path::file_name).and_then(|name| name.to_str()).filter(|id| identity(id)) else { continue; };
        let mut document = NativeDocument::open_with_cancel(home, &path, DocumentKind::GrokSubagentMetadata, Some(stop))?;
        document.source_label = Some(id.into());
        documents.push(document);
    }
    // These are advisory display sources, not transcript/lineage evidence.
    // Preserve the original catalogue fallback when absent or unreadable,
    // while still refusing links, non-regular files and paths outside home.
    let mut unavailable_advisories = 0;
    for (name, kind) in [("models_cache.json", DocumentKind::GrokModelsCache), ("config.toml", DocumentKind::GrokConfig)] {
        let path = home.join(name);
        if matches!(path.try_exists(), Ok(false)) { continue; }
        match NativeDocument::open_with_cancel(home, &path, kind, Some(stop)) {
            Ok(document) => documents.push(document),
            Err(error) if stop.load(std::sync::atomic::Ordering::Acquire) => return Err(error),
            Err(_) => unavailable_advisories += 1,
        }
    }
    Ok((documents, unavailable_advisories))
}

#[cfg(all(test, feature = "extended-tests"))]
mod tests;
