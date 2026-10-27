//! Explicit immutable batches; original tombstones fence resumed uploads.
use super::*;
use serde_json::{Value, json};
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Plan {
    version: u32,
    index_etag: String,
    keep_backups: usize,
    keep_configurations: usize,
    delete: Vec<Snapshot>,
    remaining: usize,
}
fn choose(index: &Index, kind: &str, keep: usize) -> Result<Vec<Snapshot>> {
    let mut entries = Vec::new();
    for entry in index.entries.iter().filter(|entry| entry.kind == kind) {
        entries.push((
            chrono::DateTime::parse_from_rfc3339(&entry.created_at)?,
            entry.clone(),
        ));
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    Ok(entries
        .into_iter()
        .skip(keep)
        .map(|(_, entry)| entry)
        .collect())
}
impl WebDav {
    pub async fn retention_preview(
        &self,
        keep_backups: usize,
        keep_configurations: usize,
    ) -> Result<Value> {
        ensure!(
            (1..=1000).contains(&keep_backups) && (1..=1000).contains(&keep_configurations),
            "webdav.invalid_retention"
        );
        let (index, etag) = self.index().await?;
        let mut delete = choose(&index, "backup", keep_backups)?;
        delete.extend(choose(&index, "configuration", keep_configurations)?);
        let remaining = delete.len().saturating_sub(20);
        delete.truncate(20);
        ensure!(
            index.deleted.len() + delete.len() <= 10000,
            "webdav.tombstone_limit"
        );
        Ok(serde_json::to_value(Plan {
            version: 1,
            index_etag: etag.context("webdav.index_missing")?,
            keep_backups,
            keep_configurations,
            delete,
            remaining,
        })?)
    }
    pub async fn retention_apply(&self, value: Value) -> Result<Value> {
        let plan: Plan = serde_json::from_value(value.clone())?;
        ensure!(
            plan.version == 1 && plan.delete.len() <= 20,
            "webdav.invalid_retention_plan"
        );
        let (mut index, etag) = self.index().await?;
        if !plan.delete.is_empty()
            && plan.delete.iter().all(|chosen| {
                index.deleted.iter().any(|removed| {
                    removed.snapshot.id == chosen.id && removed.snapshot.sha256 == chosen.sha256
                })
            })
        {
            return self.retention_status(value).await;
        }
        ensure!(
            etag.as_deref() == Some(plan.index_etag.as_str()),
            "webdav.index_changed"
        );
        ensure!(
            self.retention_preview(plan.keep_backups, plan.keep_configurations)
                .await?
                == value,
            "webdav.retention_selection_changed"
        );
        if plan.delete.is_empty() {
            return Ok(json!({"removed":true,"items":[],"remaining":plan.remaining}));
        }
        let chosen: std::collections::HashSet<_> =
            plan.delete.iter().map(|entry| entry.id.as_str()).collect();
        index
            .entries
            .retain(|entry| !chosen.contains(entry.id.as_str()));
        let now = chrono::Utc::now().to_rfc3339();
        index
            .deleted
            .extend(plan.delete.iter().cloned().map(|snapshot| DeletedSnapshot {
                snapshot,
                removed_at: now.clone(),
            }));
        index.format_version = 2;
        let result = self
            .request(Method::PUT, self.url("coaxi-index-v1.json")?)
            .header("if-match", &plan.index_etag)
            .header("content-type", "application/json")
            .timeout(Duration::from_secs(30))
            .body(serde_json::to_vec(&index)?)
            .send()
            .await;
        match result {
            Ok(response) if response.status().is_success() => {}
            Ok(response) if response.status() == StatusCode::PRECONDITION_FAILED => {
                anyhow::bail!("webdav.index_changed")
            }
            Ok(response) => anyhow::bail!("webdav.index_write_rejected: {}", response.status()),
            Err(_) => {
                let (current, _) = self.index().await?;
                ensure!(
                    plan.delete
                        .iter()
                        .all(|entry| current
                            .deleted
                            .iter()
                            .any(|removed| removed.snapshot.id == entry.id
                                && removed.snapshot.sha256 == entry.sha256)),
                    "webdav.original_retention_outcome_unknown"
                );
            }
        }
        // Keep tombstones even after deleting blobs: a suspended old upload must
        // never be allowed to re-publish a deliberately removed snapshot.
        let mut items = Vec::new();
        for entry in &plan.delete {
            items.push(
                self.cleanup_removed(&entry.id, &entry.sha256)
                    .await
                    .unwrap_or_else(
                        |_| json!({"snapshotId":entry.id,"cleaned":false,"unknown":true}),
                    ),
            );
        }
        Ok(json!({"removed":true,"items":items,"remaining":plan.remaining}))
    }
    pub async fn retention_status(&self, value: Value) -> Result<Value> {
        let plan: Plan = serde_json::from_value(value)?;
        ensure!(
            plan.version == 1 && plan.delete.len() <= 20,
            "webdav.invalid_retention_plan"
        );
        let (index, _) = self.index().await?;
        let items:Vec<_>=plan.delete.iter().map(|entry|json!({"snapshotId":entry.id,"removed":index.deleted.iter().any(|removed|removed.snapshot.id==entry.id&&removed.snapshot.sha256==entry.sha256),"present":index.entries.iter().any(|live|live.id==entry.id&&live.sha256==entry.sha256)})).collect();
        Ok(
            json!({"removed":items.iter().all(|item|item["removed"]==true),"items":items,"remaining":plan.remaining}),
        )
    }
    pub async fn cleanup_preview(&self, after: Option<&str>) -> Result<Value> {
        let (index, _) = self.index().await?;
        let ordered: Vec<_> = index.deleted.iter().rev().collect();
        let start = if let Some(id) = after {
            ordered
                .iter()
                .position(|entry| entry.snapshot.id == id)
                .context("webdav.cleanup_cursor_missing")?
                + 1
        } else {
            0
        };
        let batch: Vec<_> = ordered.iter().skip(start).take(20).collect();
        let next = if start + batch.len() < ordered.len() {
            batch.last().map(|entry| entry.snapshot.id.clone())
        } else {
            None
        };
        let mut items = Vec::new();
        for entry in &batch {
            let response = self
                .request(Method::HEAD, self.url(&Self::blob_name(&entry.snapshot))?)
                .timeout(Duration::from_secs(15))
                .send()
                .await?;
            if response.status() == StatusCode::NOT_FOUND {
                continue;
            }
            ensure!(
                response.status().is_success(),
                "webdav.cleanup_read_failed: {}",
                response.status()
            );
            items.push(json!({"snapshot":entry.snapshot,"removedAt":entry.removed_at}));
        }
        Ok(json!({"items":items,"scanned":batch.len(),"nextCursor":next}))
    }
    pub async fn cleanup_removed(&self, id: &str, digest: &str) -> Result<Value> {
        uuid::Uuid::parse_str(id)?;
        let (index, _) = self.index().await?;
        let entry = index
            .deleted
            .iter()
            .find(|entry| entry.snapshot.id == id && entry.snapshot.sha256 == digest)
            .context("webdav.removal_record_required")?;
        // Only names in this product's tombstones qualify. Never enumerate or
        // delete arbitrary files in a shared WebDAV collection.
        let response = self
            .request(Method::DELETE, self.url(&Self::blob_name(&entry.snapshot))?)
            .timeout(Duration::from_secs(30))
            .send()
            .await?;
        ensure!(
            response.status().is_success() || response.status() == StatusCode::NOT_FOUND,
            "webdav.cleanup_rejected: {}",
            response.status()
        );
        Ok(json!({"snapshotId":id,"cleaned":true}))
    }
}
