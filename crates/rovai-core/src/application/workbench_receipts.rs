//! Read recorded owner operations without executing their original side effects.
use super::*;
use anyhow::ensure;
use std::fs;
impl Core {
    pub(super) async fn reconcile_workspace_operation(&self,value:&Value)->Result<Option<Value>>{
        let operation=value["operation"].as_str().context("workbench.operation_required")?;
        if operation=="forge.operation"{return self.reconcile_forge_operation(value["params"].clone()).await.map(Some)}
        if !matches!(operation,"workbench.files.mutate"|"workbench.files.tools"|"workbench.git.execute"){return Ok(None)}
        let request=&value["params"];
        let scope:super::workbench::Scope=serde_json::from_value(request["scope"].clone())?;
        let root=self.workbench_root(&scope).await?;
        let id=request["commandId"].as_str().context("workbench.command_id_required")?;uuid::Uuid::parse_str(id)?;
        let path=self.data_dir.join("workspace-operations").join(format!("{id}.json"));
        if !path.exists(){return Ok(Some(json!({"state":"unknown"})))}
        let identity=crate::command::canonical_json_digest(&json!({"root":root,"request":request}))?;
        let receipt:Value=serde_json::from_slice(&fs::read(path)?)?;
        ensure!(receipt["identity"]==identity,"workbench.command_conflict");
        let result=match receipt["state"].as_str(){
            Some("applied"|"finished")=>json!({"state":"recorded","result":receipt["result"]}),
            Some("failed")=>json!({"state":"recorded","error":{"code":"workbench.operation_failed","message":receipt["error"]}}),
            _=>json!({"state":"unknown"})
        };
        Ok(Some(result))
    }
}
