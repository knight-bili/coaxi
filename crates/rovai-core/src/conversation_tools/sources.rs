//! Core 签发的来源记录；真实 Conversation 与历史导入各有定位，原生编号不能充当权限。
use super::*;
use rusqlite::{Connection, OptionalExtension, params};
use std::{io::Read, path::Path};

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Locator {
    Live {
        conversation_id: String,
    },
    Imported {
        import_id: String,
        manifest_digest: String,
        transcript_digest: String,
    },
}
#[derive(Clone)]
pub(crate) struct Source {
    pub id: String,
    pub owner_thread_id: String,
    pub source_kind: String,
    pub legacy_session_id: Option<i64>,
    pub locator: Locator,
}
fn encryption_context(id: &str) -> String {
    format!("conversation-tool-source:{id}")
}
pub(crate) fn load(connection: &Connection, id: &str) -> Result<Source> {
    uuid::Uuid::parse_str(id)?;
    let(owner,kind,legacy,cipher):(String,String,Option<i64>,Vec<u8>)=connection.query_row("SELECT owner_thread_id,source_kind,legacy_session_id,locator_cipher FROM conversation_tool_source WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).context("conversation_tools.source_unavailable")?;
    let available: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM camp WHERE id=?1 AND deletion_operation_id IS NULL)",
        [&owner],
        |row| row.get(0),
    )?;
    ensure!(available, "conversation_tools.source_unavailable");
    let locator = crate::member_environment::decrypt(connection, &encryption_context(id), &cipher)?;
    Ok(Source {
        id: id.into(),
        owner_thread_id: owner,
        source_kind: kind,
        legacy_session_id: legacy,
        locator,
    })
}
/// live 来源复用 Core 已签发的 Conversation UUID，不接受客户端提供目录或来源种类。
pub(crate) fn live_record(connection: &Connection, id: &str) -> Result<Source> {
    uuid::Uuid::parse_str(id)?;
    let(owner,kind):(String,String)=connection.query_row("SELECT c.camp_id,c.kind FROM conversation c JOIN camp ON camp.id=c.camp_id WHERE c.id=?1 AND camp.deletion_operation_id IS NULL",[id],|row|Ok((row.get(0)?,row.get(1)?))).context("conversation_tools.source_unavailable")?;
    let source_kind = match kind.as_str() {
        "camp_member" => "public",
        "single_chat" => "single_chat",
        "delegation_child" => "delegation_child",
        _ => anyhow::bail!("conversation_tools.source_kind_unavailable"),
    };
    Ok(Source {
        id: id.into(),
        owner_thread_id: owner,
        source_kind: source_kind.into(),
        legacy_session_id: None,
        locator: Locator::Live {
            conversation_id: id.into(),
        },
    })
}
pub(crate) fn register_live(connection: &Connection, id: &str) -> Result<Source> {
    uuid::Uuid::parse_str(id)?;
    let(owner,kind):(String,String)=connection.query_row("SELECT c.camp_id,c.kind FROM conversation c JOIN camp ON camp.id=c.camp_id WHERE c.id=?1 AND camp.deletion_operation_id IS NULL",[id],|row|Ok((row.get(0)?,row.get(1)?))).context("conversation_tools.source_unavailable")?;
    let source_kind = match kind.as_str() {
        "camp_member" => "public",
        "single_chat" => "single_chat",
        "delegation_child" => "delegation_child",
        _ => anyhow::bail!("conversation_tools.source_kind_unavailable"),
    };
    let previous: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation_tool_source WHERE id=?1)",
        [id],
        |row| row.get(0),
    )?;
    if previous {
        let source = load(connection, id)?;
        ensure!(
            source.owner_thread_id == owner
                && source.source_kind == source_kind
                && matches!(&source.locator,Locator::Live{conversation_id} if conversation_id==id),
            "conversation_tools.source_changed"
        );
        return Ok(source);
    }
    let locator = Locator::Live {
        conversation_id: id.into(),
    };
    let cipher = crate::member_environment::encrypt(connection, &encryption_context(id), &locator)?;
    connection.execute("INSERT INTO conversation_tool_source(id,owner_thread_id,source_kind,conversation_id,locator_cipher,created_at) VALUES(?1,?2,?3,?1,?4,?5)",params![id,owner,source_kind,cipher,chrono::Utc::now().to_rfc3339()])?;
    load(connection, id)
}
fn import_id(id: &str) -> Result<()> {
    ensure!(
        id.len() == 64
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "conversation_tools.import_id_invalid"
    );
    Ok(())
}
fn read(root: &Path, id: &str, suffix: &str, limit: u64) -> Result<Vec<u8>> {
    import_id(id)?;
    let path = root.join("imported-history").join(format!("{id}.{suffix}"));
    let mut file = crate::platform::private_storage::open_private_read_file(&path)?;
    ensure!(
        file.metadata()?.len() <= limit,
        "conversation_tools.import_too_large"
    );
    let mut bytes = Vec::new();
    file.by_ref().take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "conversation_tools.import_too_large"
    );
    Ok(bytes)
}
fn file_digest(root: &Path, id: &str, suffix: &str) -> Result<String> {
    use sha2::{Digest, Sha256};
    import_id(id)?;
    let path = root.join("imported-history").join(format!("{id}.{suffix}"));
    let mut file = crate::platform::private_storage::open_private_read_file(&path)?;
    ensure!(
        file.metadata()?.len() <= 512 * 1024 * 1024,
        "conversation_tools.import_too_large"
    );
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut total = 0u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        ensure!(
            total <= 512 * 1024 * 1024,
            "conversation_tools.import_too_large"
        );
        digest.update(&buffer[..count]);
    }
    Ok(format!("sha256:{:x}", digest.finalize()))
}
/// 导入来源只有受管 manifest 和 transcript 的准确版本，不借原始日志路径读取本机文件。
pub(crate) fn register_import(
    connection: &Connection,
    root: &Path,
    owner: &str,
    id: &str,
) -> Result<Source> {
    let manifest: Value = serde_json::from_slice(&read(root, id, "manifest.json", 1024 * 1024)?)?;
    ensure!(
        manifest["id"] == id && manifest["linkedThreadId"] == owner,
        "conversation_tools.import_owner_changed"
    );
    let revision = crate::command::canonical_json_digest(&manifest)?;
    let previous:Option<String>=connection.query_row("SELECT id FROM conversation_tool_source WHERE source_kind='imported' AND owner_thread_id=?1 AND import_id=?2 AND import_revision=?3",params![owner,id,revision],|row|row.get(0)).optional()?;
    if let Some(previous) = previous {
        return load(connection, &previous);
    }
    let source_id = uuid::Uuid::new_v4().to_string();
    let locator = Locator::Imported {
        import_id: id.into(),
        manifest_digest: revision.clone(),
        transcript_digest: file_digest(root, id, "transcript.json")?,
    };
    let cipher =
        crate::member_environment::encrypt(connection, &encryption_context(&source_id), &locator)?;
    connection.execute("INSERT INTO conversation_tool_source(id,owner_thread_id,source_kind,import_id,import_revision,locator_cipher,created_at) VALUES(?1,?2,'imported',?3,?4,?5,?6)",params![source_id,owner,id,revision,cipher,chrono::Utc::now().to_rfc3339()])?;
    load(connection, &source_id)
}
pub(crate) fn import_transcript(root: &Path, source: &Source) -> Result<(Value, Value)> {
    let Locator::Imported {
        import_id,
        manifest_digest,
        transcript_digest,
    } = &source.locator
    else {
        anyhow::bail!("conversation_tools.import_required")
    };
    let manifest: Value =
        serde_json::from_slice(&read(root, import_id, "manifest.json", 1024 * 1024)?)?;
    ensure!(
        manifest["id"] == *import_id
            && manifest["linkedThreadId"] == source.owner_thread_id
            && crate::command::canonical_json_digest(&manifest)? == *manifest_digest,
        "conversation_tools.source_changed"
    );
    ensure!(
        file_digest(root, import_id, "transcript.json")? == *transcript_digest,
        "conversation_tools.source_changed"
    );
    let transcript = serde_json::from_slice(&read(
        root,
        import_id,
        "transcript.json",
        512 * 1024 * 1024,
    )?)?;
    Ok((manifest, transcript))
}
