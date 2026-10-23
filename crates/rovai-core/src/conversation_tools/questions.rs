use super::*;
use super::scope::RunLease;
use rusqlite::{Connection,OptionalExtension,Transaction,params};

/// 只能由已认证的MCP桥构造，模型输入Schema没有这些字段。
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct NativeQuestionTarget{
    pub bridge_instance_id:String,pub native_binding_id:String,pub native_binding_generation:i64,pub locator:Value,
}
fn context(id:&str,part:&str)->String{format!("conversation-tool-question:{id}:{part}")}
pub(super) fn target_guard(connection:&Connection,lease:&RunLease,target:&NativeQuestionTarget)->Result<()>{
    scope::require_run(connection,&lease.run_id,lease.epoch)?;
    uuid::Uuid::parse_str(&target.bridge_instance_id)?;
    ensure!(target.native_binding_generation>=0&&serde_json::to_vec(&target.locator)?.len()<=8192,"conversation_tools.native_target_invalid");
    let present:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM agent_run r JOIN conversation c ON c.id=r.conversation_id WHERE r.id=?1 AND r.execution_epoch=?2 AND c.native_binding_id=?3 AND c.native_binding_generation=?4 AND c.native_session_id IS NOT NULL)",params![lease.run_id,lease.epoch,target.native_binding_id,target.native_binding_generation],|row|row.get(0))?;
    ensure!(present,"conversation_tools.native_binding_changed");Ok(())
}
pub(crate) fn set_view(connection:&Connection,id:&str)->Result<Value>{
    let(raw,state,revision):(Vec<u8>,String,i64)=connection.query_row("SELECT q.questions_cipher,r.state,r.revision FROM conversation_tool_question q JOIN conversation_tool_request r ON r.id=q.request_id WHERE q.request_id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).context("conversation_tools.question_unavailable")?;
    let mut value:Value=crate::member_environment::decrypt(connection,&context(id,"set"),&raw)?;
    value["revision"]=json!(revision);
    value["state"]=json!(match state.as_str(){"awaiting_answer"=>"awaiting_answer","answered"=>"answered","applied"=>"delivered","cancelled"|"rejected"=>"cancelled",_=>"outcome_unknown"});
    validate("QuestionSetV1",&value)?;Ok(value)
}
pub(crate) fn prepare_on(tx:&Transaction<'_>,lease:&RunLease,id:&str,target:&NativeQuestionTarget)->Result<Value>{
    target_guard(tx,lease,target)?;
    let target_digest=digest(&serde_json::to_value(target)?)?;
    let previous:Option<String>=tx.query_row("SELECT target_digest FROM conversation_tool_question WHERE request_id=?1",[id],|row|row.get(0)).optional()?;
    if let Some(previous)=previous{ensure!(previous==target_digest,"conversation_tools.original_native_call_changed");return set_view(tx,id)}
    let input=requests::prepared(tx,lease,id,"ask_user_question")?;
    let mut questions=Vec::new();
    for question in input["questions"].as_array().context("conversation_tools.questions_required")?{
        validate("QuestionInputV1",question)?;
        ensure!(question["question"].as_str().is_some_and(|text|!text.trim().is_empty())&&question["header"].as_str().is_some_and(|text|!text.trim().is_empty()),"conversation_tools.empty_question");
        let mut labels=std::collections::HashSet::new();
        for option in question["options"].as_array().context("conversation_tools.options_required")?{let label=option["label"].as_str().context("conversation_tools.option_label_required")?;ensure!(!label.trim().is_empty()&&labels.insert(label),"conversation_tools.option_labels_invalid");}
        questions.push(json!({"id":uuid::Uuid::new_v4().to_string(),"spec":question}));
    }
    let now=chrono::Utc::now();
    let expires:i64=tx.query_row("SELECT expires_at_ms FROM conversation_tool_request WHERE id=?1",[id],|row|row.get(0))?;
    let expires=chrono::DateTime::from_timestamp_millis(expires).context("conversation_tools.expiry_invalid")?;
    let value=json!({"requestId":id,"revision":2,"scope":lease.frozen.scope,"agentId":lease.frozen.agent_id,"agentRunId":lease.run_id,"executionEpoch":lease.epoch,"state":"awaiting_answer","questions":questions,"createdAt":now.to_rfc3339(),"expiresAt":expires.to_rfc3339()});
    validate("QuestionSetV1",&value)?;
    let question_cipher=crate::member_environment::encrypt(tx,&context(id,"set"),&value)?;
    let target_cipher=crate::member_environment::encrypt(tx,&context(id,"target"),target)?;
    tx.execute("INSERT INTO conversation_tool_question(request_id,questions_cipher,target_cipher,target_digest,created_at) VALUES(?1,?2,?3,?4,?5)",params![id,question_cipher,target_cipher,target_digest,now.to_rfc3339()])?;
    ensure!(tx.execute("UPDATE conversation_tool_request SET state='awaiting_answer',revision=revision+1,updated_at=?2 WHERE id=?1 AND state='prepared'",params![id,now.to_rfc3339()])?==1,"conversation_tools.question_changed");
    set_view(tx,id)
}
fn outcome(question_set:&Value,answers:&Value,decline:bool)->Result<Value>{
    if decline{return Ok(json!({"answers":[],"declined":true}))}
    let questions=question_set["questions"].as_array().context("conversation_tools.questions_required")?;
    let answers=answers.as_array().context("conversation_tools.answers_required")?;
    ensure!(questions.len()==answers.len(),"conversation_tools.all_answers_required");
    let mut seen=std::collections::HashSet::new();let mut result=Vec::new();
    for question in questions{
        let id=question["id"].as_str().context("conversation_tools.question_id_invalid")?;
        let matching=answers.iter().filter(|answer|answer["questionId"]==id).collect::<Vec<_>>();ensure!(matching.len()==1&&seen.insert(id),"conversation_tools.answer_question_mismatch");
        let selected=matching[0]["labels"].as_array().context("conversation_tools.answer_labels_required")?;
        let spec=&question["spec"];let multiple=spec["multiSelect"].as_bool().context("conversation_tools.selection_mode_required")?;
        ensure!(!selected.is_empty()&&(multiple||selected.len()==1),"conversation_tools.selection_count_invalid");
        let options=spec["options"].as_array().context("conversation_tools.options_required")?;
        let mut unique=std::collections::HashSet::new();let mut free_text=0;
        for label in selected{let text=label.as_str().context("conversation_tools.answer_text_required")?;
            ensure!(!text.trim().is_empty()&&text.chars().count()<=4096&&unique.insert(text),"conversation_tools.answer_text_invalid");
            if !options.iter().any(|option|option["label"]==text){free_text+=1;}
        }
        ensure!(free_text<=1,"conversation_tools.one_extra_text_allowed");
        result.push(json!({"header":spec["header"],"question":spec["question"],"multi_select":multiple,"selected":selected}));
    }
    let value=json!({"answers":result,"declined":false});validate("QuestionOutcomeV1",&value)?;Ok(value)
}
/// User命令的外层事务调用；先保存的答案与同一份待发送结果一起提交。
pub(crate) fn answer_on(tx:&Transaction<'_>,lease:&RunLease,client:&str,command:&str,id:&str,expected:i64,answers:&Value,decline:bool)->Result<Value>{
    uuid::Uuid::parse_str(command)?;
    let input_digest=digest(&json!({"requestId":id,"expectedRevision":expected,"answers":answers,"decline":decline}))?;
    let prior_command:Option<(String,String)>=tx.query_row("SELECT request_id,input_digest FROM conversation_tool_answer WHERE command_id=?1",[command],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
    if let Some((request,stored))=prior_command{ensure!(request==id&&stored==input_digest,"conversation_tools.answer_command_changed");return requests::status(tx,lease,id)}
    let answered:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tool_answer WHERE request_id=?1)",[id],|row|row.get(0))?;
    if answered{return requests::status(tx,lease,id)}
    let view=set_view(tx,id)?;
    ensure!(view["agentRunId"]==lease.run_id&&view["executionEpoch"]==lease.epoch&&view["scope"]==serde_json::to_value(&lease.frozen.scope)?,"conversation_tools.question_owner_changed");
    ensure!(view["revision"].as_i64()==Some(expected)&&view["state"]=="awaiting_answer","conversation_tools.question_changed");
    let expires=chrono::DateTime::parse_from_rfc3339(view["expiresAt"].as_str().context("conversation_tools.expiry_required")?)?;
    ensure!(expires.with_timezone(&chrono::Utc)>chrono::Utc::now(),"conversation_tools.question_expired");
    let cipher:Vec<u8>=tx.query_row("SELECT target_cipher FROM conversation_tool_question WHERE request_id=?1",[id],|row|row.get(0))?;
    let target:NativeQuestionTarget=crate::member_environment::decrypt(tx,&context(id,"target"),&cipher)?;target_guard(tx,lease,&target)?;
    let result=outcome(&view,answers,decline)?;
    let answer_cipher=crate::member_environment::encrypt(tx,&context(id,"answer"),&result)?;
    let result_cipher=crate::member_environment::encrypt(tx,&format!("conversation-tool:{id}:result"),&result)?;
    let locator_cipher=crate::member_environment::encrypt(tx,&context(id,"delivery"),&target.locator)?;
    let now=chrono::Utc::now().to_rfc3339();
    tx.execute("INSERT INTO conversation_tool_answer(request_id,command_id,client_id,input_digest,value_cipher,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id,command,client,input_digest,answer_cipher,now])?;
    ensure!(tx.execute("UPDATE conversation_tool_request SET state='answered',revision=revision+1,result_cipher=?3,result_kind='question',updated_at=?4 WHERE id=?1 AND revision=?2 AND state='awaiting_answer'",params![id,expected,result_cipher,now])?==1,"conversation_tools.question_changed");
    tx.execute("INSERT INTO conversation_tool_delivery(request_id,agent_run_id,execution_epoch,bridge_instance_id,native_binding_id,native_binding_generation,native_locator_cipher,result_digest,state,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'pending',?9,?9)",params![id,lease.run_id,lease.epoch,target.bridge_instance_id,target.native_binding_id,target.native_binding_generation,locator_cipher,digest(&result)?,now])?;
    requests::status(tx,lease,id)
}
pub(crate) fn list(connection:&Connection,input:&Value)->Result<Value>{
    validate_operation("conversationTools.questions.list",input)?;let scope:ScopeV1=serde_json::from_value(input["scope"].clone())?;scope::require_retained_scope(connection,&scope)?;
    let mut query=connection.prepare("SELECT q.request_id FROM conversation_tool_question q JOIN conversation_tool_request r ON r.id=q.request_id JOIN conversation_tool_run owned ON owned.agent_run_id=r.agent_run_id AND owned.execution_epoch=r.execution_epoch WHERE owned.thread_id=?1 AND owned.scope_kind=?2 AND owned.conversation_id IS ?3 AND (?4 IS NULL OR q.request_id>?4) ORDER BY q.request_id LIMIT ?5")?;
    let limit=input["limit"].as_u64().context("conversation_tools.limit_required")?;
    let mut ids=query.query_map(params![scope.thread_id,scope::kind(&scope.scope_kind),scope.conversation_id,input["after"].as_str(),limit+1],|row|row.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let more=ids.len()>limit as usize;ids.truncate(limit as usize);let next=if more{ids.last().cloned()}else{None};
    let values=ids.into_iter().map(|id|set_view(connection,&id)).collect::<Result<Vec<_>>>()?;Ok(json!({"items":values,"next":next}))
}
/// Core生命周期事务调用；停止与到期只取消投递，不能伪造用户拒答或默认选择。
pub(crate) fn close_unavailable(connection:&Connection)->Result<usize>{
    let mut query=connection.prepare(r#"
        SELECT request.id FROM conversation_tool_request request
        JOIN conversation_tool_run owned ON owned.agent_run_id=request.agent_run_id AND owned.execution_epoch=request.execution_epoch
        LEFT JOIN agent_run run ON run.id=request.agent_run_id AND run.execution_epoch=request.execution_epoch
        WHERE request.tool_name='ask_user_question' AND request.state IN ('awaiting_answer','answered')
          AND(request.expires_at_ms<=?1 OR owned.revoked_at IS NOT NULL OR run.id IS NULL OR run.status<>'running' OR run.cancel_requested_at IS NOT NULL)
        ORDER BY request.created_at LIMIT 1000
    "#)?;
    let ids=query.query_map([chrono::Utc::now().timestamp_millis()],|row|row.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;drop(query);
    let now=chrono::Utc::now().to_rfc3339();
    for id in &ids{
        let writing:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tool_delivery WHERE request_id=?1 AND state IN ('claimed','outcome_unknown'))",[id],|row|row.get(0))?;
        connection.execute("UPDATE conversation_tool_request SET state=?2,revision=revision+1,updated_at=?3 WHERE id=?1 AND state IN ('awaiting_answer','answered')",params![id,if writing{"outcome_unknown"}else{"cancelled"},now])?;
        connection.execute("UPDATE conversation_tool_delivery SET state=CASE WHEN state='pending' THEN 'cancelled' ELSE 'outcome_unknown' END,updated_at=?2 WHERE request_id=?1 AND state IN ('pending','claimed')",params![id,now])?;
    }
    Ok(ids.len())
}
