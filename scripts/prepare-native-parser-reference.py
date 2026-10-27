from pathlib import Path
import argparse, hashlib, json, re, shutil, subprocess

arguments=argparse.ArgumentParser(description="Prepare unchanged fixed Coaxi parser components for readonly differential fixtures")
arguments.add_argument("--source-root",type=Path,required=True)
arguments.add_argument("--output",type=Path,required=True)
args=arguments.parse_args()
recipe=Path(__file__).resolve().parents[1]/"integration/verification/native-parser-reference"
fixed=json.loads((recipe/"source-lock.json").read_text("utf-8"))
donor=args.source_root.resolve()
destination=args.output.resolve()
assert not destination.exists(), 'One-shot reference scaffold; preserve subsequent fixes'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=donor,text=True).strip()=='aff1e6c278453ed8d0b96a5f664edcdca621aa3f'
(destination/'src/parsers').mkdir(parents=True)
(destination/'src/acp').mkdir()
(destination/'src/models').mkdir()
sources={}
def read(path):
    data=(donor/path).read_bytes();sources[path]={'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)}
    assert sources[path]==fixed['sources'][path], 'Fixed source drift: '+path
    return data.decode('utf-8').replace('\r\n','\n')
def part(text,start,end):return '\n'.join(text.splitlines()[start-1:end])+'\n'
def function(text,name):
    match=re.search(r'^(?:(?:pub(?:\([^)]*\))?) )?fn '+name+r'\b',text,re.M);assert match,name
    return text[match.start():text.index('\n}',match.start())+3]+'\n'
def write(path,text): (destination/'src'/path).write_text(text,encoding='utf-8',newline='\n')
def copy(path,to,bridge=''):
    read(path)
    (destination/'src'/to).write_bytes((donor/path).read_bytes()+bridge.encode('utf-8'))

copy('src-tauri/src/parsers/codex.rs','parsers/codex.rs','''
pub(crate) fn reference_detail(home: &std::path::Path, path: &std::path::Path, id: &str) -> Result<crate::models::ConversationDetail, super::ParseError> {
    CodexParser { base_dir: home.join("sessions") }.parse_conversation_detail(path, id)
}
''')
copy('src-tauri/src/parsers/grok.rs','parsers/grok.rs','''
pub(crate) fn reference_detail(home: &std::path::Path, path: &std::path::Path, id: &str) -> crate::models::ConversationDetail {
    GrokParser { base_dir: home.join("sessions") }.build_detail(path, id)
}
''')
copy('src-tauri/src/parsers/codex_code_mode.rs','parsers/codex_code_mode.rs')
copy('src-tauri/src/parsers/codex_desktop_attachments.rs','parsers/codex_desktop_attachments.rs')
for name in ['agent_mentions','codex_context','codex_goal','js_text','service_error']:
    copy('src-tauri/src/acp/'+name+'.rs','acp/'+name+'.rs')
copy('src-tauri/src/models/agent.rs','models/agent.rs')
copy('src-tauri/src/models/message.rs','models/message.rs')
copy('src-tauri/src/intern.rs','intern.rs')
conversation=read('src-tauri/src/models/conversation.rs')
write('models/mod.rs','use chrono::{DateTime,Utc};\nuse serde::{Serialize,Deserialize};\npub mod agent;\npub mod message;\npub use agent::*;\npub use message::*;\n'+part(conversation,8,26)+part(conversation,100,129))
types=read('src-tauri/src/acp/types.rs')
write('acp/types.rs','use serde::{Serialize,Deserialize};\n'+part(types,4,33)+part(types,931,941)+part(types,955,1000)+part(types,1077,1192)+part(types,1209,1269))
connection=read('src-tauri/src/acp/connection.rs')
write('acp/connection.rs',part(connection,15685,15698))
air=read('src-tauri/src/acp/air_contract.rs')
write('acp/air_contract.rs','use serde_json::Value;\n'+function(air,'is_codex_web_search_input'))
title=read('src-tauri/src/acp/session_title.rs')
write('acp/session_title.rs',function(title,'native_title_from_session_info'))
write('acp/mod.rs','''pub mod agent_mentions;
pub mod codex_context;
pub mod codex_goal;
pub mod js_text;
pub mod service_error;
pub mod types;
pub mod connection;
pub mod air_contract;
pub mod session_title;
// No DB/native registry participates in a detail parse; built-ins only here.
pub mod custom_registry { pub fn display_name(_: &str) -> Option<String> { None } }
''')
claude=read('src-tauri/src/parsers/claude.rs')
marker=re.search(r'^(?:pub(?:\(crate\))? )?const BACKGROUND_TASK_MARKER:[^;]+;',claude,re.M).group(0)
write('parsers/claude.rs','use super::truncate_str;\n'+marker+'\n'+function(claude,'displayed_session_title'))
pi=read('src-tauri/src/parsers/pi.rs')
write('parsers/pi.rs','use serde_json::Value;\n'+function(pi,'tool_result_content_text'))
shared=read('src-tauri/src/parsers/mod.rs')
error=part(shared,350,362).replace('    #[error("Database error: {0}")]\n    Db(#[from] sea_orm::DbErr),\n','')
text='''use std::collections::{HashMap,HashSet};
use std::sync::OnceLock;
use regex::Regex;
use chrono::{DateTime,Utc};
use crate::models::*;
pub mod codex;
pub mod grok;
pub mod codex_code_mode;
pub mod codex_desktop_attachments;
pub mod claude;
pub mod pi;
// The list cache is outside this harness's detail path.
pub mod summary_cache {
    pub fn get_or_parse<T>(_:crate::models::AgentType,_: &std::path::Path,parse:impl FnOnce()->T)->T { parse() }
}
'''+error+part(shared,364,367)
for name in ['truncate_str','is_safe_subagent_id','user_turn_block','user_turn_block_from_wire','turn_error_message','message_turn_count']:
    text+=function(shared,name)
text+=part(shared,578,753)+part(shared,859,1217)+part(shared,1268,1736)
write('parsers/mod.rs',text)
browser=read('src-tauri/src/browser/types.rs')
handoff=re.search(r'^pub const HANDOFF_BLOCK_HEADER:[^;]+;',browser,re.M).group(0)
main='''#![allow(dead_code)]
// Test fixture only: unchanged fixed donor parser bodies, not product code.
mod acp;
mod models;
mod parsers;
mod intern;
mod browser { pub mod types { '''+handoff+''' } }
fn main() -> Result<(),Box<dyn std::error::Error>> {
    let args:Vec<_>=std::env::args().collect();
    if args.len()!=5 { return Err("adapter home source-path session-id required".into()); }
    let home=std::path::Path::new(&args[2]);
    let path=std::path::Path::new(&args[3]);
    if !home.is_absolute() || !path.canonicalize()?.starts_with(home.canonicalize()?) { return Err("fixture source outside root".into()); }
    let detail=match args[1].as_str() {
        "codex-cli"=>parsers::codex::reference_detail(home,path,&args[4])?,
        "grok-build"=>parsers::grok::reference_detail(home,path,&args[4]),
        _=>return Err("unsupported fixture adapter".into()),
    };
    let summary=serde_json::to_value(&detail.summary)?;
    let selected:serde_json::Map<String,serde_json::Value>=["title","model","git_branch","started_at","ended_at","message_count"].into_iter()
        .map(|name|(name.to_string(),summary[name].clone())).collect();
    println!("{}",serde_json::json!({"turns":detail.turns,"session_stats":detail.session_stats,"summary":selected}));
    Ok(())
}
'''
write('main.rs',main)

# The lock contains only fixed donor versions/checksums; Cargo may download
# missing crates, but a build must use --locked and cannot refresh versions.
read('src-tauri/Cargo.lock')
shutil.copyfile(recipe/"reference-manifest.toml",destination/"Cargo.toml")
shutil.copyfile(recipe/"reference-cargo.lock",destination/"Cargo.lock")
(destination/"source-manifest.json").write_text(json.dumps({"fixed_commit":fixed["commit"],"sources":sources,
    "original_parser_bodies_byte_identical":True,"fixture_only":True,"boundary_bridges":fixed["boundary_bridges"]},ensure_ascii=False,indent=2)+"\n",encoding="utf-8",newline="\n")
print(json.dumps({"prepared":str(destination),"source_files":len(sources),"build":"cargo build --locked --manifest-path <output>/Cargo.toml"},ensure_ascii=False))
