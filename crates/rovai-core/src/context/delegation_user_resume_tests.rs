// Same original Context owner/database. Core commands/receipts/factory/context
// are real; native Session/input/terminal/cleanup acknowledgments are fixtures.
fn user_resume_request(task: &str, attempt: i64) -> Value {
    json!({"commandId":Uuid::new_v4().to_string(),"command":{"delegationId":task,"expectedAttemptNo":attempt,"reason":"中断后继续原任务"}})
}
fn user_resume_call(
    f: &mut Fixture,
    request: Value,
) -> anyhow::Result<crate::command::CommandExecution> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let db = tokio::sync::Mutex::new(&mut f.database);
    rt.block_on(crate::application::delegation_queries::resume(
        &db,
        &std::sync::Arc::new(tokio::sync::Semaphore::new(2)),
        request,
    ))
}
fn user_resume_snapshot(f: &Fixture, task: &str) -> Value {
    let control = crate::delegation::control(f.database.connection(), task).unwrap();
    let (version,sequence):(i64,i64)=f.database.connection().query_row("SELECT c.version,c.last_message_sequence FROM delegation d JOIN conversation c ON c.id=d.child_conversation_id WHERE d.id=?1",[task],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    let events: i64 = f
        .database
        .connection()
        .query_row("SELECT COUNT(*) FROM event_log", [], |r| r.get(0))
        .unwrap();
    json!({"graph":factory_counts(f),"attempt":control.current_attempt_no,"control":control.version,"conversationVersion":version,"sequence":sequence,"events":events})
}
fn assert_user_delegation_resume(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    request: &crate::delegation::FrozenRequest<'_>,
    plan: &crate::delegation_factory::ChildRunPlan<'_>,
) {
    for cancelled_parent in [false, true] {
        assert_user_resume_scenario(f, binding, source, request, plan, cancelled_parent);
    }
}
fn assert_user_resume_scenario(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    request: &crate::delegation::FrozenRequest<'_>,
    plan: &crate::delegation_factory::ChildRunPlan<'_>,
    cancelled_parent: bool,
) {
    use crate::{
        application::delegation_queries as api,
        delegation_contract::{CancelParams, ResumeParams},
        delegation_model_command::{self as model, Invocation},
        delegation_resume::user,
    };
    let parent = model_start(
        f,
        binding,
        source,
        request,
        plan,
        &format!("user-resume-source-{cancelled_parent}"),
    )
    .unwrap();
    let parent_id = parent.result.payload["task_id"].as_str().unwrap();
    let parent_run: String = f
        .database
        .connection()
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [parent_id],
            |r| r.get(0),
        )
        .unwrap();
    let parent_epoch = claim_context_child(f, &parent_run);
    resume_bind(
        f,
        &parent_run,
        parent_epoch,
        &format!("fixture-user-resume-parent-session-{parent_id}"),
    );
    let parent_context = resume_context(f, &parent_run, parent_epoch);
    resume_accept_input(
        f,
        &parent_run,
        parent_epoch,
        &parent_context,
        "fixture-user-resume-parent-input",
    );
    let parent_binding = TeamToolService::default()
        .prepare_binding_credential(&mut f.database, &parent_run, parent_epoch, false)
        .unwrap();
    let parent_source = crate::team_tool::AuthenticatedTeamToolRun {
        agent_run_id: parent_run.clone(),
        execution_epoch: parent_epoch,
        ..source.clone()
    };
    let parent_workspace: String = f
        .database
        .connection()
        .query_row(
            "SELECT workspace_json FROM agent_run WHERE id=?1",
            [&parent_run],
            |r| r.get(0),
        )
        .unwrap();
    let parent_workspace: crate::runtime::AgentRunWorkspace =
        serde_json::from_str(&parent_workspace).unwrap();
    let directory = std::path::Path::new(&parent_workspace.execution_root)
        .join("user-resume-canonical-workspace");
    std::fs::create_dir_all(&directory).unwrap();
    let working = crate::delegation_contract::WorkingDirectory::try_from(
        directory
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned(),
    )
    .unwrap();
    let request = crate::delegation::FrozenRequest {
        working_dir: Some(&working),
        ..*request
    };
    let initial = model_start(
        f,
        &parent_binding,
        &parent_source,
        &request,
        plan,
        &format!("user-resume-task-{cancelled_parent}"),
    )
    .unwrap();
    let task = initial.result.payload["task_id"].as_str().unwrap();
    let conversation = initial.result.payload["child_conversation_id"]
        .as_str()
        .unwrap();
    let run: String = f
        .database
        .connection()
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [task],
            |r| r.get(0),
        )
        .unwrap();
    let epoch = claim_context_child(f, &run);
    let session = format!("fixture-user-exact-session-{task}");
    assert_eq!(
        resume_bind(f, &run, epoch, &session).result.status,
        crate::command::CommandResultStatus::Applied
    );
    let context = resume_context(f, &run, epoch);
    let delivery = resume_accept_input(f, &run, epoch, &context, "fixture-user-original-accepted");
    let old_payload = context.runtime_payload.clone();
    assert_eq!(
        user_resume_call(f, user_resume_request(task, 1))
            .unwrap()
            .result
            .code,
        "delegation.not_resumable"
    );
    let stop = CancelParams {
        task_id: task.to_owned().try_into().unwrap(),
    };
    model::cancel(
        &mut f.database,
        &Invocation {
            binding: &parent_binding,
            source: &parent_source,
            tool_call: "user-resume-stop-original",
            input: &stop,
        },
    )
    .unwrap();
    let early = user_resume_request(task, 1);
    assert_eq!(
        user_resume_call(f, early.clone()).unwrap().result.code,
        "delegation.cleanup_pending"
    );
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &run, epoch)
        .unwrap();
    assert_eq!(
        user_resume_call(f, early).unwrap().result.code,
        "delegation.cleanup_pending",
        "first refusal stays durable after cleanup changes"
    );
    if cancelled_parent {
        let stop = CancelParams {
            task_id: parent_id.to_owned().try_into().unwrap(),
        };
        model::cancel(
            &mut f.database,
            &Invocation {
                binding,
                source,
                tool_call: "user-resume-parent-cancel",
                input: &stop,
            },
        )
        .unwrap();
        ExecutionRuntimeService::default()
            .record_runtime_cleanup_completed(&f.database, &parent_run, parent_epoch)
            .unwrap();
    } else {
        lifecycle_succeed(f, &parent_run, parent_epoch);
    }
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &parent_run, parent_epoch)
        .unwrap();
    assert_eq!(
        lifecycle_status(f, &parent_run),
        if cancelled_parent {
            "cancelled"
        } else {
            "succeeded"
        }
    );
    assert_eq!(lifecycle_status(f, &run), "cancelled");
    let model_input = ResumeParams {
        task_id: task.to_owned().try_into().unwrap(),
        reason: None,
    };
    let changes = f.database.connection().total_changes();
    assert!(
        model_resume(
            f,
            &parent_binding,
            &parent_source,
            "terminal-parent-model-refused",
            &model_input
        )
        .is_err()
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    for command in [
        json!({"delegationId":task,"expectedAttemptNo":1,"reason":null}),
        json!({"delegationId":task,"expectedAttemptNo":1,"task":"新任务"}),
        json!({"delegationId":task,"expectedAttemptNo":1,"reason":"中".repeat(700)}),
    ] {
        let before = f.database.connection().total_changes();
        assert!(
            api::resume_envelope(json!({"commandId":Uuid::new_v4().to_string(),"command":command}))
                .is_err()
        );
        assert_eq!(f.database.connection().total_changes(), before);
    }
    assert_eq!(
        user_resume_call(f, user_resume_request(task, 0))
            .unwrap()
            .result
            .code,
        "delegation.attempt_changed"
    );
    let moved = directory.with_file_name("user-resume-moved-workspace");
    let unavailable = user_resume_request(task, 1);
    let e = api::resume_envelope(unavailable.clone()).unwrap();
    let prepared = user::prepare(&f.database, &e).unwrap();
    std::fs::rename(&directory, &moved).unwrap();
    let observation = prepared.observe_directory();
    let result = user::commit(&mut f.database, &e, &prepared, &observation).unwrap();
    assert_eq!(result.result.code, "delegation.directory_unavailable");
    std::fs::rename(&moved, &directory).unwrap();
    assert_eq!(
        user_resume_call(f, unavailable).unwrap().result,
        result.result
    );
    let successful = user_resume_request(task, 1);
    let e = api::resume_envelope(successful.clone()).unwrap();
    let prepared = user::prepare(&f.database, &e).unwrap();
    let observation = prepared.observe_directory();
    let concurrent = api::resume_envelope(user_resume_request(task, 1)).unwrap();
    let concurrent_prepared = user::prepare(&f.database, &concurrent).unwrap();
    let concurrent_observation = concurrent_prepared.observe_directory();
    // Unrelated settings revisions must not invalidate this frozen original task.
    let saved_settings = api::read(&f.database, "delegations.settings.get", json!({})).unwrap();
    let cache = saved_settings["completedCacheMaxMb"].as_i64().unwrap();
    api::update_settings(
        &mut f.database,
        settings_request(
            saved_settings["version"].as_i64().unwrap(),
            json!({"completedCacheMaxMb":cache+1}),
        ),
    )
    .unwrap();
    // Suppressing the User event must not bypass the candidate SQL parent fence.
    for trigger in [
        "CREATE TEMP TRIGGER fail_user_resume BEFORE INSERT ON event_log WHEN NEW.event_type='delegation.resumed' AND NEW.actor_type='user' BEGIN SELECT RAISE(IGNORE); END;",
        "CREATE TEMP TRIGGER fail_user_resume BEFORE INSERT ON event_log WHEN NEW.event_type='delegation.resumed' AND NEW.actor_type='user' BEGIN SELECT RAISE(ABORT,'user-resume-event-fault'); END;",
        "CREATE TEMP TRIGGER fail_user_resume BEFORE INSERT ON event_log WHEN NEW.event_type='command.result' AND NEW.command_type='delegations.resume' BEGIN SELECT RAISE(ABORT,'user-resume-receipt-fault'); END;",
    ] {
        let before = user_resume_snapshot(f, task);
        f.database.connection().execute_batch(trigger).unwrap();
        assert!(user::commit(&mut f.database, &e, &prepared, &observation).is_err());
        assert_eq!(user_resume_snapshot(f, task), before);
        assert_eq!(
            api::reconcile_resume(&f.database, successful.clone()).unwrap()["state"],
            "unknown"
        );
        f.database
            .connection()
            .execute_batch("DROP TRIGGER fail_user_resume")
            .unwrap();
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let resumed=rt.block_on(async {
        let db=tokio::sync::Mutex::new(&mut f.database);
        let capacity=std::sync::Arc::new(tokio::sync::Semaphore::new(0));
        let pending=api::resume(&db,&capacity,successful.clone());
        tokio::pin!(pending);
        tokio::select! { result=&mut pending=>panic!("worker must wait for capacity: {}",result.is_ok()), _=tokio::time::sleep(std::time::Duration::from_millis(2))=>{} }
        assert!(db.try_lock().is_ok(),"worker-capacity wait cannot hold the database");
        capacity.add_permits(1);
        pending.await.unwrap()
    });
    assert_eq!(resumed.result.code, "delegation.resumed");
    assert_eq!(resumed.result.payload["attemptNo"], 2);
    assert_eq!(resumed.result.payload["conversationId"], conversation);
    let settings = api::read(&f.database, "delegations.settings.get", json!({})).unwrap();
    api::update_settings(
        &mut f.database,
        settings_request(
            settings["version"].as_i64().unwrap(),
            json!({"completedCacheMaxMb":cache}),
        ),
    )
    .unwrap();
    let next = resumed.result.payload["agentRunId"].as_str().unwrap();
    let (stored_parent,stored_epoch,call,task_blob,stored_session,body,why):(String,i64,String,String,String,String,String)=f.database.connection().query_row(
        "SELECT a.trigger_parent_run_id,a.trigger_parent_epoch,a.trigger_tool_call_id,d.task_blob_id,c.native_session_id,m.body,a.interruption_reason FROM delegation d
         JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=2 JOIN conversation c ON c.id=d.child_conversation_id
         JOIN agent_run r ON r.id=a.agent_run_id JOIN conversation_message m ON m.id=r.trigger_conversation_message_id WHERE d.id=?1",[task],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).unwrap();
    assert_eq!(
        (stored_parent, stored_epoch),
        (parent_run.clone(), parent_epoch)
    );
    assert_eq!(call, format!("user-command:{}", e.command_id));
    assert_eq!(task_blob, request.task_blob_id);
    assert_eq!(stored_session, session);
    assert_eq!(
        body,
        include_str!("../../resources/delegation-resume.txt").trim()
    );
    assert_eq!(why, "中断后继续原任务");
    let stale = user::commit(
        &mut f.database,
        &concurrent,
        &concurrent_prepared,
        &concurrent_observation,
    )
    .unwrap();
    assert_eq!(stale.result.code, "delegation.attempt_changed");
    let before = f.database.connection().total_changes();
    assert_eq!(
        user_resume_call(f, successful.clone()).unwrap().result,
        resumed.result
    );
    assert_eq!(
        api::reconcile_resume(&f.database, successful.clone()).unwrap()["result"],
        serde_json::to_value(&resumed.result).unwrap()
    );
    assert_eq!(f.database.connection().total_changes(), before);
    let mut changed = successful.clone();
    changed["command"]["reason"] = json!("不同原因");
    assert!(user_resume_call(f, changed).is_err());
    let next_epoch = claim_context_child(f, next);
    let execution = ExecutionRuntimeService::default()
        .load_agent_run_execution(&f.database, next, next_epoch)
        .unwrap()
        .unwrap();
    assert_eq!(
        ExecutionRuntimeService::default()
            .prepare_native_session_resume(&mut f.database, &execution)
            .unwrap(),
        crate::runtime::NativeSessionResumeDisposition::Compatible
    );
    assert!(
        !crate::run_continuation::can_replace_session_before_dispatch(
            f.database.connection(),
            next,
            next_epoch
        )
        .unwrap()
    );
    assert_eq!(
        resume_bind(f, next, next_epoch, "incorrect-cold-user-session")
            .result
            .code,
        "delegation.session_changed"
    );
    assert_eq!(
        resume_bind(f, next, next_epoch, &session).result.status,
        crate::command::CommandResultStatus::Applied
    );
    let resumed_context = resume_context(f, next, next_epoch);
    assert_eq!(
        context_section(&resumed_context.rendered_payload, "CURRENT_INPUT")["message"],
        include_str!("../../resources/delegation-resume.txt").trim()
    );
    // A stopped Run cannot materialize new context. Read its exact frozen Blob.
    let original_blob: String = f
        .database
        .connection()
        .query_row(
            "SELECT rendered_payload_blob_id FROM context_manifest WHERE id=?1",
            [&context.manifest_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        ManagedBlobStore::new(&f.directory)
            .read_text(&f.database, &original_blob)
            .unwrap(),
        context.rendered_payload
    );
    assert_ne!(old_payload, resumed_context.runtime_payload);
    assert_eq!(
        context_section(&resumed_context.rendered_payload, "CURRENT_INPUT")["interruptionContext"],
        "中断后继续原任务"
    );
    assert!(!resumed_context.runtime_payload.contains(plan.task));
    resume_accept_input(
        f,
        next,
        next_epoch,
        &resumed_context,
        "fixture-user-resume-accepted",
    );
    crate::application::delegation_control::handle(
        &mut f.database,
        "delegations.cancel",
        control_request(task, "attemptNo", 2),
    )
    .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, next, next_epoch)
        .unwrap();
    let race = api::resume_envelope(user_resume_request(task, 2)).unwrap();
    let race_prepared = user::prepare(&f.database, &race).unwrap();
    let race_observation = race_prepared.observe_directory();
    crate::application::delegation_control::handle(
        &mut f.database,
        "delegations.takeover",
        control_request(task, "expectedControlVersion", 2),
    )
    .unwrap();
    assert_eq!(
        user::commit(&mut f.database, &race, &race_prepared, &race_observation)
            .unwrap()
            .result
            .code,
        "delegation.not_resumable"
    );
    let accepted: String = f
        .database
        .connection()
        .query_row(
            "SELECT status FROM runtime_input_delivery WHERE id=?1",
            [delivery],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(accepted, "accepted");
    // Actual User takeover and closure of the parent fences fresh resumes, while
    // the original accepted global User receipt remains readable and unchanged.
    crate::application::delegation_control::handle(
        &mut f.database,
        "delegations.takeover",
        control_request(parent_id, "expectedControlVersion", 1),
    )
    .unwrap();
    let parent_conversation = parent.result.payload["child_conversation_id"]
        .as_str()
        .unwrap();
    crate::application::delegation_control::handle(&mut f.database,"delegations.conversation.end",json!({"commandId":Uuid::new_v4().to_string(),"command":{"conversationId":parent_conversation}})).unwrap();
    let closed = user_resume_call(f, user_resume_request(task, 2)).unwrap();
    assert_eq!(closed.result.code, "delegation.parent_unavailable");
    let before = f.database.connection().total_changes();
    assert_eq!(
        user_resume_call(f, successful.clone()).unwrap().result,
        resumed.result
    );
    assert_eq!(
        api::reconcile_resume(&f.database, successful).unwrap()["result"],
        serde_json::to_value(&resumed.result).unwrap()
    );
    assert_eq!(f.database.connection().total_changes(), before);
}
