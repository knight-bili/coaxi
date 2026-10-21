mod scoped_fs;
mod models;
mod parsers;
mod acp;
mod intern;
mod paths;
mod turn_timings;
use anyhow::{Context,Result,ensure};
use serde::Deserialize;
use serde_json::{Value,json};
use std::{io::{Read,Write},path::PathBuf};

#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request{protocol_version:u32,root:PathBuf,provider:String,operation:String,id:Option<String>}
fn run()->Result<Value>{
    let mut input=Vec::new();std::io::stdin().take(65537).read_to_end(&mut input)?;ensure!(input.len()<=65536,"history.request_too_large");let request:Request=serde_json::from_slice(&input)?;ensure!(request.protocol_version==1,"history.protocol_version_unsupported");
    scoped_fs::initialize(&request.root)?;
    let parser=parsers::build_parser_at(&request.provider,scoped_fs::root())?;
    let result=match request.operation.as_str(){
        "list"=>{let entries=parser.list_conversations()?;ensure!(entries.len()<=5000,"history.candidate_limit");serde_json::to_value(entries)?},
        "read"=>{let id=request.id.context("history.session_id_required")?;ensure!(!id.is_empty()&&id.len()<=1024&&!id.contains(['\0','\r','\n'])&&!std::path::Path::new(&id).is_absolute()&&!std::path::Path::new(&id).components().any(|part|matches!(part,std::path::Component::ParentDir)),"history.invalid_session_id");serde_json::to_value(parser.get_conversation(&id)?)?},
        _=>anyhow::bail!("history.operation_not_admitted"),
    };
    Ok(json!({"protocolVersion":1,"result":result}))
}
fn main(){
    match run(){Ok(value)=>match serde_json::to_vec(&value){Ok(bytes)if bytes.len()<=64*1024*1024=>{if std::io::stdout().write_all(&bytes).is_err(){std::process::exit(1)}},_=>{eprintln!("history.result_too_large");std::process::exit(1)}},Err(error)=>{eprintln!("history.import_read_failed: {error}");std::process::exit(1)}}
}
