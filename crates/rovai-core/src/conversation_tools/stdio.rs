//! 独立stdio通道只转发四个已冻结工具，不开放User RPC。
use super::broker::Binding;
use super::*;
use crate::{builtin_tool_transport::LocalIpcEndpoint, platform::local_ipc::LocalIpcClientStream};
use std::{collections::HashMap, io::Read, path::Path, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::Mutex,
};
use tokio_util::sync::CancellationToken;

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
pub fn endpoint(value: &str) -> Result<LocalIpcEndpoint> {
    #[cfg(windows)]
    let endpoint = LocalIpcEndpoint::WindowsNamedPipe { name: value.into() };
    #[cfg(unix)]
    let endpoint = LocalIpcEndpoint::UnixSocket { path: value.into() };
    #[cfg(not(any(windows, unix)))]
    anyhow::bail!("conversation_tools.runtime_unavailable");
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
        ensure!(bytes.len() < MAX_FRAME, "conversation_tools.invalid_input");
        bytes.push(b'\n');
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            let stream = LocalIpcClientStream::connect(&endpoint(&self.core_endpoint)?).await?;
            let (reader, mut writer) = tokio::io::split(stream);
            writer.write_all(&bytes).await?;
            writer.shutdown().await?;
            let mut frame = Vec::new();
            let mut reader = BufReader::new(reader.take(MAX_FRAME as u64 + 1));
            reader.read_until(b'\n', &mut frame).await?;
            ensure!(
                frame.len() <= MAX_FRAME && frame.last() == Some(&b'\n'),
                "conversation_tools.runtime_unavailable"
            );
            let response: Value = serde_json::from_slice(&frame)?;
            ensure!(
                response["schemaVersion"] == 1,
                "conversation_tools.runtime_unavailable"
            );
            if response["ok"] != true {
                anyhow::bail!(
                    "{}",
                    response["code"].as_str().unwrap_or("runtime_unavailable")
                )
            }
            Ok(response["result"].clone())
        })
        .await
        .context("conversation_tools.runtime_unavailable")?
    }
}
fn load(path: &Path) -> Result<ProcessConfig> {
    ensure!(path.is_absolute(), "conversation_tools.config_invalid");
    let mut bytes = Vec::new();
    crate::platform::private_storage::open_private_read_file(path)?
        .take(65537)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 65536, "conversation_tools.config_invalid");
    let config: ProcessConfig = serde_json::from_slice(&bytes)?;
    config.binding.validate()?;
    ensure!(
        config.schema_version == 1
            && config.max_frame_bytes == MAX_FRAME
            && config.authorization_token.len() == 64
            && config
                .authorization_token
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            && digest(&serde_json::to_value(&config.binding)?)? == config.binding_digest,
        "conversation_tools.config_invalid"
    );
    endpoint(&config.core_endpoint)?;
    Ok(config)
}
async fn write(output: &Arc<Mutex<tokio::io::Stdout>>, value: &Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    ensure!(
        bytes.len() < MAX_FRAME,
        "conversation_tools.result_too_large"
    );
    bytes.push(b'\n');
    let mut output = output.lock().await;
    output.write_all(&bytes).await?;
    output.flush().await?;
    Ok(())
}
pub fn run_if_requested() -> Result<bool> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args
        .first()
        .is_none_or(|value| value != "--conversation-tools-mcp")
    {
        return Ok(false);
    }
    ensure!(
        args.len() == 3 && args[1] == "--config",
        "conversation_tools.config_invalid"
    );
    let config = load(Path::new(&args[2]))?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(serve(config))?;
    Ok(true)
}
async fn call(
    config: ProcessConfig,
    output: Arc<Mutex<tokio::io::Stdout>>,
    id: Value,
    params: Value,
    cancel: CancellationToken,
) -> Result<()> {
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let name = params["name"].as_str().unwrap_or("");
    let request_id = arguments["request_id"].as_str().map(str::to_owned);
    let response = config
        .request(
            "call",
            json!({"name":name,"arguments":arguments,"rpcId":id}),
        )
        .await;
    let response=match response{Ok(response)=>response,Err(error)=>return write(&output,&json!({"jsonrpc":"2.0","id":id,"result":broker::failure(&broker::error_code(&error),request_id.as_deref())})).await};
    if response["state"] != "awaiting_answer" {
        return write(
            &output,
            &json!({"jsonrpc":"2.0","id":id,"result":response["result"]}),
        )
        .await;
    }
    let request_id = response["requestId"]
        .as_str()
        .context("conversation_tools.request_id_required")?;
    let mut locator =
        json!({"requestId":request_id,"rpcId":id,"argumentsDigest":digest(&arguments)?});
    loop {
        tokio::select! {
            _=cancel.cancelled()=>{let _=config.request("cancel",locator.clone()).await;return Ok(())},
            _=tokio::time::sleep(std::time::Duration::from_millis(500))=>{},
        }
        let response=match config.request("poll",locator.clone()).await{Ok(response)=>response,Err(error)=>return write(&output,&json!({"jsonrpc":"2.0","id":id,"result":broker::failure(broker::error_code(&error),Some(request_id))})).await};
        match response["state"].as_str(){
            Some("awaiting_answer"|"prepared")=>continue,
            Some("claimed")=>{
                let result=&response["result"];ensure!(digest(&result["structuredContent"])?==response["resultDigest"].as_str().context("conversation_tools.receipt_required")?,"conversation_tools.delivery_changed");
                let written=write(&output,&json!({"jsonrpc":"2.0","id":id,"result":result})).await;
                locator["receiptDigest"]=response["receiptDigest"].clone();locator["resultDigest"]=response["resultDigest"].clone();locator["written"]=json!(written.is_ok());
                // 重试的只是同一写入回执；绝不再次把答案写到stdout。
                for attempt in 0..3{if config.request("wrote",locator.clone()).await.is_ok(){break}if attempt<2{tokio::time::sleep(std::time::Duration::from_millis(300)).await;}}
                return written
            },
            _=>return write(&output,&json!({"jsonrpc":"2.0","id":id,"result":broker::failure(if response["state"]=="outcome_unknown"{"outcome_unknown"}else{"request_cancelled"},Some(request_id))})).await,
        }
    }
}
async fn serve(config: ProcessConfig) -> Result<()> {
    let output = Arc::new(Mutex::new(tokio::io::stdout()));
    let mut input = BufReader::new(tokio::io::stdin());
    let mut frame = Vec::new();
    let mut initialized = false;
    let calls: Arc<Mutex<HashMap<String, CancellationToken>>> = Default::default();
    let mut tasks = tokio::task::JoinSet::<Result<()>>::new();
    let result:Result<()>=async{
        loop{
            let mut bounded=(&mut input).take(MAX_FRAME as u64+1);
            let count=tokio::select!{
                done=tasks.join_next(),if !tasks.is_empty()=>{if let Some(done)=done{done??;}continue},
                count=bounded.read_until(b'\n',&mut frame)=>count?,
            };
            if count==0{break}ensure!(frame.len()<=MAX_FRAME&&frame.last()==Some(&b'\n'),"conversation_tools.invalid_input");
            let parsed=serde_json::from_slice::<Value>(&frame);frame.clear();let request=match parsed{Ok(value)=>value,Err(_)=>{write(&output,&json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})).await?;continue}};
            if request["method"]=="notifications/cancelled"{let key=serde_json::to_string(&request["params"]["requestId"])?;if let Some(token)=calls.lock().await.get(&key){token.cancel();}continue}
            let Some(id)=request.get("id").cloned()else{continue};
            ensure!(request["jsonrpc"]=="2.0"&&(id.is_string()||id.is_i64()||id.is_u64())&&serde_json::to_vec(&id)?.len()<=1024,"conversation_tools.invalid_input");
            match request["method"].as_str(){
                Some("initialize")=>{let proposed=request["params"]["protocolVersion"].as_str().unwrap_or("");let protocol=if matches!(proposed,"2024-11-05"|"2025-03-26"|"2025-06-18"){proposed}else{"2025-06-18"};write(&output,&json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":protocol,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":SERVER_NAME,"version":"1"}}})).await?;initialized=true;},
                _ if !initialized=>write(&output,&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"Initialize first"}})).await?,
                Some("ping")=>write(&output,&json!({"jsonrpc":"2.0","id":id,"result":{}})).await?,
                Some("tools/list")=>{let tools=config.request("catalog",json!({})).await;write(&output,&crate::mcp::tool_discovery::response(id,tools)).await?;},
                Some("tools/call")=>{
                    let key=serde_json::to_string(&id)?;let token=CancellationToken::new();let mut pending=calls.lock().await;
                    if pending.len()>=16||pending.contains_key(&key){drop(pending);write(&output,&json!({"jsonrpc":"2.0","id":id,"result":broker::failure("capacity_reached",None)})).await?;continue}
                    pending.insert(key.clone(),token.clone());drop(pending);let config=config.clone();let output=output.clone();let calls=calls.clone();let params=request["params"].clone();
                    tasks.spawn(async move{let result=call(config,output,id,params,token).await;calls.lock().await.remove(&key);result});
                },
                _=>write(&output,&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}})).await?,
            }
        }Ok(())
    }.await;
    for token in calls.lock().await.values() {
        token.cancel();
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    let _ = config.request("disconnect", json!({})).await;
    result
}
