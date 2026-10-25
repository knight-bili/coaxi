//! 此进程只读私有配置并拉取已确认反馈，不创建预览、不发送普通消息。
use super::*;
use crate::{builtin_tool_transport::LocalIpcEndpoint, platform::local_ipc::LocalIpcClientStream};
use std::{io::Read, path::Path};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessConfig {
    pub schema_version: u32,
    pub binding: Binding,
    pub binding_digest: String,
    pub core_endpoint: String,
    pub authorization_token: String,
    pub max_frame_bytes: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IpcRequest {
    pub schema_version: u32,
    pub bridge_instance_id: String,
    pub authorization_token: String,
    pub binding_digest: String,
    pub operation: String,
    pub payload: Value,
}
pub fn create_endpoint() -> Result<String> {
    #[cfg(windows)]
    let value = format!(
        r"\\.\pipe\rovai-ai-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    );
    #[cfg(unix)]
    let value = std::env::temp_dir()
        .join(format!("rvf-{}", uuid::Uuid::new_v4().simple()))
        .join("i")
        .to_string_lossy()
        .into_owned();
    #[cfg(not(any(windows, unix)))]
    anyhow::bail!("runtime_unavailable");
    endpoint(&value)?;
    Ok(value)
}

pub fn endpoint(value: &str) -> Result<LocalIpcEndpoint> {
    #[cfg(windows)]
    let endpoint = LocalIpcEndpoint::WindowsNamedPipe {
        name: value.to_string(),
    };
    #[cfg(unix)]
    let endpoint = LocalIpcEndpoint::UnixSocket {
        path: value.to_string(),
    };
    #[cfg(not(any(windows, unix)))]
    anyhow::bail!("runtime_unavailable");
    endpoint.validate()?;
    Ok(endpoint)
}
impl ProcessConfig {
    pub async fn request(&self, operation: &str, payload: Value) -> Result<Value> {
        let request = IpcRequest {
            schema_version: 1,
            bridge_instance_id: self.binding.bridge_instance_id.clone(),
            authorization_token: self.authorization_token.clone(),
            binding_digest: self.binding_digest.clone(),
            operation: operation.into(),
            payload,
        };
        let mut bytes = serde_json::to_vec(&request)?;
        ensure!(bytes.len() <= MAX_FRAME_BYTES, "transport_failed");
        bytes.push(b'\n');
        let stream = LocalIpcClientStream::connect(&endpoint(&self.core_endpoint)?).await?;
        let (reader, mut writer) = tokio::io::split(stream);
        writer.write_all(&bytes).await?;
        writer.shutdown().await?;
        let mut frame = Vec::new();
        let mut reader = BufReader::new(reader.take((MAX_FRAME_BYTES * 2 + 1) as u64));
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            reader.read_until(b'\n', &mut frame),
        )
        .await??;
        ensure!(
            frame.last() == Some(&b'\n') && frame.len() <= MAX_FRAME_BYTES * 2,
            "transport_failed"
        );
        let response: Value = serde_json::from_slice(&frame)?;
        ensure!(response["schemaVersion"] == 1, "transport_failed");
        if response["ok"] != true {
            anyhow::bail!(
                "{}",
                response["code"].as_str().unwrap_or("transport_failed")
            )
        }
        Ok(response["result"].clone())
    }
}
fn load(path: &Path) -> Result<ProcessConfig> {
    ensure!(path.is_absolute(), "feedback.absolute_config_required");
    let mut bytes = Vec::new();
    crate::platform::private_storage::open_private_read_file(path)?
        .take(65537)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 65536, "feedback.invalid_config");
    let value: Value = serde_json::from_slice(&bytes)?;
    validate_shape("privateProcessConfig", &value)?;
    let config: ProcessConfig = serde_json::from_value(value)?;
    ensure!(
        config.authorization_token.len() == 64
            && config
                .authorization_token
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            && digest(&serde_json::to_value(&config.binding)?)? == config.binding_digest,
        "feedback.invalid_config"
    );
    endpoint(&config.core_endpoint)?;
    Ok(config)
}
async fn answer(writer: &mut tokio::io::Stdout, value: &Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    ensure!(bytes.len() <= MAX_FRAME_BYTES, "result_too_large");
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}
pub fn run_if_requested() -> Result<bool> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_none_or(|arg| arg != "--ui-feedback-mcp") {
        return Ok(false);
    }
    ensure!(
        args.len() == 3 && args[1] == "--config",
        "feedback.invalid_cli"
    );
    let config = load(Path::new(&args[2]))?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(serve(config))?;
    Ok(true)
}
fn error_code(error: &anyhow::Error) -> &'static str {
    match error.to_string().as_str() {
        "feature_disabled" => "feature_disabled",
        "runtime_unavailable" => "runtime_unavailable",
        "scope_changed" => "scope_changed",
        "run_not_active" => "run_not_active",
        "epoch_changed" => "epoch_changed",
        "binding_changed" => "binding_changed",
        "member_changed" => "member_changed",
        "policy_changed" => "policy_changed",
        "result_too_large" => "result_too_large",
        "original_request_pending" => "original_request_pending",
        "feedback_unknown" => "feedback_unknown",
        _ => "transport_failed",
    }
}
async fn serve(config: ProcessConfig) -> Result<()> {
    let mut input = BufReader::new(tokio::io::stdin());
    let mut output = tokio::io::stdout();
    let mut initialized = false;
    loop {
        let mut frame = Vec::new();
        let read = (&mut input)
            .take(MAX_FRAME_BYTES as u64 + 1)
            .read_until(b'\n', &mut frame)
            .await?;
        if read == 0 {
            break;
        }
        ensure!(
            frame.len() <= MAX_FRAME_BYTES && frame.last() == Some(&b'\n'),
            "feedback.invalid_frame"
        );
        let request: Value = match serde_json::from_slice(&frame) {
            Ok(value) => value,
            Err(_) => {
                answer(&mut output,&json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})).await?;
                continue;
            }
        };
        let Some(id) = request.get("id").cloned() else {
            continue;
        };
        if request["jsonrpc"] != "2.0" || !(id.is_string() || id.is_i64() || id.is_u64()) {
            answer(&mut output,&json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request"}})).await?;
            continue;
        }
        let method = request["method"].as_str().unwrap_or("");
        if method == "initialize" {
            let proposed = request["params"]["protocolVersion"].as_str().unwrap_or("");
            let protocol = if matches!(proposed, "2024-11-05" | "2025-03-26" | "2025-06-18") {
                proposed
            } else {
                "2025-06-18"
            };
            answer(&mut output,&json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":protocol,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":SERVER_NAME,"version":"1"}}})).await?;
            initialized = true;
            continue;
        }
        if !initialized {
            answer(&mut output,&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"Initialize first"}})).await?;
            continue;
        }
        if method == "ping" {
            answer(&mut output, &json!({"jsonrpc":"2.0","id":id,"result":{}})).await?;
            continue;
        }
        if method == "tools/list" {
            let tools = config
                .request("catalog", json!({}))
                .await
                .unwrap_or_else(|_| json!([]));
            answer(
                &mut output,
                &json!({"jsonrpc":"2.0","id":id,"result":{"tools":tools}}),
            )
            .await?;
            continue;
        }
        if method != "tools/call" {
            answer(&mut output,&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}})).await?;
            continue;
        }
        let arguments = request["params"]
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if request["params"]["name"] != "check_user_feedback" || !arguments.is_object() {
            answer(
                &mut output,
                &json!({"jsonrpc":"2.0","id":id,"result":failure("rejected","binding_changed")}),
            )
            .await?;
            continue;
        }
        let params = json!({"schemaVersion":1,"bindingDigest":config.binding_digest,"transportRpcId":id,"argumentsDigest":digest(&arguments)?});
        let first = config.request("prepare", params.clone()).await;
        let prepared = match first {
            Ok(value) => value,
            Err(error) => {
                let code = error_code(&error);
                answer(
                    &mut output,
                    &json!({"jsonrpc":"2.0","id":id,"result":failure("unavailable",code)}),
                )
                .await?;
                continue;
            }
        };
        // 写出前复核同一冻结请求，关闭或撤权时不发送已准备的旧正文。
        let checked = config.request("prepare", params).await;
        if !matches!(checked,Ok(ref value) if value==&prepared) {
            answer(
                &mut output,
                &json!({"jsonrpc":"2.0","id":id,"result":failure("unavailable","binding_changed")}),
            )
            .await?;
            continue;
        }
        let result = &prepared["result"];
        ensure!(
            digest(result)?
                == prepared["resultDigest"]
                    .as_str()
                    .context("transport_failed")?,
            "transport_failed"
        );
        let written = answer(
            &mut output,
            &json!({"jsonrpc":"2.0","id":id,"result":result}),
        )
        .await;
        let receipt = json!({"schemaVersion":1,"attemptId":prepared["attemptId"],"bindingDigest":config.binding_digest,"resultDigest":prepared["resultDigest"],"writeStatus":if written.is_ok(){"written"}else{"unknown"}});
        let _ = config.request("wrote", receipt).await;
        if let Err(error) = written {
            return Err(error);
        }
    }
    let _ = config.request("disconnect", json!({})).await;
    Ok(())
}
