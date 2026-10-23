//! Immutable encrypted snapshots and an ETag-protected WebDAV index. Account
//! matching and URL construction follow Codeg network/webdav's closed base URL.
use anyhow::{Context, Result, ensure};
use reqwest::{Client, Method, StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::AsyncWriteExt;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub endpoint: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub allow_insecure: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub id: String,
    pub created_at: String,
    pub sha256: String,
    pub bytes: u64,
    #[serde(default = "backup_kind")]
    pub kind: String,
}
fn backup_kind() -> String {
    "backup".into()
}
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Index {
    format_version: u32,
    entries: Vec<Snapshot>,
    #[serde(default)]
    deleted: Vec<DeletedSnapshot>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeletedSnapshot {
    snapshot: Snapshot,
    removed_at: String,
}
pub struct WebDav {
    client: Client,
    config: Config,
    base: Url,
}
impl WebDav {
    pub fn new(config: Config) -> Result<Self> {
        let mut base = Url::parse(&config.endpoint)?;
        ensure!(
            matches!(base.scheme(), "https" | "http")
                && (base.scheme() == "https" || config.allow_insecure)
                && base.username().is_empty()
                && base.password().is_none()
                && base.query().is_none()
                && base.fragment().is_none()
                && base.host_str().is_some(),
            "webdav.invalid_endpoint"
        );
        ensure!(
            !config.username.contains(['\r', '\n'])
                && !config.password.contains(['\r', '\n'])
                && config.username.len() <= 1024
                && config.password.len() <= 16000,
            "webdav.invalid_credentials"
        );
        if !base.path().ends_with('/') {
            base.set_path(&(base.path().to_owned() + "/"));
        }
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            client,
            config,
            base,
        })
    }
    fn url(&self, name: &str) -> Result<Url> {
        ensure!(
            !name.contains(['/', ':', '\\', '?', '#', '\r', '\n']) && !name.is_empty(),
            "webdav.invalid_name"
        );
        let url = self.base.join(name)?;
        ensure!(
            url.origin() == self.base.origin() && url.path().starts_with(self.base.path()),
            "webdav.path_escaped"
        );
        Ok(url)
    }
    fn request(&self, method: Method, url: Url) -> reqwest::RequestBuilder {
        self.client
            .request(method, url)
            .basic_auth(&self.config.username, Some(&self.config.password))
    }
    pub async fn probe(&self) -> Result<serde_json::Value> {
        let response = self
            .request(Method::from_bytes(b"PROPFIND")?, self.base.clone())
            .header("depth", "0")
            .timeout(Duration::from_secs(15))
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "webdav.request_rejected: {}",
            response.status()
        );
        Ok(serde_json::json!({"connected":true}))
    }
    pub async fn create_collection(&self) -> Result<serde_json::Value> {
        let response = self
            .request(Method::from_bytes(b"MKCOL")?, self.base.clone())
            .timeout(Duration::from_secs(30))
            .send()
            .await?;
        ensure!(
            response.status().is_success() || response.status() == StatusCode::METHOD_NOT_ALLOWED,
            "webdav.collection_rejected: {}",
            response.status()
        );
        self.probe().await
    }
    async fn index(&self) -> Result<(Index, Option<String>)> {
        let mut response = self
            .request(Method::GET, self.url("rovai-index-v1.json")?)
            .timeout(Duration::from_secs(30))
            .send()
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok((
                Index {
                    format_version: 2,
                    entries: Vec::new(),
                    deleted: Vec::new(),
                },
                None,
            ));
        }
        ensure!(
            response.status().is_success(),
            "webdav.index_read_failed: {}",
            response.status()
        );
        let etag = response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        ensure!(
            etag.as_deref().is_none_or(|etag| !etag.starts_with("W/")),
            "webdav.strong_etag_required"
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len() + chunk.len() <= 8 * 1024 * 1024,
                "webdav.index_limit"
            );
            bytes.extend_from_slice(&chunk)
        }
        let index: Index = serde_json::from_slice(&bytes)?;
        ensure!(
            [1, 2].contains(&index.format_version)
                && index.entries.len() <= 10000
                && index.deleted.len() <= 10000,
            "webdav.unsupported_index"
        );
        let mut seen = std::collections::HashSet::new();
        for entry in &index.entries {
            uuid::Uuid::parse_str(&entry.id)?;
            ensure!(
                ["backup", "configuration"].contains(&entry.kind.as_str()),
                "webdav.invalid_snapshot_kind"
            );
            ensure!(
                entry.sha256.len() == 64
                    && entry.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
                    && entry.bytes <= 64 * 1024 * 1024 * 1024
                    && seen.insert(&entry.id),
                "webdav.invalid_snapshot"
            );
        }
        for deleted in &index.deleted {
            uuid::Uuid::parse_str(&deleted.snapshot.id)?;
            ensure!(
                deleted.snapshot.sha256.len() == 64
                    && !index
                        .entries
                        .iter()
                        .any(|entry| entry.id == deleted.snapshot.id),
                "webdav.invalid_tombstone"
            );
        }
        ensure!(
            etag.is_some(),
            "webdav.server_does_not_support_conditional_updates"
        );
        Ok((index, etag))
    }
    pub async fn list(&self) -> Result<Vec<Snapshot>> {
        let (mut index, _) = self.index().await?;
        index
            .entries
            .sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(index.entries)
    }
    fn blob_name(snapshot: &Snapshot) -> String {
        format!("rovai-{}-{}.rovai-backup", snapshot.id, snapshot.sha256)
    }
    async fn verify_remote(&self, snapshot: &Snapshot) -> Result<()> {
        let mut response = self
            .request(Method::GET, self.url(&Self::blob_name(snapshot))?)
            .timeout(Duration::from_secs(43200))
            .send()
            .await?
            .error_for_status()?;
        let mut hash = Sha256::new();
        let mut length = 0_u64;
        while let Some(chunk) = response.chunk().await? {
            length = length
                .checked_add(chunk.len() as u64)
                .context("webdav.length_overflow")?;
            ensure!(length <= snapshot.bytes, "webdav.remote_size_changed");
            hash.update(&chunk)
        }
        ensure!(
            length == snapshot.bytes && format!("{:x}", hash.finalize()) == snapshot.sha256,
            "webdav.remote_checksum_mismatch"
        );
        Ok(())
    }
    pub async fn push(
        &self,
        path: &Path,
        id: &str,
        expected_digest: Option<&str>,
    ) -> Result<Snapshot> {
        self.push_kind(path, id, expected_digest, "backup").await
    }
    pub async fn push_kind(
        &self,
        path: &Path,
        id: &str,
        expected_digest: Option<&str>,
        kind: &str,
    ) -> Result<Snapshot> {
        ensure!(
            ["backup", "configuration"].contains(&kind),
            "webdav.invalid_snapshot_kind"
        );
        uuid::Uuid::parse_str(id)?;
        ensure!(
            super::crypto::is_encrypted(path)?,
            "webdav.encrypted_snapshot_required"
        );
        let (bytes, sha256) = tokio::task::spawn_blocking({
            let path = path.to_path_buf();
            move || super::hash_file(&path)
        })
        .await??;
        ensure!(bytes <= 64 * 1024 * 1024 * 1024, "webdav.snapshot_limit");
        ensure!(
            expected_digest.is_none_or(|expected| expected == sha256),
            "webdav.local_selection_changed"
        );
        let snapshot = Snapshot {
            id: id.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            sha256,
            bytes,
            kind: kind.into(),
        };
        let (index, _) = self.index().await?;
        ensure!(
            !index.deleted.iter().any(|entry| entry.snapshot.id == id),
            "webdav.snapshot_removed"
        );
        if let Some(existing) = index.entries.iter().find(|entry| entry.id == id) {
            ensure!(
                existing.sha256 == snapshot.sha256
                    && existing.bytes == snapshot.bytes
                    && existing.kind == snapshot.kind,
                "webdav.operation_identity_changed"
            );
            return Ok(existing.clone());
        }
        let stream = tokio_util::io::ReaderStream::new(tokio::fs::File::open(path).await?);
        let upload = self
            .request(Method::PUT, self.url(&Self::blob_name(&snapshot))?)
            .header("if-none-match", "*")
            .header("content-type", "application/octet-stream")
            .header("content-length", bytes)
            .timeout(Duration::from_secs(43200))
            .body(reqwest::Body::wrap_stream(stream))
            .send()
            .await;
        match upload {
            Ok(response) if response.status().is_success() => {}
            Ok(response) if response.status() == StatusCode::PRECONDITION_FAILED => {
                self.verify_remote(&snapshot).await?
            }
            Ok(response) => anyhow::bail!("webdav.upload_rejected: {}", response.status()),
            Err(_) => self
                .verify_remote(&snapshot)
                .await
                .context("webdav.original_upload_outcome_unknown")?,
        }
        ensure!(
            tokio::task::spawn_blocking({
                let path = path.to_path_buf();
                move || super::hash_file(&path)
            })
            .await??
                == (snapshot.bytes, snapshot.sha256.clone()),
            "webdav.local_snapshot_changed"
        );
        for _ in 0..3 {
            let (mut index, etag) = self.index().await?;
            ensure!(
                !index.deleted.iter().any(|entry| entry.snapshot.id == id),
                "webdav.snapshot_removed"
            );
            if let Some(existing) = index.entries.iter().find(|entry| entry.id == id) {
                ensure!(
                    existing.sha256 == snapshot.sha256
                        && existing.bytes == snapshot.bytes
                        && existing.kind == snapshot.kind,
                    "webdav.operation_identity_changed"
                );
                return Ok(existing.clone());
            }
            index.format_version = 2;
            index.entries.push(snapshot.clone());
            let mut request = self
                .request(Method::PUT, self.url("rovai-index-v1.json")?)
                .header("content-type", "application/json")
                .timeout(Duration::from_secs(30));
            request = if let Some(etag) = etag {
                request.header("if-match", etag)
            } else {
                request.header("if-none-match", "*")
            };
            match request.body(serde_json::to_vec(&index)?).send().await {
                Ok(response) if response.status().is_success() => return Ok(snapshot),
                Ok(response) if response.status() == StatusCode::PRECONDITION_FAILED => continue,
                Ok(response) => anyhow::bail!("webdav.index_write_rejected: {}", response.status()),
                Err(_) => {
                    let (index, _) = self.index().await?;
                    if index
                        .entries
                        .iter()
                        .any(|entry| entry.id == id && entry.sha256 == snapshot.sha256)
                    {
                        return Ok(snapshot);
                    }
                    anyhow::bail!("webdav.original_index_write_outcome_unknown")
                }
            }
        }
        anyhow::bail!("webdav.concurrent_index_update; encrypted blob retained")
    }
    /// Remove the selected manifest entry with its exact ETag before deleting
    /// the immutable blob. Concurrent writers require a new explicit preview.
    pub async fn remove(
        &self,
        id: &str,
        expected_digest: &str,
        expected_index_etag: &str,
    ) -> Result<serde_json::Value> {
        uuid::Uuid::parse_str(id)?;
        let (mut index, etag) = self.index().await?;
        if index
            .deleted
            .iter()
            .any(|entry| entry.snapshot.id == id && entry.snapshot.sha256 == expected_digest)
        {
            return Ok(serde_json::json!({"removed":true,"snapshotId":id,"replayed":true}));
        }
        ensure!(
            etag.as_deref() == Some(expected_index_etag),
            "webdav.index_changed"
        );
        let snapshot = index
            .entries
            .iter()
            .find(|entry| entry.id == id && entry.sha256 == expected_digest)
            .context("webdav.remote_selection_changed")?
            .clone();
        index.entries.retain(|entry| entry.id != id);
        ensure!(index.deleted.len() < 10000, "webdav.tombstone_limit");
        index.format_version = 2;
        index.deleted.push(DeletedSnapshot {
            snapshot: snapshot.clone(),
            removed_at: chrono::Utc::now().to_rfc3339(),
        });
        let response = self
            .request(Method::PUT, self.url("rovai-index-v1.json")?)
            .header("if-match", expected_index_etag)
            .header("content-type", "application/json")
            .timeout(Duration::from_secs(30))
            .body(serde_json::to_vec(&index)?)
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {}
            Ok(response) if response.status() == StatusCode::PRECONDITION_FAILED => {
                anyhow::bail!("webdav.index_changed")
            }
            Ok(response) => anyhow::bail!("webdav.index_write_rejected: {}", response.status()),
            Err(_) => {
                let (current, _) = self.index().await?;
                ensure!(
                    current
                        .deleted
                        .iter()
                        .any(|entry| entry.snapshot.id == id
                            && entry.snapshot.sha256 == expected_digest),
                    "webdav.original_removal_outcome_unknown"
                );
            }
        }
        let result = self
            .request(Method::DELETE, self.url(&Self::blob_name(&snapshot))?)
            .timeout(Duration::from_secs(30))
            .send()
            .await;
        let orphan = match result {
            Ok(response) => {
                !(response.status().is_success() || response.status() == StatusCode::NOT_FOUND)
            }
            Err(_) => true,
        };
        Ok(serde_json::json!({"removed":true,"orphanRetained":orphan,"snapshotId":id}))
    }
    pub async fn removal_status(
        &self,
        id: &str,
        expected_digest: &str,
    ) -> Result<serde_json::Value> {
        uuid::Uuid::parse_str(id)?;
        let (index, _) = self.index().await?;
        if let Some(entry) = index
            .deleted
            .iter()
            .find(|entry| entry.snapshot.id == id && entry.snapshot.sha256 == expected_digest)
        {
            return Ok(
                serde_json::json!({"removed":true,"snapshotId":id,"removedAt":entry.removed_at}),
            );
        }
        Ok(
            serde_json::json!({"removed":false,"present":index.entries.iter().any(|entry|entry.id==id&&entry.sha256==expected_digest)}),
        )
    }
    pub async fn removal_preview(
        &self,
        id: &str,
        expected_digest: &str,
    ) -> Result<serde_json::Value> {
        uuid::Uuid::parse_str(id)?;
        let (index, etag) = self.index().await?;
        let snapshot = index
            .entries
            .iter()
            .find(|entry| entry.id == id && entry.sha256 == expected_digest)
            .context("webdav.remote_selection_changed")?;
        Ok(
            serde_json::json!({"snapshot":snapshot,"indexEtag":etag.context("webdav.index_missing")?}),
        )
    }
    pub async fn pull_resumable(
        &self,
        id: &str,
        destination: &Path,
        expected_digest: Option<&str>,
        operation_id: &str,
        cancel: &tokio_util::sync::CancellationToken,
    ) -> Result<Snapshot> {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        uuid::Uuid::parse_str(id)?;
        uuid::Uuid::parse_str(operation_id)?;
        let (index, _) = self.index().await?;
        let snapshot = index
            .entries
            .into_iter()
            .find(|entry| entry.id == id)
            .context("webdav.snapshot_missing")?;
        ensure!(
            expected_digest.is_none_or(|digest| digest == snapshot.sha256),
            "webdav.remote_selection_changed"
        );
        let parent = std::fs::canonicalize(
            destination
                .parent()
                .context("webdav.destination_parent_required")?,
        )?;
        let temporary = parent.join(format!(".rovai-download-{operation_id}.part"));
        if destination.exists() {
            super::no_link(destination)?;
            let found = super::hash_file(destination)?;
            ensure!(
                found == (snapshot.bytes, snapshot.sha256.clone()),
                "webdav.destination_changed"
            );
            return Ok(snapshot);
        }
        let mut file = if temporary.exists() {
            super::no_link(&temporary)?;
            tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&temporary)
                .await?
        } else {
            tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&temporary)
                .await?
        };
        let mut length = file.metadata().await?.len();
        ensure!(length <= snapshot.bytes, "webdav.partial_size_invalid");
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            ensure!(!cancel.is_cancelled(), "webdav.cancelled_outcome_unknown");
            let count = file.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if length < snapshot.bytes {
            let mut request = self
                .request(Method::GET, self.url(&Self::blob_name(&snapshot))?)
                .timeout(Duration::from_secs(43200));
            if length > 0 {
                request = request.header("range", format!("bytes={length}-"));
            }
            let mut response = tokio::select! {_ = cancel.cancelled()=>anyhow::bail!("webdav.cancelled_outcome_unknown"),result=request.send()=>result?};
            if length > 0 && response.status() == StatusCode::PARTIAL_CONTENT {
                let expected = format!("bytes {length}-");
                let range = response
                    .headers()
                    .get("content-range")
                    .and_then(|value| value.to_str().ok())
                    .context("webdav.range_missing")?;
                ensure!(
                    range.starts_with(&expected)
                        && range.ends_with(&format!("/{}", snapshot.bytes)),
                    "webdav.range_changed"
                );
            } else {
                ensure!(
                    response.status() == StatusCode::OK,
                    "webdav.download_rejected: {}",
                    response.status()
                );
                file.set_len(0).await?;
                file.seek(std::io::SeekFrom::Start(0)).await?;
                length = 0;
                hash = Sha256::new();
            }
            loop {
                let chunk = tokio::select! {_ = cancel.cancelled()=>{file.sync_all().await?;anyhow::bail!("webdav.cancelled_outcome_unknown")},result=response.chunk()=>result?};
                let Some(chunk) = chunk else { break };
                length = length
                    .checked_add(chunk.len() as u64)
                    .context("webdav.length_overflow")?;
                ensure!(length <= snapshot.bytes, "webdav.download_limit");
                hash.update(&chunk);
                file.write_all(&chunk).await?;
                progress(length, snapshot.bytes, "download");
            }
        }
        file.sync_all().await?;
        drop(file);
        ensure!(
            length == snapshot.bytes && format!("{:x}", hash.finalize()) == snapshot.sha256,
            "webdav.download_checksum_mismatch"
        );
        ensure!(
            super::crypto::is_encrypted(&temporary)?,
            "webdav.encrypted_snapshot_required"
        );
        ensure!(!cancel.is_cancelled(), "webdav.cancelled_outcome_unknown");
        std::fs::hard_link(&temporary, destination)?;
        let _ = tokio::fs::remove_file(&temporary).await;
        Ok(snapshot)
    }
    pub async fn pull(
        &self,
        id: &str,
        destination: &Path,
        expected_digest: Option<&str>,
    ) -> Result<Snapshot> {
        uuid::Uuid::parse_str(id)?;
        let (index, _) = self.index().await?;
        let snapshot = index
            .entries
            .into_iter()
            .find(|entry| entry.id == id)
            .context("webdav.snapshot_missing")?;
        ensure!(
            expected_digest.is_none_or(|digest| digest == snapshot.sha256),
            "webdav.remote_selection_changed"
        );
        ensure!(!destination.exists(), "webdav.destination_exists");
        let parent = std::fs::canonicalize(
            destination
                .parent()
                .context("webdav.destination_parent_required")?,
        )?;
        let temporary: PathBuf =
            parent.join(format!(".rovai-download-{}.tmp", uuid::Uuid::new_v4()));
        let result = async {
            let mut response = self
                .request(Method::GET, self.url(&Self::blob_name(&snapshot))?)
                .timeout(Duration::from_secs(43200))
                .send()
                .await?
                .error_for_status()?;
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .await?;
            let mut hash = Sha256::new();
            let mut length = 0_u64;
            while let Some(chunk) = response.chunk().await? {
                length = length
                    .checked_add(chunk.len() as u64)
                    .context("webdav.length_overflow")?;
                ensure!(length <= snapshot.bytes, "webdav.download_limit");
                hash.update(&chunk);
                file.write_all(&chunk).await?;
                progress(length, snapshot.bytes, "download");
            }
            file.sync_all().await?;
            drop(file);
            ensure!(
                length == snapshot.bytes && format!("{:x}", hash.finalize()) == snapshot.sha256,
                "webdav.download_checksum_mismatch"
            );
            ensure!(
                super::crypto::is_encrypted(&temporary)?,
                "webdav.encrypted_snapshot_required"
            );
            std::fs::hard_link(&temporary, destination)?;
            Ok::<_, anyhow::Error>(snapshot)
        }
        .await;
        let _ = tokio::fs::remove_file(&temporary).await;
        result
    }
}
fn progress(done: u64, total: u64, phase: &str) {
    eprintln!(
        "{}",
        serde_json::json!({"phase":phase,"done":done,"total":total})
    );
}

#[path = "webdav_retention.rs"]
mod retention;
