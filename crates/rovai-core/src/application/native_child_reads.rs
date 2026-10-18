//! User transport. Source paths, native IDs and actors are never request fields.
use super::*;
use crate::runtime_subagent_session::{
    NativeChildSessionService,
    reader::{PageRead, ReadId, StartRead},
};

impl Core {
    pub(super) async fn native_child_read(&self, method: &str, value: Value) -> Result<Value> {
        let actor = ActorRef::User {
            user_id: CURRENT_USER_ID.into(),
        };
        match method {
            "runtime.childReads.start" => {
                let request: StartRead = serde_json::from_value(value)?;
                anyhow::ensure!(
                    !self.planned_shutdown.shutdown_started(),
                    "native_child.reader_closing"
                );
                let database = self.database.lock().await;
                let root = NativeChildSessionService::resolve(
                    &database,
                    &ManagedBlobStore::new(&self.data_dir),
                    &actor,
                    &request.reference,
                )?;
                let created_at: String = database.connection().query_row(
                    "SELECT created_at FROM agent_run WHERE id=?1",
                    [&root.agent_run_id],
                    |row| row.get(0),
                )?;
                let fallback = chrono::DateTime::parse_from_rfc3339(&created_at)
                    .context("native_child.invalid_run_time")?
                    .with_timezone(&chrono::Utc);
                Ok(serde_json::to_value(
                    self.native_child_readers
                        .start(&actor, request, &root, fallback)?,
                )?)
            }
            "runtime.childReads.poll" => {
                let request: ReadId = serde_json::from_value(value)?;
                let reference = self
                    .native_child_readers
                    .reference(&actor, &request.read_id)?;
                let database = self.database.lock().await;
                let root = NativeChildSessionService::resolve(
                    &database,
                    &ManagedBlobStore::new(&self.data_dir),
                    &actor,
                    &reference,
                );
                let root = match root {
                    Ok(root) => root,
                    Err(error) => {
                        let _ = self.native_child_readers.close(&actor, &request.read_id);
                        return Err(error);
                    }
                };
                Ok(serde_json::to_value(self.native_child_readers.poll(
                    &actor,
                    &request.read_id,
                    &root,
                )?)?)
            }
            "runtime.childReads.page" => {
                let request: PageRead = serde_json::from_value(value)?;
                let reference = self
                    .native_child_readers
                    .reference(&actor, &request.read_id)?;
                let database = self.database.lock().await;
                let root = NativeChildSessionService::resolve(
                    &database,
                    &ManagedBlobStore::new(&self.data_dir),
                    &actor,
                    &reference,
                );
                let root = match root {
                    Ok(root) => root,
                    Err(error) => {
                        let _ = self.native_child_readers.close(&actor, &request.read_id);
                        return Err(error);
                    }
                };
                Ok(serde_json::to_value(
                    self.native_child_readers.page(&actor, request, &root)?,
                )?)
            }
            "runtime.childReads.close" => {
                let request: ReadId = serde_json::from_value(value)?;
                self.native_child_readers.close(&actor, &request.read_id)?;
                Ok(json!({"closed":true}))
            }
            _ => anyhow::bail!("native_child.operation_not_admitted"),
        }
    }
}
