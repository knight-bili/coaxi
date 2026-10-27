//! OfficeCLI provides source bytes; installation/member selection/frozen model
//! delivery use the existing, customer-approved skill-pack r1 contract.
use super::*;
use crate::{
    coaxi_skill_catalog::{InstallationFile, InstallationSource, SkillPackFamily},
    command::DomainCommandGateway,
    managed_process::{
        ManagedProcess, ManagedProcessLaunchSpec, ManagedProcessPurpose, ManagedStdinPolicy,
        ManagedWindowsArgvDialect,
    },
    skill_pack_installation::{InstallBeginning, InstallSkillPackCommand, SkillPackStore},
};
use anyhow::ensure;
use serde::Deserialize;
use std::{borrow::Cow, fs};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Prepare {
    skill_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Install {
    command_id: String,
    command: InstallSkillPackCommand,
    plan_id: String,
}
async fn load_cli(binary: &Path, id: &str, cwd: &Path) -> Result<String> {
    use tokio::io::AsyncReadExt;
    let mut command = crate::office_preview::tokio_command(binary);
    command.current_dir(cwd).arg("load_skill").arg(id);
    let spec = ManagedProcessLaunchSpec::capture(
        &command,
        ManagedProcessPurpose::RuntimeOneShot,
        ManagedStdinPolicy::Null,
        ManagedWindowsArgvDialect::MicrosoftCrt,
        format!("office-skill-{id}"),
    )?;
    let mut child = ManagedProcess::spawn(spec)?;
    async fn read(
        reader: impl tokio::io::AsyncRead + Unpin,
        limit: u64,
    ) -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        reader.take(limit + 1).read_to_end(&mut bytes).await?;
        Ok(bytes)
    }
    let stdout = tokio::spawn(read(
        child.take_stdout().context("office.stdout_unavailable")?,
        8 * 1024 * 1024,
    ));
    let stderr = tokio::spawn(read(
        child.take_stderr().context("office.stderr_unavailable")?,
        64 * 1024,
    ));
    let status = match tokio::time::timeout(std::time::Duration::from_secs(45), child.wait()).await
    {
        Ok(status) => status?,
        Err(_) => {
            let _ = child.force_terminate_tree();
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await;
            stdout.abort();
            stderr.abort();
            anyhow::bail!("office.load_skill_timeout")
        }
    };
    let output = stdout.await??;
    let error = stderr.await??;
    ensure!(
        output.len() <= 8 * 1024 * 1024 && error.len() <= 64 * 1024,
        "office.load_skill_output_limit"
    );
    ensure!(
        status.success(),
        "office.load_skill_failed: {}",
        String::from_utf8_lossy(&error)
    );
    let content = String::from_utf8(output)?;
    ensure!(!content.trim().is_empty(), "office.empty_skill");
    Ok(content)
}
impl Core {
    pub(super) async fn office_skill_prepare(&self, value: Value) -> Result<Value> {
        let request: Prepare = serde_json::from_value(value)?;
        let catalog = crate::coaxi_skill_catalog::list()?;
        let entry = catalog
            .entries
            .iter()
            .find(|entry| entry.id == request.skill_id && entry.family == SkillPackFamily::Office)
            .context("office.skill_unavailable")?;
        let load_id = entry.load_id.as_deref().context("office.load_id_missing")?;
        let binary = crate::office_preview::resolve_officecli().context("office.NOT_INSTALLED")?;
        let before = crate::backup::file_digest(&binary)?;
        let plans = self.data_dir.join("office-skill-previews");
        crate::platform::private_storage::prepare_private_directory(&plans)?;
        let id = uuid::Uuid::new_v4().to_string();
        let directory = plans.join(&id);
        crate::platform::private_storage::prepare_private_directory(&directory)?;
        let content = load_cli(&binary, load_id, &directory).await?;
        ensure!(
            crate::backup::file_digest(&binary)? == before,
            "office.binary_changed"
        );
        use sha2::{Digest, Sha256};
        let content_digest = format!("{:x}", Sha256::digest(content.as_bytes()));
        let revision = format!("officecli:{}:{load_id}:{content_digest}", before.1);
        crate::platform::private_storage::atomic_write_private_bytes(
            &directory.join("SKILL.md"),
            content.as_bytes(),
        )?;
        crate::platform::private_storage::atomic_write_private_json(
            &directory.join("source.json"),
            &json!({"skillId":request.skill_id,"sourceRevision":revision,"contentDigest":content_digest,"binaryDigest":before.1,"createdAt":Utc::now().to_rfc3339()}),
        )?;
        Ok(
            json!({"planId":id,"skillId":request.skill_id,"sourceRevision":revision,"content":content,"contentDigest":content_digest,"binaryDigest":before.1}),
        )
    }
    pub(super) async fn office_skill_install(&self, value: Value) -> Result<Value> {
        let request: Install = serde_json::from_value(value)?;
        uuid::Uuid::parse_str(&request.plan_id)?;
        let envelope = user_command_envelope(request.command_id, request.command);
        let store = SkillPackStore::new(&self.data_dir)?;
        {
            let database = self.database.lock().await;
            if let Some(execution) =
                DomainCommandGateway.replay_if_recorded(&database, &envelope)?
            {
                return Ok(serde_json::to_value(execution.result)?);
            }
        }
        let directory = self
            .data_dir
            .join("office-skill-previews")
            .join(request.plan_id);
        let metadata: Value = serde_json::from_slice(&fs::read(directory.join("source.json"))?)?;
        let bytes = fs::read(directory.join("SKILL.md"))?;
        use sha2::{Digest, Sha256};
        ensure!(
            metadata["skillId"].as_str() == Some(envelope.payload.skill_id.as_str())
                && metadata["sourceRevision"].as_str()
                    == Some(envelope.payload.source_revision.as_str())
                && metadata["contentDigest"].as_str()
                    == Some(format!("{:x}", Sha256::digest(&bytes)).as_str()),
            "office.prepared_source_changed"
        );
        let source = InstallationSource {
            revision: Cow::Owned(envelope.payload.source_revision.clone()),
            files: vec![InstallationFile {
                path: "SKILL.md".into(),
                bytes: Cow::Owned(bytes),
                executable: false,
            }],
        };
        let beginning = {
            let mut database = self.database.lock().await;
            store.begin_install_with_source(&mut database, &envelope, Ok(source))?
        };
        let execution = match beginning {
            InstallBeginning::Complete(execution) => execution,
            InstallBeginning::Prepare(plan) => {
                let preparing = store.clone();
                let prepared =
                    tokio::task::spawn_blocking(move || preparing.prepare(plan)).await??;
                let mut database = self.database.lock().await;
                store.commit_install(&mut database, &envelope, prepared)?
            }
        };
        Ok(serde_json::to_value(execution.result)?)
    }
}
