use super::*;
use crate::{command::{ActorRef,CommandEnvelope,CommandExecution,CommandHandlerResult,DomainCommand,DomainCommandGateway,EntityReference,sealed},db::Database};
use chrono::Utc;
use rusqlite::{Connection,OptionalExtension,params};

#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Upsert{pub expected_revision:i64,pub definition:Value}
impl sealed::Sealed for Upsert{}
impl DomainCommand for Upsert{const TYPE:&'static str="runtime.custom.upsert";const EXACT_REPLAY:bool=true;}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Remove{pub id:String,pub expected_revision:i64}
impl sealed::Sealed for Remove{}
impl DomainCommand for Remove{const TYPE:&'static str="runtime.custom.remove";const EXACT_REPLAY:bool=true;}
fn entity(id:&str)->Option<EntityReference>{Some(EntityReference{entity_type:"custom_runtime_definition".into(),entity_id:id.into()})}
fn reject(code:&str)->CommandHandlerResult{CommandHandlerResult::rejected(code,json!({}))}
pub fn latest(connection:&Connection,id:&str)->Result<Option<Value>>{
    let raw:Option<String>=connection.query_row("SELECT d.value_json FROM custom_runtime_head h JOIN custom_runtime_definition d ON d.id=h.id AND d.revision=h.revision WHERE h.id=?1 AND h.removed=0",[id],|r|r.get(0)).optional()?;
    raw.map(|text|serde_json::from_str(&text).map_err(Into::into)).transpose()
}
pub fn resolve(connection:&Connection,reference:&CustomRuntimeRefV1,selectable:bool)->Result<Value>{
    reference.validate()?;
    let raw:String=connection.query_row("SELECT d.value_json FROM custom_runtime_definition d WHERE d.id=?1 AND d.revision=?2 AND d.digest=?3",params![reference.id,reference.definition_revision,reference.definition_digest],|r|r.get(0)).context("custom_runtime.definition_missing")?;
    let value:Value=serde_json::from_str(&raw)?;validate_definition(&value,false)?;
    if selectable{let head=latest(connection,&reference.id)?.context("custom_runtime.removed")?;ensure!(head["revision"]==value["revision"]&&value["enabled"]==true,"custom_runtime.definition_changed")}
    Ok(value)
}
pub fn list(connection:&Connection,after:Option<&str>,limit:u32)->Result<Value>{
    ensure!((1..=100).contains(&limit),"custom_runtime.limit_invalid");
    let mut statement=connection.prepare("SELECT d.value_json FROM custom_runtime_head h JOIN custom_runtime_definition d ON d.id=h.id AND d.revision=h.revision WHERE h.removed=0 AND (?1 IS NULL OR h.id>?1) ORDER BY h.id LIMIT ?2")?;
    let mut rows=statement.query(params![after,limit+1])?;let mut items=Vec::new();while let Some(row)=rows.next()?{items.push(serde_json::from_str::<Value>(&row.get::<_,String>(0)?)?)}
    let more=items.len()>limit as usize;items.truncate(limit as usize);let next=more.then(||items.last().and_then(|v|v["id"].as_str()).unwrap_or_default().to_owned());Ok(json!({"items":items,"next":next}))
}
fn secret_context(owner:&str,path:&str,name:&str,id:&str,revision:i64)->String{format!("custom-acp:{owner}:{path}:{name}:{id}:{revision}")}
/// 回显只有引用；引用不能跨定义、分发平台或环境变量名借用。
fn freeze_secrets(connection:&Connection,value:&mut Value)->Result<()>{
    let owner=text(value,"id")?.to_owned();
    for path in environments(value){
        let map=value.pointer_mut(&path).and_then(Value::as_object_mut).context("custom_runtime.environment_invalid")?;
        for(name,item)in map.iter_mut(){
            if let Some(plain)=item.as_str(){
                let id=uuid::Uuid::new_v4().to_string();let revision=1i64;
                let cipher=crate::member_environment::encrypt(connection,&secret_context(&owner,&path,name,&id,revision),&plain)?;
                connection.execute("INSERT INTO custom_runtime_secret(id,revision,definition_id,environment_path,variable_name,ciphertext) VALUES(?1,?2,?3,?4,?5,?6)",params![id,revision,owner,path,name,cipher])?;
                *item=json!({"secretId":id,"revision":revision});
            }else{
                let valid:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM custom_runtime_secret WHERE id=?1 AND revision=?2 AND definition_id=?3 AND environment_path=?4 AND variable_name=?5)",params![item["secretId"].as_str(),item["revision"].as_i64(),owner,path,name],|r|r.get(0))?;
                ensure!(valid,"custom_runtime.secret_scope_mismatch");
            }
        }
    }Ok(())
}
/// 唯一明文读取入口供 Core 原生进程创建使用，用户操作及快照不返回此结果。
pub(crate) fn launch_environment(connection:&Connection,definition:&Value,path:&str)->Result<std::collections::BTreeMap<String,String>>{
    let owner=text(definition,"id")?;let values=definition.pointer(path).and_then(Value::as_object).context("custom_runtime.environment_missing")?;let mut result=std::collections::BTreeMap::new();
    for(name,item)in values{
        let id=text(item,"secretId")?;let revision=item["revision"].as_i64().context("custom_runtime.secret_revision_invalid")?;
        let bytes:Vec<u8>=connection.query_row("SELECT ciphertext FROM custom_runtime_secret WHERE id=?1 AND revision=?2 AND definition_id=?3 AND environment_path=?4 AND variable_name=?5",params![id,revision,owner,path,name],|r|r.get(0)).context("custom_runtime.secret_missing")?;
        let plain:String=crate::member_environment::decrypt(connection,&secret_context(owner,path,name,id,revision),&bytes)?;result.insert(name.clone(),plain);
    }Ok(result)
}
pub fn upsert(database:&mut Database,envelope:&CommandEnvelope<Upsert>)->Result<CommandExecution>{
    DomainCommandGateway.execute(database,envelope,|tx|{
        if !matches!(&envelope.actor,ActorRef::User{..}){return Ok(reject("custom_runtime.user_required"))}
        let input=&envelope.payload;let mut value=input.definition.clone();
        if validate_definition(&value,true).is_err()||!(0..9_007_199_254_740_991).contains(&input.expected_revision){return Ok(reject("custom_runtime.definition_invalid"))}
        let id=text(&value,"id")?.to_owned();let previous=latest(tx,&id)?;
        let current:i64=tx.query_row("SELECT COALESCE(MAX(revision),0) FROM custom_runtime_definition WHERE id=?1",[&id],|r|r.get(0))?;
        if current!=input.expected_revision{return Ok(reject("custom_runtime.definition_changed"))}
        if current==0{let count:i64=tx.query_row("SELECT COUNT(*) FROM custom_runtime_head",[],|r|r.get(0))?;if count>=MAX_DEFINITIONS{return Ok(reject("custom_runtime.definition_capacity"))}}
        let collision:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM custom_runtime_head WHERE registry_id=?1 AND id<>?2 AND removed=0)",params![text(&value,"registryId")?,id],|r|r.get(0))?;
        if collision{return Ok(reject("custom_runtime.registry_id_conflict"))}
        // 先确认所有外来引用的归属，失败不留下局部秘密版本。
        for path in environments(&value){for(name,item)in value.pointer(&path).and_then(Value::as_object).context("custom_runtime.environment_invalid")?{if item.is_object(){let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM custom_runtime_secret WHERE id=?1 AND revision=?2 AND definition_id=?3 AND environment_path=?4 AND variable_name=?5)",params![item["secretId"].as_str(),item["revision"].as_i64(),id,path,name],|r|r.get(0))?;if !valid{return Ok(reject("custom_runtime.secret_scope_mismatch"))}}}}
        freeze_secrets(tx,&mut value)?;
        let now=Utc::now().to_rfc3339();let revision=current+1;let object=value.as_object_mut().context("custom_runtime.definition_invalid")?;
        object.insert("revision".into(),json!(revision));object.insert("createdAt".into(),previous.as_ref().and_then(|v|v.get("createdAt")).cloned().unwrap_or_else(||json!(now)));object.insert("updatedAt".into(),json!(now));
        let identity=digest(&value)?;value["definitionDigest"]=json!(identity);validate_definition(&value,false)?;
        tx.execute("INSERT INTO custom_runtime_definition(id,revision,digest,value_json) VALUES(?1,?2,?3,?4)",params![id,revision,identity,serde_json::to_string(&value)?])?;
        tx.execute("INSERT INTO custom_runtime_head(id,revision,registry_id,removed) VALUES(?1,?2,?3,0) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,registry_id=excluded.registry_id,removed=0",params![id,revision,text(&value,"registryId")?])?;
        Ok(CommandHandlerResult::applied("custom_runtime.saved",json!({"id":id,"revision":revision,"definitionDigest":identity}),entity(&id)))
    })
}
pub fn remove(database:&mut Database,envelope:&CommandEnvelope<Remove>)->Result<CommandExecution>{
    DomainCommandGateway.execute(database,envelope,|tx|{
        if !matches!(&envelope.actor,ActorRef::User{..}){return Ok(reject("custom_runtime.user_required"))}
        let input=&envelope.payload;let Some(value)=latest(tx,&input.id)?else{return Ok(reject("custom_runtime.not_found"))};
        if value["revision"].as_i64()!=Some(input.expected_revision){return Ok(reject("custom_runtime.definition_changed"))}
        tx.execute("UPDATE custom_runtime_head SET removed=1 WHERE id=?1 AND revision=?2",params![input.id,input.expected_revision])?;
        // 不删除不可变定义、秘密或产物；旧运行仍可按原引用恢复。
        Ok(CommandHandlerResult::applied("custom_runtime.removed",json!({"id":input.id,"revision":input.expected_revision,"removed":true}),entity(&input.id)))
    })
}
pub fn migrate(database:&mut Database)->Result<()>{
    database.connection().execute_batch("PRAGMA foreign_keys=OFF;")?;
    let result=(||->Result<()>{
    let tx=database.connection_mut().transaction()?;
    ensure!(matches!(crate::db::classify_database_contract(&tx)?,crate::db::DatabaseContractClassification::SupportedMigrationSource(ref marker) if marker.projection_schema_version==163)&&absent(&tx)?,"custom_runtime.schema_source_invalid");
    crate::db::admit_custom_runtime_kind(&tx)?;
    tx.execute_batch(include_str!("schema.sql"))?;
    tx.execute_batch("INSERT INTO schema_migration(version,applied_at) VALUES(214,datetime('now'));UPDATE rovai_data_contract SET projection_schema_version=164,updated_at=datetime('now') WHERE singleton=1;")?;
    ensure!(matches!(crate::db::classify_database_contract(&tx)?,crate::db::DatabaseContractClassification::Current(ref marker) if marker.projection_schema_version==164),"custom_runtime.schema_target_invalid");tx.commit()?;Ok(())
    })();let foreign_keys=database.connection().execute_batch("PRAGMA foreign_keys=ON;");result?;foreign_keys?;Ok(())
}
fn installed(connection:&Connection)->rusqlite::Result<Vec<(String,String)>>{let mut statement=connection.prepare("SELECT name,sql FROM sqlite_schema WHERE lower(name) GLOB 'custom_runtime_*' AND sql IS NOT NULL ORDER BY name")?;statement.query_map([],|r|Ok((r.get(0)?,r.get(1)?)))?.collect()}
pub(crate) fn absent(connection:&Connection)->rusqlite::Result<bool>{Ok(installed(connection)?.is_empty())}
pub(crate) fn schema_matches(connection:&Connection)->rusqlite::Result<bool>{let reference=Connection::open_in_memory()?;reference.execute_batch(include_str!("schema.sql"))?;Ok(installed(connection)?==installed(&reference)?)}
