//! Electron 独占的子进程管道；不监听网络，不读取 Core 数据库或模型凭据。
use anyhow::{Result,ensure};
use serde::Deserialize;
use serde_json::{json,Value};
use std::sync::Arc;
use tokio::io::{AsyncReadExt,AsyncWriteExt};

const MAX_IN:usize=1024*1024;
const MAX_OUT:usize=32*1024*1024;
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Init{protocol_version:u32,desktop_peer_id:String,owner_window_id:String,token:String}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request{id:String,token:String,method:String,params:Value}
async fn read(input:&mut tokio::io::Stdin)->Result<Option<Value>>{
    let mut length=[0_u8;4];
    match input.read_exact(&mut length).await{Ok(_)=>{},Err(error)if error.kind()==std::io::ErrorKind::UnexpectedEof=>return Ok(None),Err(error)=>return Err(error.into())}
    let length=u32::from_le_bytes(length)as usize;ensure!(length<=MAX_IN,"computer.frame_limit");
    let mut bytes=vec![0;length];input.read_exact(&mut bytes).await?;Ok(Some(serde_json::from_slice(&bytes)?))
}
fn token_matches(left:&str,right:&str)->bool{if left.len()!=64||right.len()!=64{return false}left.as_bytes().iter().zip(right.as_bytes()).fold(0_u8,|difference,(left,right)|difference|(left^right))==0}
pub fn run_if_requested()->Result<bool>{
    if std::env::args().nth(1).as_deref()!=Some("--interaction-desktop"){return Ok(false)}
    ensure!(std::env::args_os().count()==2,"computer.invalid_cli");
    tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?.block_on(serve())?;Ok(true)
}
async fn serve()->Result<()> {
    let mut input=tokio::io::stdin();
    let init:Init=serde_json::from_value(read(&mut input).await?.ok_or_else(||anyhow::anyhow!("computer.handshake_required"))?)?;
    ensure!(init.protocol_version==1&&init.token.len()==64&&init.token.bytes().all(|byte|byte.is_ascii_hexdigit())&&!init.owner_window_id.is_empty()&&init.owner_window_id.len()<=256,"computer.invalid_handshake");uuid::Uuid::parse_str(&init.desktop_peer_id)?;
    let (sender,mut receiver)=tokio::sync::mpsc::channel::<Value>(64);
    let events=sender.clone();
    let registry=Arc::new(crate::native_registry::NativeRegistry::new(move|kind,payload|{let _=events.try_send(json!({"event":kind,"payload":payload}));}));
    let writer=tokio::spawn(async move{let mut output=tokio::io::stdout();while let Some(value)=receiver.recv().await{let bytes=serde_json::to_vec(&value)?;ensure!(bytes.len()<=MAX_OUT,"computer.result_too_large");output.write_all(&(bytes.len()as u32).to_le_bytes()).await?;output.write_all(&bytes).await?;output.flush().await?}Ok::<(),anyhow::Error>(())});
    sender.send(json!({"protocolVersion":1,"ready":true,"desktopPeerId":init.desktop_peer_id,"stopGeneration":0})).await?;
    let mut tasks=tokio::task::JoinSet::new();let capacity=Arc::new(tokio::sync::Semaphore::new(32));
    while let Some(value)=read(&mut input).await?{
        while tasks.try_join_next().is_some(){}
        let request:Request=serde_json::from_value(value)?;ensure!(token_matches(&init.token,&request.token),"computer.invalid_authentication");uuid::Uuid::parse_str(&request.id)?;
        let permit=if matches!(request.method.as_str(),"stop_all"|"stop_scope"){None}else{Some(capacity.clone().try_acquire_owned()?)};
        let sender=sender.clone();let registry=registry.clone();
        tasks.spawn(async move{let _permit=permit;let result=registry.dispatch(&request.method,request.params).await;let response=match result{Ok(result)=>json!({"id":request.id,"ok":true,"result":result}),Err(error)=>json!({"id":request.id,"ok":false,"error":error.to_string()})};let _=sender.send(response).await;});
    }
    registry.stop_all().await;
    tasks.abort_all();while tasks.join_next().await.is_some(){}
    drop(registry);drop(sender);
    tokio::time::timeout(std::time::Duration::from_secs(5),writer).await???;
    Ok(())
}
