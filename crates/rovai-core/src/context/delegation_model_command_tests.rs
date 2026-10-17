// Original Context owner: real Core receipts/factory/auth/settlement and SQLite.
// Binding transport and terminal/cleanup signals are explicit fixtures, no model.
fn model_start(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    request: &crate::delegation::FrozenRequest<'_>,
    plan: &crate::delegation_factory::ChildRunPlan<'_>,
    call: &str,
) -> anyhow::Result<crate::command::CommandExecution> {
    use crate::delegation_model_command as model;
    let input = crate::delegation_contract::StartParams {
        agent_type: request.runtime.clone(),
        task: plan.task.to_string().try_into().unwrap(),
        working_dir: request.working_dir.cloned(),
    };
    let invocation = model::Invocation {
        binding,
        source,
        tool_call: call,
        input: &input,
    };
    if let Some(replay) = model::replay_start(&f.database, &invocation)? {
        return Ok(replay);
    }
    let admission = crate::delegation_admission::authenticate_new_child(
        f.database.connection(),
        binding,
        source,
        call,
    )?;
    let observation = admission.observe_workspace(request.working_dir)?;
    let workspace = serde_json::to_value(observation.candidate())?;
    let request = crate::delegation::FrozenRequest {
        workspace: &workspace,
        ..*request
    };
    let plan = crate::delegation_factory::ChildRunPlan {
        workspace: observation.candidate(),
        policy: admission.child_policy(),
        ..*plan
    };
    model::start(
        &mut f.database,
        &invocation,
        model::PreparedStart {
            admission: &admission,
            observation: &observation,
            request: &request,
            plan: &plan,
        },
    )
}

