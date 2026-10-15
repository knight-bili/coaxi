use crate::local_attachment_snapshot::{PrivateSourceObservation, observe_private_source};
use crate::local_attachment_source::LocalAttachmentSourceRef;
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

fn load(
    connection: &Connection,
    conversation: &str,
    source: &LocalAttachmentSourceRef,
) -> Result<Option<(String, String, PrivateSourceObservation)>> {
    let value:Option<(String,String,String)>=connection.query_row(
        "SELECT source_path,kind,observation_json FROM single_chat_source_observation WHERE conversation_id=?1 AND source_ref_id=?2",
        params![conversation,source.id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    value
        .map(|(path, kind, json)| {
            Ok((
                path,
                kind,
                serde_json::from_str(&json).context("private source observation corrupted")?,
            ))
        })
        .transpose()
}

// 观察完成并核对所有旧记录后才写新行；不授予文件权限，不因 scope 移动改变源身份。
pub(crate) fn capture_sources(
    tx: &Transaction<'_>,
    conversation: &str,
    sources: &[LocalAttachmentSourceRef],
) -> Result<Option<String>> {
    let mut pending = Vec::new();
    for source in sources {
        let saved = load(tx, conversation, source)?;
        let Ok(observed) = observe_private_source(std::path::Path::new(&source.source_path)) else {
            return Ok(Some(source.id.clone()));
        };
        if observed.kind != source.kind
            || (source.kind == crate::local_attachment_source::LocalAttachmentKind::File
                && source.observed_byte_size != Some(observed.byte_size))
        {
            return Ok(Some(source.id.clone()));
        }
        if let Some((path, kind, expected)) = saved {
            if path != source.source_path || kind != source.kind.as_str() || expected != observed {
                return Ok(Some(source.id.clone()));
            }
        } else {
            pending.push((source, observed));
        }
    }
    for (source, observed) in pending {
        tx.execute("INSERT INTO single_chat_source_observation(conversation_id,source_ref_id,source_path,kind,observation_json) VALUES(?1,?2,?3,?4,?5)",
            params![conversation,source.id,source.source_path,source.kind.as_str(),serde_json::to_string(&observed)?])?;
    }
    Ok(None)
}

pub(crate) fn capture_upload(
    tx: &Transaction<'_>,
    conversation: &str,
    source: &LocalAttachmentSourceRef,
    digest: &str,
) -> Result<()> {
    let observed = observe_private_source(std::path::Path::new(&source.source_path))?;
    ensure!(
        observed.kind == source.kind && observed.content_digest == digest,
        "upload content changed"
    );
    if let Some((path, kind, expected)) = load(tx, conversation, source)? {
        ensure!(
            path == source.source_path && kind == source.kind.as_str() && expected == observed,
            "upload content changed"
        );
    } else {
        tx.execute("INSERT INTO single_chat_source_observation(conversation_id,source_ref_id,source_path,kind,observation_json) VALUES(?1,?2,?3,?4,?5)",
            params![conversation,source.id,source.source_path,source.kind.as_str(),serde_json::to_string(&observed)?])?;
    }
    Ok(())
}

// 缺少旧版观察时只允许原默认/历史排队兼容；新的卡片输入必须有真实观察。
// 存储读取错误向上传播；文件失效返回具体材料 ID，发送方保存业务拒绝并保留输入。
pub(crate) fn changed_source(
    connection: &Connection,
    conversation: &str,
    sources: &[LocalAttachmentSourceRef],
    require_observation: bool,
) -> Result<Option<String>> {
    for source in sources {
        let Some((path, kind, expected)) = load(connection, conversation, source)? else {
            if require_observation {
                return Ok(Some(source.id.clone()));
            }
            continue;
        };
        let current = observe_private_source(std::path::Path::new(&source.source_path));
        if path != source.source_path
            || kind != source.kind.as_str()
            || !matches!(current, Ok(ref value) if value==&expected)
        {
            return Ok(Some(source.id.clone()));
        }
    }
    Ok(None)
}
