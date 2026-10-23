//! 运行租约绑定原桥与原生代次；不把模型请求转换成User权限。
use super::*;
use rusqlite::{Connection, params};
use std::{collections::HashMap, path::Path, sync::Mutex};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub schema_version: u32,
    pub core_instance_id: String,
    pub bridge_instance_id: String,
    pub agent_run_id: String,
    pub execution_epoch: i64,
    pub native_binding_id: String,
    pub native_binding_generation: i64,
    pub native_session_id: Option<String>,
    pub frozen_digest: String,
    pub catalog_digest: String,
    pub expires_at: String,
}
impl Binding {
    pub fn validate(&self) -> Result<()> {
        self.validate_retained(false)
    }
    fn validate_retained(&self, receipt: bool) -> Result<()> {
        ensure!(
            self.schema_version == 1
                && self.execution_epoch >= 0
                && self.native_binding_generation >= 0,
            "conversation_tools.binding_changed"
        );
        for id in [
            &self.core_instance_id,
            &self.bridge_instance_id,
            &self.agent_run_id,
            &self.native_binding_id,
        ] {
            uuid::Uuid::parse_str(id)?;
        }
        for digest in [&self.frozen_digest, &self.catalog_digest] {
            ensure!(
                digest.len() == 71
                    && digest.starts_with("sha256:")
                    && digest[7..]
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
                "conversation_tools.binding_changed"
            );
        }
        let expires =
            chrono::DateTime::parse_from_rfc3339(&self.expires_at)?.with_timezone(&chrono::Utc);
        let now = chrono::Utc::now();
        ensure!(
            expires <= now + chrono::Duration::seconds(7201)
                && if receipt {
                    now <= expires + chrono::Duration::seconds(30)
                } else {
                    expires > now
                },
            "conversation_tools.binding_expired"
        );
        Ok(())
    }
}
#[derive(Clone)]
struct Entry {
    binding: Binding,
    token: String,
    revoked: bool,
}
#[derive(Default)]
pub struct Registry {
    entries: Mutex<HashMap<String, Entry>>,
}
fn native_current(connection: &Connection, binding: &Binding) -> Result<()> {
    let present:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM agent_run r JOIN conversation c ON c.id=r.conversation_id WHERE r.id=?1 AND r.execution_epoch=?2 AND c.native_binding_id=?3 AND c.native_binding_generation=?4 AND (?5 IS NULL OR c.native_session_id=?5))",params![binding.agent_run_id,binding.execution_epoch,binding.native_binding_id,binding.native_binding_generation,binding.native_session_id],|row|row.get(0))?;
    ensure!(present, "conversation_tools.native_binding_changed");
    Ok(())
}
impl Registry {
    pub fn register(
        &self,
        database: &mut crate::db::Database,
        core_instance: &str,
        binding: Binding,
    ) -> Result<String> {
        binding.validate()?;
        ensure!(
            binding.core_instance_id == core_instance,
            "conversation_tools.binding_changed"
        );
        let lease = runtime::authorize(
            database.connection_mut(),
            &binding.agent_run_id,
            binding.execution_epoch,
        )?;
        ensure!(
            digest(&serde_json::to_value(&lease.frozen)?)? == binding.frozen_digest
                && lease.frozen.catalog_digest == binding.catalog_digest
                && lease.frozen.flags.any(),
            "conversation_tools.binding_changed"
        );
        native_current(database.connection(), &binding)?;
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| anyhow::anyhow!("conversation_tools.binding_lock"))?;
        entries.retain(|_, entry| {
            chrono::DateTime::parse_from_rfc3339(&entry.binding.expires_at)
                .is_ok_and(|expires| expires.with_timezone(&chrono::Utc) > chrono::Utc::now())
        });
        ensure!(
            entries.len() < 4096 && !entries.contains_key(&binding.bridge_instance_id),
            "conversation_tools.capacity_reached"
        );
        for entry in entries.values_mut() {
            if entry.binding.agent_run_id == binding.agent_run_id {
                entry.revoked = true;
            }
        }
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        entries.insert(
            binding.bridge_instance_id.clone(),
            Entry {
                binding,
                token: token.clone(),
                revoked: false,
            },
        );
        Ok(token)
    }
    fn authenticate(
        &self,
        connection: &Connection,
        core_instance: &str,
        input: &super::stdio::IpcRequest,
        receipt: bool,
    ) -> Result<(Binding, scope::RunLease)> {
        ensure!(
            input.schema_version == 1 && input.authorization_token.len() == 64,
            "conversation_tools.binding_changed"
        );
        let entry = self
            .entries
            .lock()
            .map_err(|_| anyhow::anyhow!("conversation_tools.binding_lock"))?
            .get(&input.bridge_instance_id)
            .cloned()
            .context("conversation_tools.binding_changed")?;
        let diff = entry
            .token
            .as_bytes()
            .iter()
            .zip(input.authorization_token.as_bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b));
        ensure!(
            diff == 0
                && entry.token.len() == input.authorization_token.len()
                && (!entry.revoked || receipt)
                && entry.binding.core_instance_id == core_instance
                && digest(&serde_json::to_value(&entry.binding)?)? == input.binding_digest,
            "conversation_tools.binding_changed"
        );
        entry.binding.validate_retained(receipt)?;
        let lease = if receipt {
            scope::retained_run(
                connection,
                &entry.binding.agent_run_id,
                entry.binding.execution_epoch,
            )?
        } else {
            scope::require_run(
                connection,
                &entry.binding.agent_run_id,
                entry.binding.execution_epoch,
            )?
        };
        ensure!(
            digest(&serde_json::to_value(&lease.frozen)?)? == entry.binding.frozen_digest,
            "conversation_tools.binding_changed"
        );
        if !receipt {
            native_current(connection, &entry.binding)?;
        }
        Ok((entry.binding, lease))
    }
    pub fn valid(&self, connection: &Connection, core_instance: &str, binding: &Binding) -> bool {
        let registered = self.entries.lock().is_ok_and(|entries| {
            entries
                .get(&binding.bridge_instance_id)
                .is_some_and(|entry| {
                    !entry.revoked
                        && entry.binding.core_instance_id == core_instance
                        && entry.binding.agent_run_id == binding.agent_run_id
                        && entry.binding.execution_epoch == binding.execution_epoch
                        && entry.binding.frozen_digest == binding.frozen_digest
                })
        });
        registered
            && binding.validate().is_ok()
            && binding.core_instance_id == core_instance
            && native_current(connection, binding).is_ok()
            && scope::require_run(connection, &binding.agent_run_id, binding.execution_epoch)
                .is_ok()
    }
    pub fn revoke(&self, connection: &Connection, binding: &Binding) -> Result<()> {
        if let Some(entry) = self
            .entries
            .lock()
            .map_err(|_| anyhow::anyhow!("conversation_tools.binding_lock"))?
            .get_mut(&binding.bridge_instance_id)
        {
            entry.revoked = true;
        }
        if let Ok(lease) =
            scope::retained_run(connection, &binding.agent_run_id, binding.execution_epoch)
        {
            question_delivery::revoke(connection, &lease)?;
        }
        Ok(())
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        self.entries
            .lock()
            .map_err(|_| anyhow::anyhow!("conversation_tools.binding_lock"))?
            .remove(id);
        Ok(())
    }
    pub fn request(
        &self,
        database: &mut crate::db::Database,
        root: &Path,
        core_instance: &str,
        input: super::stdio::IpcRequest,
    ) -> Result<Value> {
        let (binding, lease) = self.authenticate(
            database.connection(),
            core_instance,
            &input,
            input.operation == "wrote",
        )?;
        match input.operation.as_str() {
            "catalog" => {
                ensure!(
                    input.payload == json!({}),
                    "conversation_tools.invalid_input"
                );
                Ok(json!(runtime::catalog(lease.frozen.flags)?))
            }
            "disconnect" => {
                ensure!(
                    input.payload == json!({}),
                    "conversation_tools.invalid_input"
                );
                let tx = database.connection_mut().transaction()?;
                self.revoke(&tx, &binding)?;
                tx.commit()?;
                Ok(json!({}))
            }
            "call" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase", deny_unknown_fields)]
                struct Call {
                    name: String,
                    arguments: Value,
                    rpc_id: Value,
                }
                let call: Call = serde_json::from_value(input.payload)?;
                validate_tool(&call.name, &call.arguments)?;
                ensure!(
                    lease.frozen.flags.permits(&call.name),
                    "conversation_tools.not_assigned"
                );
                ensure!(
                    valid_rpc_id(&call.rpc_id),
                    "conversation_tools.invalid_input"
                );
                if call.name == "get_session_info" {
                    return Ok(
                        json!({"state":"ready","result":result(runtime::session_info(database.connection(),root,&lease,&call.arguments)?,false)?}),
                    );
                }
                let id = call.arguments["request_id"]
                    .as_str()
                    .context("conversation_tools.request_id_required")?;
                if call.arguments["operation"] == "status" {
                    let stored: Option<String> = database
                        .connection()
                        .query_row(
                            "SELECT tool_name FROM conversation_tool_request WHERE id=?1",
                            [id],
                            |row| row.get(0),
                        )
                        .optional()?;
                    ensure!(
                        stored.as_ref().is_none_or(|name| name == &call.name),
                        "conversation_tools.request_payload_changed"
                    );
                    return Ok(
                        json!({"state":"ready","result":result(requests::status(database.connection(),&lease,id)?,false)?}),
                    );
                }
                let status = requests::begin(
                    database.connection_mut(),
                    &lease,
                    &call.name,
                    &call.arguments,
                )?;
                if call.name == "ask_user_question" {
                    let target =
                        question_target(&binding, &call.rpc_id, &digest(&call.arguments)?)?;
                    let tx = database
                        .connection_mut()
                        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                    let set = questions::prepare_on(&tx, &lease, id, &target)?;
                    tx.commit()?;
                    return Ok(json!({"state":"awaiting_answer","requestId":id,"questions":set}));
                }
                if status["state"] != "prepared" {
                    return Ok(json!({"state":"ready","result":result(status,false)?}));
                }
                let execution = match call.name.as_str() {
                    "create_work_task" => authoring::create_mission(database, &lease, id)?,
                    "create_automation" => authoring::create_automation(database, &lease, id)?,
                    _ => anyhow::bail!("conversation_tools.invalid_input"),
                };
                ensure!(
                    execution.result.status != crate::command::CommandResultStatus::Rejected,
                    "conversation_tools.request_cancelled"
                );
                Ok(json!({"state":"ready","result":result(execution.result.payload,false)?}))
            }
            "poll" | "wrote" | "cancel" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase", deny_unknown_fields)]
                struct Delivery {
                    request_id: String,
                    rpc_id: Value,
                    arguments_digest: String,
                    #[serde(default)]
                    receipt_digest: Option<String>,
                    #[serde(default)]
                    result_digest: Option<String>,
                    #[serde(default)]
                    written: Option<bool>,
                }
                let p: Delivery = serde_json::from_value(input.payload)?;
                uuid::Uuid::parse_str(&p.request_id)?;
                let target = question_target(&binding, &p.rpc_id, &p.arguments_digest)?;
                match input.operation.as_str() {
                    "poll" => question_delivery::claim(
                        database.connection_mut(),
                        &lease,
                        &p.request_id,
                        &target,
                    ),
                    "wrote" => question_delivery::receipt(
                        database.connection_mut(),
                        &lease,
                        &p.request_id,
                        &target,
                        p.receipt_digest
                            .as_deref()
                            .context("conversation_tools.receipt_required")?,
                        p.result_digest
                            .as_deref()
                            .context("conversation_tools.receipt_required")?,
                        p.written.context("conversation_tools.receipt_required")?,
                    ),
                    _ => {
                        let tx = database
                            .connection_mut()
                            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                        question_delivery::require_target(&tx, &p.request_id, &lease, &target)?;
                        requests::cancel_remaining(&tx, &p.request_id)?;
                        let status = requests::status(&tx, &lease, &p.request_id)?;
                        tx.commit()?;
                        Ok(status)
                    }
                }
            }
            _ => anyhow::bail!("conversation_tools.invalid_input"),
        }
    }
}
use rusqlite::OptionalExtension;
fn valid_rpc_id(id: &Value) -> bool {
    id.as_str()
        .is_some_and(|text| !text.is_empty() && text.len() <= 512)
        || id
            .as_i64()
            .is_some_and(|id| id.abs_diff(0) <= 9_007_199_254_740_991)
        || id.as_u64().is_some_and(|id| id <= 9_007_199_254_740_991)
}
fn question_target(
    binding: &Binding,
    rpc_id: &Value,
    arguments_digest: &str,
) -> Result<questions::NativeQuestionTarget> {
    ensure!(
        valid_rpc_id(rpc_id)
            && arguments_digest.len() == 71
            && arguments_digest.starts_with("sha256:")
            && arguments_digest[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "conversation_tools.invalid_input"
    );
    Ok(questions::NativeQuestionTarget {
        bridge_instance_id: binding.bridge_instance_id.clone(),
        native_binding_id: binding.native_binding_id.clone(),
        native_binding_generation: binding.native_binding_generation,
        locator: json!({"rpcId":rpc_id,"argumentsDigest":arguments_digest}),
    })
}
pub fn error_code(error: &anyhow::Error) -> &'static str {
    match error.to_string().as_str() {
        "conversation_tools.not_assigned" => "not_assigned",
        "conversation_tools.scope_changed" | "conversation_tools.run_unavailable" => {
            "scope_changed"
        }
        "conversation_tools.request_payload_changed" => "request_payload_changed",
        "conversation_tools.original_request_unresolved" => "question_busy",
        "conversation_tools.request_cancelled" | "conversation_tools.question_expired" => {
            "request_cancelled"
        }
        "conversation_tools.capacity_reached" | "conversation_tools.preview_capacity" => {
            "capacity_reached"
        }
        "conversation_tools.native_binding_changed" | "conversation_tools.binding_changed" => {
            "native_session_rebuild_required"
        }
        "conversation_tools.invalid_input" => "invalid_input",
        _ => "runtime_unavailable",
    }
}
pub fn failure(code: &str, id: Option<&str>) -> Value {
    let known = CONTRACT["schemas"]["ToolErrorV1"]["properties"]["code"]["enum"]
        .as_array()
        .is_some_and(|codes| codes.iter().any(|value| value == code));
    let mut value = json!({"code":if known{code}else{"runtime_unavailable"},"note":"本次操作未取得可确认的成功结果。请保留原编号核对，不另建重复请求；用户可在原任务中处理。"});
    if let Some(id) = id.filter(|id| uuid::Uuid::parse_str(id).is_ok()) {
        value["request_id"] = json!(id);
    }
    result(value, true).unwrap_or_else(
        |_| json!({"isError":true,"content":[{"type":"text","text":"Operation unavailable"}]}),
    )
}
