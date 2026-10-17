// Extends the existing owner/database. Native Session, ACK and Fleet hosts are
// explicit fixtures; Core registration, input persistence and recovery are real.
fn restart_input_identity(f: &Fixture, delivery: &str) -> String {
    f.database
        .connection()
        .query_row(
        "SELECT json_array(agent_run_id,execution_epoch,context_manifest_id,dynamic_payload_digest,
            runtime_request_digest,native_binding_id,native_binding_generation,native_input_id)
         FROM runtime_input_delivery WHERE id=?1",
            [delivery],
            |row| row.get(0),
        )
        .unwrap()
}
fn assert_delegation_restart_settlement(f: &mut Fixture, parent: &str, epoch: i64) {
    let (id, root, root_epoch) =
        context_child_of(f, parent, epoch, "restart accepted root", "restart-root");
    let (_, child, child_epoch) = context_child_of(
        f,
        &root,
        root_epoch,
        "restart accepted child",
        "restart-child",
    );
    let (_, leaf, _) = context_child_of(
        f,
        &child,
        child_epoch,
        "restart controlled leaf",
        "restart-leaf",
    );
    let (detached_id, detached, detached_epoch) =
        context_child_of(f, &root, root_epoch, "restart takeover cut", "restart-cut");
    let (_, uncertain, uncertain_epoch) = context_child_of(
        f,
        parent,
        epoch,
        "restart uncertain input",
        "restart-unknown",
    );
    let (_, uncertain_child, _) = context_child_of(
        f,
        &uncertain,
        uncertain_epoch,
        "restart uncertain child",
        "restart-unknown-child",
    );

    // Real Core binding IDs exist before provider Session IDs. The same keys
    // consumed by all three production adapter families keep these scopes apart.
    let binding = |run: &str| {
        f.database.connection().query_row(
        "SELECT c.native_binding_id,c.native_session_id FROM agent_run r JOIN conversation c ON c.id=r.conversation_id WHERE r.id=?1",
        [run],|row|Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?))).unwrap()
    };
    let root_binding = binding(&root);
    let child_binding = binding(&child);
    assert!(root_binding.1.is_none() && child_binding.1.is_none());
    assert_ne!(root_binding.0, child_binding.0);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        use crate::runtime_fleet::*;
        let fleet = AgentRuntimeFleetManager::new(AgentRuntimeFleetConfig::default());
        let request = |run: &str, binding: &str| FleetAcquireRequest {
            agent_run_id: run.into(),
            execution_epoch: 1,
            adapter_kind: AdapterKind::CodexCli,
            compatibility: RuntimeCompatibilityKey::member_session(
                &f.camp_id,
                "agent_1",
                Some(binding),
                "same-host-config",
            ),
        };
        let first = fleet
            .acquire(request("restart-scope-a", &root_binding.0), || async {
                Ok(fake_runtime_process_host("restart-host-a"))
            })
            .await
            .unwrap();
        fleet
            .release("restart-scope-a", 1, FleetReleaseDisposition::Reusable)
            .await;
        let second = fleet
            .acquire(request("restart-scope-b", &child_binding.0), || async {
                Ok(fake_runtime_process_host("restart-host-b"))
            })
            .await
            .unwrap();
        assert_ne!(first.process_id, second.process_id);
        fleet
            .release("restart-scope-b", 1, FleetReleaseDisposition::Reusable)
            .await;
        let again = fleet
            .acquire(request("restart-scope-a2", &root_binding.0), || async {
                panic!("same binding must keep its warm host")
            })
            .await
            .unwrap();
        assert_eq!(first.process_id, again.process_id);
        fleet.shutdown_all().await;
    });

    for (run, run_epoch, session) in [
        (&root, root_epoch, "fixture-restart-root"),
        (&child, child_epoch, "fixture-restart-child"),
        (&uncertain, uncertain_epoch, "fixture-restart-unknown"),
    ] {
        assert_eq!(
            resume_bind(f, run, run_epoch, session).result.status,
            crate::command::CommandResultStatus::Applied
        );
    }
    let root_context = resume_context(f, &root, root_epoch);
    let root_input =
        resume_accept_input(f, &root, root_epoch, &root_context, "fixture-root-accepted");
    let child_context = resume_context(f, &child, child_epoch);
    let child_input = resume_accept_input(
        f,
        &child,
        child_epoch,
        &child_context,
        "fixture-child-accepted",
    );
    let unknown_context = resume_context(f, &uncertain, uncertain_epoch);
    let unknown_input = ContextService
        .prepare_input_delivery_for_context(
            &mut f.database,
            &uncertain,
            uncertain_epoch,
            &unknown_context,
        )
        .unwrap();
    assert!(
        ContextService
            .begin_runtime_input_dispatch(
                &mut f.database,
                &unknown_input.id,
                &uncertain,
                uncertain_epoch
            )
            .unwrap()
    );
    let identities = [
        restart_input_identity(f, &root_input),
        restart_input_identity(f, &child_input),
        restart_input_identity(f, &unknown_input.id),
    ];
    let frozen: String = f
        .database
        .connection()
        .query_row(
            "SELECT effective_config_json FROM agent_run WHERE id=?1",
            [&root],
            |r| r.get(0),
        )
        .unwrap();
    let original_blob = f
        .database
        .connection()
        .query_row(
            "SELECT task_blob_id FROM delegation WHERE id=?1",
            [&id],
            |r| r.get::<_, String>(0),
        )
        .unwrap();
    let original_task = ManagedBlobStore::new(&f.directory)
        .read_text(&f.database, &original_blob)
        .unwrap();

    let control = crate::delegation::control(f.database.connection(), &detached_id).unwrap();
    let takeover = lifecycle_command(
        f,
        queue_actor(),
        crate::delegation_control::TakeoverDelegationCommand {
            delegation_id: detached_id.clone(),
            expected_control_version: control.version,
        },
    );
    assert_ne!(
        crate::delegation_control::takeover(&mut f.database, &takeover)
            .unwrap()
            .result
            .status,
        crate::command::CommandResultStatus::Rejected
    );
    // Fixture-only adapter acknowledgement: takeover does not imply process cleanup.
    crate::runtime::ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &detached, detached_epoch)
        .unwrap();
    let conversation: String = f
        .database
        .connection()
        .query_row(
            "SELECT conversation_id FROM agent_run WHERE id=?1",
            [&detached],
            |r| r.get(0),
        )
        .unwrap();
    let user = child_end_send(
        f,
        &conversation,
        "User keeps this queued input across the old parent's restart",
    );
    let user_run = user["agentRunId"].as_str().unwrap().to_owned();
    assert_eq!(lifecycle_status(f, &user_run), "queued");

    let before = control_rows(f);
    let count = factory_counts(f);
    f.database.connection().execute_batch(&format!("CREATE TEMP TRIGGER reject_restart_tree BEFORE UPDATE OF status ON agent_run WHEN OLD.id='{child}' AND NEW.status='cancelled' BEGIN SELECT RAISE(ABORT,'restart tree rollback'); END;")).unwrap();
    assert!(
        f.database
            .prepare_v2_recovery()
            .unwrap_err()
            .to_string()
            .contains("restart tree rollback")
    );
    assert_eq!(control_rows(f), before);
    assert_eq!(restart_input_identity(f, &unknown_input.id), identities[2]);
    assert_eq!(
        f.database
            .connection()
            .query_row(
                "SELECT status FROM runtime_input_delivery WHERE id=?1",
                [&unknown_input.id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "prepared"
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER reject_restart_tree; PRAGMA reverse_unordered_selects=ON;")
        .unwrap();
    let recovered = f.database.prepare_v2_recovery().unwrap();
    f.database
        .connection()
        .execute_batch("PRAGMA reverse_unordered_selects=OFF;")
        .unwrap();
    assert!(recovered.accepted_input_recovery_blockers_created > 0);
    assert_eq!(lifecycle_status(f, &root), "failed");
    assert!(crate::delegation_control::cleanup_pending(f.database.connection(), &id).unwrap());
    assert_eq!(lifecycle_status(f, &child), "cancelled");
    assert_eq!(lifecycle_status(f, &leaf), "cancelled");
    assert_eq!(lifecycle_status(f, &user_run), "queued");
    assert_eq!(lifecycle_status(f, &uncertain), "waiting");
    assert_ne!(lifecycle_status(f, &uncertain_child), "cancelled");
    assert_eq!(
        f.database
            .connection()
            .query_row(
                "SELECT wait_reason FROM agent_run WHERE id=?1",
                [&uncertain],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "delivery_unknown"
    );
    assert_eq!(
        f.database
            .connection()
            .query_row(
                "SELECT status FROM runtime_input_delivery WHERE id=?1",
                [&unknown_input.id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "delivery_unknown"
    );
    for (input, expected) in [
        (&root_input, &identities[0]),
        (&child_input, &identities[1]),
        (&unknown_input.id, &identities[2]),
    ] {
        assert_eq!(&restart_input_identity(f, input), expected);
    }
    assert_eq!(
        f.database
            .connection()
            .query_row(
                "SELECT effective_config_json FROM agent_run WHERE id=?1",
                [&root],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        frozen
    );
    assert_eq!(
        ManagedBlobStore::new(&f.directory)
            .read_text(&f.database, &original_blob)
            .unwrap(),
        original_task
    );
    assert_eq!(factory_counts(f), count);
    assert!(
        ExecutionRuntimeService::default()
            .list_dispatchable_agent_runs(&f.database, 100)
            .unwrap()
            .iter()
            .all(|c| c.agent_run_id != root && c.agent_run_id != uncertain)
    );
    let version = f
        .database
        .connection()
        .query_row(
            "SELECT version FROM agent_run WHERE id=?1",
            [&uncertain],
            |r| r.get(0),
        )
        .unwrap();
    let claim = lifecycle_command(
        f,
        ActorRef::System {
            component_id: "agent-run-scheduler".into(),
        },
        ClaimAgentRunCommand {
            agent_run_id: uncertain,
            expected_version: version,
            lease_owner: "restart-unknown-fixture".into(),
            lease_seconds: 60,
            workspace: None,
            starting_git_observation: None,
        },
    );
    let blocked = ExecutionRuntimeService::default()
        .claim_agent_run(&mut f.database, &claim)
        .unwrap();
    assert_eq!(
        blocked.result.status,
        crate::command::CommandResultStatus::Rejected
    );
    assert_eq!(blocked.result.code, "agent_run.not_claimable");
    for input in [&root_input, &child_input] {
        let status: String = f
            .database
            .connection()
            .query_row(
                "SELECT status FROM runtime_input_delivery WHERE id=?1",
                [input],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "accepted");
    }
    let changes = f.database.connection().total_changes();
    let second = f.database.prepare_v2_recovery().unwrap();
    assert_eq!(second.accepted_input_recovery_blockers_created, 0);
    assert_eq!(f.database.connection().total_changes(), changes);
}
