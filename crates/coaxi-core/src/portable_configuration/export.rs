//! 完整格式2的单次快照；只有明确包含秘密时读取确切引用，不导出登录态或安装锁。
use super::*;
use rusqlite::Connection;
use std::{collections::BTreeMap, path::Path};

struct Secrets {
    included: bool,
    rows: Vec<Value>,
    known: BTreeMap<String, String>,
}
impl Secrets {
    fn add(&mut self, key: String, value: String) -> Result<String> {
        if let Some(id) = self.known.get(&key) {
            return Ok(id.clone());
        }
        ensure!(
            self.included && self.rows.len() < 2048 && value.len() <= 1024 * 1024,
            "configuration.export_secret_limit"
        );
        let id = uuid::Uuid::new_v4().to_string();
        self.rows.push(json!({"archiveSecretId":id,"value":value}));
        self.known.insert(key, id.clone());
        Ok(id)
    }
    fn reference(
        &mut self,
        connection: &Connection,
        value: &Value,
        purpose: &str,
    ) -> Result<Option<String>> {
        if !self.included || value.is_null() {
            return Ok(None);
        }
        let reference: SecretRefV1 = serde_json::from_value(value.clone())?;
        self.add(
            format!("{purpose}:{}:{}", reference.id, reference.revision),
            secrets::retained(connection, &reference, purpose)?,
        )
        .map(Some)
    }
}
fn ids(connection: &Connection, sql: &str) -> Result<Vec<String>> {
    let mut query = connection.prepare(sql)?;
    let values = query
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(values.len() <= 8192, "configuration.export_entry_limit");
    Ok(values)
}
fn model_text(value: &Value) -> Result<Option<String>> {
    if value["kind"] == "gemini" {
        return Ok(value["model"].as_str().map(str::to_owned));
    }
    let mut model = value.clone();
    model
        .as_object_mut()
        .context("configuration.model_invalid")?
        .remove("kind");
    Ok(Some(serde_json::to_string(&model)?))
}
/// 自动同步比较实际所有者修订，不受每次导出的时间、归档编号和秘密别名影响。
pub(crate) fn configuration_digest(
    connection: &Connection,
    root: &Path,
    include_secrets: bool,
) -> Result<String> {
    let mut revisions = Vec::new();
    for id in ids(
        connection,
        "SELECT id FROM portable_provider_head WHERE removed=0 ORDER BY id",
    )? {
        let value=connection.query_row("SELECT h.revision,r.digest FROM portable_provider_head h JOIN portable_provider_revision r ON r.id=h.id AND r.revision=h.revision WHERE h.id=?1",[&id],|row|Ok(json!({"id":id,"revision":row.get::<_,i64>(0)?,"digest":row.get::<_,String>(1)?})))?;
        revisions.push(value);
    }
    let mut customs = Vec::new();
    for id in ids(
        connection,
        "SELECT id FROM custom_runtime_head WHERE removed=0 ORDER BY id",
    )? {
        customs.push(
            crate::custom_runtime::storage::latest(connection, &id)?
                .context("configuration.custom_definition_missing")?,
        );
    }
    let mut values = serde_json::Map::new();
    for target in [
        "appearance",
        "general",
        "windowClose",
        "logging",
        "messageLibrary",
        "taskTemplates",
        "agentSettings",
    ] {
        if target == "windowClose" && !cfg!(windows) {
            continue;
        }
        values.insert(target.into(), files::read(root, target)?);
    }
    digest(
        &json!({"providers":revisions,"customAgents":customs,"files":values,"sourceMemberMappings":member_mappings(connection)?,"channelPreferences":channel_preferences::portable(root)?,"delegations":crate::delegation_settings::read(connection)?,"conversationTools":policies::basis(connection,"conversationToolsPolicy")?["value"],"includeSecrets":include_secrets}),
    )
}
pub(crate) fn cache_for_sync(
    root: &Path,
    snapshot: &Value,
    id: &str,
    passphrase: &str,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<std::path::PathBuf> {
    uuid::Uuid::parse_str(id)?;
    ensure!(
        !cancel.is_cancelled() && passphrase.chars().count() >= 8,
        "configuration.export_cancelled_or_passphrase_invalid"
    );
    let directory = root.join("config-sync-cache").join(id);
    let path = directory.join("configuration.coaxi-config");
    ensure!(!path.exists(), "configuration.export_original_cache_exists");
    let bytes = crate::backup::crypto::encrypt_bytes(
        &serde_json::to_vec(snapshot)?,
        passphrase,
        MAX_ARCHIVE,
    )?;
    ensure!(!cancel.is_cancelled(), "configuration.export_cancelled");
    crate::platform::private_storage::prepare_private_directory(&directory)?;
    crate::platform::private_storage::atomic_write_private_bytes(&path, &bytes)?;
    Ok(path)
}
pub(crate) fn collect(
    connection: &Connection,
    root: &Path,
    include_secrets: bool,
) -> Result<Value> {
    let mut secrets = Secrets {
        included: include_secrets,
        rows: Vec::new(),
        known: BTreeMap::new(),
    };
    let mut domains = BTreeMap::<String, Value>::new();
    let mut providers = Vec::new();
    for id in ids(
        connection,
        "SELECT id FROM portable_provider_head WHERE removed=0 ORDER BY id",
    )? {
        let reference=connection.query_row("SELECT h.revision,r.digest FROM portable_provider_head h JOIN portable_provider_revision r ON r.id=h.id AND r.revision=h.revision WHERE h.id=?1",[&id],|row|Ok(PortableConnectionRefV1{id:id.clone(),revision:row.get(0)?,digest:row.get(1)?}))?;
        let value = providers::resolve(connection, &reference, false)?;
        let mut url = reqwest::Url::parse(
            value["apiUrl"]
                .as_str()
                .context("configuration.provider_url_required")?,
        )?;
        let query_requires_secret = !include_secrets && url.query().is_some();
        if query_requires_secret {
            url.set_query(None);
        }
        let secret = secrets.reference(connection, &value["credential"], "provider_key")?;
        providers.push(json!({"name":value["name"],"agentType":value["agentType"],"apiUrl":url.to_string(),"apiKey":"","agentTypesJson":value["rawAgentTypesJson"],"model":model_text(&value["model"])?,"enabled":value["enabled"],"archiveCredentialId":secret,"credentialRequired":!value["credential"].is_null(),"queryRequiresSecret":query_requires_secret,"sourceProviderId":id,"originalRawModel":value["rawModel"]}));
    }
    domains.insert("modelProviders".into(), json!(providers));
    let mut customs = Vec::new();
    for id in ids(
        connection,
        "SELECT id FROM custom_runtime_head WHERE removed=0 ORDER BY id",
    )? {
        let mut definition = crate::custom_runtime::storage::latest(connection, &id)?
            .context("configuration.custom_definition_missing")?;
        let mut environment = Vec::new();
        let mut required = Vec::new();
        for path in crate::custom_runtime::environments(&definition) {
            let names = definition
                .pointer(&path)
                .and_then(Value::as_object)
                .context("configuration.custom_environment_invalid")?
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            if include_secrets {
                for (name, plain) in crate::custom_runtime::storage::launch_environment(
                    connection,
                    &definition,
                    &path,
                )? {
                    let archive_id = secrets.add(
                        format!("custom:{id}:{path}:{name}:{}", definition["revision"]),
                        plain,
                    )?;
                    environment.push(json!({"path":path,"name":name,"archiveSecretId":archive_id}));
                }
            } else {
                for name in names {
                    required.push(json!({"path":path,"name":name}));
                }
            }
            let mut placeholders = serde_json::Map::new();
            if !include_secrets {
                for name in definition
                    .pointer(&path)
                    .and_then(Value::as_object)
                    .context("configuration.custom_environment_invalid")?
                    .keys()
                {
                    placeholders.insert(name.clone(), json!(""));
                }
            }
            *definition
                .pointer_mut(&path)
                .context("configuration.custom_environment_invalid")? = Value::Object(placeholders);
        }
        let local_skill_directory = !definition["skillsDir"].is_null();
        definition["skillsDir"] = Value::Null;
        if !required.is_empty() || local_skill_directory {
            definition["enabled"] = json!(false);
        }
        for key in ["revision", "definitionDigest", "createdAt", "updatedAt"] {
            definition
                .as_object_mut()
                .context("configuration.custom_definition_invalid")?
                .remove(key);
        }
        customs.push(json!({"registryId":definition["registryId"],"name":definition["name"],"description":definition["description"],"version":definition["version"],"distributionKind":definition["distributionKind"],"specJson":serde_json::to_string(&definition["spec"])?,"iconUrl":definition["iconUrl"],"skillsSharedStore":definition["skillsSharedStore"],"source":definition["source"],"versionProbe":definition["versionProbe"],"supportsMcp":definition["supportsMcp"],"fusionDefinition":definition,"archiveEnvironment":environment,"requiredEnvironment":required,"localSkillDirectoryRequired":local_skill_directory}));
    }
    domains.insert("customAgents".into(), json!(customs));
    let presets = files::read(root, "agentSettings")?;
    let mut settings = Vec::new();
    for row in presets["items"].as_array().into_iter().flatten() {
        let mut environment = Vec::new();
        let mut names = Vec::new();
        for entry in row["environment"].as_array().into_iter().flatten() {
            names.push(entry["name"].clone());
            if let Some(secret) =
                secrets.reference(connection, &entry["secret"], "member_environment")?
            {
                environment.push(json!({"name":entry["name"],"archiveSecretId":secret}));
            }
        }
        settings.push(json!({"agentType":row["agentType"],"registryId":row["registryId"],"enabled":row["enabled"],"sortOrder":row["sortOrder"],"provider":row["provider"],"envJson":null,"archiveEnvironment":environment,"environmentNames":names}));
    }
    domains.insert("agentSettings".into(), json!(settings));
    let library = files::read(root, "messageLibrary")?;
    domains.insert("quickMessages".into(),Value::Array(library["quickMessages"].as_array().into_iter().flatten().enumerate().map(|(index,row)|json!({"title":row["title"],"content":row["text"],"sortOrder":index})).collect()));
    domains.insert(
        "messageLibrary".into(),
        json!({"tags":library["tags"].as_array().cloned().unwrap_or_default()}),
    );
    let templates = files::read(root, "taskTemplates")?;
    domains.insert(
        "taskTemplates".into(),
        Value::Array(
            templates["items"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|row| json!({"name":row["name"],"title":row["title"],"config":row["config"]}))
                .collect(),
        ),
    );
    for target in ["appearance", "general", "windowClose"] {
        if target == "windowClose" && !cfg!(windows) {
            continue;
        }
        let source = files::read(root, target)?;
        if source.is_null() {
            continue;
        }
        let mut portable = serde_json::Map::new();
        for (key, field) in preferences::NATIVE_FIELDS {
            if key.split('.').nth(1) == Some(target) && !source[*field].is_null() {
                portable.insert(
                    if *field == "themePreference" {
                        "preference".into()
                    } else {
                        (*field).into()
                    },
                    source[*field].clone(),
                );
            }
        }
        domains.insert(target.into(), Value::Object(portable));
    }
    let delegation = crate::delegation_settings::read(connection)?;
    domains.insert("preferences".into(),json!({"delegation.enabled":serde_json::to_string(&delegation.enabled)?,"delegation.depth_limit":serde_json::to_string(&delegation.depth_limit)?,"delegation.agent_defaults":serde_json::to_string(&delegation.per_runtime_defaults)?,"delegation.completed_cache_max_mb":serde_json::to_string(&delegation.completed_cache_max_mb)?}));
    let conversation = policies::basis(connection, "conversationToolsPolicy")?;
    for (key, field, _) in policies::CONVERSATION_FIELDS {
        let value = &conversation["value"][*field];
        if !value.is_null() {
            domains
                .get_mut("preferences")
                .context("configuration.preferences_missing")?[*key] =
                json!(serde_json::to_string(value)?);
        }
    }
    let logging = files::read(root, "logging")?;
    if !logging.is_null() {
        domains.get_mut("preferences").unwrap()["logging.level"] = json!(serde_json::to_string(
            &json!({"level":logging["level"],"targets":logging["targets"]})
        )?);
    }
    // 成员引用只供接收端人工对照，绝不视作接收端成员主键或自动恢复绑定。
    domains.insert(
        "sourceMemberMappings".into(),
        json!(member_mappings(connection)?),
    );
    domains.insert(
        "channelPreferences".into(),
        json!(channel_preferences::portable(root)?),
    );
    let snapshot = json!({"formatVersion":2,"product":"coaxi-config","archiveId":uuid::Uuid::new_v4().to_string(),"appVersion":env!("CARGO_PKG_VERSION"),"createdAt":chrono::Utc::now().to_rfc3339(),"sourcePlatform":std::env::consts::OS,"domains":domains,"secretsIncluded":include_secrets,"secrets":secrets.rows});
    validate("ConfigurationArchiveV2", &snapshot)?;
    ensure!(
        serde_json::to_vec(&snapshot)?.len() <= MAX_ARCHIVE - 1024 * 1024,
        "configuration.export_too_large"
    );
    Ok(snapshot)
}
fn member_mappings(connection: &Connection) -> Result<Vec<Value>> {
    let mut members = Vec::new();
    let mut query=connection.prepare("SELECT p.id,p.display_name,c.reference_json FROM portable_member_connection c JOIN agent_profile p ON p.id=c.agent_id WHERE p.profile_status='present' ORDER BY p.id")?;
    for row in query.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })? {
        let (id, name, reference) = row?;
        members.push(json!({"sourceMemberId":id,"displayName":name,"connection":serde_json::from_str::<Value>(&reference)?}));
    }
    Ok(members)
}
