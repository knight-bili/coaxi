//! 原配置只做有界解码，未知域和源行原文交给受管加密归档保留。
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const COAXI_DOMAINS: [&str; 6] = [
    "modelProviders",
    "agentSettings",
    "customAgents",
    "quickMessages",
    "taskTemplates",
    "preferences",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CoaxiProvider {
    pub name: String,
    pub api_url: String,
    pub api_key: String,
    #[serde(default)]
    pub agent_types_json: String,
    pub agent_type: String,
    #[serde(default)]
    pub model: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CoaxiProviderRef {
    pub agent_type: String,
    pub name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CoaxiAgentSetting {
    pub agent_type: String,
    #[serde(default)]
    pub registry_id: String,
    pub enabled: bool,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub env_json: Option<String>,
    #[serde(default)]
    pub provider: Option<CoaxiProviderRef>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CoaxiCustomAgent {
    pub registry_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub distribution_kind: String,
    #[serde(default)]
    pub spec_json: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub skills_shared_store: bool,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub version_probe: Option<String>,
    #[serde(default)]
    pub supports_mcp: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CoaxiQuickMessage {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub sort_order: i32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CoaxiTaskTemplate {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub config: String,
}

pub(crate) struct ParsedArchive {
    pub source_kind: &'static str,
    pub source_digest: String,
    pub original: Value,
    pub domains: BTreeMap<String, Value>,
}
pub(crate) struct Entry {
    pub id: String,
    pub domain: String,
    pub index: usize,
    pub value: Value,
}

fn rows<T: serde::de::DeserializeOwned>(value: &Value) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    let values = value
        .as_array()
        .context("configuration.domain_rows_invalid")?;
    ensure!(values.len() <= 8192, "configuration.domain_too_many_rows");
    for value in values {
        let _: T = serde_json::from_value(value.clone())
            .map_err(|_| anyhow::anyhow!("configuration.domain_row_invalid"))?;
    }
    Ok(())
}
fn check_coaxi_domain(name: &str, value: &Value) -> Result<()> {
    match name {
        "modelProviders" => rows::<CoaxiProvider>(value),
        "agentSettings" => rows::<CoaxiAgentSetting>(value),
        "customAgents" => rows::<CoaxiCustomAgent>(value),
        "quickMessages" => rows::<CoaxiQuickMessage>(value),
        "taskTemplates" => rows::<CoaxiTaskTemplate>(value),
        "preferences" => {
            ensure!(
                value.is_null()
                    || value
                        .as_object()
                        .is_some_and(|map| map.values().all(Value::is_string)),
                "configuration.preference_values_invalid"
            );
            Ok(())
        }
        _ => Ok(()),
    }
}
pub(crate) fn parse(
    bytes: &[u8],
    passphrase: &str,
    manifest: Option<&Value>,
) -> Result<ParsedArchive> {
    parse_inner(bytes, passphrase, manifest, false)
}
pub(super) fn parse_retained(bytes: &[u8]) -> Result<ParsedArchive> {
    parse_inner(bytes, "", None, true)
}
fn parse_inner(
    bytes: &[u8],
    passphrase: &str,
    manifest: Option<&Value>,
    retained: bool,
) -> Result<ParsedArchive> {
    ensure!(
        bytes.len() <= MAX_ARCHIVE,
        "configuration.archive_too_large"
    );
    let source_digest = format!("sha256:{:x}", Sha256::digest(bytes));
    let binary = crate::backup::crypto::has_envelope_magic(bytes);
    let decoded = if binary {
        Some(
            crate::backup::crypto::decrypt_bytes(bytes, passphrase, MAX_ARCHIVE)
                .map_err(|_| anyhow::anyhow!("configuration.archive_decryption_failed"))?,
        )
    } else {
        None
    };
    let mut value: Value = serde_json::from_slice(decoded.as_deref().unwrap_or(bytes))
        .map_err(|_| anyhow::anyhow!("configuration.archive_json_invalid"))?;
    let coaxi_encrypted = value.get("coaxiConfigEncryption").is_some() || value.get("codegConfigEncryption").is_some();
    let encrypted = binary || coaxi_encrypted;
    if let Some(manifest) = manifest {
        ensure!(
            serde_json::to_vec(manifest)?.len() <= 64 * 1024 && manifest["schemaVersion"] == 1,
            "configuration.manifest_invalid"
        );
        ensure!(
            manifest["config"]["size"].as_u64() == Some(bytes.len() as u64)
                && manifest["config"]["sha256"].as_str()
                    == Some(source_digest.trim_start_matches("sha256:")),
            "configuration.manifest_content_changed"
        );
        ensure!(
            manifest["encryption"].as_str() == Some(if encrypted { "aes-256-gcm" } else { "none" }),
            "configuration.manifest_encryption_mismatch"
        );
    }
    if coaxi_encrypted {
        ensure!(!binary, "configuration.nested_encryption_forbidden");
        let plain = super::coaxi_crypto::decrypt(value, passphrase)?;
        value = serde_json::from_slice(&plain)
            .map_err(|_| anyhow::anyhow!("configuration.archive_json_invalid"))?;
    }
    super::bounded(&value, 0)?;
    let original = value.clone();
    // 只用规范视图验证，原归档和摘要仍保持原文。
    if value["product"] == "codeg-rovai-config" { value["product"] = Value::String("coaxi-config".into()); }
    let source_kind = if value["schemaVersion"] == 1 && value.get("formatVersion").is_none() {
        "coaxi_v1"
    } else if value["formatVersion"] == 1 && value["product"] == "coaxi-config" {
        "fusion_v1"
    } else if value["formatVersion"] == 2 && value["product"] == "coaxi-config" {
        validate("ConfigurationArchiveV2", &value)?;
        ensure!(
            value["secretsIncluded"] == true
                || value["secrets"].as_array().is_some_and(Vec::is_empty),
            "configuration.unexpected_archive_secrets"
        );
        ensure!(
            value["secretsIncluded"] != true || binary || retained,
            "configuration.secret_archive_requires_encryption"
        );
        "fusion_v2"
    } else {
        anyhow::bail!("configuration.archive_version_unsupported")
    };
    let map = value["domains"]
        .as_object()
        .context("configuration.domains_required")?;
    ensure!(map.len() <= 32, "configuration.too_many_domains");
    let mut total = 0usize;
    for (name, domain) in map {
        ensure!(name.len() <= 128, "configuration.domain_name_invalid");
        total = total
            .checked_add(domain.as_array().map_or_else(
                || domain.as_object().map_or(0, |map| map.len()),
                |items| items.len(),
            ))
            .context("configuration.too_many_entries")?;
        ensure!(total <= 8192, "configuration.too_many_entries");
        if source_kind == "coaxi_v1" {
            check_coaxi_domain(name, domain)?;
        }
    }
    let domains = map
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    Ok(ParsedArchive {
        source_kind,
        source_digest,
        original,
        domains,
    })
}
impl ParsedArchive {
    pub(crate) fn entries(&self) -> Result<Vec<Entry>> {
        if self.source_kind != "coaxi_v1" {
            return super::fusion_archive::entries(self);
        }
        let mut entries = Vec::new();
        for (domain, value) in &self.domains {
            if let Some(rows) = value.as_array() {
                let mut indexed = rows.iter().enumerate().collect::<Vec<_>>();
                if matches!(domain.as_str(), "quickMessages" | "agentSettings") {
                    indexed.sort_by_key(|(index, value)| {
                        (value["sortOrder"].as_i64().unwrap_or(0), *index)
                    });
                }
                for (index, value) in indexed {
                    entries.push(Entry {
                        id: digest(
                            &json!({"source":self.source_digest,"domain":domain,"index":index}),
                        )?,
                        domain: domain.clone(),
                        index,
                        value: value.clone(),
                    });
                }
            } else if let Some(map) = value.as_object() {
                for (index, (key, value)) in map.iter().enumerate() {
                    entries.push(Entry{id:digest(&json!({"source":self.source_digest,"domain":domain,"index":index,"key":key}))?,domain:domain.clone(),index,value:json!({"key":key,"value":value})});
                }
            }
        }
        ensure!(entries.len() <= 8192, "configuration.too_many_entries");
        Ok(entries)
    }
}
