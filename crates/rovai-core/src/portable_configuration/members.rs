use super::*;
use crate::{
    command::{
        ActorRef, CommandEnvelope, CommandExecution, CommandHandlerResult, DomainCommand,
        DomainCommandGateway, sealed,
    },
    draft_client::DraftClient,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

pub(crate) fn get(connection: &Connection, member: &str) -> Result<Value> {
    let(version,kind,installation,model):(i64,Option<String>,Option<String>,Option<String>)=connection.query_row("SELECT version,selected_runtime_adapter_kind,default_runtime_installation_id,default_model_selection_json FROM agent_profile WHERE id=?1 AND profile_status<>'removed'",[member],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).context("configuration.member_missing")?;
    let stored: Option<(i64, String)> = connection
        .query_row(
            "SELECT revision,reference_json FROM portable_member_connection WHERE agent_id=?1",
            [member],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let binding = stored
        .map(|(revision, reference)| -> Result<_> {
            let reference: Option<PortableConnectionRefV1> = serde_json::from_str(&reference)?;
            Ok(PortableMemberBindingV1 {
                schema_version: 1,
                agent_id: member.into(),
                revision,
                connection: reference,
            })
        })
        .transpose()?;
    let generation = installation
        .as_deref()
        .map(|id| {
            connection
                .query_row(
                    "SELECT generation FROM adapter_installation WHERE id=?1 AND enabled=1",
                    [id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
        })
        .transpose()?
        .flatten();
    let runtime = match (kind, installation, generation, model) {
        (Some(kind), Some(id), Some(generation), Some(model)) => Some(
            json!({"adapterKind":kind,"installationId":id,"installationGeneration":generation,"modelSelection":serde_json::from_str::<Value>(&model)?}),
        ),
        _ => None,
    };
    Ok(json!({"agentId":member,"agentVersion":version,"binding":binding,"runtime":runtime}))
}

pub(crate) fn set_on(
    tx: &Transaction<'_>,
    member: &str,
    expected: i64,
    reference: Option<&PortableConnectionRefV1>,
) -> Result<Value> {
    let(version,kind):(i64,Option<String>)=tx.query_row("SELECT version,selected_runtime_adapter_kind FROM agent_profile WHERE id=?1 AND profile_status='present'",[member],|row|Ok((row.get(0)?,row.get(1)?))).context("configuration.member_missing")?;
    ensure!(version == expected, "configuration.member_changed");
    if let Some(reference) = reference {
        let provider = providers::resolve(tx, reference, true)?;
        let expected_kind = match provider["agentType"].as_str() {
            Some("claude_code") => "claude-code-cli",
            Some("codex") => "codex-cli",
            Some("gemini") => "gemini-cli",
            _ => anyhow::bail!("configuration.agent_type_unsupported"),
        };
        ensure!(
            kind.as_deref() == Some(expected_kind),
            "configuration.member_runtime_kind_mismatch"
        );
    }
    let previous: i64 = tx.query_row(
        "SELECT COALESCE((SELECT revision FROM portable_member_connection WHERE agent_id=?1),0)",
        [member],
        |row| row.get(0),
    )?;
    let revision = previous
        .checked_add(1)
        .context("configuration.revision_exhausted")?;
    let now = chrono::Utc::now().to_rfc3339();
    ensure!(tx.execute("UPDATE agent_profile SET version=version+1,updated_at=?3 WHERE id=?1 AND version=?2 AND profile_status='present'",params![member,expected,now])?==1,"configuration.member_changed");
    tx.execute("INSERT INTO portable_member_connection(agent_id,revision,reference_json,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(agent_id) DO UPDATE SET revision=excluded.revision,reference_json=excluded.reference_json,environment_json='[]',updated_at=excluded.updated_at",params![member,revision,serde_json::to_string(&reference)?,now])?;
    let result = get(tx, member)?;
    crate::collaboration::append_domain_event(
        tx,
        "portableConfiguration.memberChanged",
        None,
        Some(("agent_profile", member)),
        &ActorRef::User {
            user_id: "local_user".into(),
        },
        None,
        &json!({"agentId":member,"version":expected+1}),
    )?;
    Ok(result)
}

fn scope_id(
    member: &str,
    reference: &PortableConnectionRefV1,
    platform: &str,
    environment: &[FrozenPortableEnvironmentV1],
    runtime: &crate::agent_profile::FrozenAgentRuntimeConfig,
) -> Result<String> {
    use sha2::{Digest, Sha256};
    let bytes = Sha256::digest(serde_json::to_vec(
        &json!({"scope":"portable-native-v1","member":member,"reference":reference,"platform":platform,"environment":environment,"installation":runtime.installation_id,"generation":runtime.installation_generation}),
    )?);
    let mut id = [0u8; 16];
    id.copy_from_slice(&bytes[..16]);
    id[6] = (id[6] & 0x0f) | 0x80;
    id[8] = (id[8] & 0x3f) | 0x80;
    Ok(uuid::Uuid::from_bytes(id).to_string())
}

pub(crate) fn selected(connection: &Connection, member: &str) -> Result<bool> {
    if storage::absent(connection)? {
        return Ok(false);
    }
    let stored: Option<String> = connection
        .query_row(
            "SELECT reference_json FROM portable_member_connection WHERE agent_id=?1",
            [member],
            |row| row.get(0),
        )
        .optional()?;
    Ok(stored
        .map(|value| serde_json::from_str::<Option<PortableConnectionRefV1>>(&value))
        .transpose()?
        .flatten()
        .is_some())
}

/// 仅新 Run 冻结调用；已保存的 Run 必须读取自己的 FrozenPortableConnectionV1。
pub(crate) fn freeze_for_member(
    connection: &Connection,
    member: &str,
    runtime: &crate::agent_profile::FrozenAgentRuntimeConfig,
) -> Result<Option<FrozenPortableConnectionV1>> {
    if storage::absent(connection)? {
        return Ok(None);
    }
    let stored:Option<(String,String)>=connection.query_row("SELECT reference_json,environment_json FROM portable_member_connection WHERE agent_id=?1",[member],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
    let Some((reference, environment)) = stored else {
        return Ok(None);
    };
    let Some(reference): Option<PortableConnectionRefV1> = serde_json::from_str(&reference)? else {
        return Ok(None);
    };
    let enabled:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM portable_provider_head h JOIN portable_provider_revision r ON r.id=h.id AND r.revision=h.revision WHERE h.id=?1 AND h.removed=0 AND json_extract(r.value_json,'$.enabled')=1)",[&reference.id],|row|row.get(0))?;
    ensure!(enabled, "configuration.provider_not_selectable");
    let provider = providers::resolve(connection, &reference, false)?;
    let mut environment: Vec<FrozenPortableEnvironmentV1> = serde_json::from_str(&environment)?;
    environment.sort_by(|left, right| left.name.cmp(&right.name));
    let platform = crate::custom_runtime::installer::platform()?.to_owned();
    let credential: Option<SecretRefV1> = serde_json::from_value(provider["credential"].clone())?;
    if let Some(reference) = &credential {
        let _ = secrets::retained(connection, reference, "provider_key")?;
    }
    for entry in &environment {
        let _ = secrets::retained(connection, &entry.secret, "member_environment")?;
    }
    let native_scope = scope_id(member, &reference, &platform, &environment, runtime)?;
    let mut frozen = FrozenPortableConnectionV1 {
        schema_version: 1,
        reference: reference.clone(),
        projection_version: 1,
        agent_type: provider["agentType"]
            .as_str()
            .context("configuration.agent_type_required")?
            .into(),
        api_url: provider["apiUrl"]
            .as_str()
            .context("configuration.api_url_required")?
            .into(),
        credential,
        model: provider["model"].clone(),
        environment_digest: digest(&serde_json::to_value(&environment)?)?,
        catalog_digest: digest(&provider["model"])?,
        environment,
        projection_digest: String::new(),
        installation_id: runtime.installation_id.clone(),
        installation_generation: runtime.installation_generation,
        host_platform: platform.clone(),
        native_scope_id: native_scope,
    };
    frozen.projection_digest = digest(&serde_json::to_value(&frozen)?)?;
    frozen.validate()?;
    ensure!(
        frozen.adapter_kind()? == runtime.adapter_kind,
        "configuration.member_runtime_kind_mismatch"
    );
    Ok(Some(frozen))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SetConnection {
    #[serde(default, skip_deserializing)]
    pub client: DraftClient,
    pub schema_version: u32,
    pub agent_id: String,
    pub expected_version: i64,
    pub connection: Option<PortableConnectionRefV1>,
}
impl sealed::Sealed for SetConnection {}
impl DomainCommand for SetConnection {
    const TYPE: &'static str = "configuration.memberConnection.set";
    const EXACT_REPLAY: bool = true;
    fn draft_client(&self) -> Option<&DraftClient> {
        Some(&self.client)
    }
}
pub(crate) fn set(
    database: &mut crate::db::Database,
    envelope: &CommandEnvelope<SetConnection>,
) -> Result<CommandExecution> {
    DomainCommandGateway.execute(database,envelope,|tx|{
        ensure!(matches!(&envelope.actor,ActorRef::User{..})&&envelope.camp_id.is_none(),"configuration.user_required");
        let input=&envelope.payload;
        validate_operation(SetConnection::TYPE,&json!({"schemaVersion":input.schema_version,"commandId":envelope.command_id,"agentId":input.agent_id,"expectedVersion":input.expected_version,"connection":input.connection}))?;
        Ok(CommandHandlerResult::applied("configuration.member_connection_saved",set_on(tx,&input.agent_id,input.expected_version,input.connection.as_ref())?,None))
    })
}
