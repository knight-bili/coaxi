//! 显式选择的当前草稿材料先复制成 Core 私有快照，投递只读快照编号。
use super::*;
use crate::{
    draft_client::DraftClient,
    local_attachment_source::LocalAttachmentSourceRef,
    single_chat_composer_scope::{PrivateComposerKey, SingleChatComposerScope},
};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub(crate) struct Candidate {
    pub original_id: String,
    pub source: PathBuf,
    pub display_name: String,
}
pub(crate) fn selected_sources(
    connection: &Connection,
    target: &NativeSteerTargetV1,
    client: &DraftClient,
    composer: Option<&SingleChatComposerScope>,
    ids: &[String],
) -> Result<Vec<Candidate>> {
    super::target::require(connection, target)?;
    ensure!(
        !ids.is_empty() && ids.len() <= 10,
        "native_lifecycle.material_count_invalid"
    );
    let mut selected = std::collections::HashSet::new();
    for id in ids {
        uuid::Uuid::parse_str(id)?;
        ensure!(selected.insert(id), "native_lifecycle.duplicate_material");
    }
    let scope = &target.scope;
    let raw: Option<String> = if scope.scope_kind == "public" {
        ensure!(composer.is_none(), "native_lifecycle.scope_invalid");
        connection.query_row("SELECT source_attachments_json FROM camp_composer_draft WHERE camp_id=?1 AND client_id=?2",params![scope.thread_id,client.id()],|row|row.get(0)).optional()?
    } else {
        let conversation = scope
            .conversation_id
            .as_deref()
            .context("native_lifecycle.conversation_required")?;
        if let Some(composer) = composer {
            ensure!(
                scope.scope_kind == "single_chat"
                    && composer.matches_current_pin(connection, &scope.thread_id, conversation)?,
                "native_lifecycle.composer_scope_changed"
            )
        }
        let key = PrivateComposerKey::for_scope(composer)?;
        connection.query_row("SELECT source_attachments_json FROM single_chat_composer_draft WHERE conversation_id=?1 AND client_id=?2 AND scope_key=?3",params![conversation,client.id(),key.sql_key()],|row|row.get(0)).optional()?
    };
    let sources: Vec<LocalAttachmentSourceRef> = match raw {
        Some(raw) => serde_json::from_str(&raw)?,
        None => Vec::new(),
    };
    let mut result = Vec::new();
    for id in ids {
        if let Some(source) = sources.iter().find(|source| source.id == *id) {
            ensure!(
                source.kind == crate::local_attachment_source::LocalAttachmentKind::File,
                "native_lifecycle.only_file_material_supported"
            );
            result.push(Candidate {
                original_id: id.clone(),
                source: PathBuf::from(&source.source_path),
                display_name: source.display_name.clone(),
            });
            continue;
        }
        ensure!(
            scope.scope_kind == "public",
            "native_lifecycle.selected_material_missing"
        );
        if let Some(source) =
            crate::web_upload::owned_public_source(connection, client, &scope.thread_id, id)?
        {
            result.push(Candidate {
                original_id: id.clone(),
                source: source.source_path.into(),
                display_name: source.display_name,
            });
            continue;
        }
        let row:Option<(String,String)>=connection.query_row("SELECT storage_path,display_name FROM prepared_attachment WHERE id=?1 AND camp_id=?2 AND client_id=?3 AND state='ready'",params![id,scope.thread_id,client.id()],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
        let (path, name) = row.context("native_lifecycle.selected_material_missing")?;
        result.push(Candidate {
            original_id: id.clone(),
            source: path.into(),
            display_name: name,
        });
    }
    Ok(result)
}
pub(crate) struct FrozenMaterial {
    pub descriptor: NativeSteerMaterialV1,
    pub relative_path: String,
    pub mime: String,
    pub original_id: String,
}
pub(crate) fn freeze(root: &Path, candidate: &Candidate) -> Result<FrozenMaterial> {
    ensure!(
        candidate.source.is_absolute(),
        "native_lifecycle.material_path_invalid"
    );
    let source =
        crate::local_attachment_snapshot::open_source_without_following(&candidate.source)?;
    let metadata = source.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= 20 * 1024 * 1024,
        "native_lifecycle.material_too_large"
    );
    drop(source);
    let id = uuid::Uuid::new_v4().to_string();
    let relative_path = format!("native-lifecycle/materials/{id}/content");
    let path = root.join(&relative_path);
    crate::platform::private_storage::prepare_private_directory(
        path.parent()
            .context("native_lifecycle.material_parent_missing")?,
    )?;
    let snapshot = crate::local_attachment_snapshot::copy_and_inspect(&candidate.source, &path)?;
    ensure!(
        snapshot.kind == "file" && snapshot.byte_size <= 20 * 1024 * 1024,
        "native_lifecycle.material_too_large"
    );
    let kind = if snapshot.preview_kind == "image"
        && ["image/png", "image/jpeg", "image/webp", "image/gif"]
            .contains(&snapshot.media_type.as_str())
    {
        "image"
    } else {
        "file"
    };
    let descriptor = NativeSteerMaterialV1 {
        source_id: id,
        snapshot_digest: snapshot.content_digest,
        display_name: candidate.display_name.chars().take(512).collect(),
        kind: kind.into(),
        byte_size: snapshot.byte_size,
    };
    validate_shape("NativeSteerMaterialV1", &serde_json::to_value(&descriptor)?)?;
    Ok(FrozenMaterial {
        descriptor,
        relative_path,
        mime: snapshot.media_type,
        original_id: candidate.original_id.clone(),
    })
}
pub(crate) fn register(
    connection: &Connection,
    target: &NativeSteerTargetV1,
    client: &DraftClient,
    value: &FrozenMaterial,
) -> Result<()> {
    super::target::require(connection, target)?;
    validate_shape(
        "NativeSteerMaterialV1",
        &serde_json::to_value(&value.descriptor)?,
    )?;
    connection.execute("INSERT INTO native_lifecycle_material(source_id,owner_thread_id,conversation_id,scope_kind,client_id,original_id,value_json,relative_path,media_type,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![value.descriptor.source_id,target.scope.thread_id,target.scope.conversation_id,target.scope.scope_kind,client.id(),value.original_id,serde_json::to_string(&value.descriptor)?,value.relative_path,value.mime,chrono::Utc::now().to_rfc3339()])?;
    Ok(())
}
pub(crate) fn require(
    connection: &Connection,
    scope: &NativeSteerScopeV1,
    client: &DraftClient,
    descriptor: &NativeSteerMaterialV1,
) -> Result<(String, String)> {
    validate_shape("NativeSteerMaterialV1", &serde_json::to_value(descriptor)?)?;
    let(raw,path,mime):(String,String,String)=connection.query_row("SELECT value_json,relative_path,media_type FROM native_lifecycle_material WHERE source_id=?1 AND owner_thread_id=?2 AND conversation_id IS ?3 AND scope_kind=?4 AND client_id=?5",params![descriptor.source_id,scope.thread_id,scope.conversation_id,scope.scope_kind,client.id()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).context("native_lifecycle.material_scope_changed")?;
    ensure!(
        serde_json::from_str::<NativeSteerMaterialV1>(&raw)? == *descriptor,
        "native_lifecycle.material_snapshot_changed"
    );
    ensure!(
        path == format!(
            "native-lifecycle/materials/{}/content",
            descriptor.source_id
        ),
        "native_lifecycle.material_path_changed"
    );
    Ok((path, mime))
}
pub(crate) fn blocks(
    root: &Path,
    input: &NativeSteerInputV1,
    locations: &[(String, String)],
) -> Result<Vec<Value>> {
    input.validate()?;
    ensure!(
        input.materials.len() == locations.len(),
        "native_lifecycle.material_snapshot_changed"
    );
    let mut blocks = Vec::new();
    if !input.text.is_empty() {
        blocks.push(json!({"type":"text","text":input.text}));
    }
    use base64::Engine;
    for (descriptor, (relative, mime)) in input.materials.iter().zip(locations) {
        ensure!(
            *relative
                == format!(
                    "native-lifecycle/materials/{}/content",
                    descriptor.source_id
                ),
            "native_lifecycle.material_path_changed"
        );
        let path = root.join(relative);
        let mut file = crate::local_attachment_snapshot::open_source_without_following(&path)?;
        let metadata = file.metadata()?;
        ensure!(
            metadata.is_file()
                && metadata.len() == descriptor.byte_size
                && metadata.len() <= 20 * 1024 * 1024,
            "native_lifecycle.material_snapshot_changed"
        );
        let mut bytes = Vec::new();
        file.by_ref()
            .take(20 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 == descriptor.byte_size
                && format!("sha256:{:x}", Sha256::digest(&bytes)) == descriptor.snapshot_digest,
            "native_lifecycle.material_snapshot_changed"
        );
        let uri = format!("rovai-material://{}", descriptor.source_id);
        blocks.push(if descriptor.kind=="image"{json!({"type":"image","data":base64::engine::general_purpose::STANDARD.encode(bytes),"mimeType":mime,"uri":uri})}
            else if descriptor.kind=="text_reference"{json!({"type":"resource","resource":{"uri":uri,"mimeType":mime,"text":String::from_utf8(bytes)?}})}
            else{json!({"type":"resource","resource":{"uri":uri,"mimeType":mime,"blob":base64::engine::general_purpose::STANDARD.encode(bytes)}})});
    }
    ensure!(
        serde_json::to_vec(&blocks)?.len() <= 64 * 1024 * 1024,
        "native_lifecycle.native_request_too_large"
    );
    Ok(blocks)
}
