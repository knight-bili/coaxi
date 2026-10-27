//! 原程序明确未消耗后，由用户另行确认整份普通输入；不与原生通道竞争。
use super::*;
use crate::{
    command::{CommandHandlerResult, CommandResultStatus, DomainCommand, sealed},
    draft_client::DraftClient,
    local_attachment_source::{LocalAttachmentKind, LocalAttachmentSourceRef},
    single_chat::SendSingleChatMessageCommand,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reference {
    pub source_command_id: String,
    pub preview_id: String,
    pub preview_digest: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub reference: Reference,
    pub client: DraftClient,
    pub scope: NativeSteerScopeV1,
    pub agent_id: String,
    pub input: NativeSteerInputV1,
    pub sources: Vec<LocalAttachmentSourceRef>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicCommand {
    pub native_input: Input,
    pub message: crate::collaboration::SendUserAutomationThreadMessageCommand,
}
impl sealed::Sealed for PublicCommand {}
impl DomainCommand for PublicCommand {
    const TYPE: &'static str = "camp.native_input.continue";
    const EXACT_REPLAY: bool = true;
    fn draft_client(&self) -> Option<&DraftClient> {
        Some(&self.native_input.client)
    }
    fn native_input_continuation(&self) -> Option<Input> {
        Some(self.native_input.clone())
    }
}
macro_rules! private_command {
    ($name:ident,$owner:ident,$command_type:literal) => {
        #[derive(Debug, Clone, Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            pub native_input: Input,
            pub message: SendSingleChatMessageCommand,
        }
        impl sealed::Sealed for $name {}
        impl DomainCommand for $name {
            const TYPE: &'static str = $command_type;
            const EXACT_REPLAY: bool = true;
            fn draft_client(&self) -> Option<&DraftClient> {
                Some(&self.message.draft_client)
            }
            fn native_input_continuation(&self) -> Option<Input> {
                Some(self.native_input.clone())
            }
        }
        impl crate::single_chat::PrivateSendCommand for $name {
            const OWNER: crate::single_chat_composer::ComposerOwner =
                crate::single_chat_composer::ComposerOwner::$owner;
            fn input(&self) -> SendSingleChatMessageCommand {
                self.message.clone()
            }
        }
    };
}
private_command!(
    SingleCommand,
    SingleChat,
    "single_chat.native_input.continue"
);
private_command!(
    ChildCommand,
    DelegationChild,
    "delegations.native_input.continue"
);

pub(crate) fn eligible(
    connection: &Connection,
    source_command: &str,
    client: &str,
) -> Result<Value> {
    uuid::Uuid::parse_str(source_command)?;
    let(raw,preview):(String,String)=connection.query_row("SELECT i.value_json,p.value_json FROM native_lifecycle_intent i JOIN native_lifecycle_preview p ON p.id=i.preview_id WHERE i.command_id=?1 AND i.client_id=?2 AND i.kind='steer' AND i.state='not_consumed'",params![source_command,client],|row|Ok((row.get(0)?,row.get(1)?))).context("native_lifecycle.normal_not_eligible")?;
    let receipt: Value = serde_json::from_str(&raw)?;
    ensure!(
        receipt["status"] == "not_consumed" && receipt["nativeOutcome"] == "promptRequired",
        "native_lifecycle.normal_not_eligible"
    );
    let consumed:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM native_lifecycle_normal_continuation WHERE source_command_id=?1)",[source_command],|row|row.get(0))?;
    ensure!(!consumed, "native_lifecycle.normal_already_continued");
    let preview: Value = serde_json::from_str(&preview)?;
    validate_shape("NativeSteerPreviewV1", &preview)?;
    let scope: NativeSteerScopeV1 = serde_json::from_value(preview["target"]["scope"].clone())?;
    crate::ui_feedback::storage::scope(
        connection,
        &serde_json::from_value(serde_json::to_value(&scope)?)?,
        preview["target"]["agentId"]
            .as_str()
            .context("native_lifecycle.member_missing")?,
    )?;
    Ok(preview)
}
pub(crate) fn preview(
    connection: &mut Connection,
    source_command: &str,
    client: &DraftClient,
) -> Result<Value> {
    let tx = connection.transaction()?;
    let original = eligible(&tx, source_command, client.id())?;
    let now = chrono::Utc::now();
    let expires = now + chrono::Duration::minutes(10);
    let id = uuid::Uuid::new_v4().to_string();
    let mut value = json!({"schemaVersion":1,"previewId":id,"previewDigest":"","sourceCommandId":source_command,"scope":original["target"]["scope"],"agentId":original["target"]["agentId"],"input":original["input"],"inputDigest":original["inputDigest"],"expiresAt":expires.to_rfc3339()});
    value["previewDigest"] = json!(digest(&value)?);
    tx.execute("INSERT INTO native_lifecycle_normal_preview(id,source_command_id,client_id,digest,value_json,expires_at_ms,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,source_command,client.id(),value["previewDigest"].as_str(),serde_json::to_string(&value)?,expires.timestamp_millis(),now.to_rfc3339()])?;
    tx.commit()?;
    Ok(value)
}
pub(crate) fn resolve(
    connection: &Connection,
    reference: &Reference,
    client: &DraftClient,
) -> Result<Value> {
    let original = eligible(connection, &reference.source_command_id, client.id())?;
    let(raw,expires):(String,i64)=connection.query_row("SELECT value_json,expires_at_ms FROM native_lifecycle_normal_preview WHERE id=?1 AND source_command_id=?2 AND client_id=?3 AND digest=?4",params![reference.preview_id,reference.source_command_id,client.id(),reference.preview_digest],|row|Ok((row.get(0)?,row.get(1)?)))?;
    ensure!(
        chrono::Utc::now().timestamp_millis() < expires,
        "native_lifecycle.normal_preview_expired"
    );
    let value: Value = serde_json::from_str(&raw)?;
    let mut basis = value.clone();
    basis["previewDigest"] = json!("");
    ensure!(
        digest(&basis)? == reference.preview_digest
            && value["input"] == original["input"]
            && value["inputDigest"] == original["inputDigest"],
        "native_lifecycle.normal_preview_changed"
    );
    Ok(value)
}
pub(crate) fn input(
    connection: &Connection,
    root: &Path,
    reference: Reference,
    client: DraftClient,
) -> Result<Input> {
    let value = resolve(connection, &reference, &client)?;
    let scope: NativeSteerScopeV1 = serde_json::from_value(value["scope"].clone())?;
    let input: NativeSteerInputV1 = serde_json::from_value(value["input"].clone())?;
    input.validate()?;
    let sources = input
        .materials
        .iter()
        .map(|descriptor| {
            let (relative, mime) = material::require(connection, &scope, &client, descriptor)?;
            Ok(LocalAttachmentSourceRef {
                id: descriptor.source_id.clone(),
                source_path: root.join(relative).to_string_lossy().into_owned(),
                display_name: descriptor.display_name.clone(),
                kind: LocalAttachmentKind::File,
                media_type: Some(mime),
                observed_byte_size: Some(descriptor.byte_size),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Input {
        reference,
        client,
        scope,
        agent_id: value["agentId"]
            .as_str()
            .context("native_lifecycle.member_missing")?
            .into(),
        input,
        sources,
    })
}
pub(crate) fn validate(connection: &Connection, input: &Input) -> Result<()> {
    let value = resolve(connection, &input.reference, &input.client)?;
    ensure!(
        value["scope"] == serde_json::to_value(&input.scope)?
            && value["agentId"] == input.agent_id
            && value["input"] == serde_json::to_value(&input.input)?
            && input.sources.len() == input.input.materials.len(),
        "native_lifecycle.normal_input_changed"
    );
    for (source, descriptor) in input.sources.iter().zip(&input.input.materials) {
        let (relative, mime) =
            material::require(connection, &input.scope, &input.client, descriptor)?;
        ensure!(
            source.id == descriptor.source_id
                && source.display_name == descriptor.display_name
                && source.kind == LocalAttachmentKind::File
                && source.media_type.as_deref() == Some(mime.as_str())
                && source.observed_byte_size == Some(descriptor.byte_size)
                && Path::new(&source.source_path).is_absolute()
                && Path::new(&source.source_path).ends_with(Path::new(&relative)),
            "native_lifecycle.normal_material_changed"
        );
    }
    crate::local_attachment_source::validate_source_attachments(&input.sources)?;
    Ok(())
}
pub(crate) fn complete(
    tx: &Transaction<'_>,
    input: &Input,
    command: &str,
    result: &CommandHandlerResult,
) -> Result<()> {
    if result.status == CommandResultStatus::Rejected {
        return Ok(());
    }
    validate(tx, input)?;
    ensure!(
        [
            "threadMessageId",
            "campMessageId",
            "conversationMessageId",
            "pendingInputId"
        ]
        .iter()
        .any(|key| result.payload[*key].as_str().is_some()),
        "native_lifecycle.normal_result_missing"
    );
    tx.execute("INSERT INTO native_lifecycle_normal_continuation(source_command_id,command_id,preview_id,client_id,result_json,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![input.reference.source_command_id,command,input.reference.preview_id,input.client.id(),serde_json::to_string(result)?,chrono::Utc::now().to_rfc3339()])?;
    Ok(())
}
pub(crate) fn result(
    connection: &Connection,
    command: &str,
    client: &str,
) -> Result<Option<Value>> {
    let raw:Option<String>=connection.query_row("SELECT result_json FROM native_lifecycle_normal_continuation WHERE command_id=?1 AND client_id=?2",params![command,client],|row|row.get(0)).optional()?;
    raw.map(|raw| serde_json::from_str(&raw).map_err(Into::into))
        .transpose()
}
pub(crate) fn receipt(
    connection: &Connection,
    command: &str,
    client: &str,
    instance: &str,
) -> Result<Value> {
    uuid::Uuid::parse_str(command)?;
    let row:Option<(String,String,String,Option<String>)>=connection.query_row("SELECT state,client_id,core_instance_id,error FROM native_lifecycle_normal_request WHERE command_id=?1",[command],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    let Some((state, owner, core, error)) = row else {
        return Ok(
            json!({"schemaVersion":1,"commandId":command,"state":"unknown","commandResult":null,"error":null}),
        );
    };
    ensure!(
        owner == client,
        "native_lifecycle.normal_command_scope_changed"
    );
    let owned:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM event_log WHERE event_type='command.result' AND command_id=?1 AND command_client_id=?2 AND actor_type='user' AND actor_id='local_user' AND command_type IN ('camp.native_input.continue','single_chat.native_input.continue','delegations.native_input.continue'))",params![command,client],|row|row.get(0))?;
    if owned {
        return Ok(
            json!({"schemaVersion":1,"commandId":command,"state":"recorded","commandResult":crate::command::load_stored_result(connection,command)?,"error":null}),
        );
    }
    let state = if state == "prepared" && (core != instance || !steer_delivery::active(command)) {
        "unknown"
    } else {
        state.as_str()
    };
    Ok(
        json!({"schemaVersion":1,"commandId":command,"state":state,"commandResult":null,"error":error}),
    )
}
