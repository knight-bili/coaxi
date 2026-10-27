//! User 的任务模板库；保存和删除只登记文件计划，不创建 Mission 或运行。
use super::*;
use crate::{
    command::{
        ActorRef, CommandEnvelope, CommandHandlerResult, DomainCommand, DomainCommandGateway,
        sealed,
    },
    db::Database,
    draft_client::DraftClient,
};
use rusqlite::{Connection, params};
use std::path::Path;
const TARGET: &str = "taskTemplates";
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Read {
    operation: String,
    id: Option<String>,
    after: Option<String>,
    limit: Option<u32>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MutationInput {
    operation: String,
    command_id: String,
    id: String,
    expected_revision: Option<i64>,
    value: Option<Value>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Mutation {
    input: MutationInput,
    client: DraftClient,
}
impl sealed::Sealed for Mutation {}
impl DomainCommand for Mutation {
    const TYPE: &'static str = "configuration.taskTemplates.mutate";
    const EXACT_REPLAY: bool = true;
    fn draft_client(&self) -> Option<&DraftClient> {
        Some(&self.client)
    }
}
fn read(root: &Path, input: Read) -> Result<Value> {
    let value = files::read(root, TARGET)?;
    let library = library::initial(TARGET, &value)?;
    if input.operation == "get" {
        ensure!(
            input.after.is_none() && input.limit.is_none(),
            "configuration.template_read_invalid"
        );
        let id = input.id.context("configuration.template_id_required")?;
        uuid::Uuid::parse_str(&id)?;
        return Ok(library::item(TARGET, &library, &id)?
            .cloned()
            .unwrap_or(Value::Null));
    }
    ensure!(
        input.operation == "list" && input.id.is_none(),
        "configuration.template_read_invalid"
    );
    let limit = input.limit.unwrap_or(100);
    ensure!(
        (1..=100).contains(&limit),
        "configuration.template_page_invalid"
    );
    if let Some(after) = &input.after {
        uuid::Uuid::parse_str(after)?;
    }
    let mut rows = library["items"]
        .as_array()
        .context("configuration.library_invalid")?
        .iter()
        .filter(|row| {
            row["id"]
                .as_str()
                .is_some_and(|id| input.after.as_deref().is_none_or(|after| id > after))
        })
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next = if more {
        rows.last()
            .map(|row| row["id"].clone())
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    // 列表不批量传送原任务说明；明确打开一条时才读取完整原配置。
    for row in &mut rows {
        row.as_object_mut()
            .context("configuration.library_invalid")?
            .remove("config");
    }
    Ok(json!({"items":rows,"next":next,"revision":library["revision"]}))
}
fn receipt(connection: &Connection, client: &str, command: &str) -> Result<Value> {
    uuid::Uuid::parse_str(command)?;
    let owned:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM event_log WHERE event_type='command.result' AND command_id=?1 AND command_type=?2 AND command_client_id=?3 AND actor_type='user' AND actor_id='local_user')",params![command,Mutation::TYPE,client],|row|row.get(0))?;
    if !owned {
        return Ok(json!({"commandId":command,"state":"unknown","result":null,"files":[]}));
    }
    Ok(
        json!({"commandId":command,"state":"recorded","result":crate::command::load_stored_result(connection,command)?,"files":files::statuses(connection,command)?}),
    )
}
pub(crate) fn access(
    database: &mut Database,
    root: &Path,
    client: &DraftClient,
    input: Value,
) -> Result<Value> {
    match input["operation"].as_str() {
        Some("get" | "list") => return read(root, serde_json::from_value(input)?),
        Some("receipt" | "finish") => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Request {
                operation: String,
                command_id: String,
            }
            let request: Request = serde_json::from_value(input)?;
            let original = receipt(database.connection(), client.id(), &request.command_id)?;
            if request.operation == "finish" {
                ensure!(
                    original["state"] == "recorded",
                    "configuration.template_original_result_required"
                );
                files::drain(database.connection(), root, &request.command_id)?;
                return receipt(database.connection(), client.id(), &request.command_id);
            }
            return Ok(original);
        }
        _ => {}
    }
    let input: MutationInput = serde_json::from_value(input)?;
    ensure!(
        matches!(input.operation.as_str(), "put" | "remove"),
        "configuration.template_operation_invalid"
    );
    uuid::Uuid::parse_str(&input.command_id)?;
    uuid::Uuid::parse_str(&input.id)?;
    ensure!(
        input
            .expected_revision
            .is_none_or(|revision| (1..9007199254740991).contains(&revision)),
        "configuration.template_revision_invalid"
    );
    let command = input.command_id.clone();
    let envelope = CommandEnvelope {
        command_id: command.clone(),
        actor: ActorRef::User {
            user_id: "local_user".into(),
        },
        camp_id: None,
        expected_versions: vec![],
        execution_epoch: None,
        payload: Mutation {
            input,
            client: client.clone(),
        },
    };
    DomainCommandGateway.execute(database,&envelope,|tx|{
        let input=&envelope.payload.input;
        let before=files::read(root,TARGET)?;let mut next=library::initial(TARGET,&before)?;
        let current=library::revision(TARGET,&next,&input.id)?;
        if current!=input.expected_revision{return Ok(CommandHandlerResult::rejected("configuration.template_changed",json!({"id":input.id})))}
        if input.operation=="put"{
            let value=input.value.as_ref().context("configuration.template_value_required")?;
            ensure!(value.as_object().is_some_and(|fields|fields.keys().all(|key|["name","title","config"].contains(&key.as_str()))),"configuration.template_fields_invalid");
            let candidate=library::candidate(tx,client.id(),TARGET,&input.id,value,false)?;
            library::merge(TARGET,&mut next,candidate,if current.is_some(){"replace"}else{"create"},current)?;
        }else{
            ensure!(input.value.is_none()&&current.is_some(),"configuration.template_remove_invalid");
            next["items"].as_array_mut().context("configuration.library_invalid")?.retain(|row|row["id"]!=input.id);
            next["revision"]=json!(next["revision"].as_i64().unwrap_or(0)+1);
        }
        files::enqueue(tx,root,&command,TARGET,TARGET,&before,&next)?;
        Ok(CommandHandlerResult::applied("configuration.template_saved",json!({"id":input.id,"removed":input.operation=="remove","revision":library::revision(TARGET,&next,&input.id)?}),None))
    })?;
    files::drain(database.connection(), root, &command)?;
    receipt(database.connection(), client.id(), &command)
}
