use super::*;
use rusqlite::{Connection,OptionalExtension,Transaction,params};
use std::path::Path;

pub(crate) fn preview(connection:&Connection,root:&Path,client:&str,input:&Value)->Result<Value>{
    validate_operation("conversationTools.grants.preview",input)?;
    let scope:ScopeV1=serde_json::from_value(input["scope"].clone())?;scope::require_scope(connection,&scope)?;
    let source_id=input["sourceSessionId"].as_str().context("conversation_tools.source_required")?;
    let source=match sources::load(connection,source_id){Ok(source)=>source,Err(_)=>sources::register_live(connection,source_id)?};
    require_private_boundary(&source.source_kind,&scope)?;
    let through=input["throughSequence"].as_i64().context("conversation_tools.boundary_required")?;
    let snapshot=source_projection::read(connection,root,&source,Some(through))?;
    ensure!(input["sourceDigest"].as_str()==Some(snapshot.source_digest.as_str()),"conversation_tools.source_changed");
    let now=chrono::Utc::now();let preview_expires=now+chrono::Duration::seconds(600);let grant_expires=now+chrono::Duration::seconds(7200);
    let grant=SourceGrantV1{grant_id:uuid::Uuid::new_v4().to_string(),revision:1,scope:scope.clone(),source_session_id:source.id,source_kind:source.source_kind,source_digest:snapshot.source_digest,through_sequence:snapshot.through_sequence,expires_at:grant_expires.to_rfc3339(),legacy_session_id:source.legacy_session_id};
    validate("SourceGrantV1",&serde_json::to_value(&grant)?)?;
    let id=uuid::Uuid::new_v4().to_string();let mut output=json!({"previewId":id,"previewDigest":"","expiresAt":preview_expires.to_rfc3339(),"grant":grant});
    output["previewDigest"]=json!(digest(&output)?);
    let count:i64=connection.query_row("SELECT COUNT(*) FROM conversation_tool_grant_preview WHERE client_id=?1 AND expires_at_ms>?2",params![client,now.timestamp_millis()],|row|row.get(0))?;
    ensure!(count<4096,"conversation_tools.preview_capacity");
    connection.execute("INSERT INTO conversation_tool_grant_preview(id,client_id,scope_digest,source_id,digest,value_json,expires_at_ms,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![id,client,digest(&serde_json::to_value(scope)?)?,grant.source_session_id,output["previewDigest"].as_str(),serde_json::to_string(&output)?,preview_expires.timestamp_millis(),now.to_rfc3339()])?;
    Ok(output)
}
fn require_private_boundary(source_kind:&str,scope:&ScopeV1)->Result<()>{
    ensure!(source_kind=="public"||!matches!(&scope.scope_kind,ScopeKindV1::Public),"conversation_tools.private_source_requires_private_scope");Ok(())
}
pub(crate) fn confirm_on(tx:&Transaction<'_>,root:&Path,client:&str,preview_id:&str,expected_digest:&str)->Result<SourceGrantV1>{
    uuid::Uuid::parse_str(preview_id)?;
    let(raw,expires,stored_digest):(String,i64,String)=tx.query_row("SELECT value_json,expires_at_ms,digest FROM conversation_tool_grant_preview WHERE id=?1 AND client_id=?2",params![preview_id,client],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).context("conversation_tools.preview_unavailable")?;
    ensure!(expires>chrono::Utc::now().timestamp_millis()&&stored_digest==expected_digest,"conversation_tools.preview_changed");
    let mut preview:Value=serde_json::from_str(&raw)?;preview["previewDigest"]=json!("");ensure!(digest(&preview)?==expected_digest,"conversation_tools.preview_changed");
    let grant:SourceGrantV1=serde_json::from_value(preview["grant"].clone())?;validate("SourceGrantV1",&serde_json::to_value(&grant)?)?;
    scope::require_scope(tx,&grant.scope)?;require_private_boundary(&grant.source_kind,&grant.scope)?;
    let source=sources::load(tx,&grant.source_session_id)?;
    ensure!(source.source_kind==grant.source_kind&&source.legacy_session_id==grant.legacy_session_id,"conversation_tools.source_changed");
    let snapshot=source_projection::read(tx,root,&source,Some(grant.through_sequence))?;
    ensure!(snapshot.source_digest==grant.source_digest,"conversation_tools.source_changed");
    let prior:Option<(String,Option<String>)>=tx.query_row("SELECT value_json,revoked_at FROM conversation_tool_source_grant WHERE id=?1",[&grant.grant_id],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
    if let Some((raw,revoked))=prior{ensure!(revoked.is_none()&&serde_json::from_str::<SourceGrantV1>(&raw)?==grant,"conversation_tools.original_grant_changed");return Ok(grant)}
    let expires=chrono::DateTime::parse_from_rfc3339(&grant.expires_at)?.timestamp_millis();ensure!(expires>chrono::Utc::now().timestamp_millis(),"conversation_tools.grant_expired");
    tx.execute("INSERT INTO conversation_tool_source_grant(id,revision,client_id,scope_digest,source_id,value_json,expires_at_ms,created_at) VALUES(?1,1,?2,?3,?4,?5,?6,?7)",params![grant.grant_id,client,digest(&serde_json::to_value(&grant.scope)?)?,grant.source_session_id,serde_json::to_string(&grant)?,expires,chrono::Utc::now().to_rfc3339()])?;
    Ok(grant)
}
pub(crate) fn list(connection:&Connection,input:&Value)->Result<Value>{
    validate_operation("conversationTools.grants.list",input)?;let scope:ScopeV1=serde_json::from_value(input["scope"].clone())?;
    scope::require_scope(connection,&scope)?;let key=digest(&serde_json::to_value(&scope)?)?;
    let limit=input["limit"].as_u64().context("conversation_tools.limit_required")?;
    let mut query=connection.prepare("SELECT value_json FROM conversation_tool_source_grant WHERE scope_digest=?1 AND revoked_at IS NULL AND expires_at_ms>?2 AND (?3 IS NULL OR id>?3) ORDER BY id LIMIT ?4")?;
    let mut values=query.query_map(params![key,chrono::Utc::now().timestamp_millis(),input["after"].as_str(),limit+1],|row|row.get::<_,String>(0))?.map(|row|Ok(serde_json::from_str::<SourceGrantV1>(&row?)?)).collect::<Result<Vec<_>>>()?;
    let more=values.len()>limit as usize;values.truncate(limit as usize);
    for grant in &values{ensure!(grant.scope==scope,"conversation_tools.scope_changed");validate("SourceGrantV1",&serde_json::to_value(grant)?)?;}
    let next=if more{values.last().map(|value|value.grant_id.clone())}else{None};Ok(json!({"items":values,"next":next}))
}
pub(crate) fn revoke_on(tx:&Transaction<'_>,id:&str,expected:i64)->Result<Value>{
    let(raw,revision,revoked):(String,i64,Option<String>)=tx.query_row("SELECT value_json,revision,revoked_at FROM conversation_tool_source_grant WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).context("conversation_tools.grant_unavailable")?;
    let grant:SourceGrantV1=serde_json::from_str(&raw)?;scope::require_scope(tx,&grant.scope)?;
    ensure!(revision==expected,"conversation_tools.grant_changed");
    if revoked.is_none(){ensure!(tx.execute("UPDATE conversation_tool_source_grant SET revision=revision+1,revoked_at=?3 WHERE id=?1 AND revision=?2 AND revoked_at IS NULL",params![id,expected,chrono::Utc::now().to_rfc3339()])?==1,"conversation_tools.grant_changed");}
    Ok(json!({"grantId":id,"revoked":true}))
}
/// 新Run只能冻结本次User输入明确选中的来源，不读取同scope的其他授权补位。
pub(crate) fn selected(connection:&Connection,scope:&ScopeV1,source_ids:&[String])->Result<Vec<SourceGrantV1>>{
    ensure!(source_ids.len()<=128,"conversation_tools.source_limit");let key=digest(&serde_json::to_value(scope)?)?;let mut result=Vec::new();
    let mut seen=std::collections::HashSet::new();
    for source in source_ids{uuid::Uuid::parse_str(source)?;if !seen.insert(source){continue}
        let raw:Option<String>=connection.query_row("SELECT value_json FROM conversation_tool_source_grant WHERE scope_digest=?1 AND source_id=?2 AND revoked_at IS NULL AND expires_at_ms>?3 ORDER BY created_at DESC,id DESC LIMIT 1",params![key,source,chrono::Utc::now().timestamp_millis()],|row|row.get(0)).optional()?;
        let Some(raw)=raw else{continue};let grant:SourceGrantV1=serde_json::from_str(&raw)?;ensure!(&grant.scope==scope,"conversation_tools.scope_changed");require_private_boundary(&grant.source_kind,scope)?;result.push(grant);
    }Ok(result)
}
pub(crate) fn require_current(connection:&Connection,scope:&ScopeV1,grant:&SourceGrantV1)->Result<()>{
    ensure!(&grant.scope==scope,"conversation_tools.scope_changed");require_private_boundary(&grant.source_kind,scope)?;
    let raw:Option<String>=connection.query_row("SELECT value_json FROM conversation_tool_source_grant WHERE id=?1 AND revision=?2 AND revoked_at IS NULL AND expires_at_ms>?3",params![grant.grant_id,grant.revision,chrono::Utc::now().timestamp_millis()],|row|row.get(0)).optional()?;
    ensure!(raw.map(|raw|serde_json::from_str::<SourceGrantV1>(&raw)).transpose()?.as_ref()==Some(grant),"conversation_tools.grant_unavailable");Ok(())
}
