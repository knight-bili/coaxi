//! 仅从私有 User 凭据通道调用；明文不进入领域命令、事件或普通恢复日志。
use super::*;
use chrono::{Duration,Utc};
use rusqlite::{Connection,OptionalExtension,params};

pub(crate) fn prepare(database:&mut crate::db::Database,client:&str,input:&Value)->Result<Value>{
    validate_operation("configuration.secrets.prepare",input)?;
    ensure!(!client.is_empty()&&client.len()<=1024,"configuration.client_required");
    let operation=input["operationId"].as_str().context("configuration.operation_required")?;
    let purpose=input["purpose"].as_str().context("configuration.secret_purpose_required")?;
    let plain=input["value"].as_str().context("configuration.secret_value_required")?;
    let tx=database.connection_mut().transaction()?;
    let request_hmac=crate::member_environment::private_input_hmac(&tx,&format!("portable-secret:{client}:{purpose}:{operation}"),plain.as_bytes())?;
    let previous:Option<(String,i64,String,i64,bool,String)>=tx.query_row(
        "SELECT id,revision,request_hmac,expires_at_ms,attached,purpose FROM portable_secret_revision WHERE client_id=?1 AND operation_id=?2",
        params![client,operation],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
    ).optional()?;
    if let Some((id,revision,stored,expires,attached,stored_purpose))=previous{
        ensure!(stored==request_hmac&&stored_purpose==purpose,"configuration.secret_operation_changed");
        ensure!(attached||Utc::now().timestamp_millis()<expires,"configuration.secret_preparation_expired");
        return Ok(json!({"reference":SecretRefV1{id,revision},"expiresAt":chrono::DateTime::from_timestamp_millis(expires).context("configuration.secret_expiry_invalid")?.to_rfc3339()}))
    }
    let count:i64=tx.query_row("SELECT COUNT(*) FROM portable_secret_revision WHERE client_id=?1 AND attached=0 AND expires_at_ms>?2",params![client,Utc::now().timestamp_millis()],|row|row.get(0))?;
    ensure!(count<2048,"configuration.secret_preparation_limit");
    let reference=SecretRefV1{id:uuid::Uuid::new_v4().to_string(),revision:1};
    let context=format!("portable-secret:{}:{}:{client}:{purpose}",reference.id,reference.revision);
    let cipher=crate::member_environment::encrypt(&tx,&context,&plain)?;
    let now=Utc::now();let expires=now+Duration::minutes(10);
    tx.execute("INSERT INTO portable_secret_revision(id,revision,client_id,purpose,context,cipher,operation_id,request_hmac,expires_at_ms,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![reference.id,reference.revision,client,purpose,context,cipher,operation,request_hmac,expires.timestamp_millis(),now.to_rfc3339()])?;
    tx.commit()?;Ok(json!({"reference":reference,"expiresAt":expires.to_rfc3339()}))
}

pub(crate) fn attach(connection:&Connection,reference:&SecretRefV1,client:&str,purpose:&str)->Result<()>{
    reference.validate()?;
    let changed=connection.execute("UPDATE portable_secret_revision SET attached=1 WHERE id=?1 AND revision=?2 AND client_id=?3 AND purpose=?4 AND (attached=1 OR expires_at_ms>?5)",params![reference.id,reference.revision,client,purpose,Utc::now().timestamp_millis()])?;
    ensure!(changed==1,"configuration.secret_reference_unavailable");Ok(())
}

/// 运行层只取冻结引用，不能回退到最新密钥、当前环境或别的服务商。
pub(crate) fn retained(connection:&Connection,reference:&SecretRefV1,purpose:&str)->Result<String>{
    reference.validate()?;
    let(context,cipher):(String,Vec<u8>)=connection.query_row("SELECT context,cipher FROM portable_secret_revision WHERE id=?1 AND revision=?2 AND purpose=?3 AND attached=1",params![reference.id,reference.revision,purpose],|row|Ok((row.get(0)?,row.get(1)?))).context("configuration.frozen_secret_unavailable")?;
    let value:String=crate::member_environment::decrypt(connection,&context,&cipher)?;
    ensure!(value.len()<=1024*1024&&!value.contains('\0'),"configuration.frozen_secret_invalid");Ok(value)
}

/// 已确认导入事务中的新秘密；失败随整批事务回滚，不产生公开明文回执。
pub(crate) fn import_on(connection:&Connection,client:&str,purpose:&str,plain:&str)->Result<SecretRefV1>{
    ensure!(matches!(purpose,"provider_key"|"member_environment")&&plain.len()<=1024*1024&&!plain.contains('\0'),"configuration.secret_invalid");
    let reference=SecretRefV1{id:uuid::Uuid::new_v4().to_string(),revision:1};
    let operation=uuid::Uuid::new_v4().to_string();let context=format!("portable-secret:{}:1:{client}:{purpose}",reference.id);
    let cipher=crate::member_environment::encrypt(connection,&context,&plain)?;
    let hmac=crate::member_environment::private_input_hmac(connection,&format!("portable-secret:{client}:{purpose}:{operation}"),plain.as_bytes())?;
    connection.execute("INSERT INTO portable_secret_revision(id,revision,client_id,purpose,context,cipher,operation_id,request_hmac,attached,expires_at_ms,created_at) VALUES(?1,1,?2,?3,?4,?5,?6,?7,1,?8,?9)",params![reference.id,client,purpose,context,cipher,operation,hmac,i64::MAX,Utc::now().to_rfc3339()])?;
    Ok(reference)
}
