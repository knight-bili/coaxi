// Original Context owner, real Registry/factory/User takeover and send. Transport,
// executable metadata and cleanup are fixtures; no native agent or model runs.
fn assert_delegation_runtime_defaults(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    request: &crate::delegation::FrozenRequest<'_>,
    plan: &crate::delegation_factory::ChildRunPlan<'_>,
) {
    use crate::delegation_runtime_defaults as defaults;
    let saved: String = f
        .database
        .connection()
        .query_row(
            "SELECT per_runtime_defaults FROM delegation_settings WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let preset = json!({"mode_id":"read-only","config_values":{"approval_policy":"on-request"}});
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET per_runtime_defaults=?1 WHERE singleton=1",
            [serde_json::to_string(&json!({"codex":preset})).unwrap()],
        )
        .unwrap();
    let parent = crate::delegation_admission::authenticate_new_child(
        f.database.connection(),
        binding,
        source,
        "actual-frozen-defaults",
    )
    .unwrap();
    let observed = parent.observe_workspace(request.working_dir).unwrap();
    let selected = crate::delegation::FrozenRequest {
        runtime_preset: &preset,
        ..*request
    };
    let before = factory_counts(f);
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        parent
            .register_initial_within_parent(&tx, request, plan, &observed, "2026-10-17T00:00:00Z")
            .err()
            .unwrap()
            .to_string()
            .contains("runtime_defaults_plan_changed")
    );
    tx.rollback().unwrap();
    assert_eq!(factory_counts(f), before);
    // The original Registry, rather than a copied parent configuration, rejects
    // an unsupported User permission value before any child graph is written.
    let bad = json!({"mode_id":"read-only","config_values":{"approval_policy":"not-supported"}});
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET per_runtime_defaults=?1 WHERE singleton=1",
            [serde_json::to_string(&json!({"codex":bad})).unwrap()],
        )
        .unwrap();
    let bad_parent = crate::delegation_admission::authenticate_new_child(
        f.database.connection(),
        binding,
        source,
        "invalid-native-defaults",
    )
    .unwrap();
    let bad_observed = bad_parent.observe_workspace(request.working_dir).unwrap();
    let bad_request = crate::delegation::FrozenRequest {
        runtime_preset: &bad,
        ..*request
    };
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        bad_parent
            .register_initial_within_parent(
                &tx,
                &bad_request,
                plan,
                &bad_observed,
                "2026-10-17T00:00:00Z"
            )
            .is_err()
    );
    tx.rollback().unwrap();
    assert_eq!(factory_counts(f), before);
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET per_runtime_defaults=?1 WHERE singleton=1",
            [serde_json::to_string(&json!({"codex":preset})).unwrap()],
        )
        .unwrap();
    let tx = f.database.connection_mut().transaction().unwrap();
    let registered = parent
        .register_initial_within_parent(&tx, &selected, plan, &observed, "2026-10-17T00:00:00Z")
        .unwrap();
    let run: String = tx
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [&registered.identity.id],
            |r| r.get(0),
        )
        .unwrap();
    tx.commit().unwrap();
    let frozen = defaults::for_run(f.database.connection(), &run)
        .unwrap()
        .unwrap();
    assert_eq!(serde_json::to_value(&frozen).unwrap(), preset);
    let config: String = f
        .database
        .connection()
        .query_row(
            "SELECT effective_config_json FROM agent_run WHERE id=?1",
            [&run],
            |r| r.get(0),
        )
        .unwrap();
    let config: Value = serde_json::from_str(&config).unwrap();
    assert_eq!(
        config["runtime"]["permissions"]["values"]["sandbox_mode"],
        "read-only"
    );
    assert_eq!(
        config["runtime"]["permissions"]["values"]["approval_policy"],
        "on-request"
    );
    let mut unsigned = config.clone();
    let digest = unsigned
        .as_object_mut()
        .unwrap()
        .remove("configDigest")
        .unwrap();
    assert_eq!(
        digest,
        crate::command::canonical_json_digest(&unsigned).unwrap()
    );
    let runtime: crate::agent_profile::FrozenAgentRuntimeConfig =
        serde_json::from_value(config["runtime"].clone()).unwrap();
    assert!(defaults::empty(
        &defaults::pending_acp(&frozen, &runtime).unwrap()
    ));
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET per_runtime_defaults='{}' WHERE singleton=1",
            [],
        )
        .unwrap();
    assert!(parent.revalidate(f.database.connection()).is_err());
    assert_eq!(
        defaults::for_run(f.database.connection(), &run)
            .unwrap()
            .unwrap(),
        frozen
    );
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
    assert_eq!(defaults::for_run(&reopened, &run).unwrap().unwrap(), frozen);
    drop(reopened);
    let epoch = claim_context_child(f, &run);
    crate::application::delegation_control::handle(
        &mut f.database,
        "delegations.takeover",
        control_request(&registered.identity.id, "expectedControlVersion", 1),
    )
    .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &run, epoch)
        .unwrap();
    let user = child_end_send(
        f,
        &registered.identity.child_conversation_id,
        "沿用本帮手已冻结的运行设置",
    );
    let user_run = user["agentRunId"].as_str().unwrap();
    assert_eq!(
        defaults::for_run(f.database.connection(), user_run)
            .unwrap()
            .unwrap(),
        frozen
    );
    let followup: String = f
        .database
        .connection()
        .query_row(
            "SELECT effective_config_json FROM agent_run WHERE id=?1",
            [user_run],
            |r| r.get(0),
        )
        .unwrap();
    let followup: Value = serde_json::from_str(&followup).unwrap();
    assert_eq!(
        followup["runtime"]["permissions"],
        config["runtime"]["permissions"]
    );
    let epoch = claim_context_child(f, user_run);
    let stop = lifecycle_command(
        f,
        queue_actor(),
        crate::runtime::CancelAgentRunCommand {
            camp_id: f.camp_id.clone(),
            agent_run_id: user_run.into(),
            expected_version: lifecycle_version(f, user_run),
        },
    );
    ExecutionRuntimeService::default()
        .request_agent_run_cancellation(&mut f.database, &stop)
        .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, user_run, epoch)
        .unwrap();
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET per_runtime_defaults=?1 WHERE singleton=1",
            [saved],
        )
        .unwrap();
}
