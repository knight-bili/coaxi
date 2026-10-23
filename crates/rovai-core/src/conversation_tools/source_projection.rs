//! 会话查询只投影可见文字和工具名称；原日志块、附件、思考及主机定位不会序列化。
use super::*;
use super::sources::{Source,Locator};
use rusqlite::{Connection,params};
use std::path::Path;

pub(crate) struct Snapshot{
    pub source_digest:String,pub through_sequence:i64,pub total:i64,pub items:Vec<Value>,pub metadata:Value,pub text_truncated:bool,
}
fn limited(text:&str,maximum:usize)->String{text.chars().filter(|character|*character!='\0').take(maximum).collect()}
fn stats_unavailable()->Value{json!({"total_usage":null,"coverage":"unavailable"})}
fn finish(source:&Source,through:i64,total:i64,items:Vec<Value>,metadata:Value,truncated:bool)->Result<Snapshot>{
    let source_digest=digest(&json!({"sourceSessionId":source.id,"sourceKind":source.source_kind,"throughSequence":through,"total":total,"items":items,"metadata":metadata}))?;
    Ok(Snapshot{source_digest,through_sequence:through,total,items,metadata,text_truncated:truncated})
}
pub(crate) fn live(connection:&Connection,source:&Source,through:Option<i64>)->Result<Snapshot>{
    let Locator::Live{conversation_id}=&source.locator else{anyhow::bail!("conversation_tools.live_source_required")};
    let(title,created,kind):(String,String,String)=connection.query_row("SELECT COALESCE(camp.title,''),c.created_at,c.kind FROM conversation c JOIN camp ON camp.id=c.camp_id WHERE c.id=?1 AND camp.id=?2 AND camp.deletion_operation_id IS NULL",params![conversation_id,source.owner_thread_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
    let public=source.source_kind=="public";ensure!(public==(kind=="camp_member"),"conversation_tools.source_changed");
    let prefix=if public{format!("WITH {} ",crate::camp_message_publication::public_camp_message_publication_cte())}else{String::new()};
    let from=if public{"camp_message m JOIN public_camp_message_publication p ON p.message_id=m.id"}else{"conversation_message m"};
    let scope=if public{"m.camp_id=?1 AND m.tombstoned_at IS NULL AND m.recall_state<>'withdrawn'"}else{"m.conversation_id=?1"};
    let owner=if public{&source.owner_thread_id}else{conversation_id};
    let maximum:i64=connection.query_row(&format!("{prefix}SELECT COALESCE(MAX(m.sequence),0) FROM {from} WHERE {scope}"),[owner],|row|row.get(0))?;
    let through=through.unwrap_or(maximum);ensure!(through>=0&&through<=maximum,"conversation_tools.source_boundary_changed");
    let total:i64=connection.query_row(&format!("{prefix}SELECT COUNT(*) FROM {from} WHERE {scope} AND m.sequence<=?2"),params![owner,through],|row|row.get(0))?;
    let mut query=connection.prepare(&format!("{prefix}SELECT m.author_type,substr(m.body,1,4096),length(m.body),m.created_at FROM {from} WHERE {scope} AND m.sequence<=?2 ORDER BY m.sequence DESC,m.id DESC LIMIT 200"))?;
    let mut rows=query.query_map(params![owner,through],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?,row.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    rows.reverse();let text_truncated=rows.iter().any(|row|row.2>4096);
    let updated=rows.last().map(|row|row.3.clone());
    let items=rows.into_iter().map(|(role,text,_,_)|json!({"role":if role=="agent"{"assistant"}else if role=="user"{"user"}else{"system"},"text":limited(&text,4096),"tools":[]})).collect();
    let mut metadata=json!({"title":limited(&title,4096),"stats":stats_unavailable(),"note":"本次来源投影包含已发布的消息文字；工具名称需使用对应原运行的可见证据核对。"});
    if chrono::DateTime::parse_from_rfc3339(&created).is_ok(){metadata["created_at"]=json!(created);}
    if let Some(updated)=updated.filter(|updated|chrono::DateTime::parse_from_rfc3339(updated).is_ok()){metadata["updated_at"]=json!(updated);}
    finish(source,through,total,items,metadata,text_truncated)
}
pub(crate) fn imported(root:&Path,source:&Source,through:Option<i64>)->Result<Snapshot>{
    let(manifest,transcript)=sources::import_transcript(root,source)?;
    let turns=transcript["turns"].as_array().context("conversation_tools.import_transcript_invalid")?;
    let maximum=i64::try_from(turns.len())?;let through=through.unwrap_or(maximum);
    ensure!(through>=0&&through<=maximum,"conversation_tools.source_boundary_changed");
    let visible=turns.iter().take(through as usize).filter(|turn|matches!(turn["role"].as_str(),Some("user"|"assistant"))).collect::<Vec<_>>();
    let total=i64::try_from(visible.len())?;let mut items=Vec::new();let mut text_truncated=false;
    for turn in visible.into_iter().rev().take(200).collect::<Vec<_>>().into_iter().rev(){
        let blocks=turn["blocks"].as_array().context("conversation_tools.import_blocks_invalid")?;
        let mut text=String::new();let mut tools=Vec::new();
        for block in blocks{
            match block["type"].as_str(){
                Some("text")=>if let Some(part)=block["text"].as_str(){if !text.is_empty(){text.push('\n');}let remaining=4096usize.saturating_sub(text.chars().count());if part.chars().count()>remaining{text_truncated=true;}text.push_str(&limited(part,remaining));},
                Some("tool_use")=>if let Some(name)=block["tool_name"].as_str(){if tools.len()<128{let name=limited(name,256);if !tools.contains(&name){tools.push(name);}}},
                _=>{},
            }
        }
        items.push(json!({"role":turn["role"],"text":limited(&text,4096),"tools":tools}));
    }
    let summary=&transcript["summary"];
    let mut metadata=json!({"stats":stats_unavailable(),"note":"仅返回原导入中可见的文字与工具名称；图片、思考、工具参数和原生日志定位不随本次查询返回。"});
    for(key,source_key,max)in[("title","title",4096),("model","model",512),("git_branch","git_branch",1024),("workspace_name","folder_name",1024)]{
        if let Some(text)=summary[source_key].as_str(){metadata[key]=json!(limited(text,max));}
    }
    if let Some(provider)=manifest["provider"].as_str(){metadata["agent_type"]=json!(limited(provider,128));}
    for(key,source_key)in[("created_at","started_at"),("updated_at","ended_at")]{if let Some(text)=summary[source_key].as_str().filter(|text|chrono::DateTime::parse_from_rfc3339(text).is_ok()){metadata[key]=json!(text);}}
    finish(source,through,total,items,metadata,text_truncated)
}
pub(crate) fn read(connection:&Connection,root:&Path,source:&Source,through:Option<i64>)->Result<Snapshot>{match &source.locator{Locator::Live{..}=>live(connection,source,through),Locator::Imported{..}=>imported(root,source,through)}}
pub(crate) fn info(source:&Source,snapshot:&Snapshot,session_id:&Value,maximum:usize)->Result<Value>{
    ensure!(maximum<=200,"conversation_tools.message_limit");
    let mut result=snapshot.metadata.clone();result["found"]=json!(true);result["session_id"]=session_id.clone();result["is_delegation_child"]=json!(source.source_kind=="delegation_child");result["message_count"]=json!(snapshot.total);
    if maximum>0{
        let mut items=Vec::new();
        for item in snapshot.items.iter().rev().take(maximum){
            let mut candidate=items.clone();candidate.push(item.clone());candidate.reverse();let mut next=result.clone();next["messages"]=json!({"total":snapshot.total,"included":candidate.len(),"truncated":true,"items":candidate});
            if serde_json::to_vec(&next)?.len()>65536{break}items.push(item.clone());
        }
        items.reverse();result["messages"]=json!({"total":snapshot.total,"included":items.len(),"truncated":snapshot.text_truncated||items.len()as i64<snapshot.total,"items":items});
    }
    validate("SessionInfoV1",&result)?;ensure!(serde_json::to_vec(&result)?.len()<=65536,"conversation_tools.session_info_budget");Ok(result)
}
