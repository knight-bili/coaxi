// Existing owner: actual credential/attested scope and factory; no agent process or permission grant.
fn assert_parent_start_admission(f: &mut Fixture, legacy_private_run: &str) {
    use crate::{
        delegation_admission::authenticate_new_child,
        delegation_contract::{RuntimeTarget, WorkingDirectory},
        delegation_factory::ChildRunPlan,
        team_tool::AuthenticatedTeamToolRun,
    };
    let previous: (i64, i64) = f
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
    let actual_root = f.directory.join("实际运行目录");
    let nested = actual_root.join("nested");
    let outside = f.directory.join("需要另行授权");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let root = f.run_id.clone();
    let root_epoch = f.execution_epoch;
    let (id, run, epoch) = context_child_in(
        f,
        &root,
        root_epoch,
        "鉴权父任务",
        "admission-parent",
        &json!({"executionRoot":actual_root,"access":"read_only","isolation":"shared"}),
    );
    let binding = TeamToolService::default()
        .prepare_binding_credential(&mut f.database, &run, epoch, false)
        .unwrap();
    let source = AuthenticatedTeamToolRun {
        camp_id: f.camp_id.clone(),
        agent_id: "agent_1".into(),
        agent_run_id: run.clone(),
        execution_epoch: epoch,
    };
    let parent = authenticate_new_child(
        f.database.connection(),
        &binding,
        &source,
        "admission-actual-start",
    )
    .unwrap();
    let default = parent.observe_workspace(None).unwrap();
    assert_eq!(
        std::path::Path::new(&default.authorized_workspace().unwrap().execution_root),
        actual_root.canonicalize().unwrap()
    );
    assert_ne!(
        std::path::Path::new(&default.candidate().execution_root),
        f.directory.canonicalize().unwrap()
    );
    assert_eq!(default.authorized_workspace().unwrap().access, "read_only");
    assert_eq!(parent.child_policy().depth_limit, 8);
    let mut wrong = binding.clone();
    wrong.binding_credential.push_str("-forged");
    assert!(
        authenticate_new_child(
            f.database.connection(),
            &wrong,
            &source,
            "admission-actual-start"
        )
        .is_err()
    );
    let mut wrong = binding.clone();
    wrong.native_binding_generation += 1;
    assert!(
        authenticate_new_child(
            f.database.connection(),
            &wrong,
            &source,
            "admission-actual-start"
        )
        .is_err()
    );
    let mut wrong = source.clone();
    wrong.execution_epoch += 1;
    assert!(
        authenticate_new_child(
            f.database.connection(),
            &binding,
            &wrong,
            "admission-actual-start"
        )
        .is_err()
    );
    let mut wrong = source.clone();
    wrong.agent_id = "agent_2".into();
    assert!(
        authenticate_new_child(
            f.database.connection(),
            &binding,
            &wrong,
            "admission-actual-start"
        )
        .is_err()
    );
    for sql in [
        "UPDATE delegation_settings SET enabled=0 WHERE singleton=1",
        "UPDATE delegation_settings SET depth_limit=1 WHERE singleton=1",
        "UPDATE delegation_settings SET per_runtime_defaults='{\"codex\":{\"mode_id\":\"read-only\"}}' WHERE singleton=1",
        "UPDATE agent_profile SET profile_status='away' WHERE id='agent_1'",
        "UPDATE camp_member SET version=version+1 WHERE agent_id='agent_1'",
    ] {
        let tx = f.database.connection_mut().transaction().unwrap();
        tx.execute(sql, []).unwrap();
        assert!(parent.revalidate(&tx).is_err(), "{sql}");
        tx.rollback().unwrap();
        parent.revalidate(f.database.connection()).unwrap();
    }
    // A valid old private credential is still not new delegation eligibility.
    let legacy_epoch: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT execution_epoch FROM agent_run WHERE id=?1",
            [legacy_private_run],
            |r| r.get(0),
        )
        .unwrap();
    let legacy = TeamToolService::default()
        .prepare_binding_credential(&mut f.database, legacy_private_run, legacy_epoch, false)
        .unwrap();
    let legacy_source = AuthenticatedTeamToolRun {
        agent_run_id: legacy_private_run.into(),
        execution_epoch: legacy_epoch,
        ..source.clone()
    };
    assert!(
        authenticate_new_child(
            f.database.connection(),
            &legacy,
            &legacy_source,
            "legacy-cannot-gain-tools"
        )
        .err()
        .unwrap()
        .to_string()
        .contains("frozen_policy_required")
    );
    let requested = WorkingDirectory::try_from(nested.to_string_lossy().into_owned()).unwrap();
    let observed = parent.observe_workspace(Some(&requested)).unwrap();
    assert!(!observed.requires_user_permission());
    let external = WorkingDirectory::try_from(outside.to_string_lossy().into_owned()).unwrap();
    let denied = parent.observe_workspace(Some(&external)).unwrap();
    assert!(denied.requires_user_permission());
    assert!(
        denied
            .authorized_workspace()
            .unwrap_err()
            .to_string()
            .contains("workspace_permission_required")
    );
    let traversal = WorkingDirectory::try_from(
        actual_root
            .join("..")
            .join("需要另行授权")
            .to_string_lossy()
            .into_owned(),
    )
    .unwrap();
    assert!(
        parent
            .observe_workspace(Some(&traversal))
            .unwrap()
            .requires_user_permission()
    );
    let prefix = f.directory.join("实际运行目录-旁支");
    std::fs::create_dir(&prefix).unwrap();
    assert!(
        parent
            .observe_workspace(Some(
                &WorkingDirectory::try_from(prefix.to_string_lossy().into_owned()).unwrap()
            ))
            .unwrap()
            .requires_user_permission()
    );
    let file = actual_root.join("not-a-directory");
    std::fs::write(&file, b"fixture").unwrap();
    assert!(
        parent
            .observe_workspace(Some(
                &WorkingDirectory::try_from(file.to_string_lossy().into_owned()).unwrap()
            ))
            .is_err()
    );
    assert!(
        parent
            .observe_workspace(Some(
                &WorkingDirectory::try_from(
                    actual_root.join("missing").to_string_lossy().into_owned()
                )
                .unwrap()
            ))
            .is_err()
    );
    let link = actual_root.join("outside-link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let escape = |p: &std::path::Path| {
            let text = p.to_string_lossy();
            let ordinary = if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
                format!(r"\\{unc}")
            } else {
                text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
            };
            ordinary.replace('\'', "''")
        };
        let status=std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",
            &format!("New-Item -ItemType Junction -Path '{}' -Target '{}' -ErrorAction Stop | Out-Null",escape(&link),escape(&outside))])
            .creation_flags(0x08000000).stdout(std::process::Stdio::null()).status().unwrap();
        assert!(status.success(), "temporary junction creation");
    }
    assert_eq!(
        link.canonicalize().unwrap(),
        outside.canonicalize().unwrap()
    );
    assert!(
        parent
            .observe_workspace(Some(
                &WorkingDirectory::try_from(link.to_string_lossy().into_owned()).unwrap()
            ))
            .unwrap()
            .requires_user_permission()
    );
    #[cfg(windows)]
    std::fs::remove_dir(&link).unwrap();
    #[cfg(unix)]
    std::fs::remove_file(&link).unwrap();
    assert!(
        outside.is_dir(),
        "removing the link must preserve its target"
    );
    let task = "只使用本次独立任务和获准目录";
    let blob = ManagedBlobStore::new(&f.directory)
        .put_bytes(&mut f.database, task.as_bytes(), "text/plain", "sensitive")
        .unwrap();
    let value: String = f
        .database
        .connection()
        .query_row(
            "SELECT effective_config_json FROM agent_run WHERE id=?1",
            [&run],
            |r| r.get(0),
        )
        .unwrap();
    let config: Value = serde_json::from_str(&value).unwrap();
    let runtime: crate::agent_profile::FrozenAgentRuntimeConfig =
        serde_json::from_value(config["runtime"].clone()).unwrap();
    let configuration = crate::agent_profile::MemberRuntimeConfiguration {
        adapter_kind: runtime.adapter_kind,
        model: crate::agent_profile::ModelSelection::RuntimeDefault,
        permissions: runtime.permissions.clone(),
    };
    let target = RuntimeTarget::try_from("codex".to_string()).unwrap();
    let workspace = serde_json::to_value(observed.authorized_workspace().unwrap()).unwrap();
    let preset = json!({});
    let request = crate::delegation::FrozenRequest {
        task_blob_id: &blob.id,
        runtime: &target,
        working_dir: Some(&requested),
        workspace: &workspace,
        runtime_preset: &preset,
    };
    let plan = ChildRunPlan {
        task,
        target: &target,
        installation_id: &runtime.installation_id,
        configuration: &configuration,
        workspace: observed.authorized_workspace().unwrap(),
        policy: parent.child_policy(),
    };
    let before = factory_counts(f);
    let other = authenticate_new_child(
        f.database.connection(),
        &binding,
        &source,
        "another-request",
    )
    .unwrap();
    let foreign = other.observe_workspace(Some(&requested)).unwrap();
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        parent
            .register_initial_within_parent(&tx, &request, &plan, &foreign, "2026-10-17T00:00:00Z")
            .err()
            .unwrap()
            .to_string()
            .contains("workspace_owner_mismatch")
    );
    tx.rollback().unwrap();
    assert_eq!(factory_counts(f), before);
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        parent
            .register_initial_within_parent(&tx, &request, &plan, &default, "2026-10-17T00:00:00Z")
            .is_err()
    );
    tx.rollback().unwrap();
    assert_eq!(factory_counts(f), before);
    let mut widened = observed.candidate().clone();
    widened.access = "write".into();
    let widened_plan = ChildRunPlan {
        workspace: &widened,
        ..plan
    };
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        parent
            .register_initial_within_parent(
                &tx,
                &request,
                &widened_plan,
                &observed,
                "2026-10-17T00:00:00Z"
            )
            .is_err()
    );
    tx.rollback().unwrap();
    assert_eq!(factory_counts(f), before);
    let external_workspace = serde_json::to_value(denied.candidate()).unwrap();
    let external_request = crate::delegation::FrozenRequest {
        working_dir: Some(&external),
        workspace: &external_workspace,
        ..request
    };
    let external_plan = ChildRunPlan {
        workspace: denied.candidate(),
        ..plan
    };
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        parent
            .register_initial_within_parent(
                &tx,
                &external_request,
                &external_plan,
                &denied,
                "2026-10-17T00:00:00Z"
            )
            .err()
            .unwrap()
            .to_string()
            .contains("workspace_permission_required")
    );
    tx.rollback().unwrap();
    assert_eq!(factory_counts(f), before);
    let tx = f.database.connection_mut().transaction().unwrap();
    let registered = parent
        .register_initial_within_parent(&tx, &request, &plan, &observed, "2026-10-17T00:00:00Z")
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(
        registered.identity.parent_conversation_id,
        parent.conversation_id()
    );
    assert_eq!(registered.identity.depth, 2);
    let stored: Value = serde_json::from_str(
        &f.database
            .connection()
            .query_row(
                "SELECT resolved_workspace_json FROM delegation WHERE id=?1",
                [&registered.identity.id],
                |r| r.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stored, workspace);
    let child_run: String = f
        .database
        .connection()
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [&registered.identity.id],
            |r| r.get(0),
        )
        .unwrap();
    let candidate = ExecutionRuntimeService::default()
        .list_dispatchable_agent_runs(&f.database, 8)
        .unwrap()
        .into_iter()
        .find(|c| c.agent_run_id == child_run)
        .unwrap();
    assert_eq!(
        candidate.execution_workspace(),
        *observed.authorized_workspace().unwrap()
    );
    assert!(
        ExecutionRuntimeService::default()
            .begin_workspace_preparation(&mut f.database, &candidate)
            .unwrap()
    );
    let child_epoch = claim_context_child(f, &child_run);
    // Actual takeover fences the old source; the new User Run has its own authenticated authority.
    crate::application::delegation_control::handle(
        &mut f.database,
        "delegations.takeover",
        control_request(&id, "expectedControlVersion", 1),
    )
    .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &run, epoch)
        .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &child_run, child_epoch)
        .unwrap();
    assert!(parent.revalidate(f.database.connection()).is_err());
    let user = child_end_send(f, parent.conversation_id(), "接管后的新任务");
    let user_run = user["agentRunId"].as_str().unwrap();
    let user_epoch = claim_context_child(f, user_run);
    let user_binding = TeamToolService::default()
        .prepare_binding_credential(&mut f.database, user_run, user_epoch, false)
        .unwrap();
    let user_source = AuthenticatedTeamToolRun {
        agent_run_id: user_run.into(),
        execution_epoch: user_epoch,
        ..source
    };
    let user_parent = authenticate_new_child(
        f.database.connection(),
        &user_binding,
        &user_source,
        "user-own-child",
    )
    .unwrap();
    let user_observed = user_parent.observe_workspace(None).unwrap();
    assert_eq!(
        user_observed.candidate().execution_root,
        actual_root.canonicalize().unwrap().to_string_lossy()
    );
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
    assert!(user_parent.revalidate(f.database.connection()).is_err());
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=?1,depth_limit=?2 WHERE singleton=1",
            params![previous.0, previous.1],
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
