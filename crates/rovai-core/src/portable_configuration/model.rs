use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecretRefV1 {
    pub id: String,
    pub revision: i64,
}
impl SecretRefV1 {
    pub fn validate(&self) -> Result<()> {
        super::validate("SecretRefV1", &serde_json::to_value(self)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableConnectionRefV1 {
    pub id: String,
    pub revision: i64,
    pub digest: String,
}
impl PortableConnectionRefV1 {
    pub fn validate(&self) -> Result<()> {
        super::validate("PortableConnectionRefV1", &serde_json::to_value(self)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenPortableEnvironmentV1 {
    pub name: String,
    pub secret: SecretRefV1,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenPortableConnectionV1 {
    pub schema_version: u32,
    pub reference: PortableConnectionRefV1,
    pub projection_version: u32,
    pub agent_type: String,
    pub api_url: String,
    pub credential: Option<SecretRefV1>,
    pub model: Value,
    pub environment: Vec<FrozenPortableEnvironmentV1>,
    pub environment_digest: String,
    pub catalog_digest: String,
    pub projection_digest: String,
    pub installation_id: String,
    pub installation_generation: i64,
    pub host_platform: String,
    pub native_scope_id: String,
}
impl std::fmt::Debug for FrozenPortableConnectionV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrozenPortableConnectionV1")
            .field("reference", &self.reference)
            .field("agent_type", &self.agent_type)
            .finish_non_exhaustive()
    }
}
impl FrozenPortableConnectionV1 {
    pub fn validate(&self) -> Result<()> {
        super::validate("FrozenPortableConnectionV1", &serde_json::to_value(self)?)?;
        validate_url(&self.api_url)?;
        validate_model(&self.agent_type, &self.model)?;
        let mut names = std::collections::HashSet::new();
        for item in &self.environment {
            item.secret.validate()?;
            ensure!(
                names.insert(if cfg!(windows) {
                    item.name.to_ascii_uppercase()
                } else {
                    item.name.clone()
                }),
                "configuration.duplicate_environment_name"
            );
        }
        ensure!(
            digest(&serde_json::to_value(&self.environment)?)? == self.environment_digest,
            "configuration.environment_digest_mismatch"
        );
        ensure!(
            digest(&self.model)? == self.catalog_digest,
            "configuration.catalog_digest_mismatch"
        );
        let mut unsigned = serde_json::to_value(self)?;
        unsigned["projectionDigest"] = json!("");
        ensure!(
            digest(&unsigned)? == self.projection_digest,
            "configuration.projection_digest_mismatch"
        );
        Ok(())
    }
    pub fn adapter_kind(&self) -> Result<crate::agent_profile::AdapterKind> {
        use crate::agent_profile::AdapterKind;
        Ok(match self.agent_type.as_str() {
            "claude_code" => AdapterKind::ClaudeCodeCli,
            "codex" => AdapterKind::CodexCli,
            "gemini" => AdapterKind::GeminiCli,
            _ => anyhow::bail!("configuration.agent_type_unsupported"),
        })
    }
}
pub(crate) fn validate_url(value: &str) -> Result<()> {
    let url = reqwest::Url::parse(value).context("configuration.api_url_invalid")?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "configuration.api_url_invalid"
    );
    Ok(())
}
pub(crate) fn validate_model(agent: &str, model: &Value) -> Result<()> {
    ensure!(
        model["kind"].as_str()
            == Some(match agent {
                "claude_code" => "claude",
                "codex" => "codex",
                "gemini" => "gemini",
                _ => anyhow::bail!("configuration.agent_type_unsupported"),
            }),
        "configuration.model_kind_mismatch"
    );
    if agent == "codex" {
        let mut seen = std::collections::HashSet::new();
        for custom in model["customs"]
            .as_array()
            .context("configuration.custom_models_invalid")?
        {
            let slug = custom["slug"]
                .as_str()
                .context("configuration.model_id_required")?;
            ensure!(
                !slug.trim().is_empty() && seen.insert(slug),
                "configuration.duplicate_model"
            );
            ensure!(
                custom["base"]
                    .as_str()
                    .is_some_and(|base| !base.trim().is_empty()),
                "configuration.base_model_required"
            );
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableMemberBindingV1 {
    pub schema_version: u32,
    pub agent_id: String,
    pub revision: i64,
    pub connection: Option<PortableConnectionRefV1>,
}
