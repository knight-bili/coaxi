// Extends the existing Context materialization owner and the same SQLite/Blob.
// Native ids are explicit fixtures; no native file, process or provider is read.
fn assert_native_child_session_reference(f: &mut Fixture) {
    let source = crate::team_tool::AuthenticatedTeamToolRun {
        camp_id: f.camp_id.clone(),
        agent_id: "agent_1".into(),
        agent_run_id: f.run_id.clone(),
        execution_epoch: f.execution_epoch,
    };
    use crate::{
        execution_evidence::ExecutionEvidenceService,
        runtime_subagent_session::{NativeChildSessionReference, NativeChildSessionService},
    };
    let conversation: (String, String) = f.database.connection().query_row(
        "SELECT c.id,c.native_session_id FROM agent_run r JOIN conversation c ON c.id=r.conversation_id WHERE r.id=?1",
        [&source.agent_run_id], |r| Ok((r.get(0)?,r.get(1)?)),
    ).unwrap();
    let child = format!("native-child-{}", Uuid::new_v4());
    let evidence = ExecutionEvidenceService.record_runtime_event(
        &mut f.database, &ManagedBlobStore::new(&f.directory), &source.agent_run_id,
        source.execution_epoch, "activity.started", &json!({"threadId":conversation.1,"item":{
            "id":format!("native-spawn-{}",Uuid::new_v4()),"type":"collabAgentToolCall","status":"inProgress",
            "senderThreadId":conversation.1,"receiverThreadIds":[child],
            "agentsStates":{&child:{"status":"running","message":"PRIVATE_REPORT_MUST_NOT_ENTER_TARGET"}}
        }}),
    ).unwrap().unwrap();
    let reference = NativeChildSessionReference {
        thread_id: f.camp_id.clone(),
        agent_run_id: source.agent_run_id.clone(),
        evidence_id: evidence.id.clone(),
        receiver_index: 0,
    };
    let user = ActorRef::User {
        user_id: crate::current_user::CURRENT_USER_ID.into(),
    };
    let read = |db: &Database, input: &NativeChildSessionReference, actor: &ActorRef| {
        NativeChildSessionService::resolve(db, &ManagedBlobStore::new(&f.directory), actor, input)
    };
    let changes = f.database.connection().total_changes();
    let target = read(&f.database, &reference, &user).unwrap();
    assert_eq!(target.child_native_session_id, child);
    assert_eq!(target.parent_native_session_id, conversation.1);
    assert_eq!(target.conversation_id, conversation.0);
    assert_eq!(target.agent_run_id, source.agent_run_id);
    assert_eq!(target.execution_epoch, source.execution_epoch);
    assert_eq!(target.relationship, "runtime_reported_receiver");
    assert!(target.live);
    assert!(!format!("{target:?}").contains("PRIVATE_REPORT"));
    for rejected in [
        NativeChildSessionReference {
            thread_id: "other-thread".into(),
            ..reference.clone()
        },
        NativeChildSessionReference {
            agent_run_id: "other-run".into(),
            ..reference.clone()
        },
        NativeChildSessionReference {
            evidence_id: "other-evidence".into(),
            ..reference.clone()
        },
        NativeChildSessionReference {
            receiver_index: 1,
            ..reference.clone()
        },
        NativeChildSessionReference {
            receiver_index: 64,
            ..reference.clone()
        },
    ] {
        assert!(read(&f.database, &rejected, &user).is_err());
    }
    for actor in [
        ActorRef::System {
            component_id: "renderer-is-not-user".into(),
        },
        ActorRef::Agent {
            agent_id: "agent_1".into(),
            source_agent_run_id: source.agent_run_id.clone(),
        },
        ActorRef::User {
            user_id: "other-user".into(),
        },
    ] {
        assert!(read(&f.database, &reference, &actor).is_err());
    }
    for field in ["nativeSessionId", "path", "adapterKind", "nativeBindingId"] {
        let mut input = json!({"threadId":reference.thread_id,"agentRunId":reference.agent_run_id,"evidenceId":reference.evidence_id,"receiverIndex":0});
        input[field] = json!("untrusted-override");
        assert!(serde_json::from_value::<NativeChildSessionReference>(input).is_err());
    }
    assert_eq!(
        f.database.connection().total_changes(),
        changes,
        "reads must not register, attach, or start work"
    );
    // Same database: prove current binding/epoch fences and terminal display,
    // then restore it before the original owner's remaining lifecycle checks.
    for statement in [
        "UPDATE conversation SET native_session_id=NULL WHERE id=?1",
        "UPDATE conversation SET native_binding_generation=native_binding_generation+1 WHERE id=?1",
        "UPDATE conversation SET native_binding_id='different-binding' WHERE id=?1",
        "UPDATE conversation SET native_binding_compatibility_digest='different-config' WHERE id=?1",
    ] {
        f.database
            .connection()
            .execute_batch("SAVEPOINT native_child_reference")
            .unwrap();
        f.database
            .connection()
            .execute(statement, [&conversation.0])
            .unwrap();
        assert!(read(&f.database, &reference, &user).is_err());
        f.database
            .connection()
            .execute_batch("ROLLBACK TO native_child_reference; RELEASE native_child_reference")
            .unwrap();
    }
    f.database
        .connection()
        .execute_batch("SAVEPOINT native_child_epoch")
        .unwrap();
    f.database
        .connection()
        .execute(
            "UPDATE agent_run SET execution_epoch=execution_epoch+1 WHERE id=?1",
            [&source.agent_run_id],
        )
        .unwrap();
    assert!(read(&f.database, &reference, &user).is_err());
    f.database
        .connection()
        .execute_batch("ROLLBACK TO native_child_epoch; RELEASE native_child_epoch")
        .unwrap();
    f.database
        .connection()
        .execute_batch("SAVEPOINT native_child_terminal")
        .unwrap();
    f.database
        .connection()
        .execute(
            "UPDATE agent_run SET status='succeeded',ended_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",
            [&source.agent_run_id],
        )
        .unwrap();
    let terminal = read(&f.database, &reference, &user).unwrap();
    assert_eq!(
        terminal.status, "running",
        "preserve the observed status instead of inventing success"
    );
    assert!(
        !terminal.live,
        "a settled parent must not animate a stale in-progress child forever"
    );
    f.database
        .connection()
        .execute_batch("ROLLBACK TO native_child_terminal; RELEASE native_child_terminal")
        .unwrap();
}
