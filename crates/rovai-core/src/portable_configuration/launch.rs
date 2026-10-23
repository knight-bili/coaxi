//! 运行创建时才解开冻结的精确引用；独立作用域不改用户原 CLI 目录。
use super::*;
use crate::{
    agent_profile::{AdapterKind, FrozenAgentRuntimeConfig},
    member_environment::{LaunchEnvironment, LaunchEnvironmentSource},
    platform::private_storage,
    runtime_custom_api::CredentialRedactor,
};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::AsyncReadExt;

#[derive(Clone)]
pub(crate) struct Plan {
    pub environment: LaunchEnvironment,
    redactor: CredentialRedactor,
}
tokio::task_local! {static PLAN:Option<Plan>;}
static SCOPE_PREPARATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
pub(crate) async fn with_plan<F: std::future::Future>(plan: Option<Plan>, future: F) -> F::Output {
    PLAN.scope(plan, future).await
}
pub(crate) fn redactor() -> Option<CredentialRedactor> {
    PLAN.try_with(|plan| plan.as_ref().map(|plan| plan.redactor.clone()))
        .ok()
        .flatten()
}
fn remove(values: &mut BTreeMap<String, String>, name: &str) {
    values.retain(|key, _| {
        if cfg!(windows) {
            !key.eq_ignore_ascii_case(name)
        } else {
            key != name
        }
    });
}
fn insert(values: &mut BTreeMap<String, String>, name: &str, value: String) {
    remove(values, name);
    values.insert(name.into(), value);
}
const CLAUDE_MODELS: [(&str, &str); 9] = [
    ("main", "ANTHROPIC_MODEL"),
    ("reasoning", "ANTHROPIC_REASONING_MODEL"),
    ("haiku", "ANTHROPIC_DEFAULT_HAIKU_MODEL"),
    ("sonnet", "ANTHROPIC_DEFAULT_SONNET_MODEL"),
    ("opus", "ANTHROPIC_DEFAULT_OPUS_MODEL"),
    ("fable", "ANTHROPIC_DEFAULT_FABLE_MODEL"),
    ("customOption", "ANTHROPIC_CUSTOM_MODEL_OPTION"),
    ("customOptionName", "ANTHROPIC_CUSTOM_MODEL_OPTION_NAME"),
    (
        "customOptionDescription",
        "ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION",
    ),
];
fn clear_routing(values: &mut BTreeMap<String, String>) {
    let keys = values
        .keys()
        .filter(|name| {
            let name = name.to_ascii_uppercase();
            name.starts_with("ANTHROPIC_")
                || name.starts_with("CLAUDE_CODE_USE_")
                || name.starts_with("GOOGLE_GENAI_")
                || [
                    "CLAUDE_CONFIG_DIR",
                    "CLAUDE_CODE_OAUTH_TOKEN",
                    "CLAUDE_CODE_API_KEY_HELPER_TTL_MS",
                    "CODEX_HOME",
                    "CODEX_API_KEY",
                    "CODEX_BASE_URL",
                    "OPENAI_API_KEY",
                    "OPENAI_BASE_URL",
                    "OPENAI_API_BASE",
                    "OPENAI_MODEL",
                    "OPENAI_ORGANIZATION",
                    "OPENAI_ORGANIZATION_ID",
                    "OPENAI_ORG_ID",
                    "OPENAI_PROJECT",
                    "OPENAI_PROJECT_ID",
                    "GEMINI_CLI_HOME",
                    "GEMINI_API_KEY",
                    "GOOGLE_API_KEY",
                    "GOOGLE_GEMINI_BASE_URL",
                    "GEMINI_MODEL",
                    "GOOGLE_CLOUD_PROJECT",
                    "GOOGLE_CLOUD_LOCATION",
                    "GOOGLE_APPLICATION_CREDENTIALS",
                    "GOOGLE_CLOUD_ACCESS_TOKEN",
                ]
                .contains(&name.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    for key in keys {
        values.remove(&key);
    }
}
fn read_json(path: &Path, limit: usize) -> Result<Value> {
    let mut file = private_storage::open_private_read_file(path)?;
    ensure!(
        file.metadata()?.len() <= limit as u64,
        "configuration.scope_file_too_large"
    );
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "configuration.scope_file_too_large");
    Ok(serde_json::from_slice(&bytes)?)
}
fn scope(root: &Path, id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(id)?;
    let parent = private_storage::prepare_private_directory(&root.join("portable-configuration"))?;
    let parent = private_storage::prepare_private_directory(&parent.join("native-scopes"))?;
    private_storage::prepare_private_directory(&parent.join(id))
}
async fn codex_catalog(
    runtime: &FrozenAgentRuntimeConfig,
    environment: &LaunchEnvironment,
) -> Result<Vec<Value>> {
    use crate::managed_process::{
        ManagedProcess, ManagedProcessLaunchSpec, ManagedProcessPurpose, ManagedStdinPolicy,
        ManagedWindowsArgvDialect,
    };
    let mut command = tokio::process::Command::new(&runtime.executable_path);
    crate::runtime_discovery::configure_runtime_command(AdapterKind::CodexCli, &mut command);
    environment.apply(&mut command)?;
    command
        .args(["debug", "models", "--bundled"])
        .current_dir(std::env::temp_dir());
    let spec = ManagedProcessLaunchSpec::capture(
        &command,
        ManagedProcessPurpose::RuntimeProbe,
        ManagedStdinPolicy::Null,
        ManagedWindowsArgvDialect::MicrosoftCrt,
        "portable-configuration:codex-catalog",
    )?;
    let mut child = ManagedProcess::spawn(spec)?;
    let mut stdout = child
        .take_stdout()
        .context("configuration.catalog_stdout_missing")?;
    let mut stderr = child
        .take_stderr()
        .context("configuration.catalog_stderr_missing")?;
    let observed = tokio::time::timeout(Duration::from_secs(15), async {
        let output = async {
            let mut bytes = Vec::new();
            (&mut stdout)
                .take(8 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .await?;
            ensure!(
                bytes.len() <= 8 * 1024 * 1024,
                "configuration.catalog_too_large"
            );
            Ok::<_, anyhow::Error>(bytes)
        };
        let errors = async {
            let mut sink = tokio::io::sink();
            tokio::io::copy(&mut (&mut stderr).take(16 * 1024 * 1024), &mut sink).await?;
            Ok::<_, anyhow::Error>(())
        };
        let (status, bytes, _) = tokio::try_join!(
            async { Ok::<_, anyhow::Error>(child.wait().await?) },
            output,
            errors
        )?;
        ensure!(status.success(), "configuration.live_catalog_unavailable");
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("configuration.live_catalog_invalid"))?;
        let models = value["models"]
            .as_array()
            .context("configuration.live_catalog_invalid")?;
        ensure!(
            !models.is_empty() && models.len() <= 8192,
            "configuration.live_catalog_invalid"
        );
        Ok::<_, anyhow::Error>(models.clone())
    })
    .await;
    match observed {
        Ok(Ok(models)) => Ok(models),
        _ => {
            let _ = child.force_terminate_tree();
            let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
            anyhow::bail!("configuration.live_catalog_unavailable")
        }
    }
}
fn toml_text(value: &str) -> String {
    toml::Value::String(value.into()).to_string()
}
fn model_id(connection: &FrozenPortableConnectionV1) -> Option<&str> {
    match connection.agent_type.as_str() {
        "claude_code" => connection.model["main"].as_str(),
        "codex" => connection.model["default"].as_str(),
        "gemini" => connection.model["model"].as_str(),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}
async fn write_scope(
    root: &Path,
    runtime: &FrozenAgentRuntimeConfig,
    connection: &FrozenPortableConnectionV1,
    values: &BTreeMap<String, String>,
) -> Result<PathBuf> {
    let _preparation = SCOPE_PREPARATION.lock().await;
    let directory = scope(root, &connection.native_scope_id)?;
    let marker = directory.join("portable-owner-v1.json");
    let identity = json!({"schemaVersion":1,"projectionDigest":connection.projection_digest,"installationId":runtime.installation_id,"installationGeneration":runtime.installation_generation,"executableFingerprint":runtime.executable_fingerprint,"hostPlatform":connection.host_platform});
    if marker.exists() {
        let saved = read_json(&marker, 1024 * 1024)?;
        ensure!(
            saved["identity"] == identity,
            "configuration.native_scope_changed"
        );
        ensure!(
            saved["managedFiles"] == managed_files(&directory, &connection.agent_type)?,
            "configuration.native_managed_files_changed"
        );
        if connection.agent_type == "codex" {
            let catalog = read_json(&directory.join(codex_catalog::CATALOG_REL), 8 * 1024 * 1024)?;
            ensure!(
                saved["catalogDigest"].as_str() == Some(digest(&catalog)?.as_str()),
                "configuration.native_catalog_changed"
            );
        }
        return Ok(directory);
    }
    let mut catalog_digest = Value::Null;
    match connection.agent_type.as_str() {
        "claude_code" => {
            let mut env = serde_json::Map::new();
            for (_, name) in CLAUDE_MODELS {
                if let Some(value) = values.get(name) {
                    env.insert(name.into(), json!(value));
                }
            }
            if let Some(value) = values.get("ANTHROPIC_BASE_URL") {
                env.insert("ANTHROPIC_BASE_URL".into(), json!(value));
            }
            // 密钥保留在进程环境，不写明文 settings；此文件只含明确的模型和地址配置。
            private_storage::atomic_write_private_json(
                &directory.join("settings.json"),
                &json!({"env":env}),
            )?;
        }
        "codex" => {
            let environment = LaunchEnvironment {
                values: Some(values.clone()),
                overlay: None,
            };
            let snapshot = codex_catalog(runtime, &environment).await?;
            let (catalog, warnings) = normalize::codex_projection(&connection.model, &snapshot)?;
            ensure!(
                warnings.is_empty(),
                "configuration.catalog_projection_incomplete"
            );
            catalog_digest = json!(digest(&catalog)?);
            private_storage::atomic_write_private_json(
                &directory.join(codex_catalog::CATALOG_REL),
                &catalog,
            )?;
            private_storage::atomic_write_private_json(
                &directory.join(codex_catalog::SOURCE_REL),
                &connection.model,
            )?;
            let catalog_path = directory
                .join(codex_catalog::CATALOG_REL)
                .to_string_lossy()
                .into_owned();
            let mut config = format!(
                "model_provider = \"portable_provider\"\nmodel_catalog_json = {}\n",
                toml_text(&catalog_path)
            );
            if let Some(model) = model_id(connection) {
                config.push_str(&format!("model = {}\n", toml_text(model)));
            }
            config.push_str(&format!("\n[model_providers.portable_provider]\nname = \"Portable provider\"\nbase_url = {}\nenv_key = \"OPENAI_API_KEY\"\nwire_api = \"responses\"\nrequires_openai_auth = false\n",toml_text(&connection.api_url)));
            private_storage::atomic_write_private_bytes(
                &directory.join("config.toml"),
                config.as_bytes(),
            )?;
        }
        "gemini" => {
            let config = private_storage::prepare_private_directory(&directory.join(".gemini"))?;
            private_storage::atomic_write_private_json(
                &config.join("settings.json"),
                &json!({"security":{"auth":{"selectedType":"gemini-api-key"}}}),
            )?;
        }
        _ => anyhow::bail!("configuration.agent_type_unsupported"),
    }
    private_storage::atomic_write_private_json(
        &marker,
        &json!({"identity":identity,"catalogDigest":catalog_digest,"managedFiles":managed_files(&directory,&connection.agent_type)?}),
    )?;
    Ok(directory)
}
fn managed_files(directory: &Path, agent: &str) -> Result<Value> {
    use sha2::{Digest, Sha256};
    let paths = match agent {
        "claude_code" => vec!["settings.json"],
        "codex" => vec![
            "config.toml",
            codex_catalog::CATALOG_REL,
            codex_catalog::SOURCE_REL,
        ],
        "gemini" => vec![".gemini/settings.json"],
        _ => anyhow::bail!("configuration.agent_type_unsupported"),
    };
    let mut result = serde_json::Map::new();
    for relative in paths {
        let path = directory.join(relative);
        let metadata = std::fs::symlink_metadata(&path)?;
        ensure!(
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() <= 16 * 1024 * 1024,
            "configuration.native_managed_file_invalid"
        );
        let mut file = private_storage::open_private_read_file(&path)?;
        let mut bytes = Vec::new();
        file.by_ref()
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 16 * 1024 * 1024,
            "configuration.native_managed_file_too_large"
        );
        result.insert(
            relative.into(),
            json!({"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))}),
        );
    }
    Ok(Value::Object(result))
}
pub(crate) async fn prepare(
    runtime: &FrozenAgentRuntimeConfig,
    source: Option<&LaunchEnvironmentSource>,
) -> Result<Option<Plan>> {
    if runtime.portable_connection.is_none() {
        return Ok(None);
    }
    prepare_selected(runtime, source)
        .await
        .map_err(|error| super::launch_failure::public(runtime, error))
}
async fn prepare_selected(
    runtime: &FrozenAgentRuntimeConfig,
    source: Option<&LaunchEnvironmentSource>,
) -> Result<Option<Plan>> {
    let Some(connection) = runtime.portable_connection.as_ref() else {
        return Ok(None);
    };
    connection.validate()?;
    ensure!(
        runtime.custom_api.is_none()
            && runtime.custom_runtime.is_none()
            && runtime.adapter_kind == connection.adapter_kind()?
            && runtime.installation_id == connection.installation_id
            && runtime.installation_generation == connection.installation_generation,
        "configuration.frozen_connection_changed"
    );
    ensure!(
        crate::custom_runtime::installer::platform()? == connection.host_platform,
        "configuration.connection_requires_original_platform"
    );
    let source = source.context("configuration.host_source_required")?;
    let (root, credential, environment) = {
        let database = source.database.lock().await;
        let stored = providers::resolve(database.connection(), &connection.reference, false)?;
        ensure!(
            stored["agentType"] == connection.agent_type
                && stored["apiUrl"] == connection.api_url
                && stored["model"] == connection.model
                && stored["credential"] == serde_json::to_value(&connection.credential)?,
            "configuration.frozen_provider_changed"
        );
        let credential = connection
            .credential
            .as_ref()
            .map(|reference| secrets::retained(database.connection(), reference, "provider_key"))
            .transpose()?;
        let mut environment = BTreeMap::new();
        for entry in &connection.environment {
            environment.insert(
                entry.name.clone(),
                secrets::retained(database.connection(), &entry.secret, "member_environment")?,
            );
        }
        let root = PathBuf::from(
            database
                .connection()
                .path()
                .context("configuration.database_path_required")?,
        )
        .parent()
        .context("configuration.data_root_required")?
        .to_path_buf();
        (root, credential, environment)
    };
    ensure!(
        credential.as_ref().is_some_and(|value| !value.is_empty()),
        "configuration.provider_credential_required"
    );
    let mut secrets = environment.values().cloned().collect::<Vec<_>>();
    if let Some(value) = &credential {
        secrets.push(value.clone());
    }
    secrets.push(connection.api_url.clone());
    let mut values = crate::runtime_discovery::capture_host_runtime_environment();
    clear_routing(&mut values);
    for (name, value) in &environment {
        insert(&mut values, name, value.clone());
    }
    let mut overlay = environment;
    // 连接由明确的服务商版本拥有；队员环境不能重新引入另一个认证或路由来源。
    clear_routing(&mut values);
    clear_routing(&mut overlay);
    match connection.agent_type.as_str() {
        "claude_code" => {
            insert(
                &mut overlay,
                "ANTHROPIC_BASE_URL",
                connection.api_url.clone(),
            );
            insert(
                &mut overlay,
                "ANTHROPIC_API_KEY",
                credential.clone().unwrap_or_default(),
            );
            insert(
                &mut overlay,
                "CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST",
                "1".into(),
            );
            for (field, name) in CLAUDE_MODELS {
                remove(&mut values, name);
                remove(&mut overlay, name);
                if let Some(value) = connection.model[field]
                    .as_str()
                    .filter(|value| !value.is_empty())
                {
                    insert(&mut overlay, name, value.into());
                }
            }
            for name in [
                "ANTHROPIC_AUTH_TOKEN",
                "CLAUDE_CODE_OAUTH_TOKEN",
                "CLAUDE_CODE_USE_BEDROCK",
                "CLAUDE_CODE_USE_VERTEX",
                "CLAUDE_CODE_USE_FOUNDRY",
            ] {
                remove(&mut values, name);
                remove(&mut overlay, name);
            }
        }
        "codex" => {
            insert(&mut overlay, "OPENAI_BASE_URL", connection.api_url.clone());
            insert(
                &mut overlay,
                "OPENAI_API_KEY",
                credential.clone().unwrap_or_default(),
            );
            remove(&mut values, "OPENAI_MODEL");
            remove(&mut overlay, "OPENAI_MODEL");
            if let Some(model) = model_id(connection) {
                insert(&mut overlay, "OPENAI_MODEL", model.into());
            }
        }
        "gemini" => {
            insert(
                &mut overlay,
                "GOOGLE_GEMINI_BASE_URL",
                connection.api_url.clone(),
            );
            insert(
                &mut overlay,
                "GEMINI_API_KEY",
                credential.clone().unwrap_or_default(),
            );
            remove(&mut values, "GEMINI_MODEL");
            remove(&mut overlay, "GEMINI_MODEL");
            if let Some(model) = model_id(connection) {
                insert(&mut overlay, "GEMINI_MODEL", model.into());
            }
        }
        _ => anyhow::bail!("configuration.agent_type_unsupported"),
    }
    for (name, value) in &overlay {
        insert(&mut values, name, value.clone());
    }
    let directory = scope(&root, &connection.native_scope_id)?;
    let variable = match connection.agent_type.as_str() {
        "claude_code" => "CLAUDE_CONFIG_DIR",
        "codex" => "CODEX_HOME",
        "gemini" => "GEMINI_CLI_HOME",
        _ => anyhow::bail!("configuration.agent_type_unsupported"),
    };
    insert(
        &mut values,
        variable,
        directory.to_string_lossy().into_owned(),
    );
    insert(
        &mut overlay,
        variable,
        directory.to_string_lossy().into_owned(),
    );
    write_scope(&root, runtime, connection, &values).await?;
    Ok(Some(Plan {
        environment: LaunchEnvironment {
            values: Some(values),
            overlay: Some(overlay),
        },
        redactor: CredentialRedactor::from_values(secrets),
    }))
}
