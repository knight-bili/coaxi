//! 独立 MCP stdio 进程。私有 IPC 不复用或放宽内置 CLI/Envelope 合同。
use super::*;
use crate::{builtin_tool_transport::LocalIpcEndpoint, platform::local_ipc::LocalIpcClientStream};
use std::{collections::HashMap, io::Read, sync::Arc};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, sync::Mutex};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessConfig {
    pub schema_version: u32,
    pub core_endpoint: LocalIpcEndpoint,
    pub bridge_instance_id: String,
    pub secret: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IpcRequest {
    pub schema_version: u32,
    pub bridge_instance_id: String,
    pub secret: String,
    pub operation: String,
    pub params: Value,
}
impl ProcessConfig {
    pub async fn request(&self, operation: &str, params: Value) -> Result<Value> {
        let request = IpcRequest { schema_version:1, bridge_instance_id:self.bridge_instance_id.clone(), secret:self.secret.clone(), operation:operation.to_owned(), params };
        let mut bytes = serde_json::to_vec(&request)?;
        ensure!(bytes.len() <= MAX_REQUEST_BYTES, "invalid_arguments"); bytes.push(b'\n');
        let stream = LocalIpcClientStream::connect(&self.core_endpoint).await?;
        let (reader, mut writer) = tokio::io::split(stream);
        writer.write_all(&bytes).await?; writer.shutdown().await?;
        let mut frame = Vec::new();
        let mut reader = BufReader::new(reader.take(MAX_RESULT_BYTES as u64 + 1));
        tokio::time::timeout(std::time::Duration::from_secs(30), reader.read_until(b'\n', &mut frame)).await??;
        ensure!(frame.last() == Some(&b'\n') && frame.len() <= MAX_RESULT_BYTES, "transport_failed");
        let response: Value = serde_json::from_slice(&frame)?;
        ensure!(response["schemaVersion"] == 1, "transport_failed");
        if response["ok"] != true { anyhow::bail!("{}", response["code"].as_str().unwrap_or("transport_failed")); }
        Ok(response["result"].clone())
    }
}
fn load(path: &Path) -> Result<ProcessConfig> {
    ensure!(path.is_absolute(), "interaction.absolute_config_required");
    let mut bytes = Vec::new();
    crate::platform::private_storage::open_private_read_file(path)?.take(65_537).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 65_536, "interaction.invalid_config");
    let config: ProcessConfig = serde_json::from_slice(&bytes)?;
    config.core_endpoint.validate()?;
    uuid::Uuid::parse_str(&config.bridge_instance_id)?;
    ensure!(config.schema_version == 1 && config.secret.len() == 64 && config.secret.bytes().all(|byte| byte.is_ascii_hexdigit()), "interaction.invalid_config");
    Ok(config)
}
async fn answer(output: &Mutex<tokio::io::Stdout>, value: &Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    ensure!(bytes.len() <= MAX_RESULT_BYTES, "result_too_large"); bytes.push(b'\n');
    let mut writer = output.lock().await;
    writer.write_all(&bytes).await?; writer.flush().await?;
    Ok(())
}
pub fn bounded_model_result(value: Value) -> Result<Value> {
    use base64::Engine;
    ensure!(serde_json::to_vec(&value)?.len() <= MAX_RESULT_BYTES, "result_too_large");
    let blocks = value["content"].as_array().context("transport_failed")?;
    for block in blocks {
        if block["type"] == "image" {
            ensure!(matches!(block["mimeType"].as_str(), Some("image/png" | "image/jpeg")), "transport_failed");
            let data = block["data"].as_str().context("transport_failed")?;
            ensure!(data.len() <= MAX_IMAGE_BYTES.div_ceil(3) * 4, "result_too_large");
            let image = base64::engine::general_purpose::STANDARD.decode(data)?;
            ensure!(image.len() <= MAX_IMAGE_BYTES, "result_too_large");
        }
    }
    Ok(value)
}
pub fn run_if_requested() -> Result<bool> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_none_or(|arg| arg != "--interaction-mcp") { return Ok(false); }
    ensure!(args.len() == 3 && args[1] == "--config", "interaction.invalid_cli");
    let config = load(Path::new(&args[2]))?;
    tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?.block_on(serve(config))?;
    Ok(true)
}
async fn serve(config: ProcessConfig) -> Result<()> {
    let config = Arc::new(config);
    let output = Arc::new(Mutex::new(tokio::io::stdout()));
    let calls = Arc::new(Mutex::new(HashMap::<String, (String, Value)>::new()));
    let capacity = Arc::new(tokio::sync::Semaphore::new(32));
    let mut tasks = tokio::task::JoinSet::new();
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut initialized = false;
    loop {
        while tasks.try_join_next().is_some() {}
        let mut frame = Vec::new();
        let read = (&mut reader).take(MAX_REQUEST_BYTES as u64 + 1).read_until(b'\n', &mut frame).await?;
        if read == 0 { break; }
        ensure!(frame.len() <= MAX_REQUEST_BYTES && frame.last() == Some(&b'\n'), "invalid_arguments");
        let request: Value = match serde_json::from_slice(&frame) { Ok(value) => value, Err(_) => { answer(&output, &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})).await?; continue; } };
        let id = request.get("id").cloned();
        let method = request["method"].as_str().unwrap_or("");
        if method == "notifications/cancelled" {
            let key = serde_json::to_string(&request["params"]["requestId"])?;
            if let Some((call_id, _)) = calls.lock().await.get(&key).cloned() { let _ = config.request("cancel", json!({"bridgeCallId":call_id})).await; }
            continue;
        }
        let Some(id) = id else { continue; };
        if request["jsonrpc"] != "2.0" || !(id.is_string() || id.is_i64() || id.is_u64()) {
            answer(&output, &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request"}})).await?; continue;
        }
        if method == "initialize" {
            let proposed = request["params"]["protocolVersion"].as_str().unwrap_or("");
            let protocol = if matches!(proposed, "2024-11-05" | "2025-03-26" | "2025-06-18") { proposed } else { "2025-06-18" };
            answer(&output, &json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":protocol,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":SERVER_NAME,"version":"1"}}})).await?;
            initialized = true; continue;
        }
        if !initialized { answer(&output, &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"Initialize first"}})).await?; continue; }
        if method == "ping" { answer(&output, &json!({"jsonrpc":"2.0","id":id,"result":{}})).await?; continue; }
        if method == "tools/list" {
            let tools = config.request("catalog", json!({})).await;
            let result = match tools { Ok(tools) => json!({"tools":tools}), Err(_) => json!({"tools":[]}) };
            answer(&output, &json!({"jsonrpc":"2.0","id":id,"result":result})).await?; continue;
        }
        if method != "tools/call" { answer(&output, &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}})).await?; continue; }
        let name = request["params"]["name"].as_str().unwrap_or("").to_owned();
        let arguments = request["params"].get("arguments").cloned().unwrap_or_else(|| json!({}));
        let key = serde_json::to_string(&id)?;
        let mut known = calls.lock().await;
        let identity = json!({"toolName":name,"arguments":arguments});
        let call_id = if let Some((call_id, original)) = known.get(&key) {
            if original != &identity { drop(known); answer(&output, &json!({"jsonrpc":"2.0","id":id,"result":failure("rejected", "invalid_arguments")})).await?; continue; }
            call_id.clone()
        } else { let call_id = uuid::Uuid::new_v4().to_string(); known.insert(key, (call_id.clone(), identity)); call_id };
        drop(known);
        let Ok(permit) = capacity.clone().try_acquire_owned() else { answer(&output, &json!({"jsonrpc":"2.0","id":id,"result":failure("rejected", "original_request_pending")})).await?; continue; };
        let output = output.clone(); let config = config.clone();
        tasks.spawn(async move {
            let _permit = permit;
            let result = invoke(&config, &call_id, id.clone(), &name, arguments).await.unwrap_or_else(|_| failure("unavailable", "transport_failed"));
            let _ = answer(&output, &json!({"jsonrpc":"2.0","id":id,"result":result})).await;
        });
    }
    tasks.abort_all();
    let _ = config.request("disconnect", json!({})).await;
    Ok(())
}
async fn invoke(config: &ProcessConfig, call_id: &str, rpc_id: Value, name: &str, arguments: Value) -> Result<Value> {
    let call = json!({"schemaVersion":1,"bridgeInstanceId":config.bridge_instance_id,"bridgeCallId":call_id,"rpcId":rpc_id,"nativeToolCallId":null,"toolName":name,"argumentsDigest":super::broker::digest(&arguments)?,"arguments":arguments});
    config.request("call", call).await?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(600);
    loop {
        let response = config.request("result", json!({"bridgeCallId":call_id})).await?;
        match response["state"].as_str() {
            Some("completed") => return bounded_model_result(super::renderers::render(name, &response["result"])),
            Some("prepared" | "started") => {}
            _ => return bounded_model_result(response["result"].clone()),
        }
        if tokio::time::Instant::now() >= deadline { let _ = config.request("cancel", json!({"bridgeCallId":call_id})).await; return Ok(failure("unknown", "unknown_outcome")); }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}
