//! Host temporary-file ingress into the existing source-reference Draft. There
//! is no new asset lifetime: after binding, Core never deletes the source file.
use crate::{
    command::*, db::Database, draft_client::DraftClient,
    local_attachment_source::LocalAttachmentSourceRef,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::Read;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeUploadPreparation {
    pub command_id: String,
    #[serde(rename = "threadId")]
    pub camp_id: String,
    pub expected_revision: i64,
    pub target: UploadTarget,
    pub source_path: String,
    pub display_name: String,
    pub media_type: Option<String>,
}

// Host 私有入口只观察本机文件，不写草稿、不登记 scope、不执行上传。
pub fn observe_native(
    client: &DraftClient,
    params: NativeUploadPreparation,
) -> Result<serde_json::Value> {
    ensure!(
        client.is_desktop(),
        "native upload requires the desktop editor"
    );
    ensure!(
        !matches!(params.target, UploadTarget::Camp),
        "native upload requires a private target"
    );
    crate::camp_id::ThreadId::parse(&params.camp_id)?;
    let id = uuid::Uuid::parse_str(&params.command_id)?;
    ensure!(
        id.hyphenated().to_string() == params.command_id,
        "invalid upload identity"
    );
    ensure!(params.expected_revision >= 0, "invalid upload revision");
    let path = std::path::Path::new(&params.source_path);
    ensure!(path.is_absolute(), "upload path must be absolute");
    let file = crate::local_attachment_snapshot::open_source_without_following(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file(),
        "private upload directory observation is not available"
    );
    let byte_size = metadata.len();
    let source = LocalAttachmentSourceRef {
        id: params.command_id.clone(),
        source_path: params.source_path,
        display_name: crate::local_attachment_snapshot::normalize_display_name(
            &params.display_name,
        )?,
        kind: crate::local_attachment_source::LocalAttachmentKind::File,
        media_type: params
            .media_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        observed_byte_size: Some(byte_size),
    };
    drop(file);
    let sha256 = observe_file_digest(&source, byte_size)?;
    let intent = UploadIntent {
        target: params.target,
        command_id: params.command_id,
        camp_id: params.camp_id,
        expected_revision: params.expected_revision,
        display_name: source.display_name.clone(),
        byte_size,
        sha256,
    };
    Ok(json!({"intent":intent,"source":source}))
}

fn observe_file_digest(source: &LocalAttachmentSourceRef, expected_size: u64) -> Result<String> {
    ensure!(
        source.kind == crate::local_attachment_source::LocalAttachmentKind::File,
        "upload requires a file"
    );
    let file = crate::local_attachment_snapshot::open_source_without_following(
        std::path::Path::new(&source.source_path),
    )?;
    ensure!(file.metadata()?.is_file(), "attachment_kind_changed");
    let mut reader = file.take(
        expected_size
            .checked_add(1)
            .context("invalid upload size")?,
    );
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let length = reader.read(&mut buffer).context("attachment_unreadable")?;
        if length == 0 {
            break;
        }
        count += length as u64;
        digest.update(&buffer[..length]);
    }
    ensure!(count == expected_size, "upload content changed");
    Ok(format!("{:x}", digest.finalize()))
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UploadIntent {
    #[serde(default, skip_serializing_if = "UploadTarget::is_camp")]
    pub target: UploadTarget,
    pub command_id: String,
    #[serde(rename = "threadId", alias = "campId")]
    pub camp_id: String,
    pub expected_revision: i64,
    pub display_name: String,
    pub byte_size: u64,
    pub sha256: String,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UploadTarget {
    #[default]
    Camp,
    SingleChat {
        #[serde(rename = "conversationId")]
        conversation_id: String,
        #[serde(
            rename = "composerScope",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        composer_scope: Option<crate::single_chat_composer_scope::SingleChatComposerScope>,
    },
    SingleChatPending {
        #[serde(rename = "conversationId")]
        conversation_id: String,
        #[serde(rename = "pendingInputId")]
        pending_input_id: String,
        #[serde(rename = "editToken")]
        edit_token: String,
        #[serde(
            rename = "composerScope",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        composer_scope: Option<crate::single_chat_composer_scope::SingleChatComposerScope>,
    },
}
impl UploadTarget {
    fn is_camp(&self) -> bool {
        matches!(self, Self::Camp)
    }
}

pub fn snapshot(
    database: &Database,
    _data_dir: &std::path::Path,
    client: &DraftClient,
    intent: &UploadIntent,
) -> Result<serde_json::Value> {
    match &intent.target {
        UploadTarget::Camp => {
            let recorded = reconcile(database, client, intent.clone())?
                .ok_or_else(|| anyhow::anyhow!("upload receipt is missing"))?;
            Ok(recorded.result.payload["source"].clone())
        }
        UploadTarget::SingleChat {
            conversation_id,
            composer_scope,
        }
        | UploadTarget::SingleChatPending {
            conversation_id,
            composer_scope,
            ..
        } => {
            let service = crate::single_chat::SingleChatService::for_client(client.clone());
            if let Some(scope) = composer_scope {
                let draft = crate::single_chat_composer::read_draft(
                    database,
                    client,
                    &intent.camp_id,
                    conversation_id,
                    Some(scope),
                )?;
                let key =
                    crate::single_chat_composer_scope::PrivateComposerKey::for_scope(Some(scope))?;
                let snapshot = service.snapshot_for_key(database, conversation_id, &key)?;
                let mut value = serde_json::to_value(snapshot)?;
                ensure!(!value.is_null(), "single_chat.not_found");
                value["draft"] = serde_json::to_value(draft)?;
                let editing_elsewhere: bool = database.connection().query_row(
                    "SELECT EXISTS(SELECT 1 FROM single_chat_pending_input_edit_session WHERE conversation_id=?1 AND (client_id<>?2 OR scope_key<>?3))",
                    rusqlite::params![conversation_id, client.id(), key.sql_key()], |row| row.get(0),
                )?;
                value["pendingInputs"]["editingElsewhere"] = json!(editing_elsewhere);
                return Ok(value);
            }
            let snapshot = service.snapshot(database, conversation_id)?;
            ensure!(
                snapshot
                    .as_ref()
                    .is_some_and(|view| view.conversation.camp_id == intent.camp_id),
                "single_chat.camp_mismatch"
            );
            Ok(serde_json::to_value(snapshot)?)
        }
    }
}

#[derive(Serialize)]
pub struct BindUpload {
    client: DraftClient,
    intent: UploadIntent,
}
impl sealed::Sealed for BindUpload {}
impl DomainCommand for BindUpload {
    const TYPE: &'static str = "camp.source_attachment.upload.bind";
    fn draft_client(&self) -> Option<&DraftClient> {
        Some(&self.client)
    }
}

fn envelope(client: &DraftClient, intent: UploadIntent) -> Result<CommandEnvelope<BindUpload>> {
    ensure!(
        !client.is_desktop() || !matches!(intent.target, UploadTarget::Camp),
        "public upload requires a verified Web editor"
    );
    crate::camp_id::ThreadId::parse(&intent.camp_id)?;
    uuid::Uuid::parse_str(&intent.command_id)?;
    ensure!(
        intent.sha256.len() == 64 && intent.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid upload digest"
    );
    Ok(CommandEnvelope {
        command_id: intent.command_id.clone(),
        actor: ActorRef::User {
            user_id: crate::current_user::CURRENT_USER_ID.into(),
        },
        camp_id: Some(intent.camp_id.clone()),
        expected_versions: Vec::new(),
        execution_epoch: None,
        payload: BindUpload {
            client: client.clone(),
            intent,
        },
    })
}

pub fn reconcile(
    database: &Database,
    client: &DraftClient,
    intent: UploadIntent,
) -> Result<Option<CommandExecution>> {
    DomainCommandGateway.replay_if_recorded(database, &envelope(client, intent)?)
}

pub fn bind(
    database: &mut Database,
    _data_dir: &std::path::Path,
    client: &DraftClient,
    intent: UploadIntent,
    source: LocalAttachmentSourceRef,
) -> Result<CommandExecution> {
    let envelope = envelope(client, intent)?;
    ensure!(
        source.id == envelope.payload.intent.command_id,
        "upload reference changed"
    );
    ensure!(
        source.display_name == envelope.payload.intent.display_name
            && source.observed_byte_size == Some(envelope.payload.intent.byte_size),
        "upload observation changed"
    );
    DomainCommandGateway.execute(database, &envelope, |transaction| {
        let intent = &envelope.payload.intent;
        if !matches!(intent.target, UploadTarget::Camp) {
            ensure!(observe_file_digest(&source, intent.byte_size)? == intent.sha256, "upload content changed");
        }
        match &intent.target {
            UploadTarget::Camp => {
                ensure!(
                    transaction.query_row(
                        "SELECT EXISTS(SELECT 1 FROM camp WHERE id = ?1)",
                        [&intent.camp_id],
                        |row| row.get::<_, bool>(0),
                    )?,
                    "camp.not_found"
                );
            }
            UploadTarget::SingleChat { composer_scope: Some(_), .. }
            | UploadTarget::SingleChatPending { composer_scope: Some(_), .. } => {
                crate::single_chat_composer::bind_uploaded_source(transaction, client, intent, source.clone())?;
            }
            UploadTarget::SingleChat { conversation_id, composer_scope: None } => {
                ensure!(transaction.query_row("SELECT EXISTS(SELECT 1 FROM conversation WHERE id=?1 AND camp_id=?2 AND kind='single_chat' AND ended_at IS NULL)", rusqlite::params![conversation_id,intent.camp_id], |r| r.get::<_, bool>(0))?, "single_chat.camp_mismatch");
                crate::single_chat::SingleChatService::for_client(client.clone()).commit_source_attachment_in_transaction(transaction, conversation_id, intent.expected_revision, source.clone())?;
            }
            UploadTarget::SingleChatPending { conversation_id, pending_input_id, edit_token, composer_scope: None } => crate::single_chat::SingleChatService::for_client(client.clone()).commit_pending_source_attachment_in_transaction(transaction, &intent.camp_id, conversation_id, pending_input_id, intent.expected_revision, edit_token, source.clone())?,
        }
        if let UploadTarget::SingleChat { conversation_id, .. } | UploadTarget::SingleChatPending { conversation_id, .. } = &intent.target {
            crate::private_source_observation::capture_upload(transaction,conversation_id,&source,&intent.sha256)?;
        }
        let payload = if matches!(intent.target, UploadTarget::Camp) {
            json!({"attachmentRefId":intent.command_id, "source":source})
        } else {
            json!({"attachmentRefId":intent.command_id, "draftId":client.draft_id(&intent.camp_id), "revision":intent.expected_revision+1})
        };
        Ok(CommandHandlerResult::applied("attachment.upload_bound", payload, None))
    })
}
