use super::*;
use crate::portable_configuration::{self as configuration,providers,members};

impl Core{
    pub(super) async fn portable_configuration_access(&self,method:&str,mut value:Value,client:&crate::draft_client::DraftClient)->Result<Value>{
        configuration::validate_operation(method,&value)?;
        if method=="configuration.providers.list"{
            let database=self.database.lock().await;
            return providers::list(database.connection(),value["after"].as_str(),value["limit"].as_u64().context("configuration.limit_invalid")? as u32)
        }
        if method=="configuration.memberConnection.get"{
            let database=self.database.lock().await;return members::get(database.connection(),value["agentId"].as_str().context("configuration.member_required")?)
        }
        let command=value["commandId"].as_str().context("configuration.command_required")?.to_owned();
        value.as_object_mut().context("configuration.request_invalid")?.remove("commandId");
        let mut database=self.database.lock().await;
        let execution=match method{
            "configuration.providers.upsert"=>{let mut input:providers::Upsert=serde_json::from_value(value)?;input.client=client.clone();providers::upsert(&mut database,&user_command_envelope(command,input))?},
            "configuration.providers.remove"=>{let mut input:providers::Remove=serde_json::from_value(value)?;input.client=client.clone();providers::remove(&mut database,&user_command_envelope(command,input))?},
            "configuration.memberConnection.set"=>{let mut input:members::SetConnection=serde_json::from_value(value)?;input.client=client.clone();members::set(&mut database,&user_command_envelope(command,input))?},
            _=>anyhow::bail!("configuration.operation_unknown"),
        };
        let result=serde_json::to_value(execution.result)?;drop(database);
        emit(&self.output,"portableConfiguration.changed",json!({"operation":method}));Ok(result)
    }
}
