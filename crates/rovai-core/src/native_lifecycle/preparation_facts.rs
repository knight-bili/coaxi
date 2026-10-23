//! 原程序创建回执先单独保存；这些事实不能自动补造Core绑定或重发创建。
use super::*;
use crate::acp::NativePreparationProof;
use rusqlite::{Connection, params};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Fact {
    schema_version: u32,
    command_id: String,
    core_instance_id: String,
    origin_id: String,
    proof_digest: String,
    proof: NativePreparationProof,
}
fn path(root: &Path, command: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(command)?;
    Ok(root
        .join("native-lifecycle/preparation-receipts")
        .join(format!("{command}.json")))
}
pub(crate) fn record(
    root: &Path,
    command: &str,
    instance: &str,
    origin: &str,
    proof: &NativePreparationProof,
) -> Result<()> {
    uuid::Uuid::parse_str(instance)?;
    uuid::Uuid::parse_str(origin)?;
    let path = path(root, command)?;
    crate::platform::private_storage::prepare_private_directory(
        path.parent()
            .context("native_lifecycle.receipt_parent_missing")?,
    )?;
    let value = Fact {
        schema_version: 1,
        command_id: command.into(),
        core_instance_id: instance.into(),
        origin_id: origin.into(),
        proof_digest: digest(&serde_json::to_value(proof)?)?,
        proof: proof.clone(),
    };
    crate::platform::private_storage::atomic_write_private_json(&path, &value)
}
pub(crate) fn read(
    connection: &Connection,
    root: &Path,
    command: &str,
) -> Result<Option<NativePreparationProof>> {
    let path = path(root, command)?;
    let mut file = match crate::platform::private_storage::open_private_read_file(&path) {
        Ok(file) => file,
        Err(error)
            if error.chain().any(|cause| {
                cause
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            }) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.by_ref().take(65537).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 65536, "native_lifecycle.receipt_too_large");
    let fact: Fact = serde_json::from_slice(&bytes)?;
    let(instance,origin):(String,String)=connection.query_row("SELECT core_instance_id,json_extract(value_json,'$.originId') FROM native_lifecycle_intent WHERE command_id=?1 AND kind='continuation'",params![command],|row|Ok((row.get(0)?,row.get(1)?)))?;
    ensure!(
        fact.schema_version == 1
            && fact.command_id == command
            && fact.core_instance_id == instance
            && fact.origin_id == origin
            && fact.proof_digest == digest(&serde_json::to_value(&fact.proof)?)?,
        "native_lifecycle.original_creation_receipt_changed"
    );
    ensure!(
        !fact.proof.reported_session_id.is_empty()
            && fact.proof.reported_session_id.len() <= 4096
            && !fact.proof.reported_session_id.contains('\0'),
        "native_lifecycle.reported_session_missing"
    );
    Ok(Some(fact.proof))
}
