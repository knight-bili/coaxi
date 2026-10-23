use super::*;

impl Core{
    pub(super) async fn conversation_tools_access(&self,method:&str,value:Value,client:&crate::draft_client::DraftClient)->Result<Value>{
        let mut database=self.database.lock().await;
        let result=rovai_core::conversation_tools::user_api::call(&mut database,&self.data_dir,method,value,client)?;
        drop(database);
        if matches!(method,"conversationTools.policy.set"|"conversationTools.questions.answer"|"conversationTools.questions.decline"|"conversationTools.grants.confirm"|"conversationTools.grants.revoke"|"conversationTools.requests.resolve"|"conversationTools.automationOwner.set"){
            emit(&self.output,"conversationTools.changed",json!({"operation":method}));
        }
        Ok(result)
    }
}
