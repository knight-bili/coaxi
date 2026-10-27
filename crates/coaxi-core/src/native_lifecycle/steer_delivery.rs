use super::*;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashSet;
static ACTIVE: LazyLock<std::sync::Mutex<HashSet<String>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashSet::new()));
pub(crate) struct Activity(String);
impl Activity {
    pub fn register(command: &str) -> Result<Self> {
        ensure!(
            ACTIVE
                .lock()
                .map_err(|_| anyhow::anyhow!("native_lifecycle.activity_unavailable"))?
                .insert(command.into()),
            "native_lifecycle.original_request_active"
        );
        Ok(Self(command.into()))
    }
}
impl Drop for Activity {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE.lock() {
            active.remove(&self.0);
        }
    }
}
pub(crate) fn active(command: &str) -> bool {
    ACTIVE.lock().is_ok_and(|active| active.contains(command))
}

pub(crate) fn receipt(
    connection: &Connection,
    command: &str,
    client: &str,
    core_instance: &str,
) -> Result<Value> {
    uuid::Uuid::parse_str(command)?;
    let(owner,state,raw):(String,String,String)=connection.query_row("SELECT core_instance_id,state,value_json FROM native_lifecycle_intent WHERE command_id=?1 AND kind='steer' AND client_id=?2",params![command,client],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).context("native_lifecycle.receipt_unavailable")?;
    let mut value: Value = serde_json::from_str(&raw)?;
    let active = ACTIVE
        .lock()
        .map_err(|_| anyhow::anyhow!("native_lifecycle.activity_unavailable"))?
        .contains(command);
    if matches!(state.as_str(), "prepared" | "connection_written")
        && (owner != core_instance || !active)
    {
        value["status"] = json!("unknown");
        value["error"] = json!("native_lifecycle.execution_owner_unavailable");
    }
    validate_shape("NativeSteerReceiptV1", &value)?;
    Ok(value)
}
pub(crate) fn record_initial(
    connection: &Connection,
    command: &str,
    preview: &Value,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let value = json!({"schemaVersion":1,"deliveryId":uuid::Uuid::new_v4().to_string(),"commandId":command,"previewId":preview["previewId"],"previewDigest":preview["previewDigest"],"target":preview["target"],"input":preview["input"],"inputDigest":preview["inputDigest"],"nativePromptDigest":null,"nativeRequestBytes":null,"status":"prepared","nativeOutcome":null,"nativeReceiptDigest":null,"createdAt":now,"updatedAt":now,"error":null});
    validate_shape("NativeInputDeliveryV1", &value)?;
    connection.execute(
        "INSERT INTO native_lifecycle_input_delivery(command_id,value_json) VALUES(?1,?2)",
        params![command, serde_json::to_string(&value)?],
    )?;
    Ok(())
}
pub(crate) fn settle(
    connection: &mut Connection,
    command: &str,
    core_instance: &str,
    observation: Result<crate::acp::NativeSteerObservation>,
) -> Result<Value> {
    let tx = connection.transaction()?;
    let raw:String=tx.query_row("SELECT value_json FROM native_lifecycle_intent WHERE command_id=?1 AND kind='steer' AND core_instance_id=?2 AND state='prepared'",params![command,core_instance],|row|row.get(0)).context("native_lifecycle.intent_owner_changed")?;
    let delivery_raw: String = tx.query_row(
        "SELECT value_json FROM native_lifecycle_input_delivery WHERE command_id=?1",
        [command],
        |row| row.get(0),
    )?;
    let mut receipt: Value = serde_json::from_str(&raw)?;
    let mut delivery: Value = serde_json::from_str(&delivery_raw)?;
    let (status, reservation, outcome, wire_digest, request_bytes, prompt_digest, error) =
        match observation {
            Ok(observation) => {
                let (status, reservation) = match observation.outcome.as_deref() {
                    Some("injected") => ("native_acknowledged", "acknowledged"),
                    Some("promptRequired") => ("not_consumed", "not_consumed"),
                    Some("startedNewTurn") => ("requires_user_review", "acknowledged"),
                    _ => ("unknown", "unknown"),
                };
                (
                    status,
                    reservation,
                    observation.outcome,
                    observation.receipt_digest,
                    observation
                        .connection_written
                        .then_some(observation.request_bytes),
                    observation
                        .connection_written
                        .then_some(observation.prompt_digest),
                    observation.error,
                )
            }
            // Runtime 在实际写出前拒绝；完整输入留在预览，普通发送必须由用户另选。
            Err(error) => (
                "not_consumed",
                "not_consumed",
                None,
                None,
                None,
                None,
                Some(error.to_string()),
            ),
        };
    let error = error.map(|error| error.chars().take(2048).collect::<String>());
    receipt["status"] = json!(status);
    receipt["nativeOutcome"] = json!(outcome);
    receipt["nativeReceiptDigest"] = json!(wire_digest);
    receipt["error"] = json!(error);
    delivery["status"] = json!(status);
    delivery["nativeOutcome"] = json!(outcome);
    delivery["nativeReceiptDigest"] = json!(wire_digest);
    delivery["nativePromptDigest"] = json!(prompt_digest);
    delivery["nativeRequestBytes"] = json!(request_bytes);
    delivery["error"] = json!(error);
    delivery["updatedAt"] = json!(chrono::Utc::now().to_rfc3339());
    validate_shape("NativeSteerReceiptV1", &receipt)?;
    validate_shape("NativeInputDeliveryV1", &delivery)?;
    tx.execute("UPDATE native_lifecycle_intent SET state=?2,value_json=?3,updated_at=?4 WHERE command_id=?1",params![command,status,serde_json::to_string(&receipt)?,chrono::Utc::now().to_rfc3339()])?;
    tx.execute(
        "UPDATE native_lifecycle_image_reservation SET state=?2 WHERE command_id=?1",
        params![command, reservation],
    )?;
    tx.execute(
        "UPDATE native_lifecycle_input_delivery SET value_json=?2 WHERE command_id=?1",
        params![command, serde_json::to_string(&delivery)?],
    )?;
    tx.commit()?;
    Ok(receipt)
}
pub(crate) fn prior(
    connection: &Connection,
    command: &str,
    client: &str,
    input_digest: &str,
    core_instance: &str,
) -> Result<Option<Value>> {
    let row: Option<(String, String, String)> = connection
        .query_row(
            "SELECT kind,client_id,input_digest FROM native_lifecycle_intent WHERE command_id=?1",
            [command],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((kind, owner, digest)) = row {
        ensure!(
            kind == "steer" && owner == client && digest == input_digest,
            "native_lifecycle.command_payload_changed"
        );
        return receipt(connection, command, client, core_instance).map(Some);
    }
    Ok(None)
}
