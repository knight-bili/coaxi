// Existing owner: real Core factory/Registry/claim, fixture installation bytes, no Runtime process.
fn factory_counts(f: &Fixture) -> (i64, i64, i64, i64, i64) {
    f.database.connection().query_row("SELECT (SELECT count(*) FROM delegation),(SELECT count(*) FROM conversation),(SELECT count(*) FROM conversation_message),(SELECT count(*) FROM camp_turn),(SELECT count(*) FROM agent_run)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap()
}
fn assert_initial_child_factory(f: &mut Fixture) {
    use crate::{
        agent_profile::{
            AdapterKind, AdapterPermissionConfig, CreateAdapterInstallationCommand,
            InstallationSource, MemberRuntimeConfiguration, ModelSelection,
        },
        delegation_factory::{self as factory, ChildRunPlan},
        delegation_policy::{FrozenPolicy, Role},
    };
    let mapped = crate::delegation_contract::RUNTIME_TARGETS
        .iter()
        .map(|name| {
            let target =
                crate::delegation_contract::RuntimeTarget::try_from(name.to_string()).unwrap();
            factory::builtin_adapter(&target).unwrap()
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(mapped.len(), 21);
    let old: (i64, i64) = f
        .database
        .connection()
        .query_row(
            "SELECT enabled,depth_limit FROM delegation_settings WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=1,depth_limit=8 WHERE singleton=1",
            [],
        )
        .unwrap();
    let program = f.directory.join("factory-pi-fixture.exe");
    std::fs::write(&program, b"fixture identity only; never execute").unwrap();
    let mut installation = lifecycle_command(
        f,
        queue_actor(),
        CreateAdapterInstallationCommand {
            adapter_kind: AdapterKind::Pi,
            executable_path: program.to_string_lossy().into_owned(),
            command_name: "pi".into(),
            source: InstallationSource::Custom,
            auth_scope: "factory-independent-target".into(),
        },
    );
    installation.camp_id = None;
    let installed = AgentProfileService::default()
        .create_installation(&mut f.database, &installation)
        .unwrap();
    assert_eq!(installed.result.code, "adapter_installation.created");
    let installation_id = installed.result.payload["installationId"]
        .as_str()
        .unwrap()
        .to_string();
    let configuration = MemberRuntimeConfiguration {
        adapter_kind: AdapterKind::Pi,
        model: ModelSelection::RuntimeDefault,
        permissions: AdapterPermissionConfig {
            adapter_kind: AdapterKind::Pi,
            schema_version: 1,
            values: json!({}),
        },
    };
    let workspace = crate::runtime::AgentRunWorkspace {
        execution_root: f.directory.to_string_lossy().into_owned(),
        access: "read_only".into(),
        isolation: "shared".into(),
    };
    let task = "独立 Pi 完整任务：不继承 Codex 人设、Session 或执行状态。";
    let blob = ManagedBlobStore::new(&f.directory)
        .put_bytes(&mut f.database, task.as_bytes(), "text/plain", "sensitive")
        .unwrap();
    let (binding,generation):(String,i64)=f.database.connection().query_row("SELECT c.native_binding_id,c.native_binding_generation FROM conversation c JOIN agent_run r ON r.conversation_id=c.id WHERE r.id=?1",[&f.run_id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    let root = f.run_id.clone();
    let parent = crate::delegation::ParentInvocation {
        run_id: &root,
        epoch: f.execution_epoch,
        binding_id: &binding,
        binding_generation: generation,
        tool_call_id: "factory-pi-initial",
    };
    let target = crate::delegation_contract::RuntimeTarget::try_from("pi".to_string()).unwrap();
    let workspace_json = serde_json::to_value(&workspace).unwrap();
    let preset = json!({});
    let request = crate::delegation::FrozenRequest {
        task_blob_id: &blob.id,
        runtime: &target,
        working_dir: None,
        workspace: &workspace_json,
        runtime_preset: &preset,
    };
    let policy = FrozenPolicy::new(true, 8, Role::Child).unwrap();
    let plan = ChildRunPlan {
        task,
        target: &target,
        installation_id: &installation_id,
        configuration: &configuration,
        workspace: &workspace,
        policy,
    };
    // Fail only after the complete child factory has written its graph.
    let before = factory_counts(f);
    f.database.connection().execute_batch("CREATE TEMP TRIGGER reject_initial_factory BEFORE INSERT ON delegation WHEN NEW.requested_runtime='pi' BEGIN SELECT RAISE(ABORT,'injected factory rollback'); END;").unwrap();
    let tx = f.database.connection_mut().transaction().unwrap();
    let failed = factory::register_initial(&tx, &parent, &request, &plan, "2026-10-17T00:00:00Z");
    assert!(format!("{:#}", failed.unwrap_err()).contains("injected factory rollback"));
    tx.rollback().unwrap();
    assert_eq!(factory_counts(f), before);
    f.database
        .connection()
        .execute_batch("DROP TRIGGER reject_initial_factory;")
        .unwrap();
    let tx = f.database.connection_mut().transaction().unwrap();
    let created =
        factory::register_initial(&tx, &parent, &request, &plan, "2026-10-17T00:00:00Z").unwrap();
    tx.commit().unwrap();
    let run: String = f
        .database
        .connection()
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [&created.identity.id],
            |r| r.get(0),
        )
        .unwrap();
    let (adapter,installation,epoch,status,task_id,lease,final_message,config):(String,String,i64,String,Option<String>,Option<String>,Option<String>,String)=f.database.connection().query_row("SELECT runtime_adapter_kind,runtime_installation_id,execution_epoch,status,task_id,execution_lease_owner,final_conversation_message_id,effective_config_json FROM agent_run WHERE id=?1",[&run],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).unwrap();
    assert_eq!(adapter, "pi");
    assert_eq!(installation, installation_id);
    assert_eq!((epoch, status.as_str()), (0, "queued"));
    assert!(task_id.is_none() && lease.is_none() && final_message.is_none());
    let config: Value = serde_json::from_str(&config).unwrap();
    assert_eq!(config["runtime"]["adapterKind"], "pi");
    assert_eq!(config["runtime"]["installationId"], installation_id);
    assert_eq!(config["delegationPolicy"]["role"], "child");
    let fresh: (Option<String>, Option<String>) = f
        .database
        .connection()
        .query_row(
            "SELECT native_binding_id,native_session_id FROM conversation WHERE id=?1",
            [&created.identity.child_conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(fresh, (None, None));
    // The exact original call is replayed even if today's installation is disabled.
    f.database
        .connection()
        .execute(
            "UPDATE adapter_installation SET enabled=0 WHERE id=?1",
            [&installation_id],
        )
        .unwrap();
    let count = factory_counts(f);
    let tx = f.database.connection_mut().transaction().unwrap();
    let replay =
        crate::delegation::register(&tx, &parent, &request, "2026-10-17T00:00:01Z", |_, _| {
            panic!("replay must not resolve or create another target")
        })
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.identity, created.identity);
    tx.commit().unwrap();
    assert_eq!(factory_counts(f), count);
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        factory::create_child(&tx, &created.identity, &plan, &json!({}), "2026-10-17T00:00:00Z")
            .unwrap_err()
            .to_string()
            .contains("adapter_installation_disabled")
    );
    tx.rollback().unwrap();
    assert_eq!(factory_counts(f), count);
    f.database
        .connection()
        .execute(
            "UPDATE adapter_installation SET enabled=1 WHERE id=?1",
            [&installation_id],
        )
        .unwrap();
    claim_context_child(f, &run);
    // A stable custom target resolves the exact registered custom installation,
    // retaining its real Pi adapter; this is not arbitrary generic ACP support.
    let custom =
        crate::delegation_contract::RuntimeTarget::try_from(format!("custom:{installation_id}"))
            .unwrap();
    let custom_request = crate::delegation::FrozenRequest {
        runtime: &custom,
        ..request
    };
    let custom_plan = ChildRunPlan {
        target: &custom,
        ..plan
    };
    let custom_parent = crate::delegation::ParentInvocation {
        tool_call_id: "factory-custom-stable",
        ..parent
    };
    let tx = f.database.connection_mut().transaction().unwrap();
    let custom_child = factory::register_initial(
        &tx,
        &custom_parent,
        &custom_request,
        &custom_plan,
        "2026-10-17T00:00:00Z",
    )
    .unwrap();
    tx.commit().unwrap();
    let requested: String = f
        .database
        .connection()
        .query_row(
            "SELECT requested_runtime FROM delegation WHERE id=?1",
            [&custom_child.identity.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(requested, custom.as_str());
    let wrong = crate::delegation_contract::RuntimeTarget::try_from("codex".to_string()).unwrap();
    let wrong_plan = ChildRunPlan {
        target: &wrong,
        ..custom_plan
    };
    let count = factory_counts(f);
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        factory::create_child(
            &tx,
            &custom_child.identity,
            &wrong_plan,
            &json!({}),
            "2026-10-17T00:00:00Z"
        )
        .unwrap_err()
        .to_string()
        .contains("target adapter mismatch")
    );
    tx.rollback().unwrap();
    assert_eq!(factory_counts(f), count);
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        factory::register_initial(
            &tx,
            &custom_parent,
            &custom_request,
            &wrong_plan,
            "2026-10-17T00:00:00Z"
        )
        .unwrap_err()
        .to_string()
        .contains("target plan mismatch")
    );
    tx.rollback().unwrap();
    assert_eq!(factory_counts(f), count);
    let other_workspace =
        json!({"executionRoot":f.directory,"access":"write","isolation":"shared"});
    let other_request = crate::delegation::FrozenRequest {
        workspace: &other_workspace,
        ..custom_request
    };
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        factory::register_initial(
            &tx,
            &custom_parent,
            &other_request,
            &custom_plan,
            "2026-10-17T00:00:00Z"
        )
        .unwrap_err()
        .to_string()
        .contains("workspace plan mismatch")
    );
    tx.rollback().unwrap();
    assert_eq!(factory_counts(f), count);
    assert_native_interaction_storage(f);
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=?1,depth_limit=?2 WHERE singleton=1",
            params![old.0, old.1],
        )
        .unwrap();
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
}
