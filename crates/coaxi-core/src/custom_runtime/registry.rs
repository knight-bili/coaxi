//! 复用 Coaxi aff1e6c remote_registry.rs 的公开来源；只供用户选型，不进入模型工具目录。
use super::*;
use std::time::{Duration, Instant};
pub const URL: &str = "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";
static CACHE: LazyLock<tokio::sync::Mutex<Option<(Instant, Value)>>> =
    LazyLock::new(Default::default);
pub async fn catalog(refresh: bool) -> Result<Value> {
    let mut cache = CACHE.lock().await;
    if !refresh {
        if let Some((time, value)) = &*cache {
            if time.elapsed() < Duration::from_secs(600) {
                let mut value = value.clone();
                value["cached"] = json!(true);
                return Ok(value);
            }
        }
    }
    let client = reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(40))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?;
    let mut response = client
        .get(URL)
        .send()
        .await
        .context("custom_runtime.registry_unavailable")?
        .error_for_status()
        .context("custom_runtime.registry_http_error")?;
    const LIMIT: usize = 8 * 1024 * 1024;
    ensure!(
        response
            .content_length()
            .is_none_or(|length| length <= LIMIT as u64),
        "custom_runtime.registry_too_large"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= LIMIT,
            "custom_runtime.registry_too_large"
        );
        bytes.extend_from_slice(&chunk)
    }
    let document: Value =
        serde_json::from_slice(&bytes).context("custom_runtime.registry_format_invalid")?;
    let entries = document["agents"]
        .as_array()
        .context("custom_runtime.registry_agents_missing")?;
    ensure!(entries.len() <= 4096, "custom_runtime.registry_capacity");
    let value = json!({"registryUrl":URL,"sourceDigest":digest(&document)?,"fetchedAt":chrono::Utc::now().to_rfc3339(),"cached":false,"entries":entries});
    *cache = Some((Instant::now(), value.clone()));
    Ok(value)
}
