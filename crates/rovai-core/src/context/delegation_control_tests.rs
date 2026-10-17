// Existing Context owner: actual control/Run settlement, fixture child factory; no Runtime process.
fn control_request(id: &str, field: &str, value: i64) -> Value {
    let mut command = json!({"delegationId":id});
    command[field] = json!(value);
    json!({"commandId":Uuid::new_v4().to_string(),"command":command})
}
fn control_rows(f: &Fixture) -> Vec<(String, String, i64, Option<String>, Option<String>)> {
    f.database.connection().prepare("SELECT id,status,version,cancel_requested_at,cancel_acknowledged_at FROM agent_run ORDER BY id").unwrap()
        .query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap()
}
fn assert_child_takeover(f: &mut Fixture, id: &str, run: &str) {
    use crate::{application::delegation_control as api, delegation_control as service};
    let previous_settings: (i64, i64) = f
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
    let epoch: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT execution_epoch FROM agent_run WHERE id=?1",
            [run],
            |r| r.get(0),
        )
        .unwrap();
    let (a, a_run, a_epoch) =
        context_child_of(f, run, epoch, "active direct child", "takeover-owned-child");
    let (_, leaf, leaf_epoch) = context_child_of(
        f,
        &a_run,
        a_epoch,
        "active grandchild",
        "takeover-owned-leaf",
    );
    let (independent, independent_run, independent_epoch) =
        context_child_of(f, run, epoch, "independent branch", "takeover-independent");
    let (terminal, terminal_run, terminal_epoch) =
        context_child(f, "terminal parent", "takeover-terminal-parent");
    let (_, background, background_epoch) = context_child_of(
        f,
        &terminal_run,
        terminal_epoch,
        "background grandchild",
        "takeover-terminal-background",
    );
    let (cancel_id, cancel_run, cancel_epoch) =
        context_child(f, "cancel exact attempt", "cancel-exact-root");
    // New fixture registration requires enablement; existing work can still be controlled while disabled.
    f.database
        .connection()
        .execute(
            "UPDATE delegation_settings SET enabled=?1,depth_limit=?2 WHERE singleton=1",
            rusqlite::params![previous_settings.0, previous_settings.1],
        )
        .unwrap();
    let independent_request = control_request(&independent, "expectedControlVersion", 1);
    let switched =
        api::handle(&mut f.database, "delegations.takeover", independent_request).unwrap();
    assert_eq!(switched.result["code"], "delegation.taken_over");
    crate::runtime::ExecutionRuntimeService::default()
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
    let user_request = json!({"commandId":Uuid::new_v4().to_string(),"command":{"conversationId":conversation,"expectedRevision":0,"body":"我接手后的独立工作"}});
    let user_envelope = crate::application::delegation_input::send_envelope(
        &f.database,
        &crate::draft_client::DraftClient::default(),
        user_request,
    )
    .unwrap();
    let user = crate::delegation_user::send(&mut f.database, &user_envelope).unwrap();
    assert_eq!(
        user.result.status,
        crate::command::CommandResultStatus::Accepted,
        "{:?}",
        user.result
    );
    let user_run = user.result.payload["agentRunId"]
        .as_str()
        .unwrap()
        .to_string();
    let protected = control_rows(f)
        .into_iter()
        .find(|r| r.0 == user_run)
        .unwrap();

    let request = control_request(id, "expectedControlVersion", 1);
    for field in ["threadId", "actor", "draftClient", "sourcePath"] {
        let mut wrong = request.clone();
        wrong["command"][field] = json!("forged");
        assert!(api::takeover_envelope(&f.database, wrong).is_err());
    }
    let mut envelope = api::takeover_envelope(
        &f.database,
        control_request(id, "expectedControlVersion", 1),
    )
    .unwrap();
    let rows = control_rows(f);
    envelope.actor = ActorRef::System {
        component_id: "not-user".into(),
    };
    assert_eq!(
        service::takeover(&mut f.database, &envelope)
            .unwrap()
            .result
            .code,
        "delegation.user_required"
    );
    envelope = api::takeover_envelope(
        &f.database,
        control_request(id, "expectedControlVersion", 0),
    )
    .unwrap();
    assert_eq!(
        service::takeover(&mut f.database, &envelope)
            .unwrap()
            .result
            .code,
        "delegation.control_changed"
    );
    envelope = api::takeover_envelope(
        &f.database,
        control_request(id, "expectedControlVersion", 1),
    )
    .unwrap();
    envelope.camp_id = None;
    assert_eq!(
        service::takeover(&mut f.database, &envelope)
            .unwrap()
            .result
            .code,
        "delegation.thread_mismatch"
    );
    let stale = api::cancel_envelope(&f.database, control_request(id, "attemptNo", 2)).unwrap();
    assert_eq!(
        service::cancel(&mut f.database, &stale)
            .unwrap()
            .result
            .code,
        "delegation.attempt_changed"
    );
    assert_eq!(control_rows(f), rows);

    assert_eq!(
        api::reconcile(&f.database, "delegations.takeover", request.clone()).unwrap()["state"],
        "unknown"
    );
    // Fail after Run cancellations, before recording control; everything, including receipt, must roll back.
    f.database.connection().execute_batch(&format!("CREATE TEMP TRIGGER reject_takeover BEFORE UPDATE ON delegation_control WHEN OLD.delegation_id='{id}' BEGIN SELECT RAISE(ABORT,'injected takeover rollback'); END;")).unwrap();
    let failed = api::handle(&mut f.database, "delegations.takeover", request.clone());
    assert!(
        failed
            .err()
            .unwrap()
            .to_string()
            .contains("injected takeover rollback")
    );
    assert_eq!(control_rows(f), rows);
    assert_eq!(
        crate::delegation::control(f.database.connection(), id)
            .unwrap()
            .version,
        1
    );
    assert_eq!(
        api::reconcile(&f.database, "delegations.takeover", request.clone()).unwrap()["state"],
        "unknown"
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER reject_takeover;")
        .unwrap();
    let taken = api::handle(&mut f.database, "delegations.takeover", request.clone()).unwrap();
    assert_eq!(taken.result["code"], "delegation.taken_over");
    assert_eq!(taken.result["payload"]["takeoverMessageSequence"], 4);
    assert_eq!(taken.result["payload"]["controlVersion"], 2);
    assert_eq!(taken.result["payload"]["cleanupPending"], true);
    let mut stopped = taken.result["payload"]["cancelledAgentRunIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    stopped.sort();
    let mut expected = vec![run.to_string(), a_run.clone(), leaf.clone()];
    expected.sort();
    assert_eq!(stopped, expected);
    assert_eq!(
        control_rows(f)
            .into_iter()
            .find(|r| r.0 == user_run)
            .unwrap(),
        protected
    );
    let changes = f.database.connection().total_changes();
    let replay = api::handle(&mut f.database, "delegations.takeover", request.clone()).unwrap();
    assert_eq!(taken.result, replay.result);
    assert!(replay.changed.is_none());
    assert_eq!(
        api::reconcile(&f.database, "delegations.takeover", request).unwrap()["result"],
        taken.result
    );
    assert_eq!(changes, f.database.connection().total_changes());
    assert!(service::cleanup_pending(f.database.connection(), id).unwrap());
    for (child, epoch) in [(&a_run, a_epoch), (&leaf, leaf_epoch)] {
        crate::runtime::ExecutionRuntimeService::default()
            .record_runtime_cleanup_completed(&f.database, child, epoch)
            .unwrap();
    }
    // The original run is deliberately still unacknowledged: the existing User-send test must reject it.
    assert!(service::cleanup_pending(f.database.connection(), id).unwrap());
    let no_op = api::handle(
        &mut f.database,
        "delegations.cancel",
        control_request(&a, "attemptNo", 1),
    )
    .unwrap();
    assert_eq!(no_op.result["code"], "delegation.already_terminal");

    // Explicit terminal fixture: this is not a real provider completion.
    f.database
        .connection()
        .execute(
            "UPDATE agent_run SET status='succeeded',ended_at='2026-10-17T00:00:04Z' WHERE id=?1",
            [&terminal_run],
        )
        .unwrap();
    let before = control_rows(f);
    let cancelled = api::handle(
        &mut f.database,
        "delegations.cancel",
        control_request(&terminal, "attemptNo", 1),
    )
    .unwrap();
    assert_eq!(cancelled.result["payload"]["status"], "succeeded");
    assert_eq!(control_rows(f), before);
    let takeover = api::handle(
        &mut f.database,
        "delegations.takeover",
        control_request(&terminal, "expectedControlVersion", 1),
    )
    .unwrap();
    assert_eq!(takeover.result["payload"]["status"], "succeeded");
    assert_eq!(
        takeover.result["payload"]["cancelledAgentRunIds"],
        json!([background])
    );
    crate::runtime::ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &background, background_epoch)
        .unwrap();

    let cancellation = control_request(&cancel_id, "attemptNo", 1);
    let cancelled =
        api::handle(&mut f.database, "delegations.cancel", cancellation.clone()).unwrap();
    assert_eq!(cancelled.result["code"], "delegation.cancelled");
    assert_eq!(
        cancelled.result["payload"]["cancelledAgentRunIds"],
        json!([cancel_run])
    );
    assert_eq!(
        api::reconcile(&f.database, "delegations.cancel", cancellation).unwrap()["result"],
        cancelled.result
    );
    crate::runtime::ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &cancel_run, cancel_epoch)
        .unwrap();
}
