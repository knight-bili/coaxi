// Existing Context/SQLite owner; real Core factory, claim, binding and delivery.
// Executable identity, Native Session and ACK are fixtures; no Runtime process/model.
fn assert_native_interaction_migration(f: &mut Fixture) {
    use crate::db::{
        DatabaseContractClassification as Class, classify_database_contract as classify,
    };
    assert!(
        matches!(classify(f.database.connection()).unwrap(),Class::SupportedMigrationSource(m) if m.projection_schema_version==160)
    );
    f.database.connection().execute_batch("CREATE TEMP TRIGGER reject_interaction_migration BEFORE INSERT ON schema_migration WHEN NEW.version=211 BEGIN SELECT RAISE(ABORT,'interaction migration receipt fault'); END;").unwrap();
    assert!(
        format!(
            "{:#}",
            crate::db::runtime_interactions::migrate(&mut f.database).unwrap_err()
        )
        .contains("interaction migration receipt fault")
    );
    assert!(crate::db::runtime_interactions::absent(f.database.connection()).unwrap());
    assert!(
        matches!(classify(f.database.connection()).unwrap(),Class::SupportedMigrationSource(m) if m.projection_schema_version==160)
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER reject_interaction_migration;")
        .unwrap();
    crate::db::runtime_interactions::migrate(&mut f.database).unwrap();
    assert!(
        matches!(classify(f.database.connection()).unwrap(),Class::Current(m) if m.projection_schema_version==161)
    );
    let changes = f.database.connection().total_changes();
    crate::db::runtime_interactions::migrate(&mut f.database).unwrap();
    crate::db::delegation::migrate_connection(f.database.connection()).unwrap();
    assert_eq!(f.database.connection().total_changes(), changes);
    for sql in [
        "DROP TRIGGER runtime_user_interaction_immutable",
        "CREATE INDEX runtime_user_interaction_unrecognized ON runtime_user_interaction(kind)",
    ] {
        let tx = f.database.connection_mut().transaction().unwrap();
        tx.execute_batch(sql).unwrap();
        assert!(matches!(classify(&tx).unwrap(), Class::Unknown(_)));
        tx.rollback().unwrap();
    }
}

fn assert_native_interaction_storage(f: &mut Fixture) {
    use crate::{
        agent_profile::{
            AdapterKind, AdapterPermissionConfig, CreateAdapterInstallationCommand,
            InstallationSource, MemberRuntimeConfiguration, ModelSelection,
        },
        delegation_contract::RuntimeTarget,
        delegation_factory::{self as factory, ChildRunPlan},
        delegation_policy::{FrozenPolicy, Role},
        runtime_interaction::{
            Answer, PlanDecision, QuestionAnswer,
            storage::{self, NativeFrame, ResolveInteractionCommand},
        },
    };
    let adapter = AdapterKind::GrokBuild;
    let path = f.directory.join("native-grok-fixture.exe");
    std::fs::write(&path, b"fixture identity; never execute").unwrap();
    let mut command = lifecycle_command(
        f,
        queue_actor(),
        CreateAdapterInstallationCommand {
            adapter_kind: adapter,
            executable_path: path.to_string_lossy().into_owned(),
            command_name: "grok".into(),
            source: InstallationSource::Custom,
            auth_scope: "interaction-fixture".into(),
        },
    );
    command.camp_id = None;
    let installed = AgentProfileService::default()
        .create_installation(&mut f.database, &command)
        .unwrap();
    let installation = installed.result.payload["installationId"].as_str().unwrap();
    let configuration = MemberRuntimeConfiguration {
        adapter_kind: adapter,
        model: ModelSelection::RuntimeDefault,
        permissions: AdapterPermissionConfig {
            adapter_kind: adapter,
            schema_version: 1,
            values: crate::agent_runtime_adapter::AgentRuntimeAdapterRegistry::default()
                .member_permission_defaults(adapter),
        },
    };
    let workspace = crate::runtime::AgentRunWorkspace {
        execution_root: f.directory.to_string_lossy().into_owned(),
        access: "read_only".into(),
        isolation: "shared".into(),
    };
    let target = RuntimeTarget::try_from("grok".to_string()).unwrap();
    let task = "验证原生提问和计划答复的持久归属。";
    let blob = ManagedBlobStore::new(&f.directory)
        .put_bytes(&mut f.database, task.as_bytes(), "text/plain", "sensitive")
        .unwrap();
    let (binding,generation):(String,i64)=f.database.connection().query_row("SELECT c.native_binding_id,c.native_binding_generation FROM agent_run r JOIN conversation c ON c.id=r.conversation_id WHERE r.id=?1",[&f.run_id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    let tx = f.database.connection_mut().transaction().unwrap();
    let created = factory::register_initial(
        &tx,
        &crate::delegation::ParentInvocation {
            run_id: &f.run_id,
            epoch: f.execution_epoch,
            binding_id: &binding,
            binding_generation: generation,
            tool_call_id: "interaction-factory",
        },
        &crate::delegation::FrozenRequest {
            task_blob_id: &blob.id,
            runtime: &target,
            working_dir: None,
            workspace: &serde_json::to_value(&workspace).unwrap(),
            runtime_preset: &json!({}),
        },
        &ChildRunPlan {
            task,
            target: &target,
            installation_id: installation,
            configuration: &configuration,
            workspace: &workspace,
            policy: FrozenPolicy::new(true, 8, Role::Child).unwrap(),
        },
        "2026-10-18T00:00:00Z",
    )
    .unwrap();
    let run: String = tx
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [&created.identity.id],
            |r| r.get(0),
        )
        .unwrap();
    tx.commit().unwrap();
    let epoch = claim_context_child(f, &run);
    let session = "native-interaction-fixture-session";
    assert_eq!(
        resume_bind(f, &run, epoch, session).result.status,
        crate::command::CommandResultStatus::Applied
    );
    let context = resume_context(f, &run, epoch);
    let delivery = ContextService
        .prepare_input_delivery_for_context(&mut f.database, &run, epoch, &context)
        .unwrap();
    let (binding, generation): (String, i64) = f
        .database
        .connection()
        .query_row(
            "SELECT native_binding_id,native_binding_generation FROM conversation WHERE id=?1",
            [&created.identity.child_conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let mut frame = NativeFrame {
        run: run.clone(),
        epoch,
        input_delivery: delivery.id.clone(),
        core_generation: "interaction-core-one".into(),
        host_instance: "interaction-host-one".into(),
        sequence: 1,
        rpc_id: json!(" native-rpc "),
        method: "_x.ai/ask_user_question".into(),
        session: session.into(),
        binding,
        binding_generation: generation,
    };
    let params = json!({"toolCallId":"private-native-call","questions":[{"question":"先完成哪个模块？","options":[{"label":"接口"},{"label":"界面"}]}]});
    // Prepared is eligible only after the real Core dispatch marker, even before ACK.
    assert!(storage::capture(&mut f.database, &frame, &params).is_err());
    assert!(
        ContextService
            .begin_runtime_input_dispatch(&mut f.database, &delivery.id, &run, epoch)
            .unwrap()
    );
    let id = storage::capture(&mut f.database, &frame, &params).unwrap();
    let changes = f.database.connection().total_changes();
    let report = crate::delegation_model_command::task_report(
        f.database.connection(),
        &created.identity.id,
        &created.identity.parent_conversation_id,
    )
    .unwrap();
    assert_eq!(report["status"], "running");
    assert_eq!(report["message"], "等待用户回答");
    assert_eq!(
        report["blocked_on"],
        json!({"kind":"question","request_id":id})
    );
    let detail = crate::application::delegation_queries::read(
        &f.database,
        "delegations.get",
        json!({"delegationId":created.identity.id}),
    )
    .unwrap();
    assert_eq!(detail["blockedOn"], report["blocked_on"]);
    for private in [
        "先完成哪个模块",
        "private-native-call",
        "native-interaction-fixture-session",
        "native-rpc",
    ] {
        assert!(!serde_json::to_string(&report).unwrap().contains(private));
        assert!(!serde_json::to_string(&detail).unwrap().contains(private));
    }
    assert_eq!(
        crate::delegation_model_command::task_report(
            f.database.connection(),
            &created.identity.id,
            "another-parent"
        )
        .unwrap(),
        json!({"task_id":created.identity.id,"status":"unknown"})
    );
    assert!(
        storage::blocked_summary(f.database.connection(), "another-run", epoch)
            .unwrap()
            .is_none()
    );
    assert!(
        storage::blocked_summary(f.database.connection(), &run, epoch + 1)
            .unwrap()
            .is_none()
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    assert_eq!(
        storage::capture(&mut f.database, &frame, &params).unwrap(),
        id
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    let mut altered = params.clone();
    altered["questions"][0]["question"] = json!("同帧不能改题");
    assert!(storage::capture(&mut f.database, &frame, &altered).is_err());
    let mut wrong = frame.clone();
    wrong.sequence += 1;
    assert!(storage::capture(&mut f.database, &wrong, &params).is_err()); // same pending RPC
    wrong.rpc_id = json!(2);
    wrong.epoch += 1;
    assert!(storage::capture(&mut f.database, &wrong, &params).is_err());
    for field in ["binding", "session", "input"] {
        wrong = frame.clone();
        wrong.sequence += 1;
        wrong.rpc_id = json!(2);
        match field {
            "binding" => wrong.binding.push_str("-foreign"),
            "session" => wrong.session.push_str("-foreign"),
            _ => wrong.input_delivery.push_str("-foreign"),
        };
        assert!(
            storage::capture(&mut f.database, &wrong, &params).is_err(),
            "{field}"
        );
    }
    wrong = frame.clone();
    wrong.method = "cursor/ask_question".into();
    wrong.sequence += 1;
    wrong.rpc_id = json!(2);
    assert!(storage::capture(&mut f.database, &wrong, &params).is_err());
    let actor = ActorRef::User {
        user_id: crate::current_user::CURRENT_USER_ID.into(),
    };
    let system = ActorRef::System {
        component_id: "interaction-fixture-system".into(),
    };
    assert!(storage::read(&f.database, &system, &f.camp_id, &id).is_err());
    assert!(
        storage::read(&f.database, &actor, "another-camp", &id)
            .unwrap()
            .is_none()
    );
    let view = storage::read(&f.database, &actor, &f.camp_id, &id)
        .unwrap()
        .unwrap();
    assert_eq!(view.version, 1);
    let reopened = rusqlite::Connection::open(f.database.path()).unwrap();
    let original: String = reopened
        .query_row(
            "SELECT request_json FROM runtime_user_interaction WHERE id=?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<crate::runtime_interaction::Request>(&original)
            .unwrap()
            .kind(),
        "question"
    );
    drop(reopened);
    assert!(f.database.connection().execute("UPDATE runtime_user_interaction SET native_request_id_json='999',version=version+1 WHERE id=?1",[&id]).is_err());
    let serialized = serde_json::to_string(&view).unwrap();
    for private in [
        session,
        "private-native-call",
        "native-rpc",
        "interaction-core-one",
        "interaction-host-one",
        "nativeId",
    ] {
        assert!(!serialized.contains(private), "{private}");
    }
    let command = |actor, version, answer| CommandEnvelope {
        command_id: Uuid::new_v4().to_string(),
        actor,
        camp_id: Some(f.camp_id.clone()),
        expected_versions: vec![],
        execution_epoch: None,
        payload: ResolveInteractionCommand {
            interaction_id: id.clone(),
            expected_version: version,
            answer,
        },
    };
    let answer = Answer::Question {
        answers: vec![QuestionAnswer {
            question_id: "q0".into(),
            labels: vec!["接口".into()],
        }],
        declined: false,
    };
    let denied = command(system, 1, answer.clone());
    let stale = command(actor.clone(), 2, answer.clone());
    let user_request = json!({"commandId":Uuid::new_v4().to_string(),"command":{
        "interactionId":id,"expectedVersion":1,"answer":answer}});
    let accepted =
        crate::application::runtime_interactions::envelope(user_request.clone()).unwrap();
    assert!(accepted.camp_id.is_none());
    assert_eq!(accepted.actor, actor);
    for field in ["actor", "agentRunId", "request", "nativeSessionId"] {
        let mut changed = user_request.clone();
        changed[field] = json!("cannot choose authority");
        assert!(
            crate::application::runtime_interactions::envelope(changed).is_err(),
            "{field}"
        );
        let mut changed = user_request.clone();
        changed["command"][field] = json!("cannot rebind");
        assert!(
            crate::application::runtime_interactions::envelope(changed).is_err(),
            "{field}"
        );
    }
    let changes = f.database.connection().total_changes();
    assert_eq!(
        crate::application::runtime_interactions::read(
            &f.database,
            "runtime.interactions.get",
            json!({"interactionId":id})
        )
        .unwrap(),
        serde_json::to_value(&view).unwrap()
    );
    assert!(
        crate::application::runtime_interactions::read(
            &f.database,
            "runtime.interactions.get",
            json!({"interactionId":id,"request":params})
        )
        .is_err()
    );
    assert_eq!(
        crate::application::runtime_interactions::reconcile(&f.database, user_request.clone())
            .unwrap(),
        json!({"state":"unknown"})
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    assert_eq!(
        storage::resolve(&mut f.database, &denied, &frame.core_generation)
            .unwrap()
            .result
            .code,
        "interaction.user_required"
    );
    assert_eq!(
        storage::resolve(&mut f.database, &stale, &frame.core_generation)
            .unwrap()
            .result
            .code,
        "interaction.stale"
    );
    f.database.connection().execute_batch("CREATE TEMP TRIGGER reject_interaction_decision BEFORE INSERT ON event_log WHEN NEW.event_type='command.result' AND NEW.command_type='runtime_request.interaction.resolve' BEGIN SELECT RAISE(ABORT,'interaction decision receipt fault'); END;").unwrap();
    assert!(
        format!(
            "{:#}",
            storage::resolve(&mut f.database, &accepted, &frame.core_generation).unwrap_err()
        )
        .contains("interaction decision receipt fault")
    );
    assert_eq!(
        storage::read(&f.database, &actor, &f.camp_id, &id)
            .unwrap()
            .unwrap()
            .status,
        "pending"
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER reject_interaction_decision;")
        .unwrap();
    let result = crate::application::runtime_interactions::resolve(
        &mut f.database,
        user_request.clone(),
        &frame.core_generation,
    )
    .unwrap();
    assert_eq!(result.result.code, "interaction.resolved");
    assert!(
        storage::blocked_summary(f.database.connection(), &run, epoch)
            .unwrap()
            .is_none()
    );
    let changes = f.database.connection().total_changes();
    assert_eq!(
        storage::resolve(&mut f.database, &accepted, "new-process-generation")
            .unwrap()
            .result,
        result.result
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    let recovered =
        crate::application::runtime_interactions::reconcile(&f.database, user_request.clone())
            .unwrap();
    assert_eq!(recovered["state"], "recorded");
    assert_eq!(
        recovered["result"],
        serde_json::to_value(&result.result).unwrap()
    );
    // Original global User identity and full payload can recover a receipt even
    // if its target row is gone. This savepoint is an explicit storage fixture.
    f.database
        .connection()
        .execute_batch("SAVEPOINT interaction_removed_target")
        .unwrap();
    f.database
        .connection()
        .execute("DELETE FROM runtime_user_interaction WHERE id=?1", [&id])
        .unwrap();
    assert_eq!(
        crate::application::runtime_interactions::reconcile(&f.database, user_request.clone())
            .unwrap(),
        recovered
    );
    assert_eq!(
        crate::application::runtime_interactions::resolve(
            &mut f.database,
            user_request.clone(),
            "another-process"
        )
        .unwrap()
        .result,
        result.result
    );
    f.database
        .connection()
        .execute_batch("ROLLBACK TO interaction_removed_target; RELEASE interaction_removed_target")
        .unwrap();
    let mut conflict = accepted.clone();
    conflict.payload.answer = Answer::Question {
        answers: vec![],
        declined: true,
    };
    assert!(storage::resolve(&mut f.database, &conflict, &frame.core_generation).is_err());
    assert!(
        storage::begin_dispatch(
            &mut f.database,
            &id,
            2,
            "wrong-generation",
            &frame.host_instance
        )
        .is_err()
    );
    let reply = storage::begin_dispatch(
        &mut f.database,
        &id,
        2,
        &frame.core_generation,
        &frame.host_instance,
    )
    .unwrap();
    assert_eq!(reply.rpc_id, frame.rpc_id);
    assert_eq!(
        reply.response,
        json!({"outcome":"accepted","answers":{"先完成哪个模块？":"接口"},"partial_answers":{}})
    );
    assert!(
        storage::begin_dispatch(
            &mut f.database,
            &id,
            3,
            &frame.core_generation,
            &frame.host_instance
        )
        .is_err()
    );
    assert!(!storage::finish_dispatch(&mut f.database, &id, "wrong-token", true).unwrap());
    assert!(storage::finish_dispatch(&mut f.database, &id, &reply.token, false).unwrap());
    assert!(!storage::finish_dispatch(&mut f.database, &id, &reply.token, true).unwrap());
    assert_eq!(
        storage::read(&f.database, &actor, &f.camp_id, &id)
            .unwrap()
            .unwrap()
            .status,
        "delivery_unknown"
    );
    // ACK allows the next real frame; terminal RPC ids may be reused, frame identity may not.
    ContextService
        .acknowledge_active_input_delivery(
            &mut f.database,
            &delivery.id,
            "interaction-input-fixture",
        )
        .unwrap();
    frame.sequence += 1;
    let second = storage::capture(&mut f.database, &frame, &params).unwrap();
    assert_ne!(second, id);
    let list_params = json!({"conversationId":created.identity.child_conversation_id,"limit":1});
    let changes = f.database.connection().total_changes();
    let page = crate::application::runtime_interactions::read(
        &f.database,
        "runtime.interactions.list",
        list_params.clone(),
    )
    .unwrap();
    assert_eq!(page["items"][0]["id"], second);
    let cursor = page["nextCursor"].as_str().unwrap().to_owned();
    // The id exists, but belongs to the child, never this parent Conversation.
    assert!(
        crate::application::runtime_interactions::read(
            &f.database,
            "runtime.interactions.list",
            json!({"conversationId":created.identity.parent_conversation_id,"cursor":id})
        )
        .is_err()
    );
    assert!(
        !serde_json::to_string(&page)
            .unwrap()
            .contains("先完成哪个模块")
    );
    for value in [
        json!({"conversationId":created.identity.child_conversation_id,"limit":0}),
        json!({"conversationId":created.identity.child_conversation_id,"limit":51}),
        json!({"conversationId":created.identity.child_conversation_id,"cursor":"other-conversation-request"}),
        json!({"conversationId":created.identity.child_conversation_id,"actor":"agent"}),
    ] {
        assert!(
            crate::application::runtime_interactions::read(
                &f.database,
                "runtime.interactions.list",
                value
            )
            .is_err()
        );
    }
    assert_eq!(f.database.connection().total_changes(), changes);
    frame.sequence += 1;
    frame.rpc_id = json!(3);
    frame.method = "_x.ai/exit_plan_mode".into();
    let plan = storage::capture(
        &mut f.database,
        &frame,
        &json!({"planContent":"先补兼容，再做集成"}),
    )
    .unwrap();
    // The oldest still-pending question remains first. Once it closes, the next
    // plan is observable; rolling back preserves the original restart matrix.
    assert_eq!(
        storage::blocked_summary(f.database.connection(), &run, epoch).unwrap(),
        Some(json!({"kind":"question","request_id":second}))
    );
    f.database
        .connection()
        .execute_batch("SAVEPOINT native_blocked_summary")
        .unwrap();
    storage::close_request(
        &mut f.database,
        &second,
        &frame.core_generation,
        &frame.host_instance,
        "fixture_closed",
    )
    .unwrap();
    let report = crate::delegation_model_command::task_report(
        f.database.connection(),
        &created.identity.id,
        &created.identity.parent_conversation_id,
    )
    .unwrap();
    assert_eq!(
        report["blocked_on"],
        json!({"kind":"plan_approval","request_id":plan})
    );
    assert_eq!(report["message"], "等待用户确认计划");
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("先补兼容，再做集成")
    );
    f.database
        .connection()
        .execute_batch("ROLLBACK TO native_blocked_summary; RELEASE native_blocked_summary")
        .unwrap();
    let continuation = crate::application::runtime_interactions::read(
        &f.database,
        "runtime.interactions.list",
        json!({"conversationId":created.identity.child_conversation_id,"limit":1,"cursor":cursor}),
    )
    .unwrap();
    assert_eq!(continuation["items"][0]["id"], id);
    assert_eq!(continuation["nextCursor"], Value::Null);
    let resolve_plan = CommandEnvelope {
        command_id: Uuid::new_v4().to_string(),
        actor: actor.clone(),
        camp_id: Some(f.camp_id.clone()),
        expected_versions: vec![],
        execution_epoch: None,
        payload: ResolveInteractionCommand {
            interaction_id: plan.clone(),
            expected_version: 1,
            answer: Answer::PlanApproval {
                decision: PlanDecision::RequestChanges,
                feedback: Some("请保留旧版本兼容".into()),
            },
        },
    };
    assert_eq!(
        storage::resolve(&mut f.database, &resolve_plan, &frame.core_generation)
            .unwrap()
            .result
            .code,
        "interaction.resolved"
    );
    assert!(
        storage::begin_dispatch(
            &mut f.database,
            &plan,
            2,
            &frame.core_generation,
            &frame.host_instance
        )
        .unwrap_err()
        .to_string()
        .contains("followup_registration_required")
    );
    let mut approved = resolve_plan.clone();
    approved.command_id = Uuid::new_v4().to_string();
    approved.payload.answer = Answer::PlanApproval {
        decision: PlanDecision::Approve,
        feedback: None,
    };
    frame.sequence += 1;
    frame.rpc_id = json!(4);
    let sent = storage::capture(
        &mut f.database,
        &frame,
        &json!({"planContent":"明确批准的计划"}),
    )
    .unwrap();
    approved.payload.interaction_id = sent.clone();
    assert_eq!(
        storage::resolve(&mut f.database, &approved, &frame.core_generation)
            .unwrap()
            .result
            .code,
        "interaction.resolved"
    );
    let sent_reply = storage::begin_dispatch(
        &mut f.database,
        &sent,
        2,
        &frame.core_generation,
        &frame.host_instance,
    )
    .unwrap();
    assert!(storage::finish_dispatch(&mut f.database, &sent, &sent_reply.token, true).unwrap());
    assert_eq!(
        storage::read(&f.database, &actor, &f.camp_id, &sent)
            .unwrap()
            .unwrap()
            .status,
        "sent"
    );
    frame.sequence += 1;
    frame.rpc_id = json!(5);
    let inflight = storage::capture(
        &mut f.database,
        &frame,
        &json!({"planContent":"发送中断的计划"}),
    )
    .unwrap();
    approved.command_id = Uuid::new_v4().to_string();
    approved.payload.interaction_id = inflight.clone();
    storage::resolve(&mut f.database, &approved, &frame.core_generation).unwrap();
    let interrupted = storage::begin_dispatch(
        &mut f.database,
        &inflight,
        2,
        &frame.core_generation,
        &frame.host_instance,
    )
    .unwrap();
    // Restart closure is atomic and repeatable, without inventing skip/approval responses.
    for (generation, host, source, expected_epoch) in [
        (
            "other-core",
            frame.host_instance.as_str(),
            run.as_str(),
            epoch,
        ),
        (
            frame.core_generation.as_str(),
            "other-host",
            run.as_str(),
            epoch,
        ),
        (
            frame.core_generation.as_str(),
            frame.host_instance.as_str(),
            "other-run",
            epoch,
        ),
        (
            frame.core_generation.as_str(),
            frame.host_instance.as_str(),
            run.as_str(),
            epoch + 1,
        ),
    ] {
        assert_eq!(
            storage::close_host_run(&mut f.database, generation, host, source, expected_epoch)
                .unwrap(),
            0
        );
    }
    assert_eq!(
        storage::close_request(
            &mut f.database,
            &second,
            "other-core",
            &frame.host_instance,
            "fixture"
        )
        .unwrap(),
        0
    );
    f.database
        .connection()
        .execute_batch("SAVEPOINT native_host_exit")
        .unwrap();
    assert_eq!(
        storage::close_host_run(
            &mut f.database,
            &frame.core_generation,
            &frame.host_instance,
            &run,
            epoch
        )
        .unwrap(),
        3
    );
    assert_eq!(
        storage::read(&f.database, &actor, &f.camp_id, &second)
            .unwrap()
            .unwrap()
            .status,
        "closed"
    );
    assert_eq!(
        storage::read(&f.database, &actor, &f.camp_id, &inflight)
            .unwrap()
            .unwrap()
            .status,
        "delivery_unknown"
    );
    assert_eq!(
        storage::close_host_run(
            &mut f.database,
            &frame.core_generation,
            &frame.host_instance,
            &run,
            epoch
        )
        .unwrap(),
        0
    );
    f.database
        .connection()
        .execute_batch("ROLLBACK TO native_host_exit; RELEASE native_host_exit")
        .unwrap();
    f.database
        .connection()
        .execute_batch("SAVEPOINT native_one_request")
        .unwrap();
    assert_eq!(
        storage::close_request(
            &mut f.database,
            &second,
            &frame.core_generation,
            &frame.host_instance,
            "native_route_closed"
        )
        .unwrap(),
        1
    );
    assert_eq!(
        storage::read(&f.database, &actor, &f.camp_id, &plan)
            .unwrap()
            .unwrap()
            .status,
        "resolved"
    );
    f.database
        .connection()
        .execute_batch("ROLLBACK TO native_one_request; RELEASE native_one_request")
        .unwrap();
    let tx = f.database.connection_mut().transaction().unwrap();
    assert_eq!(
        storage::close_on_restart(&tx, "2026-10-18T00:00:01Z").unwrap(),
        3
    );
    tx.rollback().unwrap();
    assert_eq!(
        storage::read(&f.database, &actor, &f.camp_id, &second)
            .unwrap()
            .unwrap()
            .status,
        "pending"
    );
    let tx = f.database.connection_mut().transaction().unwrap();
    assert_eq!(
        storage::close_on_restart(&tx, "2026-10-18T00:00:01Z").unwrap(),
        3
    );
    tx.commit().unwrap();
    let tx = f.database.connection_mut().transaction().unwrap();
    assert_eq!(
        storage::close_on_restart(&tx, "2026-10-18T00:00:02Z").unwrap(),
        0
    );
    tx.commit().unwrap();
    assert_eq!(
        storage::read(&f.database, &actor, &f.camp_id, &plan)
            .unwrap()
            .unwrap()
            .status,
        "closed"
    );
    assert_eq!(
        storage::read(&f.database, &actor, &f.camp_id, &inflight)
            .unwrap()
            .unwrap()
            .status,
        "delivery_unknown"
    );
    assert!(
        !storage::finish_dispatch(&mut f.database, &inflight, &interrupted.token, true).unwrap()
    );
    assert_eq!(
        storage::resolve(&mut f.database, &resolve_plan, "new-generation")
            .unwrap()
            .result
            .code,
        "interaction.resolved"
    );
    // Original database closure trigger fences pending questions on actual run cancellation.
    frame.sequence += 1;
    frame.rpc_id = json!(6);
    frame.method = "_x.ai/ask_user_question".into();
    let pending = storage::capture(&mut f.database, &frame, &params).unwrap();
    let tx = f.database.connection_mut().transaction().unwrap();
    tx.execute(
        "UPDATE conversation SET native_session_id='fixture-binding-lost' WHERE id=?1",
        [&created.identity.child_conversation_id],
    )
    .unwrap();
    assert_eq!(
        tx.query_row(
            "SELECT status FROM runtime_user_interaction WHERE id=?1",
            [&pending],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "closed"
    );
    tx.rollback().unwrap();
    let version = f
        .database
        .connection()
        .query_row("SELECT version FROM agent_run WHERE id=?1", [&run], |r| {
            r.get(0)
        })
        .unwrap();
    let cancel = lifecycle_command(
        f,
        actor.clone(),
        crate::runtime::CancelAgentRunCommand {
            camp_id: f.camp_id.clone(),
            agent_run_id: run.clone(),
            expected_version: version,
        },
    );
    ExecutionRuntimeService::default()
        .request_agent_run_cancellation(&mut f.database, &cancel)
        .unwrap();
    assert_eq!(
        storage::read(&f.database, &actor, &f.camp_id, &pending)
            .unwrap()
            .unwrap()
            .status,
        "closed"
    );
    assert_eq!(
        f.database
            .connection()
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
    assert!(
        storage::blocked_summary(f.database.connection(), &run, epoch)
            .unwrap()
            .is_none()
    );
}
