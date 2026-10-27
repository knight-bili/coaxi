// One existing Core owner and database. Child creation is still a fixture; Runtime signals are synthetic.
fn lifecycle_version(f: &Fixture, run: &str) -> i64 {
    f.database
        .connection()
        .query_row("SELECT version FROM agent_run WHERE id=?1", [run], |r| {
            r.get(0)
        })
        .unwrap()
}
fn lifecycle_status(f: &Fixture, run: &str) -> String {
    f.database
        .connection()
        .query_row("SELECT status FROM agent_run WHERE id=?1", [run], |r| {
            r.get(0)
        })
        .unwrap()
}
fn lifecycle_command<T>(f: &Fixture, actor: ActorRef, payload: T) -> CommandEnvelope<T> {
    CommandEnvelope {
        command_id: Uuid::new_v4().to_string(),
        actor,
        camp_id: Some(f.camp_id.clone()),
        expected_versions: vec![],
        execution_epoch: None,
        payload,
    }
}
fn lifecycle_succeed(f: &mut Fixture, run: &str, epoch: i64) {
    let public_before: i64 = f
        .database
        .connection()
        .query_row("SELECT count(*) FROM camp_message", [], |r| r.get(0))
        .unwrap();
    let command = lifecycle_command(
        f,
        ActorRef::System {
            component_id: "runtime-adapter:fixture".into(),
        },
        crate::runtime::SucceedAgentRunCommand {
            agent_run_id: run.into(),
            expected_version: lifecycle_version(f, run),
            execution_epoch: epoch,
            native_turn_id: format!("native-{run}"),
            final_output: "只属于本子会话的完成结果".into(),
            missing_send_recovery_candidate: None,
            ending_git_observation: None,
        },
    );
    let result = ExecutionRuntimeService::default()
        .succeed_agent_run(&mut f.database, &command)
        .unwrap();
    assert_eq!(
        result.result.code, "agent_run.succeeded",
        "{:?}",
        result.result
    );
    assert_eq!(
        public_before,
        f.database
            .connection()
            .query_row("SELECT count(*) FROM camp_message", [], |r| r
                .get::<_, i64>(0))
            .unwrap()
    );
    let (conversation,final_conversation,final_public):(String,String,Option<String>)=f.database.connection().query_row(
        "SELECT r.conversation_id,m.conversation_id,r.final_camp_message_id FROM agent_run r JOIN conversation_message m ON m.id=r.final_conversation_message_id WHERE r.id=?1",[run],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(conversation, final_conversation);
    assert!(final_public.is_none());
    let changes = f.database.connection().total_changes();
    assert!(
        ExecutionRuntimeService::default()
            .succeed_agent_run(&mut f.database, &command)
            .unwrap()
            .replayed
    );
    assert_eq!(changes, f.database.connection().total_changes());
}
fn assert_delegation_parent_cancellation(f: &mut Fixture, private_parent: &str) {
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
    let (_, normal, normal_epoch) = context_child(f, "normal parent", "causal-normal");
    let (_, normal_child, _) = context_child_of(
        f,
        &normal,
        normal_epoch,
        "normal background",
        "causal-normal-background",
    );
    let (_, failed, failed_epoch) = context_child(f, "failed parent", "causal-failed");
    let (_, failed_child, _) = context_child_of(
        f,
        &failed,
        failed_epoch,
        "failed child",
        "causal-failed-child",
    );
    let private_epoch: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT execution_epoch FROM agent_run WHERE id=?1",
            [private_parent],
            |r| r.get(0),
        )
        .unwrap();
    let (_, closed, closed_epoch) = context_child_of(
        f,
        private_parent,
        private_epoch,
        "private parent child",
        "causal-closed",
    );
    let (_, closed_child, _) = context_child_of(
        f,
        &closed,
        closed_epoch,
        "private background",
        "causal-closed-background",
    );

    // Admit a real later User Run as parent. Its children are outside the old attempt tree.
    let user_parent:String=f.database.connection().query_row("SELECT r.id FROM agent_run r JOIN conversation c ON c.id=r.conversation_id
        JOIN delegation d ON d.child_conversation_id=c.id JOIN delegation_control control ON control.delegation_id=d.id
        WHERE r.invocation_kind='delegation_user' AND r.status='queued' AND control.takeover_at IS NOT NULL ORDER BY r.created_at DESC,r.id LIMIT 1",[],|r|r.get(0)).unwrap();
    let user_epoch = claim_context_child(f, &user_parent);
    assert_child_user_end(f, &user_parent, user_epoch);
    let (_, member_parent, member_epoch) = context_child_of(
        f,
        &user_parent,
        user_epoch,
        "new User background",
        "causal-member",
    );
    let (_, member_child, _) = context_child_of(
        f,
        &member_parent,
        member_epoch,
        "member background grandchild",
        "causal-member-background",
    );
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=?1,depth_limit=?2 WHERE singleton=1",
            params![previous.0, previous.1],
        )
        .unwrap();

    lifecycle_succeed(f, &normal, normal_epoch);
    lifecycle_succeed(f, &closed, closed_epoch);
    lifecycle_succeed(f, &member_parent, member_epoch);
    assert_eq!(lifecycle_status(f, &normal_child), "running");
    assert_eq!(lifecycle_status(f, &closed_child), "running");
    assert_eq!(lifecycle_status(f, &member_child), "running");

    let failure = lifecycle_command(
        f,
        ActorRef::System {
            component_id: "runtime-adapter:fixture".into(),
        },
        crate::runtime::FailAgentRunCommand {
            agent_run_id: failed.clone(),
            expected_version: lifecycle_version(f, &failed),
            execution_epoch: failed_epoch,
            error_code: "synthetic_parent_failure".into(),
            error_detail: None,
            failure: None,
            manual_retry_allowed: false,
            ending_git_observation: None,
        },
    );
    let before = control_rows(f);
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        crate::delegation_control::cancel_for_parent_run(
            &tx,
            &failed,
            failed_epoch + 1,
            &queue_actor(),
            "stale_parent_epoch",
            "2026-10-17T00:00:00Z"
        )
        .unwrap()
        .is_empty()
    );
    tx.rollback().unwrap();
    assert_eq!(control_rows(f), before);
    f.database.connection().execute_batch(&format!("CREATE TEMP TRIGGER reject_causal_stop BEFORE UPDATE OF status ON agent_run WHEN OLD.id='{failed_child}' AND NEW.status='cancelled' BEGIN SELECT RAISE(ABORT,'injected causal rollback'); END;")).unwrap();
    assert!(
        ExecutionRuntimeService::default()
            .fail_agent_run(&mut f.database, &failure)
            .unwrap_err()
            .to_string()
            .contains("injected causal rollback")
    );
    assert_eq!(control_rows(f), before);
    assert!(
        crate::command::DomainCommandGateway
            .replay_if_recorded(&f.database, &failure)
            .unwrap()
            .is_none()
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER reject_causal_stop;")
        .unwrap();
    let failed_result = ExecutionRuntimeService::default()
        .fail_agent_run(&mut f.database, &failure)
        .unwrap();
    assert_eq!(failed_result.result.code, "agent_run.failed");
    assert_eq!(lifecycle_status(f, &failed_child), "cancelled");
    assert_eq!(lifecycle_status(f, &normal_child), "running");

    let protected = control_rows(f)
        .into_iter()
        .filter(|r| r.0 == user_parent || r.0 == member_child)
        .collect::<Vec<_>>();
    assert_eq!(protected.len(), 2);
    let root = f.run_id.clone();
    let stop = lifecycle_command(
        f,
        queue_actor(),
        crate::runtime::CancelAgentRunCommand {
            camp_id: f.camp_id.clone(),
            agent_run_id: root.clone(),
            expected_version: lifecycle_version(f, &root),
        },
    );
    let stopped = ExecutionRuntimeService::default()
        .request_agent_run_cancellation(&mut f.database, &stop)
        .unwrap();
    assert_eq!(stopped.result.code, "agent_run.cancelled");
    assert_eq!(lifecycle_status(f, &normal_child), "cancelled");
    assert_eq!(lifecycle_status(f, &closed_child), "running");
    assert_eq!(
        control_rows(f)
            .into_iter()
            .filter(|r| r.0 == user_parent || r.0 == member_child)
            .collect::<Vec<_>>(),
        protected
    );

    let conversation: String = f
        .database
        .connection()
        .query_row(
            "SELECT conversation_id FROM agent_run WHERE id=?1",
            [private_parent],
            |r| r.get(0),
        )
        .unwrap();
    let end = lifecycle_command(
        f,
        queue_actor(),
        crate::single_chat::EndSingleChatCommand {
            camp_id: f.camp_id.clone(),
            conversation_id: conversation,
        },
    );
    assert_eq!(
        crate::single_chat::SingleChatService::default()
            .end(&mut f.database, &end)
            .unwrap()
            .result
            .code,
        "single_chat.ended"
    );
    assert_eq!(lifecycle_status(f, &closed_child), "cancelled");
    assert_eq!(lifecycle_status(f, &member_child), "running");

    assert_delegation_restart_settlement(f, &user_parent, user_epoch);

    // The base Context fixture has one current member. Add a real replacement before removal.
    let peer = AgentProfileService::default()
        .get_profile(&f.database, "agent_2")
        .unwrap()
        .unwrap();
    let mut presence = lifecycle_command(
        f,
        queue_actor(),
        crate::agent_profile::SetMemberPresenceCommand {
            agent_id: "agent_2".into(),
            expected_version: peer.version,
            presence: "present".into(),
        },
    );
    presence.camp_id = None;
    assert_ne!(
        AgentProfileService::default()
            .set_presence(&mut f.database, &presence)
            .unwrap()
            .result
            .status,
        crate::command::CommandResultStatus::Rejected
    );
    let generation: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT membership_generation FROM camp WHERE id=?1",
            [&f.camp_id],
            |r| r.get(0),
        )
        .unwrap();
    let add = lifecycle_command(
        f,
        queue_actor(),
        crate::collaboration::AddThreadMemberCommand {
            camp_id: f.camp_id.clone(),
            agent_id: "agent_2".into(),
            expected_membership_generation: generation,
            capability_overrides: json!({}),
            source: None,
        },
    );
    assert_eq!(
        CollaborationService::default()
            .add_camp_member(&mut f.database, &add)
            .unwrap()
            .result
            .code,
        "camp.member_added"
    );
    let (generation,version):(i64,i64)=f.database.connection().query_row("SELECT c.membership_generation,m.version FROM camp c JOIN camp_member m ON m.camp_id=c.id WHERE c.id=?1 AND m.agent_id='agent_1'",[&f.camp_id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    let removal = lifecycle_command(
        f,
        queue_actor(),
        crate::collaboration::RemoveThreadMemberCommand {
            camp_id: f.camp_id.clone(),
            agent_id: "agent_1".into(),
            expected_membership_generation: generation,
            expected_membership_version: version,
            replacement_default_lead_agent_id: None,
            reason: Some("fixture member removal".into()),
            source: None,
        },
    );
    let removed = CollaborationService::default()
        .remove_camp_member(&mut f.database, &removal)
        .unwrap();
    assert_eq!(
        removed.result.code, "camp.member_removed",
        "{:?}",
        removed.result
    );
    assert_eq!(lifecycle_status(f, &member_child), "cancelled");
    assert_eq!(lifecycle_status(f, &user_parent), "cancelled");

    // Aggregate deletion is checked in the same candidate database, without pretending process cleanup.
    let before = control_rows(f);
    let private_inputs: i64 = f.database.connection().query_row(
        "SELECT count(*) FROM single_chat_pending_input WHERE state='published' AND conversation_id IN (SELECT id FROM conversation WHERE camp_id=?1)",
        [&f.camp_id], |r|r.get(0)).unwrap();
    assert!(
        private_inputs > 0,
        "published private references must exercise deletion ordering"
    );
    let count: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT count(*) FROM delegation WHERE camp_id=?1",
            [&f.camp_id],
            |r| r.get(0),
        )
        .unwrap();
    f.database.connection().execute_batch("CREATE TEMP TRIGGER reject_causal_delete BEFORE DELETE ON conversation BEGIN SELECT RAISE(ABORT,'injected deletion rollback'); END;").unwrap();
    let tx = f.database.connection_mut().transaction().unwrap();
    let failed = crate::collaboration::delete_camp_aggregate(&tx, &f.camp_id);
    let error = format!("{:#}", failed.unwrap_err());
    assert!(error.contains("injected deletion rollback"), "{error}");
    tx.rollback().unwrap();
    assert_eq!(control_rows(f), before);
    assert_eq!(f.database.connection().query_row(
        "SELECT count(*) FROM single_chat_pending_input WHERE state='published' AND conversation_id IN (SELECT id FROM conversation WHERE camp_id=?1)",
        [&f.camp_id], |r|r.get::<_,i64>(0)).unwrap(), private_inputs);
    assert_eq!(
        count,
        f.database
            .connection()
            .query_row(
                "SELECT count(*) FROM delegation WHERE camp_id=?1",
                [&f.camp_id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap()
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER reject_causal_delete;")
        .unwrap();
    let tx = f.database.connection_mut().transaction().unwrap();
    crate::collaboration::delete_camp_aggregate(&tx, &f.camp_id).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        f.database
            .connection()
            .query_row(
                "SELECT count(*) FROM delegation WHERE camp_id=?1",
                [&f.camp_id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
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
}
