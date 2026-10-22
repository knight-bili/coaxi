//! 不可变服务商版本；源原文加密，公开视图不回显查询凭据或未知模型原文。
use super::*;
use crate::{command::{ActorRef,CommandEnvelope,CommandExecution,CommandHandlerResult,DomainCommand,DomainCommandGateway,sealed},draft_client::DraftClient};
use chrono::Utc;
use rusqlite::{Connection,OptionalExtension,Transaction,params};

fn field<'a>(value:&'a Value,key:&str)->Result<&'a str>{value[key].as_str().context("configuration.field_required")}
fn context(id:&str,revision:i64)->String{format!("portable-provider:{id}:{revision}")}
fn checked(value:&Value)->Result<()>{
    validate("PortableProviderV1",value)?;let mut unsigned=value.clone();unsigned["digest"]=json!("");
    ensure!(digest(&unsigned)?==field(value,"digest")?,"configuration.provider_digest_changed");Ok(())
}
fn public_view(value:&Value)->Result<Value>{
    let mut view=value.clone();let mut url=reqwest::Url::parse(field(value,"apiUrl")?)?;
    let names=url.query_pairs().map(|(name,_)|name.to_string()).collect::<Vec<_>>();
    if !names.is_empty(){url.set_query(None);{let mut query=url.query_pairs_mut();for name in names{query.append_pair(&name,"[redacted]");}}view["apiUrl"]=json!(url.to_string());}
    view["rawAgentTypesJson"]=json!("[原文已加密保留]");
    if !view["rawModel"].is_null(){view["rawModel"]=json!("[原文已加密保留]");}
    Ok(view)
}
pub(crate) fn resolve(connection:&Connection,reference:&PortableConnectionRefV1,selectable:bool)->Result<Value>{
    reference.validate()?;
    let cipher:Vec<u8>=connection.query_row("SELECT content_cipher FROM portable_provider_revision WHERE id=?1 AND revision=?2 AND digest=?3",params![reference.id,reference.revision,reference.digest],|row|row.get(0)).context("configuration.provider_version_unavailable")?;
    let value:Value=crate::member_environment::decrypt(connection,&context(&reference.id,reference.revision),&cipher)?;checked(&value)?;
    ensure!(value["id"]==reference.id&&value["revision"]==reference.revision&&value["digest"]==reference.digest,"configuration.provider_version_changed");
    if selectable{let current:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM portable_provider_head WHERE id=?1 AND revision=?2 AND removed=0)",params![reference.id,reference.revision],|row|row.get(0))?;ensure!(current&&value["enabled"]==true,"configuration.provider_not_selectable");}
    Ok(value)
}
pub(crate) fn reference(value:&Value)->Result<PortableConnectionRefV1>{
    let reference=PortableConnectionRefV1{id:field(value,"id")?.into(),revision:value["revision"].as_i64().context("configuration.revision_invalid")?,digest:field(value,"digest")?.into()};reference.validate()?;Ok(reference)
}
pub(crate) fn list(connection:&Connection,after:Option<&str>,limit:u32)->Result<Value>{
    ensure!((1..=100).contains(&limit),"configuration.limit_invalid");if let Some(after)=after{uuid::Uuid::parse_str(after)?;}
    let mut statement=connection.prepare("SELECT r.value_json FROM portable_provider_head h JOIN portable_provider_revision r ON r.id=h.id AND r.revision=h.revision WHERE h.removed=0 AND (?1 IS NULL OR h.id>?1) ORDER BY h.id LIMIT ?2")?;
    let mut items=statement.query_map(params![after,limit+1],|row|row.get::<_,String>(0))?.map(|row|Ok(serde_json::from_str::<Value>(&row?)?)).collect::<Result<Vec<_>>>()?;
    let more=items.len()>limit as usize;items.truncate(limit as usize);let next=if more{items.last().and_then(|row|row["id"].as_str()).map(str::to_owned)}else{None};Ok(json!({"items":items,"next":next}))
}
pub(crate) fn upsert_on(tx:&Transaction<'_>,client:&str,expected:i64,definition:&Value)->Result<Value>{
    validate("ProviderDefinitionV1",definition)?;
    let id=field(definition,"id")?;let name=field(definition,"name")?;
    ensure!(!name.trim().is_empty(),"configuration.provider_name_required");
    model::validate_url(field(definition,"apiUrl")?)?;model::validate_model(field(definition,"agentType")?,&definition["model"])?;
    let previous:Option<(i64,String)>=tx.query_row("SELECT h.revision,r.digest FROM portable_provider_head h JOIN portable_provider_revision r ON r.id=h.id AND r.revision=h.revision WHERE h.id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
    ensure!(expected==previous.as_ref().map_or(0,|row|row.0),"configuration.provider_revision_changed");
    let original=previous.as_ref().map(|(revision,digest)|resolve(tx,&PortableConnectionRefV1{id:id.into(),revision:*revision,digest:digest.clone()},false)).transpose()?;
    if let Some(secret)=definition.get("credential").filter(|value|!value.is_null()){
        let reference:SecretRefV1=serde_json::from_value(secret.clone())?;
        if original.as_ref().is_none_or(|old|old["credential"]!=*secret){secrets::attach(tx,&reference,client,"provider_key")?;}
        else{let _=secrets::retained(tx,&reference,"provider_key")?;}
    }
    // 撤销可以把头指回旧版本；新写入仍须越过全部已保存版本。
    let maximum:i64=tx.query_row("SELECT COALESCE(MAX(revision),0) FROM portable_provider_revision WHERE id=?1",[id],|row|row.get(0))?;
    let revision=maximum.checked_add(1).context("configuration.revision_exhausted")?;
    let now=Utc::now().to_rfc3339();let mut value=definition.clone();value["revision"]=json!(revision);value["digest"]=json!("");
    value["createdAt"]=original.as_ref().map(|old|old["createdAt"].clone()).unwrap_or_else(||json!(now));value["updatedAt"]=json!(now);value["digest"]=json!(digest(&value)?);checked(&value)?;
    let view=public_view(&value)?;let cipher=crate::member_environment::encrypt(tx,&context(id,revision),&value)?;
    tx.execute("INSERT INTO portable_provider_revision(id,revision,digest,value_json,content_cipher,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id,revision,field(&value,"digest")?,serde_json::to_string(&view)?,cipher,now])?;
    tx.execute("INSERT INTO portable_provider_head(id,revision,removed) VALUES(?1,?2,0) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,removed=0",params![id,revision])?;Ok(view)
}

#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct Upsert{
    #[serde(skip_deserializing,default)]pub client:DraftClient,
    pub schema_version:u32,pub expected_revision:i64,pub definition:Value,
}
impl sealed::Sealed for Upsert{}
impl DomainCommand for Upsert{const TYPE:&'static str="configuration.providers.upsert";const EXACT_REPLAY:bool=true;fn draft_client(&self)->Option<&DraftClient>{Some(&self.client)}}
pub(crate) fn upsert(database:&mut crate::db::Database,envelope:&CommandEnvelope<Upsert>)->Result<CommandExecution>{
    DomainCommandGateway.execute(database,envelope,|tx|{
        ensure!(matches!(&envelope.actor,ActorRef::User{..})&&envelope.camp_id.is_none(),"configuration.user_required");
        let input=&envelope.payload;validate_operation(Upsert::TYPE,&json!({"schemaVersion":input.schema_version,"commandId":envelope.command_id,"expectedRevision":input.expected_revision,"definition":input.definition}))?;
        let value=upsert_on(tx,input.client.id(),input.expected_revision,&input.definition)?;
        Ok(CommandHandlerResult::applied("configuration.provider_saved",value,None))
    })
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct Remove{#[serde(skip_deserializing,default)]pub client:DraftClient,pub schema_version:u32,pub id:String,pub expected_revision:i64}
impl sealed::Sealed for Remove{}
impl DomainCommand for Remove{const TYPE:&'static str="configuration.providers.remove";const EXACT_REPLAY:bool=true;fn draft_client(&self)->Option<&DraftClient>{Some(&self.client)}}
pub(crate) fn remove(database:&mut crate::db::Database,envelope:&CommandEnvelope<Remove>)->Result<CommandExecution>{
    DomainCommandGateway.execute(database,envelope,|tx|{
        ensure!(matches!(&envelope.actor,ActorRef::User{..})&&envelope.camp_id.is_none(),"configuration.user_required");let input=&envelope.payload;
        validate_operation(Remove::TYPE,&json!({"schemaVersion":input.schema_version,"commandId":envelope.command_id,"id":input.id,"expectedRevision":input.expected_revision}))?;
        ensure!(tx.execute("UPDATE portable_provider_head SET removed=1 WHERE id=?1 AND revision=?2 AND removed=0",params![input.id,input.expected_revision])?==1,"configuration.provider_revision_changed");
        Ok(CommandHandlerResult::applied("configuration.provider_removed",json!({"id":input.id,"revision":input.expected_revision,"historicalVersionsRetained":true}),None))
    })
}
