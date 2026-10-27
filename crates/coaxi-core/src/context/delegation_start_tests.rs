// One existing Core owner/database. Actual target selection, file IO, original
// factory/Gateway and permissions; no agent/runtime process is launched.
fn production_start(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    call: &str,
    input: &crate::delegation_contract::StartParams,
) -> crate::command::CommandExecution {
    let invocation = crate::delegation_model_command::Invocation {
        binding,
        source,
        tool_call: call,
        input,
    };
    let prepared = crate::delegation_start::prepare(
        &f.database,
        &ManagedBlobStore::new(&f.directory),
        &invocation,
    )
    .unwrap();
    let observed = std::thread::spawn(move || prepared.observe())
        .join()
        .unwrap()
        .unwrap();
    let result = observed.commit(&mut f.database, &invocation).unwrap();
    observed.finish_after(&result).unwrap();
    result
}
fn assert_production_start(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
) {
    use crate::{
        delegation_contract::{CancelParams, StartParams},
        delegation_model_command::{self as model, Invocation},
    };
    let input = StartParams {
        agent_type: "codex".to_owned().try_into().unwrap(),
        task: format!("完整任务生产准备 {} ", Uuid::new_v4())
            .repeat(4096)
            .try_into()
            .unwrap(),
        working_dir: None,
    };
    let invocation = Invocation {
        binding,
        source,
        tool_call: "production-default-start",
        input: &input,
    };
    let store = ManagedBlobStore::new(&f.directory);
    let blob_id = format!(
        "blob-sha256-{:x}",
        Sha256::digest(input.task.as_str().as_bytes())
    );
    let saved: String = f
        .database
        .connection()
        .query_row(
            "SELECT per_runtime_defaults FROM delegation_settings WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut defaults: Value = serde_json::from_str(&saved).unwrap();
    defaults["codex"] = json!({"mode_id":"read-only","config_values":{"model":"alternate-model","approval_policy":"on-request"}});
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET per_runtime_defaults=?1 WHERE singleton=1",
            [defaults.to_string()],
        )
        .unwrap();
    let before = factory_counts(f);
    let changes = f.database.connection().total_changes();
    let prepared = crate::delegation_start::prepare(&f.database, &store, &invocation).unwrap();
    assert_eq!(f.database.connection().total_changes(), changes);
    let observed = std::thread::spawn(move || prepared.observe())
        .join()
        .unwrap()
        .unwrap();
    assert_eq!(f.database.connection().total_changes(), changes);
    assert!(
        !f.database
            .connection()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM managed_blob WHERE id=?1)",
                [&blob_id],
                |r| r.get::<_, bool>(0)
            )
            .unwrap()
    );
    store.recover_gc_state(&mut f.database).unwrap();
    f.database.connection().execute_batch("CREATE TEMP TRIGGER fail_production_start_receipt BEFORE INSERT ON event_log WHEN NEW.event_type='command.result' AND NEW.command_type='delegation.start' BEGIN SELECT RAISE(ABORT,'production start receipt fault'); END;").unwrap();
    assert!(
        format!(
            "{:#}",
            observed.commit(&mut f.database, &invocation).unwrap_err()
        )
        .contains("production start receipt fault")
    );
    assert_eq!(factory_counts(f), before);
    assert!(
        !f.database
            .connection()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM managed_blob WHERE id=?1)",
                [&blob_id],
                |r| r.get::<_, bool>(0)
            )
            .unwrap()
    );
    assert!(
        model::replay_start(&f.database, &invocation)
            .unwrap()
            .is_none()
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER fail_production_start_receipt;")
        .unwrap();
    assert!(
        observed
            .commit(
                &mut f.database,
                &Invocation {
                    tool_call: "wrong-production-call",
                    ..invocation
                }
            )
            .is_err()
    );
    let result = observed.commit(&mut f.database, &invocation).unwrap();
    assert_eq!(
        result.result.status,
        crate::command::CommandResultStatus::Accepted
    );
    observed.finish_after(&result).unwrap();
    drop(observed);
    let task = result.result.payload["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let runtime_json: String = f.database.connection().query_row(
        "SELECT r.effective_config_json FROM delegation d JOIN delegation_attempt a ON a.delegation_id=d.id JOIN agent_run r ON r.id=a.agent_run_id WHERE d.id=?1",
        [&task],|r|r.get(0),
    ).unwrap();
    let config: Value = serde_json::from_str(&runtime_json).unwrap();
    assert_eq!(config["runtime"]["adapterKind"], "codex-cli");
    assert_eq!(config["runtime"]["model"]["modelId"], "alternate-model");
    assert_eq!(
        config["runtime"]["permissions"]["values"]["sandbox_mode"],
        "read-only"
    );
    assert_eq!(
        store.read_text(&f.database, &blob_id).unwrap(),
        input.task.as_str()
    );
    assert!(
        !store
            .collect_unreferenced_before(&mut f.database, "9999-01-01T00:00:00Z")
            .unwrap()
            .contains(&blob_id)
    );
    assert_eq!(
        store.read_text(&f.database, &blob_id).unwrap(),
        input.task.as_str()
    );
    let attached: bool = f.database.connection().query_row("SELECT gc_candidate_at IS NULL AND gc_candidate_owner IS NULL FROM managed_blob WHERE id=?1",[&blob_id],|r|r.get(0)).unwrap();
    assert!(attached);
    assert!(
        !store
            .collect_gc_candidates_before(&mut f.database, "9999-01-01T00:00:00Z", 1000)
            .unwrap()
            .contains(&blob_id)
    );
    assert_eq!(
        store.read_text(&f.database, &blob_id).unwrap(),
        input.task.as_str()
    );
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET per_runtime_defaults=?1 WHERE singleton=1",
            [&saved],
        )
        .unwrap();
    let changes = f.database.connection().total_changes();
    assert_eq!(
        production_start(f, binding, source, invocation.tool_call, &input).result,
        result.result
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    let mut tasks = vec![task];

    let pi = StartParams {
        agent_type: "pi".to_owned().try_into().unwrap(),
        task: "独立目标不继承父模型配置".to_owned().try_into().unwrap(),
        working_dir: None,
    };
    let unavailable =
        production_start(f, binding, source, "production-pi-before-registration", &pi);
    assert_eq!(unavailable.result.code, "delegation.runtime_unavailable");
    let program = f.directory.join("production-light-pi-fixture.exe");
    std::fs::write(&program, b"entry identity fixture only; never execute").unwrap();
    let discovered = crate::agent_profile::DiscoveredRuntimeEntry {
        adapter_kind: crate::agent_profile::AdapterKind::Pi,
        executable_path: program.to_string_lossy().into_owned(),
        executable_fingerprint: crate::agent_runtime_adapter::executable_fingerprint(&program)
            .unwrap(),
        source: crate::agent_profile::InstallationSource::InheritedPath,
        entrypoint_locator_identity: None,
    }
    .verify()
    .unwrap();
    AgentProfileService::default()
        .commit_discovered_runtime_entry(&mut f.database, discovered, None)
        .unwrap();
    let changes = f.database.connection().total_changes();
    assert_eq!(
        production_start(f, binding, source, "production-pi-before-registration", &pi).result,
        unavailable.result
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    let pi_result = production_start(f, binding, source, "production-pi-start", &pi);
    assert_eq!(
        pi_result.result.status,
        crate::command::CommandResultStatus::Accepted,
        "{:?}",
        pi_result.result
    );
    let pi_task = pi_result.result.payload["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let installation: String = f.database.connection().query_row("SELECT r.runtime_installation_id FROM delegation d JOIN delegation_attempt a ON a.delegation_id=d.id JOIN agent_run r ON r.id=a.agent_run_id WHERE d.id=?1 AND r.runtime_adapter_kind='pi'",[&pi_task],|r|r.get(0)).unwrap();
    f.database
        .connection()
        .execute(
            "UPDATE adapter_installation SET enabled=0 WHERE id=?1",
            [&installation],
        )
        .unwrap();
    let changes = f.database.connection().total_changes();
    assert_eq!(
        production_start(f, binding, source, "production-pi-start", &pi).result,
        pi_result.result
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    assert_eq!(
        production_start(f, binding, source, "production-pi-disabled", &pi)
            .result
            .code,
        "delegation.runtime_unavailable"
    );
    f.database
        .connection()
        .execute(
            "UPDATE adapter_installation SET enabled=1 WHERE id=?1",
            [&installation],
        )
        .unwrap();
    tasks.push(pi_task);

    let custom_id: String = f.database.connection().query_row("SELECT id FROM adapter_installation WHERE installation_class='custom' AND adapter_kind='pi' AND enabled=1 LIMIT 1",[],|r|r.get(0)).unwrap();
    let custom = StartParams {
        agent_type: format!("custom:{custom_id}").try_into().unwrap(),
        task: "使用登记的自定义目标".to_owned().try_into().unwrap(),
        working_dir: None,
    };
    let custom_result = production_start(f, binding, source, "production-custom-start", &custom);
    assert_eq!(
        custom_result.result.status,
        crate::command::CommandResultStatus::Accepted
    );
    let custom_task = custom_result.result.payload["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let actual: String = f.database.connection().query_row("SELECT r.runtime_installation_id FROM delegation d JOIN delegation_attempt a ON a.delegation_id=d.id JOIN agent_run r ON r.id=a.agent_run_id WHERE d.id=?1",[&custom_task],|r|r.get(0)).unwrap();
    assert_eq!(actual, custom_id);
    tasks.push(custom_task);

    let missing_path = f.directory.join("production-directory-missing");
    let missing = StartParams {
        working_dir: Some(
            missing_path
                .to_string_lossy()
                .into_owned()
                .try_into()
                .unwrap(),
        ),
        task: "目录失效不创建任务".to_owned().try_into().unwrap(),
        ..pi.clone()
    };
    let before = factory_counts(f);
    let missing_result =
        production_start(f, binding, source, "production-missing-directory", &missing);
    assert_eq!(
        missing_result.result.code,
        "delegation.directory_unavailable"
    );
    assert_eq!(factory_counts(f), before);
    std::fs::create_dir(&missing_path).unwrap();
    let changes = f.database.connection().total_changes();
    assert_eq!(
        production_start(f, binding, source, "production-missing-directory", &missing).result,
        missing_result.result
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    std::fs::remove_dir(&missing_path).unwrap();

    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=0 WHERE singleton=1",
            [],
        )
        .unwrap();
    let denied_invocation = Invocation {
        tool_call: "production-disabled-start",
        input: &pi,
        ..invocation
    };
    let denied = crate::delegation_start::prepare(&f.database, &store, &denied_invocation)
        .unwrap()
        .observe()
        .unwrap();
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=1 WHERE singleton=1",
            [],
        )
        .unwrap();
    assert_eq!(
        denied
            .commit(&mut f.database, &denied_invocation)
            .unwrap()
            .result
            .code,
        "delegation.start_disabled"
    );
    let late = StartParams {
        task: "准备后开关撤销必须回滚文件登记"
            .to_owned()
            .try_into()
            .unwrap(),
        ..pi.clone()
    };
    let late_invocation = Invocation {
        tool_call: "production-late-disable",
        input: &late,
        ..invocation
    };
    let prepared = crate::delegation_start::prepare(&f.database, &store, &late_invocation).unwrap();
    let late_observed = std::thread::spawn(move || prepared.observe())
        .join()
        .unwrap()
        .unwrap();
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=0 WHERE singleton=1",
            [],
        )
        .unwrap();
    let before = factory_counts(f);
    assert_eq!(
        late_observed
            .commit(&mut f.database, &late_invocation)
            .unwrap()
            .result
            .code,
        "delegation.start_rejected"
    );
    assert_eq!(factory_counts(f), before);
    let late_blob = format!(
        "blob-sha256-{:x}",
        Sha256::digest(late.task.as_str().as_bytes())
    );
    assert!(
        !f.database
            .connection()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM managed_blob WHERE id=?1)",
                [&late_blob],
                |r| r.get::<_, bool>(0)
            )
            .unwrap()
    );
    drop(late_observed);
    store.recover_gc_state(&mut f.database).unwrap();
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=1 WHERE singleton=1",
            [],
        )
        .unwrap();

    let outside = std::env::temp_dir().join(format!("coaxi-production-outside-{}", Uuid::new_v4()));
    std::fs::create_dir(&outside).unwrap();
    let external = StartParams {
        task: "外部目录必须等待原用户审批".to_owned().try_into().unwrap(),
        working_dir: Some(outside.to_string_lossy().into_owned().try_into().unwrap()),
        ..pi
    };
    let external_result = production_start(
        f,
        binding,
        source,
        "production-external-directory",
        &external,
    );
    assert_eq!(
        external_result.result.payload["blocked_on"]["kind"],
        "permission"
    );
    tasks.push(
        external_result.result.payload["task_id"]
            .as_str()
            .unwrap()
            .to_owned(),
    );
    std::fs::remove_dir(&outside).unwrap();
    for (index, task) in tasks.into_iter().enumerate() {
        model::cancel(
            &mut f.database,
            &Invocation {
                binding,
                source,
                tool_call: &format!("production-pipeline-cleanup-{index}"),
                input: &CancelParams {
                    task_id: task.try_into().unwrap(),
                },
            },
        )
        .unwrap();
    }
}
