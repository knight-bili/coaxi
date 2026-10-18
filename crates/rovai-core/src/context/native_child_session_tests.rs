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
    assert_native_source_origin(f);
}

fn assert_native_source_origin(f: &mut Fixture) {
    use crate::runtime_subagent_session::{
        CapturedNativeSource, NativeChildSessionReference, NativeChildSessionService,
    };
    let root = f.directory.join("ORIGIN_ONLY_NOT_MODEL");
    let mut environment = [
        (
            std::ffi::OsString::from("CODEX_HOME"),
            root.clone().into_os_string(),
        ),
        (
            std::ffi::OsString::from("GROK_HOME"),
            root.clone().into_os_string(),
        ),
        (
            std::ffi::OsString::from("API_TOKEN"),
            std::ffi::OsString::from("NEVER_SAVE_OR_SEND_ORIGIN_SECRET"),
        ),
    ]
    .into_iter()
    .collect();
    let captured = CapturedNativeSource::capture(
        AdapterKind::CodexCli,
        &environment,
        &f.directory,
        "host-origin-fixture",
    )
    .unwrap();
    environment.insert(
        std::ffi::OsString::from("CODEX_HOME"),
        f.directory.join("later-environment").into_os_string(),
    );
    assert!(!format!("{captured:?}").contains("ORIGIN_ONLY_NOT_MODEL"));
    let runtime = ExecutionRuntimeService::default();
    let execution = runtime
        .load_agent_run_execution(&f.database, &f.run_id, f.execution_epoch)
        .unwrap()
        .unwrap();
    let command = CommandEnvelope {
        command_id: Uuid::new_v4().to_string(),
        actor: ActorRef::System {
            component_id: "runtime-adapter:codex-cli".into(),
        },
        camp_id: Some(f.camp_id.clone()),
        expected_versions: Vec::new(),
        execution_epoch: None,
        payload: BindNativeSessionCommand {
            conversation_id: execution.conversation_id.clone(),
            agent_run_id: f.run_id.clone(),
            expected_conversation_version: execution.conversation_version,
            expected_execution_epoch: f.execution_epoch,
            previous_adapter_installation_id: execution.native_adapter_installation_id.clone(),
            previous_native_session_id: execution.native_session_id.clone(),
            previous_binding_compatibility_digest: execution
                .native_binding_compatibility_digest
                .clone(),
            proposed_binding_id: Some(f.native_binding_id.clone()),
            adapter_installation_id: execution.runtime.installation_id.clone(),
            native_session_id: execution.native_session_id.clone().unwrap(),
            binding_compatibility_digest: execution.runtime.binding_compatibility_digest.clone(),
        },
    };
    let applied = runtime
        .bind_native_session_with_source(&mut f.database, &command, Some(&captured))
        .unwrap();
    assert_eq!(applied.result.status, CommandResultStatus::Applied);
    assert!(
        !applied
            .result
            .payload
            .to_string()
            .contains("ORIGIN_ONLY_NOT_MODEL")
    );
    let source_rows = |db: &Database| -> Vec<String> {
        db.connection().prepare("SELECT payload_json FROM event_log WHERE event_type='conversation.native_source.recorded' ORDER BY rowid").unwrap()
            .query_map([],|r|r.get(0)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap()
    };
    let saved = source_rows(&f.database);
    assert_eq!(saved.len(), 1);
    assert!(!saved[0].contains("NEVER_SAVE_OR_SEND_ORIGIN_SECRET"));
    assert!(!saved[0].contains("API_TOKEN"));
    let mut duplicate = command.clone();
    duplicate.command_id = Uuid::new_v4().to_string();
    assert_eq!(
        runtime
            .bind_native_session_with_source(&mut f.database, &duplicate, Some(&captured))
            .unwrap()
            .result
            .status,
        CommandResultStatus::Applied
    );
    assert_eq!(
        source_rows(&f.database),
        saved,
        "warm re-observation does not duplicate provenance"
    );
    assert!(
        runtime
            .bind_native_session_with_source(&mut f.database, &command, Some(&captured))
            .unwrap()
            .replayed
    );
    let other = CapturedNativeSource::capture(
        AdapterKind::GrokBuild,
        &environment,
        &f.directory,
        "host-wrong-kind",
    )
    .unwrap();
    let mut wrong = command.clone();
    wrong.command_id = Uuid::new_v4().to_string();
    assert_eq!(
        runtime
            .bind_native_session_with_source(&mut f.database, &wrong, Some(&other))
            .unwrap()
            .result
            .status,
        CommandResultStatus::Rejected
    );
    assert_eq!(source_rows(&f.database), saved);

    let other_host = CapturedNativeSource::capture(
        AdapterKind::CodexCli,
        &environment,
        &f.directory,
        "host-conflicting-origin",
    )
    .unwrap();
    f.database.connection().execute_batch("CREATE TEMP TRIGGER fail_native_origin BEFORE INSERT ON event_log WHEN NEW.event_type='conversation.native_source.recorded' BEGIN SELECT RAISE(ABORT,'origin fault'); END;").unwrap();
    let mut failed = command.clone();
    failed.command_id = Uuid::new_v4().to_string();
    failed.payload.native_session_id = "must-rollback-native-session".into();
    assert!(
        runtime
            .bind_native_session_with_source(&mut f.database, &failed, Some(&other_host))
            .is_err()
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER fail_native_origin")
        .unwrap();
    assert_eq!(source_rows(&f.database), saved);
    let retained: String = f
        .database
        .connection()
        .query_row(
            "SELECT native_session_id FROM conversation WHERE id=?1",
            [&execution.conversation_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        retained, command.payload.native_session_id,
        "origin failure rolls back the binding too"
    );
    assert_eq!(f.database.connection().query_row("SELECT COUNT(*) FROM event_log WHERE event_type='command.result' AND command_id=?1",[&failed.command_id],|r|r.get::<_,i64>(0)).unwrap(),0);

    let child = format!("origin-child-{}", Uuid::new_v4());
    let evidence=crate::execution_evidence::ExecutionEvidenceService.record_runtime_event(&mut f.database,&ManagedBlobStore::new(&f.directory),&f.run_id,f.execution_epoch,"activity.started",
        &json!({"threadId":retained,"item":{"id":"origin-spawn","type":"collabAgentToolCall","senderThreadId":retained,"receiverThreadIds":[child],"agentsStates":{&child:{"status":"running"}}}})).unwrap().unwrap();
    let reference = NativeChildSessionReference {
        thread_id: f.camp_id.clone(),
        agent_run_id: f.run_id.clone(),
        evidence_id: evidence.id.clone(),
        receiver_index: 0,
    };
    let user = ActorRef::User {
        user_id: crate::current_user::CURRENT_USER_ID.into(),
    };
    let read = |db: &Database| {
        NativeChildSessionService::resolve(
            db,
            &ManagedBlobStore::new(&f.directory),
            &user,
            &reference,
        )
    };
    let target = read(&f.database).unwrap();
    assert_eq!(target.recorded_home.as_ref(), Some(&root));
    assert!(target.live);
    assert!(!format!("{target:?}").contains("ORIGIN_ONLY_NOT_MODEL"));
    let folder = root.join("sessions/2026/10/18");
    std::fs::create_dir_all(&folder).unwrap();
    let body = format!(
        "{}\n{}\n",
        json!({"type":"session_meta","payload":{"id":child,"parent_thread_id":retained}}),
        json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"EXACT_ORIGIN_CHILD_BODY"}]}})
    );
    std::fs::write(
        folder.join(format!("rollout-2026-10-18T12-00-00-{child}.jsonl")),
        body.as_bytes(),
    )
    .unwrap();
    let mut documents =
        crate::runtime_subagent_session::source::NativeDocuments::open_recorded(&target).unwrap();
    assert_eq!(
        documents.documents[0].read_chunk(0).unwrap(),
        body.as_bytes()
    );
    drop(documents);
    let changes = f.database.connection().total_changes();
    read(&f.database).unwrap();
    assert_eq!(f.database.connection().total_changes(), changes);
    for statement in [
        "UPDATE conversation SET native_session_id='replacement-parent' WHERE id=?1",
        "UPDATE conversation SET native_binding_id='replacement-binding',native_binding_generation=native_binding_generation+1 WHERE id=?1",
        "UPDATE agent_run SET execution_epoch=execution_epoch+1 WHERE conversation_id=?1",
    ] {
        f.database
            .connection()
            .execute_batch("SAVEPOINT source_history")
            .unwrap();
        f.database
            .connection()
            .execute(statement, [&execution.conversation_id])
            .unwrap();
        let historical = read(&f.database).unwrap();
        assert!(!historical.live);
        assert_eq!(historical.recorded_home.as_ref(), Some(&root));
        assert_eq!(historical.parent_native_session_id, retained);
        f.database
            .connection()
            .execute_batch("ROLLBACK TO source_history; RELEASE source_history")
            .unwrap();
    }
    let mut conflict = command.clone();
    conflict.command_id = Uuid::new_v4().to_string();
    runtime
        .bind_native_session_with_source(&mut f.database, &conflict, Some(&other_host))
        .unwrap();
    assert!(
        read(&f.database).is_err(),
        "conflicting Host origins are not chosen by recency"
    );
    let conflict_row: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT rowid FROM event_log WHERE event_type='conversation.native_source.recorded'
         AND json_extract(payload_json,'$.hostInstanceId')='host-conflicting-origin'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        f.database
            .connection()
            .execute("DELETE FROM event_log WHERE rowid=?1", [conflict_row])
            .unwrap(),
        1
    );
    assert_eq!(source_rows(&f.database), saved);
    assert_eq!(
        read(&f.database).unwrap().recorded_home.as_ref(),
        Some(&root)
    );
    let ContextMaterialization::Ready(prepared) = ContextService
        .materialize(
            &mut f.database,
            &ManagedBlobStore::new(&f.directory),
            &MaterializeContextRequest {
                agent_run_id: &f.run_id,
                execution_epoch: f.execution_epoch,
                charter_delivery_mode: CharterDeliveryMode::NativeAppend,
                max_payload_bytes: DEFAULT_MAX_CONTEXT_PAYLOAD_BYTES,
            },
        )
        .unwrap()
    else {
        panic!("frozen context should remain ready")
    };
    assert!(!prepared.rendered_payload.contains("ORIGIN_ONLY_NOT_MODEL"));
    assert!(
        !prepared
            .runtime_payload
            .contains("NEVER_SAVE_OR_SEND_ORIGIN_SECRET")
    );
}
