//! User transport. Source paths, native IDs and actors are never request fields.
use super::*;
use crate::runtime_subagent_session::{
    NativeChildSessionService,
    legacy::NativeAgentService,
    reader::AgentStartRead,
    reader::{PageRead, ReadId, StartRead},
};

impl Core {
    pub(super) async fn native_agent_read(&self, method: &str, value: Value) -> Result<Value> {
        let actor = ActorRef::User {
            user_id: CURRENT_USER_ID.into(),
        };
        if method == "runtime.agentReads.close" {
            let request: ReadId = serde_json::from_value(value)?;
            self.native_child_readers.close(&actor, &request.read_id)?;
            return Ok(json!({"closed":true}));
        }
        if method == "runtime.agentReads.start" {
            let request: AgentStartRead = serde_json::from_value(value)?;
            anyhow::ensure!(
                !self.planned_shutdown.shutdown_started(),
                "native_child.reader_closing"
            );
            let database = self.database.lock().await;
            let plan = NativeAgentService::prepare(
                &database,
                &ManagedBlobStore::new(&self.data_dir),
                &actor,
                &request.reference,
            )?;
            let created_at: String = database.connection().query_row(
                "SELECT created_at FROM agent_run WHERE id=?1",
                [&request.reference.agent_run_id],
                |r| r.get(0),
            )?;
            let fallback = chrono::DateTime::parse_from_rfc3339(&created_at)
                .context("native_child.invalid_run_time")?
                .with_timezone(&chrono::Utc);
            return Ok(serde_json::to_value(
                self.native_child_readers
                    .start_agent(&actor, request, &plan, fallback)?,
            )?);
        }
        let (id, page) = match method {
            "runtime.agentReads.poll" => {
                let request: ReadId = serde_json::from_value(value)?;
                (request.read_id, None)
            }
            "runtime.agentReads.page" => {
                let request: PageRead = serde_json::from_value(value)?;
                (request.read_id.clone(), Some(request))
            }
            _ => anyhow::bail!("native_child.operation_not_admitted"),
        };
        let reference = self.native_child_readers.agent_reference(&actor, &id)?;
        let database = self.database.lock().await;
        let plan = match NativeAgentService::prepare(
            &database,
            &ManagedBlobStore::new(&self.data_dir),
            &actor,
            &reference,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                let _ = self.native_child_readers.close(&actor, &id);
                return Err(error);
            }
        };
        let result = if let Some(page) = page {
            serde_json::to_value(self.native_child_readers.page_agent(&actor, page, &plan)?)?
        } else {
            serde_json::to_value(self.native_child_readers.poll_agent(&actor, &id, &plan)?)?
        };
        Ok(result)
    }

    pub(super) async fn native_child_file_source(&self, value: Value) -> Result<Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Request {
            kind: String,
            thread_id: String,
            read_id: String,
            raw_reference: String,
        }
        let request: Request = serde_json::from_value(value)?;
        anyhow::ensure!(
            request.kind == "native_child_file",
            "native_child.invalid_reference"
        );
        anyhow::ensure!(
            !request.raw_reference.is_empty()
                && request.raw_reference.len() <= 16_384
                && !request.raw_reference.contains(['\0', '\r', '\n']),
            "native_child.invalid_reference"
        );
        let actor = ActorRef::User {
            user_id: CURRENT_USER_ID.into(),
        };
        let (thread_id, directory) = if let Ok(reference) =
            self.native_child_readers.agent_reference(&actor, &request.read_id)
        {
            anyhow::ensure!(reference.thread_id == request.thread_id, "native_child.scope_changed");
            let database = self.database.lock().await;
            let plan = NativeAgentService::prepare(
                &database, &ManagedBlobStore::new(&self.data_dir), &actor, &reference,
            )?;
            let directory = self.native_child_readers.agent_file_directory(&actor, &request.read_id, &plan)?;
            (reference.thread_id, directory)
        } else {
            let reference = self.native_child_readers.reference(&actor, &request.read_id)?;
            anyhow::ensure!(reference.thread_id == request.thread_id, "native_child.scope_changed");
            let database = self.database.lock().await;
            let root = NativeChildSessionService::resolve(
                &database, &ManagedBlobStore::new(&self.data_dir), &actor, &reference,
            )?;
            let target = self.native_child_readers.file_target(&actor, &request.read_id, &root)?;
            let directory = target.recorded_working_directory.context("native_child.source_unavailable")?;
            (reference.thread_id, directory)
        };
        Ok(serde_json::to_value(
            crate::file_preview_authority::ResolvedFilePreviewSource::FileTarget {
                camp_id: thread_id,
                source_kind: "native_child_file".into(),
                source_identity: format!("native-child:{}", request.read_id),
                root_path: directory.to_string_lossy().into_owned(),
                base_path: directory.to_string_lossy().into_owned(),
                raw_reference: request.raw_reference,
                allow_children: true,
            },
        )?)
    }

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