fn assert_delegation_model_commands(
    f: &mut Fixture,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    request: &crate::delegation::FrozenRequest<'_>,
    plan: &crate::delegation_factory::ChildRunPlan<'_>,
) {
    use crate::{
        delegation_contract::{CancelParams, StartParams},
        delegation_model_command as model,
    };
    let (source_id, source_run, source_epoch) = context_child_in(
        f,
        &source.agent_run_id,
        source.execution_epoch,
        "isolated command source",
        "model-command-source",
        &serde_json::to_value(plan.workspace).unwrap(),
    );
    let source_binding = TeamToolService::default()
        .prepare_binding_credential(&mut f.database, &source_run, source_epoch, false)
        .unwrap();
    let source_owner = crate::team_tool::AuthenticatedTeamToolRun {
        agent_run_id: source_run,
        execution_epoch: source_epoch,
        ..source.clone()
    };
    let source = &source_owner;
    let binding = &source_binding;
    let lease_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let lease_registry = crate::builtin_tool_runtime::BuiltinToolLeaseRegistry::default();
    let lease_config = crate::builtin_tool_runtime::BuiltinToolProcessConfig::create(
        &std::env::current_exe().unwrap(),
        &crate::builtin_tool_transport::LocalIpcEndpoint::UnixSocket {
            path: f
                .directory
                .join("receipt-only-fixture.sock")
                .to_string_lossy()
                .into_owned(),
        },
        &f.directory,
    )
    .unwrap();
    let lease_auth = lease_runtime
        .block_on(lease_registry.bind(
            &lease_config,
            &source.agent_run_id,
            source.execution_epoch,
            binding,
            &f.directory,
        ))
        .unwrap();
    let receipt_proof = lease_runtime
        .block_on(lease_registry.authenticate_receipt(&lease_auth))
        .unwrap();
    let parent = crate::delegation_admission::authenticate_new_child(
        f.database.connection(),
        binding,
        source,
        "report-owner",
    )
    .unwrap()
    .conversation_id()
    .to_owned();
    let input = StartParams {
        agent_type: request.runtime.clone(),
        task: plan.task.to_string().try_into().unwrap(),
        working_dir: request.working_dir.cloned(),
    };
    let invocation = model::Invocation {
        binding,
        source,
        tool_call: "durable-model-start",
        input: &input,
    };
    assert!(
        model::replay_start(&f.database, &invocation)
            .unwrap()
            .is_none()
    );
    let receipt_input = serde_json::to_value(&input).unwrap();
    let receipt_changes = f.database.connection().total_changes();
    assert!(
        receipt_proof
            .replay(
                &f.database,
                "delegate_to_agent",
                invocation.tool_call,
                &receipt_input
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(f.database.connection().total_changes(), receipt_changes);
    let before = factory_counts(f);
    // A failure while committing the sole receipt must erase the complete child
    // graph and domain event. Retrying the same original request is then valid.
    f.database
        .connection()
        .execute_batch(
            "CREATE TEMP TRIGGER fail_delegation_receipt BEFORE INSERT ON event_log
        WHEN NEW.event_type='command.result' AND NEW.command_type='delegation.start'
        BEGIN SELECT RAISE(ABORT,'receipt fixture fault'); END;",
        )
        .unwrap();
    let error = model_start(f, binding, source, request, plan, invocation.tool_call).unwrap_err();
    assert!(format!("{error:#}").contains("receipt fixture fault"));
    assert_eq!(factory_counts(f), before);
    assert!(
        model::replay_start(&f.database, &invocation)
            .unwrap()
            .is_none()
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER fail_delegation_receipt;")
        .unwrap();
    let first = model_start(f, binding, source, request, plan, invocation.tool_call).unwrap();
    assert_eq!(
        first.result.status,
        crate::command::CommandResultStatus::Accepted
    );
    assert_eq!(first.result.payload["status"], "running");
    assert_eq!(first.result.payload["message"], "等待资源");
    assert!(first.result.payload.get("text").is_none());
    let id = first.result.payload["task_id"].as_str().unwrap().to_owned();
    let run: String = f
        .database
        .connection()
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    let changes = f.database.connection().total_changes();
    let replay = model_start(f, binding, source, request, plan, invocation.tool_call).unwrap();
    assert!(replay.replayed);
    assert_eq!(first.result, replay.result);
    assert_eq!(changes, f.database.connection().total_changes());
    let digest:(Option<String>,Option<i64>,String)=f.database.connection().query_row(
        "SELECT request_digest,request_digest_version,payload_json FROM event_log WHERE command_id=?1",[&first.result.command_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    let digest_text = digest.0.unwrap();
    assert_eq!(digest_text.len(), 64);
    assert!(digest_text.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(digest.1, Some(1));
    assert!(!digest.2.contains(&binding.binding_credential));
    let changed = StartParams {
        task: "同一个请求不能换任务".to_owned().try_into().unwrap(),
        ..input.clone()
    };
    assert!(
        model::replay_start(
            &f.database,
            &model::Invocation {
                input: &changed,
                ..invocation
            }
        )
        .unwrap_err()
        .to_string()
        .contains("idempotency")
    );
    let mut wrong = binding.clone();
    wrong.binding_credential.push_str("-forged");
    assert!(
        model::replay_start(
            &f.database,
            &model::Invocation {
                binding: &wrong,
                ..invocation
            }
        )
        .is_err()
    );
    let mut wrong = binding.clone();
    wrong.binding_compatibility_digest.push_str("-changed");
    assert!(
        model::replay_start(
            &f.database,
            &model::Invocation {
                binding: &wrong,
                ..invocation
            }
        )
        .is_err()
    );
    let mut wrong = binding.clone();
    wrong.native_binding_generation += 1;
    assert!(
        model::replay_start(
            &f.database,
            &model::Invocation {
                binding: &wrong,
                ..invocation
            }
        )
        .unwrap()
        .is_none()
    );
    let mut wrong_source = source.clone();
    wrong_source.agent_id = "agent_2".into();
    assert!(
        model::replay_start(
            &f.database,
            &model::Invocation {
                source: &wrong_source,
                ..invocation
            }
        )
        .is_err()
    );
    let mut wrong_source = source.clone();
    wrong_source.execution_epoch += 1;
    assert!(
        model::replay_start(
            &f.database,
            &model::Invocation {
                source: &wrong_source,
                ..invocation
            }
        )
        .unwrap()
        .is_none()
    );
    // A preparation for another call cannot be borrowed as authority for this one.
    let admission = crate::delegation_admission::authenticate_new_child(
        f.database.connection(),
        binding,
        source,
        "prepared-A",
    )
    .unwrap();
    let observation = admission.observe_workspace(request.working_dir).unwrap();
    let rejected = model::start(
        &mut f.database,
        &model::Invocation {
            tool_call: "prepared-B",
            ..invocation
        },
        model::PreparedStart {
            admission: &admission,
            observation: &observation,
            request,
            plan,
        },
    );
    assert!(
        rejected
            .unwrap_err()
            .to_string()
            .contains("preparation_scope_mismatch")
    );
    // A settings change between planning and commit is a stable refusal, with
    // neither a partial child graph nor a later silent success on retry.
    let admission = crate::delegation_admission::authenticate_new_child(
        f.database.connection(),
        binding,
        source,
        "disabled-between-plan-and-commit",
    )
    .unwrap();
    let observation = admission.observe_workspace(request.working_dir).unwrap();
    let rejection_input = model::Invocation {
        tool_call: "disabled-between-plan-and-commit",
        ..invocation
    };
    let before = factory_counts(f);
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=0 WHERE singleton=1",
            [],
        )
        .unwrap();
    let rejected = model::start(
        &mut f.database,
        &rejection_input,
        model::PreparedStart {
            admission: &admission,
            observation: &observation,
            request,
            plan,
        },
    )
    .unwrap();
    assert_eq!(
        rejected.result.status,
        crate::command::CommandResultStatus::Rejected
    );
    assert_eq!(factory_counts(f), before);
    assert_eq!(
        model::replay_start(&f.database, &invocation)
            .unwrap()
            .unwrap()
            .result,
        first.result
    );
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=1 WHERE singleton=1",
            [],
        )
        .unwrap();
    assert_eq!(
        model::replay_start(&f.database, &rejection_input)
            .unwrap()
            .unwrap()
            .result,
        rejected.result
    );
    let before = factory_counts(f);
    let bad_plan = crate::delegation_factory::ChildRunPlan {
        task: "完整正文与旧Blob不一致",
        ..*plan
    };
    let rejected = model_start(f, binding, source, request, &bad_plan, "bad-task-blob").unwrap();
    assert_eq!(
        rejected.result.status,
        crate::command::CommandResultStatus::Rejected
    );
    assert_eq!(factory_counts(f), before);
    // Real Core success stores the exact final message. Later conversation text
    // is never substituted, and the start receipt remains its original snapshot.
    let epoch = claim_context_child(f, &run);
    let full_output = "中文结果".repeat(25000);
    let terminal = lifecycle_command(
        f,
        ActorRef::System {
            component_id: "runtime-adapter:fixture".into(),
        },
        crate::runtime::SucceedAgentRunCommand {
            agent_run_id: run.clone(),
            expected_version: lifecycle_version(f, &run),
            execution_epoch: epoch,
            native_turn_id: format!("native-{run}"),
            final_output: full_output.clone(),
            missing_send_recovery_candidate: None,
            ending_git_observation: None,
        },
    );
    assert_eq!(
        ExecutionRuntimeService::default()
            .succeed_agent_run(&mut f.database, &terminal)
            .unwrap()
            .result
            .code,
        "agent_run.succeeded"
    );
    let completed = model::task_report(f.database.connection(), &id, &parent).unwrap();
    assert_eq!(completed["status"], "completed");
    let text = completed["text"].as_str().unwrap();
    assert!(text.len() <= 256 * 1024 && text.ends_with('…'));
    assert!(full_output.starts_with(text.strip_suffix('…').unwrap()));
    assert_eq!(completed["result_ref"]["truncated"], true);
    let final_body: String = f
        .database
        .connection()
        .query_row(
            "SELECT body FROM conversation_message WHERE id=?1",
            [completed["result_ref"]["message_id"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(final_body, full_output);
    assert_delegation_model_queries(f, binding, source, request, plan, &id, &run, &full_output);
    assert_delegation_resume(f, binding, source, request, plan, &id, &receipt_proof);
    assert_production_start(f, binding, source);
    assert_delegation_dispatch(f, binding, source, &id);
    assert_eq!(
        model::replay_start(&f.database, &invocation)
            .unwrap()
            .unwrap()
            .result,
        first.result
    );
    let stop = CancelParams {
        task_id: id.clone().try_into().unwrap(),
    };
    let cancel = model::Invocation {
        binding,
        source,
        tool_call: "terminal-model-cancel",
        input: &stop,
    };
    let terminal = model::cancel(&mut f.database, &cancel).unwrap();
    assert_eq!(terminal.result.code, "delegation.already_terminal");
    assert_eq!(terminal.result.payload, completed);
    for foreign in ["missing-child", source_id.as_str()] {
        let stop = CancelParams {
            task_id: foreign.to_owned().try_into().unwrap(),
        };
        let result = model::cancel(
            &mut f.database,
            &model::Invocation {
                binding,
                source,
                tool_call: foreign,
                input: &stop,
            },
        )
        .unwrap();
        assert_eq!(
            result.result.payload,
            json!({"task_id":foreign,"status":"unknown"})
        );
    }
    // The model command uses the existing mandatory permission request. Replay
    // cannot re-observe a directory or create a second approval after commit.
    let external_path = f.directory.join("model-external-directory");
    std::fs::create_dir_all(&external_path).unwrap();
    let external_dir = crate::delegation_contract::WorkingDirectory::try_from(
        external_path.to_str().unwrap().to_owned(),
    )
    .unwrap();
    let external_request = crate::delegation::FrozenRequest {
        working_dir: Some(&external_dir),
        ..*request
    };
    let external = model_start(
        f,
        binding,
        source,
        &external_request,
        plan,
        "external-model-start",
    )
    .unwrap();
    assert_eq!(external.result.payload["blocked_on"]["kind"], "permission");
    assert_eq!(external.result.payload["message"], "等待用户授权");
    let external_id = external.result.payload["task_id"].as_str().unwrap();
    let request_id = external.result.payload["blocked_on"]["request_id"]
        .as_str()
        .unwrap();
    let scope:(String,i64,String)=f.database.connection().query_row("SELECT r.conversation_id,r.execution_epoch,p.status FROM approval p JOIN action_execution x ON x.id=p.action_id JOIN agent_run r ON r.id=x.agent_run_id WHERE p.id=?1",[request_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(
        scope,
        (
            external.result.payload["child_conversation_id"]
                .as_str()
                .unwrap()
                .to_owned(),
            0,
            "pending".into()
        )
    );
    std::fs::remove_dir(&external_path).unwrap();
    assert_eq!(
        model_start(
            f,
            binding,
            source,
            &external_request,
            plan,
            "external-model-start"
        )
        .unwrap()
        .result,
        external.result
    );
    assert!(
        model_start(
            f,
            binding,
            source,
            &external_request,
            plan,
            "external-new-request"
        )
        .is_err()
    );
    let external_stop = CancelParams {
        task_id: external_id.to_owned().try_into().unwrap(),
    };
    let external_cancel = model::cancel(
        &mut f.database,
        &model::Invocation {
            binding,
            source,
            tool_call: "external-model-stop",
            input: &external_stop,
        },
    )
    .unwrap();
    assert_eq!(external_cancel.result.payload["status"], "canceled");
    assert!(
        external_cancel
            .result
            .payload
            .get("cleanup_pending")
            .is_none()
    );
    let permission_status: String = f
        .database
        .connection()
        .query_row(
            "SELECT status FROM approval WHERE id=?1",
            [request_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(permission_status, "cancelled");
    // Model cancellation follows the original causal tree, while a separately
    // taken-over child and its later User input belong to the User.
    let active = model_start(f, binding, source, request, plan, "active-model-start").unwrap();
    let active_id = active.result.payload["task_id"].as_str().unwrap();
    let active_run: String = f
        .database
        .connection()
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [active_id],
            |r| r.get(0),
        )
        .unwrap();
    let active_epoch = claim_context_child(f, &active_run);
    let (_, leaf, leaf_epoch) = context_child_of(
        f,
        &active_run,
        active_epoch,
        "stop original causal leaf",
        "model-cancel-leaf",
    );
    let (independent, independent_run, independent_epoch) = context_child_of(
        f,
        &active_run,
        active_epoch,
        "User owns this branch",
        "model-cancel-independent",
    );
    crate::application::delegation_control::handle(
        &mut f.database,
        "delegations.takeover",
        control_request(&independent, "expectedControlVersion", 1),
    )
    .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &independent_run, independent_epoch)
        .unwrap();
    let conversation: String = f
        .database
        .connection()
        .query_row(
            "SELECT child_conversation_id FROM delegation WHERE id=?1",
            [&independent],
            |r| r.get(0),
        )
        .unwrap();
    let user = child_end_send(f, &conversation, "User后续任务保持独立");
    let user_run = user["agentRunId"].as_str().unwrap();
    let user_before = lifecycle_status(f, user_run);
    let stop = CancelParams {
        task_id: active_id.to_owned().try_into().unwrap(),
    };
    let cancel = model::Invocation {
        binding,
        source,
        tool_call: "active-model-cancel",
        input: &stop,
    };
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=0,depth_limit=1 WHERE singleton=1",
            [],
        )
        .unwrap();
    let stopped = model::cancel(&mut f.database, &cancel).unwrap();
    assert_eq!(stopped.result.payload["status"], "canceled");
    assert_eq!(stopped.result.payload["cleanup_pending"], true);
    assert_eq!(lifecycle_status(f, &active_run), "cancelled");
    assert_eq!(lifecycle_status(f, &leaf), "cancelled");
    assert_eq!(lifecycle_status(f, user_run), user_before);
    let changes = f.database.connection().total_changes();
    let replay = model::cancel(&mut f.database, &cancel).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.result, stopped.result);
    assert_eq!(changes, f.database.connection().total_changes());
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &active_run, active_epoch)
        .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &leaf, leaf_epoch)
        .unwrap();
    assert!(
        model::task_report(f.database.connection(), active_id, &parent)
            .unwrap()
            .get("cleanup_pending")
            .is_none()
    );
    assert_eq!(
        model::cancel(&mut f.database, &cancel).unwrap().result,
        stopped.result
    );
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=1,depth_limit=8 WHERE singleton=1",
            [],
        )
        .unwrap();
    // Recovery uses the persisted scope and the original credential digest even
    // after the current Binding no longer admits new work.
    let tx = f.database.connection_mut().transaction().unwrap();
    tx.execute(
        "UPDATE conversation SET native_binding_generation=native_binding_generation+1 WHERE id=?1",
        [&parent],
    )
    .unwrap();
    assert!(
        crate::delegation_admission::authenticate_new_child(&tx, binding, source, "new-work")
            .is_err()
    );
    tx.commit().unwrap();
    let path: String = f
        .database
        .connection()
        .query_row(
            "SELECT file FROM pragma_database_list WHERE name='main'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let reopened = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        model::task_report(&reopened, &id, &parent).unwrap(),
        completed
    );
    drop(reopened);
    let changes = f.database.connection().total_changes();
    assert_eq!(
        model::replay_start(&f.database, &invocation)
            .unwrap()
            .unwrap()
            .result,
        first.result
    );
    assert_eq!(
        model::cancel(&mut f.database, &cancel).unwrap().result,
        stopped.result
    );
    assert_eq!(changes, f.database.connection().total_changes());
    crate::application::delegation_control::handle(
        &mut f.database,
        "delegations.takeover",
        control_request(&source_id, "expectedControlVersion", 1),
    )
    .unwrap();
    assert_eq!(lifecycle_status(f, &source.agent_run_id), "cancelled");
    assert_eq!(
        model::replay_start(&f.database, &invocation)
            .unwrap()
            .unwrap()
            .result,
        first.result
    );
    assert_eq!(
        model::cancel(&mut f.database, &cancel).unwrap().result,
        stopped.result
    );
    assert!(model_start(f, binding, source, request, plan, "stopped-source-new-work").is_err());
    lease_runtime.block_on(lease_registry.unbind(
        lease_config.process_id(),
        &source.agent_run_id,
        source.execution_epoch,
    ));
    assert!(
        lease_runtime
            .block_on(lease_registry.authenticate(&lease_auth))
            .is_err()
    );
    let retired = lease_runtime
        .block_on(lease_registry.authenticate_receipt(&lease_auth))
        .unwrap();
    let before = factory_counts(f);
    let changes = f.database.connection().total_changes();
    assert_eq!(
        retired
            .replay(
                &f.database,
                "delegate_to_agent",
                invocation.tool_call,
                &receipt_input
            )
            .unwrap()
            .unwrap()
            .result,
        first.result
    );
    assert_eq!(
        retired
            .replay(
                &f.database,
                "cancel_delegation",
                cancel.tool_call,
                &serde_json::to_value(cancel.input).unwrap()
            )
            .unwrap()
            .unwrap()
            .result,
        stopped.result
    );
    assert!(
        retired
            .replay(
                &f.database,
                "delegation.start",
                "unrecorded-new-call",
                &receipt_input
            )
            .unwrap()
            .is_none()
    );
    for operation in [
        "delegation.status",
        "delegation.read",
        "delegation.history",
        "thread.message.send",
        "delegate_to_agent_extra",
    ] {
        assert!(
            retired
                .replay(&f.database, operation, invocation.tool_call, &receipt_input)
                .is_err()
        );
    }
    let mut changed_input = receipt_input.clone();
    changed_input["task"] = json!("不能以旧租约替换原任务");
    assert!(
        retired
            .replay(
                &f.database,
                "delegation.start",
                invocation.tool_call,
                &changed_input
            )
            .is_err()
    );
    assert_eq!(factory_counts(f), before);
    assert_eq!(f.database.connection().total_changes(), changes);
    lease_runtime.block_on(lease_registry.unregister(lease_config.process_id()));
}
