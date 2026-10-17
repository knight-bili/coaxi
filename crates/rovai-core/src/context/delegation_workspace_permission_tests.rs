// Extends the original Context owner. Real Core approval/claim/cancel and SQLite;
// installation metadata and transport attestation are fixtures, never a live model.
fn assert_delegation_workspace_permission(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    request: &crate::delegation::FrozenRequest<'_>,
    plan: &crate::delegation_factory::ChildRunPlan<'_>,
) {
    use crate::action::{ActionSafetyService, ClaimActionCommand, ResolveActionApprovalCommand};
    use crate::delegation_workspace_permission as permission;
    let service = ActionSafetyService::default();
    for case in ["rollback", "allow", "deny", "cancel", "stale-binding"] {
        let parent = crate::delegation_admission::authenticate_new_child(
            f.database.connection(),
            binding,
            source,
            &format!("directory-permission-{case}"),
        )
        .unwrap();
        let observation = parent.observe_workspace(request.working_dir).unwrap();
        assert!(observation.requires_user_permission());
        let before = factory_counts(f);
        let permission_count = |f: &Fixture| -> (i64, i64) {
            f.database.connection().query_row("SELECT (SELECT count(*) FROM action_execution),(SELECT count(*) FROM approval)",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap()
        };
        let approvals_before = permission_count(f);
        if case == "rollback" {
            let tx = f.database.connection_mut().transaction().unwrap();
            tx.execute_batch("CREATE TEMP TRIGGER fail_workspace_permission BEFORE INSERT ON approval BEGIN SELECT RAISE(ABORT,'permission fixture fault'); END;").unwrap();
            let error = parent
                .register_initial_awaiting_workspace_permission(
                    &tx,
                    request,
                    plan,
                    &observation,
                    "2026-10-17T00:00:00Z",
                )
                .err()
                .unwrap();
            assert!(format!("{error:#}").contains("permission fixture fault"));
            tx.rollback().unwrap();
            assert_eq!(factory_counts(f), before);
            assert_eq!(permission_count(f), approvals_before);
            continue;
        }
        let tx = f.database.connection_mut().transaction().unwrap();
        let registered = parent
            .register_initial_awaiting_workspace_permission(
                &tx,
                request,
                plan,
                &observation,
                "2026-10-17T00:00:00Z",
            )
            .unwrap();
        assert!(!registered.replayed);
        let replay = parent
            .register_initial_awaiting_workspace_permission(
                &tx,
                request,
                plan,
                &observation,
                "2026-10-17T00:00:00Z",
            )
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.identity, registered.identity);
        tx.commit().unwrap();
        assert_eq!(
            permission_count(f),
            (approvals_before.0 + 1, approvals_before.1 + 1)
        );
        let (approval,action,run):(String,String,String)=f.database.connection().query_row(
            "SELECT p.id,x.id,x.agent_run_id FROM delegation_workspace_permission permission
             JOIN action_execution x ON x.id=permission.action_id JOIN approval p ON p.action_id=x.id WHERE permission.delegation_id=?1",
            [&registered.identity.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert!(!permission::covers_run(f.database.connection(), &run).unwrap());
        let private = crate::read_model::load_conversation_approvals(
            f.database.connection(),
            &f.camp_id,
            Some(&registered.identity.child_conversation_id),
            true,
            None,
        )
        .unwrap();
        assert_eq!(private.len(), 1);
        assert_eq!(private[0].id, approval);
        assert_eq!(
            private[0].canonical_input["permissions"]["workspace"],
            serde_json::to_value(plan.workspace).unwrap()
        );
        assert!(private[0].native_method.is_none());
        assert!(private[0].request_digest.is_none());
        assert_eq!(private[0].options.len(), 2);
        assert!(
            crate::read_model::load_conversation_approvals(
                f.database.connection(),
                &f.camp_id,
                None,
                false,
                None
            )
            .unwrap()
            .iter()
            .all(|p| p.id != approval)
        );
        if case == "allow" {
            let snapshot = crate::read_model::ReadModelService
                .camp_snapshot(&mut f.database, &f.camp_id)
                .unwrap();
            assert!(snapshot.approvals.iter().all(|p| p.id != approval));
            assert!(snapshot.actions.iter().all(|a| a.id != action));
            let open = crate::read_model::ReadModelService
                .camp_open_projection(&mut f.database, &f.camp_id)
                .unwrap();
            assert!(open.approvals.iter().all(|p| p.id != approval));
        }
        assert!(
            f.database
                .connection()
                .execute(
                    "DELETE FROM delegation_workspace_permission WHERE delegation_id=?1",
                    [&registered.identity.id]
                )
                .is_err()
        );
        assert!(f.database.connection().execute("UPDATE delegation_workspace_permission SET action_id=action_id WHERE delegation_id=?1",[&registered.identity.id]).is_err());
        assert!(
            ExecutionRuntimeService::default()
                .list_dispatchable_agent_runs(&f.database, 64)
                .unwrap()
                .into_iter()
                .all(|c| c.agent_run_id != run)
        );
        // A stale scheduler candidate cannot bypass the same transaction gate.
        let (turn, config, semantics, version, project_kind, project_path):
            (String,String,String,i64,String,String)=f.database.connection().query_row(
                "SELECT r.camp_turn_id,r.effective_config_json,r.permission_semantics,r.version,c.project_binding_kind,c.project_path
                 FROM agent_run r JOIN camp c ON c.id=?2 WHERE r.id=?1",params![run,f.camp_id],
                 |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap();
        let candidate = crate::runtime::QueuedAgentRunCandidate {
            agent_run_id: run.clone(),
            camp_id: f.camp_id.clone(),
            camp_turn_id: turn,
            conversation_id: registered.identity.child_conversation_id.clone(),
            agent_id: source.agent_id.clone(),
            task_id: None,
            version,
            permission_semantics: serde_json::from_value(json!(semantics)).unwrap(),
            project_binding_kind: project_kind,
            project_path,
            effective_config: serde_json::from_str(&config).unwrap(),
            workspace: Some(plan.workspace.clone()),
        };
        assert!(
            !ExecutionRuntimeService::default()
                .begin_workspace_preparation(&mut f.database, &candidate)
                .unwrap()
        );
        let claim = lifecycle_command(
            f,
            ActorRef::System {
                component_id: "agent-run-scheduler".into(),
            },
            ClaimAgentRunCommand {
                agent_run_id: run.clone(),
                expected_version: candidate.version,
                lease_owner: "workspace-permission-fixture".into(),
                lease_seconds: 60,
                workspace: Some(plan.workspace.clone()),
                starting_git_observation: None,
            },
        );
        assert_eq!(
            ExecutionRuntimeService::default()
                .claim_agent_run(&mut f.database, &claim)
                .unwrap()
                .result
                .code,
            "agent_run.delegation_workspace_permission_required"
        );
        let dispatch = lifecycle_command(
            f,
            ActorRef::System {
                component_id: "action-executor".into(),
            },
            ClaimActionCommand {
                action_id: action.clone(),
                expected_version: 1,
                lease_owner: "not-an-executor-action".into(),
                lease_seconds: 60,
                authorization_delivery_id: None,
                authorization_delivery_lease_owner: None,
            },
        );
        assert_eq!(
            service
                .claim_action(&mut f.database, &dispatch)
                .unwrap()
                .result
                .code,
            "action.core_permission_decision_only"
        );
        let command = ResolveActionApprovalCommand {
            approval_id: approval.clone(),
            option_id: if case == "deny" {
                "core.deny"
            } else {
                "core.allow_once"
            }
            .into(),
            expected_version: 1,
            reason: None,
        };
        for (actor, camp, version, option, expected) in [
            (
                ActorRef::System {
                    component_id: "model-cannot-approve".into(),
                },
                Some(f.camp_id.clone()),
                1,
                "core.allow_once",
                "approval.user_required",
            ),
            (
                ActorRef::User {
                    user_id: "another-user".into(),
                },
                Some(f.camp_id.clone()),
                1,
                "core.allow_once",
                "approval.wrong_user",
            ),
            (
                queue_actor(),
                None,
                1,
                "core.allow_once",
                "approval.camp_mismatch",
            ),
            (
                queue_actor(),
                Some(f.camp_id.clone()),
                2,
                "core.allow_once",
                "approval.version_conflict",
            ),
            (
                queue_actor(),
                Some(f.camp_id.clone()),
                1,
                "allow_session",
                "approval.option_not_found",
            ),
        ] {
            let mut bad = lifecycle_command(f, actor, command.clone());
            bad.camp_id = camp;
            bad.payload.expected_version = version;
            bad.payload.option_id = option.into();
            assert_eq!(
                service
                    .resolve_approval(&mut f.database, &bad)
                    .unwrap()
                    .result
                    .code,
                expected
            );
        }
        let decision = lifecycle_command(f, queue_actor(), command);
        if case == "stale-binding" {
            let tx = f.database.connection_mut().transaction().unwrap();
            tx.execute("UPDATE conversation SET native_binding_generation=native_binding_generation+1 WHERE id=?1",[parent.conversation_id()]).unwrap();
            let fenced = permission::resolve(&tx, &decision).unwrap().unwrap();
            assert_eq!(fenced.code, "approval.runtime_request_fenced");
            tx.rollback().unwrap();
            assert!(!permission::covers_run(f.database.connection(), &run).unwrap());
        }
        if matches!(case, "cancel" | "stale-binding") {
            crate::application::delegation_control::handle(
                &mut f.database,
                "delegations.cancel",
                control_request(&registered.identity.id, "attemptNo", 1),
            )
            .unwrap();
            let status: String = f
                .database
                .connection()
                .query_row(
                    "SELECT status FROM approval WHERE id=?1",
                    [&approval],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(status, "cancelled");
            assert_eq!(
                service
                    .resolve_approval(&mut f.database, &decision)
                    .unwrap()
                    .result
                    .code,
                "approval.not_pending"
            );
            assert!(!permission::covers_run(f.database.connection(), &run).unwrap());
            ExecutionRuntimeService::default()
                .record_runtime_cleanup_completed(&f.database, &run, 0)
                .unwrap();
            continue;
        }
        let approved = service
            .resolve_approval(&mut f.database, &decision)
            .unwrap();
        assert_eq!(approved.result.code, "approval.resolved");
        assert_eq!(
            approved.result.payload["decision"],
            if case == "allow" {
                "approved"
            } else {
                "denied"
            }
        );
        let replay = service
            .resolve_approval(&mut f.database, &decision)
            .unwrap();
        assert_eq!(replay.result, approved.result);
        assert!(replay.replayed);
        if case == "deny" {
            let status: String = f
                .database
                .connection()
                .query_row("SELECT status FROM agent_run WHERE id=?1", [&run], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(status, "cancelled");
            assert!(!permission::covers_run(f.database.connection(), &run).unwrap());
            ExecutionRuntimeService::default()
                .record_runtime_cleanup_completed(&f.database, &run, 0)
                .unwrap();
            continue;
        }
        assert!(permission::covers_run(f.database.connection(), &run).unwrap());
        // A separate SQLite connection observes the same grant; not a complete Host restart.
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
        assert!(permission::covers_run(&reopened, &run).unwrap());
        drop(reopened);
        let deliveries: i64 = f
            .database
            .connection()
            .query_row(
                "SELECT count(*) FROM runtime_delivery_checkpoint WHERE action_id=?1",
                [&action],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(deliveries, 0);
        let candidate = ExecutionRuntimeService::default()
            .list_dispatchable_agent_runs(&f.database, 64)
            .unwrap()
            .into_iter()
            .find(|c| c.agent_run_id == run)
            .unwrap();
        assert!(
            ExecutionRuntimeService::default()
                .begin_workspace_preparation(&mut f.database, &candidate)
                .unwrap()
        );
        let epoch = claim_context_child(f, &run);
        let stop = lifecycle_command(
            f,
            queue_actor(),
            crate::runtime::CancelAgentRunCommand {
                camp_id: f.camp_id.clone(),
                agent_run_id: run.clone(),
                expected_version: lifecycle_version(f, &run),
            },
        );
        ExecutionRuntimeService::default()
            .request_agent_run_cancellation(&mut f.database, &stop)
            .unwrap();
        // No process exists in this fixture; supply only its explicit cleanup fact.
        ExecutionRuntimeService::default()
            .record_runtime_cleanup_completed(&f.database, &run, epoch)
            .unwrap();
    }
}
