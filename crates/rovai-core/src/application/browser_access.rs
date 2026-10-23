//! Native browser grants. These methods are not admitted by public Web or the
//! generic Renderer RPC. Browser process ownership remains in the Desktop Host.
use super::*;
use anyhow::ensure;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    scope: workbench::Scope,
    id: String,
    #[serde(default)]
    origin: Option<String>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    minutes: Option<u64>,
    #[serde(default)]
    command_id: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    input_digest: Option<String>,
    #[serde(default)]
    result_digest: Option<String>,
    #[serde(default)]
    outcome: Option<String>,
}
struct Grant {
    scope: String,
    origin: String,
    level: String,
    expires: i64,
}
static GRANTS: std::sync::LazyLock<tokio::sync::Mutex<HashMap<String, Grant>>> =
    std::sync::LazyLock::new(Default::default);

impl Core {
    pub(super) async fn browser_access(&self, method: &str, value: Value) -> Result<Value> {
        let request: Request = serde_json::from_value(value)?;
        uuid::Uuid::parse_str(&request.id)?;
        let root = self.workbench_root(&request.scope).await?;
        let scope = crate::command::canonical_json_digest(
            &json!({"scope":request.scope,"root":root,"core":self.data_dir}),
        )?;
        let key = format!("{scope}:{}", request.id);
        let now = Utc::now().timestamp_millis();
        let mut grants = GRANTS.lock().await;
        grants.retain(|_, grant| grant.expires > now);
        match method {
            "browser.local.revoke" => {
                grants.remove(&key);
                return Ok(json!({"revoked":true}));
            }
            "browser.local.grant" => {
                let origin = request.origin.context("browser.origin_required")?;
                let url = reqwest::Url::parse(&origin)?;
                ensure!(
                    matches!(url.scheme(), "http" | "https")
                        && url.username().is_empty()
                        && url.password().is_none()
                        && url.origin().ascii_serialization() == origin,
                    "browser.invalid_origin"
                );
                let level = request.level.context("browser.level_required")?;
                ensure!(
                    matches!(level.as_str(), "read" | "control"),
                    "browser.invalid_level"
                );
                let expires = now + request.minutes.unwrap_or(30).clamp(1, 120) as i64 * 60_000;
                grants.insert(
                    key,
                    Grant {
                        scope,
                        origin,
                        level: level.clone(),
                        expires,
                    },
                );
                return Ok(json!({"level":level,"expiresAt":expires}));
            }
            "browser.local.begin" | "browser.local.finish" => {}
            _ => anyhow::bail!("browser.operation_not_admitted"),
        }
        let command = request.command_id.context("browser.command_required")?;
        uuid::Uuid::parse_str(&command)?;
        let directory = self.data_dir.join("browser-operations");
        crate::platform::private_storage::prepare_private_directory(&directory)?;
        let receipt = directory.join(format!("{command}.json"));
        let identity = crate::command::canonical_json_digest(
            &json!({"scope":scope,"id":request.id,"origin":request.origin,"action":request.action,"inputDigest":request.input_digest}),
        )?;
        if method == "browser.local.finish" {
            let mut previous: Value = serde_json::from_slice(&std::fs::read(&receipt)?)?;
            ensure!(
                previous["identity"].as_str() == Some(identity.as_str()),
                "browser.command_identity_changed"
            );
            ensure!(
                matches!(
                    request.outcome.as_deref(),
                    Some("completed" | "failed" | "unknown")
                ),
                "browser.invalid_outcome"
            );
            if previous["state"] == "started" {
                previous["state"] = json!(request.outcome);
                previous["resultDigest"] = json!(request.result_digest);
                previous["finishedAt"] = json!(now);
                crate::platform::private_storage::atomic_write_private_json(&receipt, &previous)?;
            }
            let valid = grants.get(&key).is_some_and(|grant| {
                grant.scope == scope && Some(grant.origin.as_str()) == request.origin.as_deref()
            });
            return Ok(json!({"recorded":true,"deliverResult":valid}));
        }
        let grant = grants.get(&key).context("browser.not_shared")?;
        ensure!(
            grant.scope == scope && Some(grant.origin.as_str()) == request.origin.as_deref(),
            "browser.origin_changed"
        );
        let action = request
            .action
            .as_deref()
            .context("browser.action_required")?;
        let read = matches!(action, "snapshot" | "screenshot" | "console");
        ensure!(
            read || (grant.level == "control"
                && matches!(
                    action,
                    "click" | "hover" | "type" | "press" | "select" | "scroll"
                )),
            "browser.control_not_shared"
        );
        if receipt.exists() {
            let previous: Value = serde_json::from_slice(&std::fs::read(&receipt)?)?;
            ensure!(
                previous["identity"].as_str() == Some(identity.as_str()),
                "browser.command_identity_changed"
            );
            anyhow::bail!(
                "browser.original_request_already_admitted: {}",
                previous["state"]
            );
        }
        crate::platform::private_storage::atomic_write_private_json(
            &receipt,
            &json!({"identity":identity,"state":"started","admittedAt":now,"action":action,"browserId":request.id}),
        )?;
        Ok(json!({"admitted":true,"expiresAt":grant.expires}))
    }
}
