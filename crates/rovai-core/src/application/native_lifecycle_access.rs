use super::*;
use crate::native_lifecycle::{self as native,storage};

impl Core{
    pub(super) async fn native_lifecycle_access(&self,method:&str,value:Value)->Result<Value>{
        native::validate_operation(method,&value)?;
        match method{
            "runtime.steer.policy.get"=>{let database=self.database.lock().await;storage::policy(database.connection())},
            "runtime.steer.policy.set"=>{
                let command_id=value["commandId"].as_str().context("native_lifecycle.command_id_required")?.to_owned();
                let mut payload=value;payload.as_object_mut().context("native_lifecycle.request_invalid")?.remove("commandId");
                let input:native::policy::SetPolicy=serde_json::from_value(payload)?;let mut database=self.database.lock().await;
                let execution=native::policy::set(&mut database,&user_command_envelope(command_id,input))?;
                let result=serde_json::to_value(execution.result)?;emit(&self.output,"nativeLifecycle.changed",json!({"reason":"policy_saved"}));Ok(result)
            },
            "runtime.steer.targets"=>{
                let scope:native::NativeSteerScopeV1=serde_json::from_value(value["scope"].clone())?;
                let candidates={let database=self.database.lock().await;native::target::candidates(database.connection(),&scope)?};
                let mut targets=Vec::new();
                for mut target in candidates{
                    let Some(runtime)=self.custom_acp.get_agent_run(&target.agent_run_id,target.execution_epoch).await else{continue};
                    if runtime.native_steer_capability().await?.is_none(){continue}
                    let Some(turn)=runtime.prompt_id().await else{continue};target.active_turn_id=turn;
                    let database=self.database.lock().await;if native::target::require(database.connection(),&target).is_ok(){targets.push(target);}
                }Ok(json!({"schemaVersion":1,"targets":targets}))
            },
            _=>anyhow::bail!("native_lifecycle.operation_unknown"),
        }
    }
}
