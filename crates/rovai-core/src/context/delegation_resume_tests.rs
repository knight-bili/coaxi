// Original Context owner/database. Real Core factory, Binding, Manifest, input
// receipts and cancellation; Native Session/ACK/cleanup are transport fixtures.
fn model_resume(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    call: &str,
    input: &crate::delegation_contract::ResumeParams,
) -> anyhow::Result<crate::command::CommandExecution> {
    let invocation = crate::delegation_model_command::Invocation {
        binding,
        source,
        tool_call: call,
        input,
    };
    if let Some(result) = crate::delegation_model_command::replay_resume(&f.database, &invocation)?
    {
        return Ok(result);
    }
    let prepared = crate::delegation_resume::prepare(&f.database, &invocation)?;
    let directory = prepared.observe_directory();
    crate::delegation_model_command::resume(&mut f.database, &invocation, &prepared, &directory)
}
fn resume_context(f: &mut Fixture, run: &str, epoch: i64) -> PreparedContext {
    let ContextMaterialization::Ready(context) = ContextService
        .materialize(
            &mut f.database,
            &ManagedBlobStore::new(&f.directory),
            &MaterializeContextRequest {
                agent_run_id: run,
                execution_epoch: epoch,
                charter_delivery_mode: CharterDeliveryMode::FirstPayload,
                max_payload_bytes: DEFAULT_MAX_CONTEXT_PAYLOAD_BYTES,
            },
        )
        .unwrap()
    else {
        panic!("delegation resume Context unavailable");
    };
    context
}
fn resume_accept_input(
    f: &mut Fixture,
    run: &str,
    epoch: i64,
    context: &PreparedContext,
    native: &str,
) -> String {
    let before = f.database.connection().total_changes();
    for (request_digest, auth_json) in [(None, None), (Some("fixture-digest"), Some("{}"))] {
        let error = f.database.connection().execute(
            "INSERT INTO runtime_input_delivery(id,agent_run_id,execution_epoch,context_manifest_id,
                native_binding_id,native_binding_generation,boundary_camp_message_sequence,dynamic_payload_digest,
                status,prepared_at,updated_at,runtime_request_digest,runtime_attachment_auth_receipt_json)
             SELECT ?1,r.id,r.execution_epoch,m.id,c.native_binding_id,c.native_binding_generation,
                m.camp_message_boundary_sequence,'sha256:'||b.sha256,'prepared',datetime('now'),datetime('now'),?4,?5
             FROM agent_run r JOIN conversation c ON c.id=r.conversation_id JOIN context_manifest m ON m.agent_run_id=r.id
             JOIN managed_blob b ON b.id=m.rendered_payload_blob_id WHERE r.id=?2 AND m.id=?3",
            params![Uuid::new_v4().to_string(),run,context.manifest_id,request_digest,auth_json],
        ).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Runtime Input Delivery attachment evidence does not match its manifest")
        );
    }
    assert_eq!(f.database.connection().total_changes(), before);
    let delivery = ContextService
        .prepare_input_delivery_for_context(&mut f.database, run, epoch, context)
        .unwrap();
    assert!(
        ContextService
            .begin_runtime_input_dispatch(&mut f.database, &delivery.id, run, epoch)
            .unwrap()
    );
    assert_eq!(
        ContextService
            .acknowledge_active_input_delivery(&mut f.database, &delivery.id, native)
            .unwrap()
            .0
            .status,
        "accepted"
    );
    delivery.id
}
fn resume_bind(
    f: &mut Fixture,
    run: &str,
    epoch: i64,
    session: &str,
) -> crate::command::CommandExecution {
    let credential = TeamToolService::default()
        .prepare_binding_credential(&mut f.database, run, epoch, false)
        .unwrap();
    let execution = ExecutionRuntimeService::default()
        .load_agent_run_execution(&f.database, run, epoch)
        .unwrap()
        .unwrap();
    let command = lifecycle_command(
        f,
        ActorRef::System {
            component_id: "runtime-adapter:codex-fixture".into(),
        },
        crate::runtime::BindNativeSessionCommand {
            conversation_id: execution.conversation_id,
            agent_run_id: run.into(),
            expected_conversation_version: credential.conversation_version,
            expected_execution_epoch: epoch,
            previous_adapter_installation_id: credential.previous_adapter_installation_id,
            previous_native_session_id: credential.previous_native_session_id,
            previous_binding_compatibility_digest: credential.previous_binding_compatibility_digest,
            proposed_binding_id: Some(credential.native_binding_id),
            adapter_installation_id: execution.runtime.installation_id,
            native_session_id: session.into(),
            binding_compatibility_digest: execution.runtime.binding_compatibility_digest,
        },
    );
    ExecutionRuntimeService::default()
        .bind_native_session(&mut f.database, &command)
        .unwrap()
}
fn assert_delegation_resume(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    request: &crate::delegation::FrozenRequest<'_>,
    plan: &crate::delegation_factory::ChildRunPlan<'_>,
    completed: &str,
) {
    use crate::{
        delegation_contract::{CancelParams, ResumeParams, StatusParams},
        delegation_model_command::{self as model, Invocation},
    };
    let initial = model_start(f, binding, source, request, plan, "resume-initial-task").unwrap();
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
    let session = format!("fixture-exact-session-{task}");
    assert_eq!(
        resume_bind(f, &run, epoch, &session).result.status,
        crate::command::CommandResultStatus::Applied
    );
    let context = resume_context(f, &run, epoch);
    assert_eq!(
        context_section(&context.rendered_payload, "CURRENT_INPUT")["message"],
        plan.task
    );
    let delivery = resume_accept_input(f, &run, epoch, &context, "fixture-accepted-original");
    let old_payload = context.runtime_payload.clone();
    let old_manifest = context.manifest_id.clone();
    let input = ResumeParams {
        task_id: task.to_owned().try_into().unwrap(),
        reason: Some("本机中断后继续原任务".to_owned().try_into().unwrap()),
    };
    let before = factory_counts(f);
    let active = model_resume(f, binding, source, "resume-active-rejected", &input).unwrap();
    assert_eq!(active.result.code, "delegation.not_resumable");
    assert_eq!(factory_counts(f), before);
    let completed_input = ResumeParams {
        task_id: completed.to_owned().try_into().unwrap(),
        reason: None,
    };
    assert_eq!(
        model_resume(
            f,
            binding,
            source,
            "resume-completed-rejected",
            &completed_input
        )
        .unwrap()
        .result
        .code,
        "delegation.not_resumable"
    );
    let foreign = ResumeParams {
        task_id: "not-owned-resume".to_owned().try_into().unwrap(),
        reason: None,
    };
    assert_eq!(
        model_resume(f, binding, source, "resume-unknown", &foreign)
            .unwrap()
            .result
            .payload,
        json!({"task_id":"not-owned-resume","status":"unknown"})
    );
    let stop = CancelParams {
        task_id: task.to_owned().try_into().unwrap(),
    };
    model::cancel(
        &mut f.database,
        &Invocation {
            binding,
            source,
            tool_call: "interrupt-original-for-resume",
            input: &stop,
        },
    )
    .unwrap();
    assert_eq!(
        model_resume(f, binding, source, "resume-cleanup-pending", &input)
            .unwrap()
            .result
            .code,
        "delegation.cleanup_pending"
    );
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &run, epoch)
        .unwrap();
    let before = factory_counts(f);
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=0 WHERE singleton=1",
            [],
        )
        .unwrap();
    let disabled = model_resume(f, binding, source, "resume-disabled", &input).unwrap();
    assert_eq!(disabled.result.code, "delegation.start_disabled");
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=1 WHERE singleton=1",
            [],
        )
        .unwrap();
    assert_eq!(
        model_resume(f, binding, source, "resume-disabled", &input)
            .unwrap()
            .result,
        disabled.result
    );
    assert_eq!(factory_counts(f), before);
    let invocation = Invocation {
        binding,
        source,
        tool_call: "resume-success",
        input: &input,
    };
    let prepared = crate::delegation_resume::prepare(&f.database, &invocation).unwrap();
    let directory = prepared.observe_directory();
    assert!(
        model::resume(
            &mut f.database,
            &Invocation {
                tool_call: "wrong-resume-call",
                ..invocation
            },
            &prepared,
            &directory
        )
        .is_err()
    );
    let before = factory_counts(f);
    let control_before = crate::delegation::control(f.database.connection(), task).unwrap();
    f.database.connection().execute_batch("CREATE TEMP TRIGGER fail_resume_receipt BEFORE INSERT ON event_log WHEN NEW.event_type='command.result' AND NEW.command_type='delegation.resume' BEGIN SELECT RAISE(ABORT,'resume receipt fixture fault'); END;").unwrap();
    assert!(
        format!(
            "{:#}",
            model::resume(&mut f.database, &invocation, &prepared, &directory).unwrap_err()
        )
        .contains("resume receipt fixture fault")
    );
    assert_eq!(factory_counts(f), before);
    assert_eq!(
        crate::delegation::control(f.database.connection(), task)
            .unwrap()
            .version,
        control_before.version
    );
    assert_eq!(
        crate::delegation::control(f.database.connection(), task)
            .unwrap()
            .current_attempt_no,
        1
    );
    assert!(
        model::replay_resume(&f.database, &invocation)
            .unwrap()
            .is_none()
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER fail_resume_receipt;")
        .unwrap();
    let resumed = model::resume(&mut f.database, &invocation, &prepared, &directory).unwrap();
    assert_eq!(resumed.result.code, "delegation.resumed");
    assert_eq!(resumed.result.payload["task_id"], task);
    assert_eq!(
        resumed.result.payload["child_conversation_id"],
        conversation
    );
    assert_eq!(resumed.result.payload["status"], "running");
    let next: String = f
        .database
        .connection()
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1 AND attempt_no=2",
            [task],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(next, run);
    let (task_blob,stored_session,stored_binding,generation,body,why):(String,String,String,i64,String,String)=f.database.connection().query_row(
        "SELECT d.task_blob_id,c.native_session_id,c.native_binding_id,c.native_binding_generation,m.body,a.interruption_reason
         FROM delegation d JOIN conversation c ON c.id=d.child_conversation_id JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=2
         JOIN agent_run r ON r.id=a.agent_run_id JOIN conversation_message m ON m.id=r.trigger_conversation_message_id WHERE d.id=?1",[task],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap();
    assert_eq!(task_blob, request.task_blob_id);
    assert_eq!(stored_session, session);
    assert_eq!(
        body,
        include_str!("../../resources/delegation-resume.txt").trim()
    );
    assert_eq!(why, input.reason.as_ref().unwrap().as_str());
    let changes = f.database.connection().total_changes();
    assert_eq!(
        model_resume(f, binding, source, invocation.tool_call, &input)
            .unwrap()
            .result,
        resumed.result
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    let changed = ResumeParams {
        reason: Some("不能更换同号原因".to_owned().try_into().unwrap()),
        ..input.clone()
    };
    assert!(
        model::replay_resume(
            &f.database,
            &Invocation {
                input: &changed,
                ..invocation
            }
        )
        .is_err()
    );
    // The original managed task, manifest, input record and receipt stay intact.
    let original: (String, String) = f
        .database
        .connection()
        .query_row(
            "SELECT status,context_manifest_id FROM runtime_input_delivery WHERE id=?1",
            [&delivery],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(original, ("accepted".into(), old_manifest));
    let candidate = ExecutionRuntimeService::default()
        .list_dispatchable_agent_runs(&f.database, 8)
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.agent_run_id == next)
        .unwrap();
    let frozen_runtime = candidate.frozen_runtime().unwrap();
    let mut incompatible = frozen_runtime.clone();
    incompatible.native_session_compatibility_key = Some("different-session-namespace".into());
    incompatible.refresh_config_digest().unwrap();
    let rebind = |runtime| {
        lifecycle_command(
            f,
            ActorRef::System {
                component_id: "agent-run-scheduler".into(),
            },
            crate::runtime::RebindAgentRunRuntimeCommand {
                agent_run_id: next.clone(),
                expected_version: candidate.version,
                drift_reason: "explicit fixture Runtime observation".into(),
                effective_runtime: runtime,
            },
        )
    };
    let incompatible_command = rebind(incompatible);
    assert_eq!(
        ExecutionRuntimeService::default()
            .rebind_agent_run_runtime(&mut f.database, &incompatible_command,)
            .unwrap()
            .result
            .code,
        "delegation.session_incompatible"
    );
    let mut compatible = frozen_runtime;
    compatible.reported_version = Some("fixture-compatible-runtime-version".into());
    compatible.refresh_config_digest().unwrap();
    let compatible_command = lifecycle_command(
        f,
        ActorRef::System {
            component_id: "agent-run-scheduler".into(),
        },
        crate::runtime::RebindAgentRunRuntimeCommand {
            agent_run_id: next.clone(),
            expected_version: candidate.version,
            drift_reason: "explicit compatible fixture version".into(),
            effective_runtime: compatible,
        },
    );
    assert_eq!(
        ExecutionRuntimeService::default()
            .rebind_agent_run_runtime(&mut f.database, &compatible_command,)
            .unwrap()
            .result
            .status,
        crate::command::CommandResultStatus::Applied
    );
    let next_epoch = claim_context_child(f, &next);
    assert!(
        !crate::run_continuation::can_replace_session_before_dispatch(
            f.database.connection(),
            &next,
            next_epoch
        )
        .unwrap()
    );
    assert!(
        TeamToolService::default()
            .prepare_binding_credential(&mut f.database, &next, next_epoch, true)
            .is_err()
    );
    let execution = ExecutionRuntimeService::default()
        .load_agent_run_execution(&f.database, &next, next_epoch)
        .unwrap()
        .unwrap();
    assert_eq!(
        ExecutionRuntimeService::default()
            .prepare_native_session_resume(&mut f.database, &execution)
            .unwrap(),
        crate::runtime::NativeSessionResumeDisposition::Compatible
    );
    let binding_before:(String,i64,String)=f.database.connection().query_row("SELECT native_binding_id,native_binding_generation,native_session_id FROM conversation WHERE id=?1",[conversation],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(
        resume_bind(f, &next, next_epoch, "silently-created-other-session")
            .result
            .code,
        "delegation.session_changed"
    );
    let binding_after:(String,i64,String)=f.database.connection().query_row("SELECT native_binding_id,native_binding_generation,native_session_id FROM conversation WHERE id=?1",[conversation],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(binding_after, binding_before);
    // Explicit isolated pointer corruption has a before snapshot and is restored;
    // neither ordinary binding preparation nor launch may turn it into a new task.
    f.database
        .connection()
        .execute(
            "UPDATE conversation SET native_session_id='lost-fixture-session' WHERE id=?1",
            [conversation],
        )
        .unwrap();
    assert!(
        ExecutionRuntimeService::default()
            .prepare_native_session_resume(&mut f.database, &execution)
            .is_err()
    );
    assert!(
        TeamToolService::default()
            .prepare_binding_credential(&mut f.database, &next, next_epoch, false)
            .is_err()
    );
    f.database
        .connection()
        .execute(
            "UPDATE conversation SET native_session_id=?1 WHERE id=?2",
            params![session, conversation],
        )
        .unwrap();
    assert_eq!(
        resume_bind(f, &next, next_epoch, &session).result.status,
        crate::command::CommandResultStatus::Applied
    );
    let resumed_context = resume_context(f, &next, next_epoch);
    let current = context_section(&resumed_context.rendered_payload, "CURRENT_INPUT");
    assert_eq!(current["message"], body);
    assert_eq!(current["interruptionContext"], why);
    assert_eq!(
        context_section(&resumed_context.rendered_payload, "DELEGATION_CONTEXT")["mode"],
        "resume"
    );
    assert!(!resumed_context.runtime_payload.contains(plan.task));
    let resumed_delivery = resume_accept_input(
        f,
        &next,
        next_epoch,
        &resumed_context,
        "fixture-accepted-resume",
    );
    assert_ne!(resumed_delivery, delivery);
    assert_eq!(
        crate::delegation_runtime_defaults::for_run(f.database.connection(), &next).unwrap(),
        crate::delegation_runtime_defaults::for_run(f.database.connection(), &run).unwrap()
    );
    assert_eq!(stored_binding, binding_after.0);
    assert_eq!(generation, binding_after.1);
    let persisted = ContextService
        .materialize(
            &mut f.database,
            &ManagedBlobStore::new(&f.directory),
            &MaterializeContextRequest {
                agent_run_id: &next,
                execution_epoch: next_epoch,
                charter_delivery_mode: CharterDeliveryMode::FirstPayload,
                max_payload_bytes: DEFAULT_MAX_CONTEXT_PAYLOAD_BYTES,
            },
        )
        .unwrap();
    let ContextMaterialization::Ready(persisted) = persisted else {
        panic!("frozen resume context");
    };
    assert_eq!(persisted.runtime_payload, resumed_context.runtime_payload);
    let first_payload: String = f
        .database
        .connection()
        .query_row(
            "SELECT runtime_request_digest FROM runtime_input_delivery WHERE id=?1",
            [&delivery],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!first_payload.is_empty());
    assert_ne!(old_payload, resumed_context.runtime_payload);
    // Same visible canceled state after a later attempt is new observation.
    model::cancel(
        &mut f.database,
        &Invocation {
            binding,
            source,
            tool_call: "stop-resume-second-attempt",
            input: &stop,
        },
    )
    .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &next, next_epoch)
        .unwrap();
    let polling: StatusParams = serde_json::from_value(json!({"task_ids":[task]})).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    let snapshot = runtime
        .block_on(crate::delegation_model_query::wait_for_status(
            f.database.execution_wake.delegation.clone(),
            &polling,
            || {
                std::future::ready(crate::delegation_model_query::status_snapshot(
                    &f.database,
                    &Invocation {
                        binding,
                        source,
                        tool_call: "resume-status",
                        input: &polling,
                    },
                ))
            },
            std::future::pending(),
        ))
        .unwrap();
    let race = Invocation {
        tool_call: "resume-race",
        ..invocation
    };
    let plan_ready = crate::delegation_resume::prepare(&f.database, &race).unwrap();
    let observed = plan_ready.observe_directory();
    f.database
        .connection()
        .execute(
            "UPDATE conversation SET version=version+1 WHERE id=?1",
            [conversation],
        )
        .unwrap();
    let refused = model::resume(&mut f.database, &race, &plan_ready, &observed).unwrap();
    assert_eq!(refused.result.code, "delegation.resume_state_changed");
    assert_eq!(
        crate::delegation::control(f.database.connection(), task)
            .unwrap()
            .current_attempt_no,
        2
    );
    let third = model_resume(f, binding, source, "resume-third-attempt", &input).unwrap();
    assert_eq!(third.result.code, "delegation.resumed");
    model::cancel(
        &mut f.database,
        &Invocation {
            binding,
            source,
            tool_call: "stop-third-attempt",
            input: &stop,
        },
    )
    .unwrap();
    let waiting: StatusParams = serde_json::from_value(
        json!({"task_ids":[task],"wait_ms":0,"after_cursor":snapshot["cursor"]}),
    )
    .unwrap();
    let observed = runtime.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            crate::delegation_model_query::wait_for_status(
                f.database.execution_wake.delegation.clone(),
                &waiting,
                || {
                    std::future::ready(crate::delegation_model_query::status_snapshot(
                        &f.database,
                        &Invocation {
                            binding,
                            source,
                            tool_call: "new-attempt-status",
                            input: &waiting,
                        },
                    ))
                },
                std::future::pending(),
            ),
        )
        .await
        .unwrap()
        .unwrap()
    });
    assert_eq!(observed["tasks"][0]["status"], "canceled");
    assert_ne!(observed["cursor"], snapshot["cursor"]);
    let third_run: String = f
        .database
        .connection()
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1 AND attempt_no=3",
            [task],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        crate::delegation_resume::expected(f.database.connection(), &third_run)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        model_resume(
            f,
            binding,
            source,
            "resume-unaccepted-current-attempt",
            &input
        )
        .unwrap()
        .result
        .code,
        "delegation.input_not_reconciled"
    );
    // Old exact command result is retained after further attempts and global revoke.
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=0 WHERE singleton=1",
            [],
        )
        .unwrap();
    assert_eq!(
        model_resume(f, binding, source, invocation.tool_call, &input)
            .unwrap()
            .result,
        resumed.result
    );
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=1 WHERE singleton=1",
            [],
        )
        .unwrap();
}
