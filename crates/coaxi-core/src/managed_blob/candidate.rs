//! A reservation is acquired under Core's DB mutex before filesystem work is
//! moved outside it. GC never waits for a writer while holding that mutex.
use super::*;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

type Reservations = HashMap<(PathBuf, Uuid), String>;
fn reservations() -> &'static Mutex<Reservations> {
    static ACTIVE: OnceLock<Mutex<Reservations>> = OnceLock::new();
    ACTIVE.get_or_init(Mutex::default)
}
pub(super) fn active_intent(root: &Path, id: &str) -> bool {
    Uuid::parse_str(id).ok().is_some_and(|id| {
        reservations()
            .lock()
            .unwrap()
            .contains_key(&(root.to_path_buf(), id))
    })
}
pub(super) fn active_digest(root: &Path, digest: &str) -> bool {
    reservations()
        .lock()
        .unwrap()
        .iter()
        .any(|((path, _), value)| path == root && value == digest)
}

pub(crate) struct CandidateReservation {
    store: ManagedBlobStore,
    intent: Uuid,
    digest: String,
    id: String,
    size: u64,
    media_type: String,
    sensitivity: String,
    owner: String,
    at: String,
}
impl Drop for CandidateReservation {
    fn drop(&mut self) {
        reservations()
            .lock()
            .unwrap()
            .remove(&(self.store.root.clone(), self.intent));
    }
}
pub(crate) struct PreparedBlobCandidate {
    reservation: CandidateReservation,
}

impl ManagedBlobStore {
    /// Call with Core's DB mutex held. The reservation then survives the lock's
    /// release and is visible to every collector for this same Core data root.
    pub(crate) fn reserve_candidate(
        &self,
        database: &Database,
        bytes: &[u8],
        media_type: &str,
        sensitivity: &str,
        owner: &str,
    ) -> Result<CandidateReservation> {
        validate_gc_owner(owner)?;
        validate_media_type(media_type)?;
        anyhow::ensure!(
            matches!(sensitivity, "normal" | "sensitive"),
            "Blob sensitivity must be normal or sensitive"
        );
        anyhow::ensure!(
            bytes.len() as u64 <= self.max_blob_bytes,
            "Managed Blob exceeds {} bytes",
            self.max_blob_bytes
        );
        let digest = format!("{:x}", Sha256::digest(bytes));
        let id = format!("blob-sha256-{digest}");
        validate_metadata(database.connection(), &digest, bytes.len() as u64)?;
        let reservation = CandidateReservation {
            store: self.clone(),
            intent: Uuid::new_v4(),
            digest,
            id,
            size: bytes.len() as u64,
            media_type: media_type.into(),
            sensitivity: sensitivity.into(),
            owner: owner.into(),
            at: chrono::Utc::now().to_rfc3339(),
        };
        reservations().lock().unwrap().insert(
            (self.root.clone(), reservation.intent),
            reservation.digest.clone(),
        );
        Ok(reservation)
    }
}

fn validate_metadata(connection: &Connection, digest: &str, size: u64) -> Result<()> {
    if let Some((existing_size, existing_path)) = connection
        .query_row(
            "SELECT byte_size,storage_relative_path FROM managed_blob WHERE sha256=?1",
            [digest],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
    {
        anyhow::ensure!(
            existing_size == size as i64 && existing_path == blob_relative_path(digest),
            "Managed Blob metadata conflicts with content address"
        );
    }
    Ok(())
}

impl CandidateReservation {
    /// Only filesystem IO; no Database/Connection or global DB mutex is needed.
    pub(crate) fn write(self, bytes: &[u8]) -> Result<PreparedBlobCandidate> {
        anyhow::ensure!(
            bytes.len() as u64 == self.size
                && format!("{:x}", Sha256::digest(bytes)) == self.digest,
            "Managed Blob candidate content changed after reservation"
        );
        let temporary_dir = self.store.root.join("tmp");
        fs::create_dir_all(&temporary_dir)?;
        let intent_path = temporary_dir.join(format!("candidate-intent-{}.json", self.intent));
        let staging = temporary_dir.join(format!("candidate-intent-write-{}", self.intent));
        let data_path = temporary_dir.join(format!("candidate-data-{}", self.intent));
        let intent = serde_json::to_vec(&CandidateWriteIntent {
            version: 1,
            digest: self.digest.clone(),
            owner: self.owner.clone(),
            created_at: self.at.clone(),
        })?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&staging)?;
        file.write_all(&intent)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&staging, &intent_path)?;
        sync_parent(&intent_path)?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&data_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        let final_path = self.store.root.join(blob_relative_path(&self.digest));
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if final_path.exists() {
            let (digest, size) = hash_file(&final_path)?;
            anyhow::ensure!(
                digest == self.digest && size == self.size,
                "Managed Blob digest collision or corrupted existing content"
            );
            fs::remove_file(&data_path)?;
        } else {
            match fs::rename(&data_path, &final_path) {
                Ok(()) => sync_parent(&final_path)?,
                Err(error) => {
                    // Another outside-lock writer may publish the same address
                    // between exists() and rename(), notably on Windows.
                    let same = hash_file(&final_path)
                        .is_ok_and(|(digest, size)| digest == self.digest && size == self.size);
                    if !same {
                        return Err(error.into());
                    }
                    fs::remove_file(&data_path)?;
                }
            }
        }
        Ok(PreparedBlobCandidate { reservation: self })
    }
}

impl PreparedBlobCandidate {
    pub(crate) fn id(&self) -> &str {
        &self.reservation.id
    }
    /// Metadata and its authoritative foreign-key reference can share the same
    /// command transaction. On rollback, leave the durable intent for old GC.
    pub(crate) fn register(&self, transaction: &Transaction<'_>) -> Result<ManagedBlobMetadata> {
        let r = &self.reservation;
        validate_metadata(transaction, &r.digest, r.size)?;
        transaction.execute(
            "INSERT INTO managed_blob(id,sha256,byte_size,media_type,storage_relative_path,state,sensitivity,
                created_at,verified_at,updated_at,gc_candidate_at,gc_candidate_owner)
             VALUES(?1,?2,?3,?4,?5,'present',?6,?7,?7,?7,?7,?8)
             ON CONFLICT(sha256) DO UPDATE SET state='present',verified_at=excluded.verified_at,updated_at=excluded.updated_at,
                gc_candidate_at=excluded.gc_candidate_at,gc_candidate_owner=excluded.gc_candidate_owner",
            params![r.id,r.digest,r.size as i64,r.media_type,blob_relative_path(&r.digest),r.sensitivity,r.at,r.owner],
        )?;
        load_blob_metadata(transaction, &r.id)?
            .context("Managed Blob candidate metadata was not persisted")
    }
    /// Only after the metadata/reference transaction commits, outside the DB
    /// mutex. Dropping without finish leaves the existing crash-recovery intent.
    pub(crate) fn finish(&self) -> Result<()> {
        let path = self
            .reservation
            .store
            .root
            .join("tmp")
            .join(format!("candidate-intent-{}.json", self.reservation.intent));
        fs::remove_file(&path)?;
        sync_parent(&path)
    }
}
