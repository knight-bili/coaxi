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
    let project = f.directory.join("PATCH_SOURCE_ONLY_NOT_MODEL");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("src/file.ts"), "head\nold\nend\n").unwrap();
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
            std::ffi::OsString::from("CLAUDE_CONFIG_DIR"),
            root.clone().into_os_string(),
        ),
        (
            std::ffi::OsString::from("QODER_CONFIG_DIR"),
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
        &project,
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
    assert_native_archive_source_guards(f, &root, &project);
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
    for kind in [
        AdapterKind::GrokBuild,
        AdapterKind::ClaudeCodeCli,
        AdapterKind::QoderCli,
    ] {
        let other =
            CapturedNativeSource::capture(kind, &environment, &f.directory, "host-wrong-kind")
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
    }

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
    assert_eq!(target.recorded_working_directory.as_ref(), Some(&project));
    assert!(target.live);
    assert!(!format!("{target:?}").contains("ORIGIN_ONLY_NOT_MODEL"));
    let folder = root.join("sessions/2026/10/18");
    std::fs::create_dir_all(&folder).unwrap();
    let patch = "*** Begin Patch\n*** Update File: src/file.ts\n@@\n-old\n+new\n*** End Patch";
    let body = format!(
        "{}\n{}\n{}\n",
        json!({"type":"session_meta","payload":{"id":child,"parent_thread_id":retained}}),
        json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"EXACT_ORIGIN_CHILD_BODY"}]}}),
        json!({"type":"response_item","payload":{"type":"function_call","name":"apply_patch","call_id":"trusted-patch","arguments":patch}})
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
    let content = |db: &Database, actor: &ActorRef, input: &NativeChildSessionReference| {
        NativeChildSessionService::read_content(
            db,
            &ManagedBlobStore::new(&f.directory),
            actor,
            input,
            chrono::DateTime::parse_from_rfc3339("2026-10-18T00:00:00Z")
                .unwrap()
                .to_utc(),
            &std::sync::atomic::AtomicBool::new(false),
        )
    };
    let projected = content(&f.database, &user, &reference).unwrap();
    let projected_body = serde_json::to_string(&projected.turns).unwrap();
    assert!(projected_body.contains("EXACT_ORIGIN_CHILD_BODY"));
    assert!(projected_body.contains("@@ -2,1 +2,1 @@"));
    let changes = f.database.connection().total_changes();
    assert_eq!(
        content(&f.database, &user, &reference)
            .unwrap()
            .snapshot_digest,
        projected.snapshot_digest
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    assert!(
        content(
            &f.database,
            &ActorRef::System {
                component_id: "not-user".into()
            },
            &reference
        )
        .is_err()
    );
    let mut wrong_reference = reference.clone();
    wrong_reference.thread_id = "wrong-thread".into();
    assert!(content(&f.database, &user, &wrong_reference).is_err());
    assert!(
        NativeChildSessionService::read_content(
            &f.database,
            &ManagedBlobStore::new(&f.directory),
            &user,
            &reference,
            chrono::DateTime::parse_from_rfc3339("2026-10-18T00:00:00Z")
                .unwrap()
                .to_utc(),
            &std::sync::atomic::AtomicBool::new(true)
        )
        .err()
        .unwrap()
        .to_string()
        .contains("read_cancelled")
    );

    // Reading mixed v1 provenance is a storage fixture. The initial event above
    // was produced by the real binding command; never nest that command inside
    // a fixture savepoint just to manufacture an additional historical event.
    let duplicate_origin = |database: &Database, payload: &serde_json::Value| {
        database.connection().execute(
            "INSERT INTO event_log(event_id,event_type,payload_json,camp_id,entity_type,entity_id,
                actor_type,actor_id,source_agent_run_id,execution_epoch,created_at)
             SELECT ?1,event_type,?2,camp_id,entity_type,entity_id,actor_type,actor_id,
                source_agent_run_id,execution_epoch,created_at FROM event_log
             WHERE event_type='conversation.native_source.recorded' LIMIT 1",
            params![Uuid::new_v4().to_string(),payload.to_string()],
        ).unwrap();
    };
    let original_origin: serde_json::Value = serde_json::from_str(&saved[0]).unwrap();
    f.database
        .connection()
        .execute_batch("SAVEPOINT legacy_patch_origin")
        .unwrap();
    f.database.connection().execute("UPDATE event_log SET payload_json=json_remove(payload_json,'$.workingDirectory') WHERE event_type='conversation.native_source.recorded'",[]).unwrap();
    assert!(
        read(&f.database)
            .unwrap()
            .recorded_working_directory
            .is_none()
    );
    let legacy = content(&f.database, &user, &reference).unwrap();
    assert!(
        !serde_json::to_string(&legacy.turns)
            .unwrap()
            .contains("@@ -2,1 +2,1 @@")
    );
    assert!(
        serde_json::to_string(&legacy.turns)
            .unwrap()
            .contains("EXACT_ORIGIN_CHILD_BODY")
    );
    duplicate_origin(&f.database, &original_origin);
    assert_eq!(source_rows(&f.database).len(), 2);
    assert!(
        serde_json::to_string(&content(&f.database, &user, &reference).unwrap().turns)
            .unwrap()
            .contains("@@ -2,1 +2,1 @@")
    );
    f.database
        .connection()
        .execute_batch("ROLLBACK TO legacy_patch_origin; RELEASE legacy_patch_origin")
        .unwrap();

    f.database
        .connection()
        .execute_batch("SAVEPOINT conflicting_patch_origin")
        .unwrap();
    let mut contradictory = original_origin;
    contradictory["workingDirectory"] = json!(f.directory.to_string_lossy());
    duplicate_origin(&f.database, &contradictory);
    assert!(
        read(&f.database).is_err(),
        "same Host with contradictory captured directories is not chosen by recency"
    );
    f.database
        .connection()
        .execute_batch("ROLLBACK TO conflicting_patch_origin; RELEASE conflicting_patch_origin")
        .unwrap();
    assert_eq!(source_rows(&f.database), saved);
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
        assert_eq!(
            historical.recorded_working_directory.as_ref(),
            Some(&project)
        );
        assert!(
            serde_json::to_string(&content(&f.database, &user, &reference).unwrap().turns)
                .unwrap()
                .contains("@@ -2,1 +2,1 @@")
        );
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
            .rendered_payload
            .contains("PATCH_SOURCE_ONLY_NOT_MODEL")
    );
    assert!(
        !prepared
            .runtime_payload
            .contains("NEVER_SAVE_OR_SEND_ORIGIN_SECRET")
    );
    assert_legacy_agent_core_reference(f);
}

// 只用本测试的来源记录验证删除目标，不连接模型或触碰用户原生历史。
fn assert_native_archive_source_guards(f: &Fixture, home: &std::path::Path, cwd: &std::path::Path) {
    use crate::runtime_subagent_session::native_archive::collect;
    std::fs::create_dir_all(home).unwrap();
    let db = f.database.connection();
    db.execute_batch("SAVEPOINT native_archive_fixture").unwrap();
    let native = Uuid::new_v4().to_string();
    db.execute("UPDATE event_log SET payload_json=json_set(payload_json,'$.parentNativeSessionId',?1) WHERE event_type='conversation.native_source.recorded'", [&native]).unwrap();
    db.execute("UPDATE conversation SET native_session_id=?1 WHERE camp_id=?2 AND native_session_id IS NOT NULL", params![native, f.camp_id]).unwrap();
    let targets = collect(&f.database, &f.camp_id).unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].session_id, native);
    assert_eq!(targets[0].home, std::fs::canonicalize(home).unwrap());
    assert_eq!(targets[0].cwd, cwd);
    for (mutation, expected) in [
        ("UPDATE event_log SET payload_json=json_set(payload_json,'$.parentNativeSessionId','not-a-session') WHERE event_type='conversation.native_source.recorded'", "invalid_session"),
        ("UPDATE event_log SET actor_id='untrusted-runtime' WHERE event_type='conversation.native_source.recorded'", "source_unproven"),
        ("UPDATE event_log SET payload_json=json_set(payload_json,'$.installationId','changed-program') WHERE event_type='conversation.native_source.recorded'", "program_unproven"),
        ("DELETE FROM event_log WHERE event_type='conversation.native_source.recorded'", "source_unproven"),
    ] {
        db.execute_batch("SAVEPOINT native_archive_negative").unwrap();
        db.execute_batch(mutation).unwrap();
        assert!(collect(&f.database, &f.camp_id).err().unwrap().to_string().contains(expected));
        db.execute_batch("ROLLBACK TO native_archive_negative; RELEASE native_archive_negative").unwrap();
    }
    db.execute("UPDATE event_log SET payload_json=json_remove(payload_json,'$.workingDirectory') WHERE event_type='conversation.native_source.recorded'", []).unwrap();
    assert_eq!(collect(&f.database, &f.camp_id).unwrap()[0].cwd, std::fs::canonicalize(home).unwrap());
    db.execute_batch("ROLLBACK TO native_archive_fixture; RELEASE native_archive_fixture").unwrap();
}

// Same DB/Blob/root owner. Adapter rows and provenance below are explicit
// fixture metadata, not a real Claude/Qoder Host/binding/provider claim.
fn assert_legacy_agent_core_reference(f: &mut Fixture) {
    use crate::agent_profile::AdapterKind;
    use crate::runtime_subagent_session::{
        ORIGIN_EVENT,
        legacy::{NativeAgentReference, NativeAgentService},
    };
    use std::sync::atomic::AtomicBool;
    let template:String=f.database.connection().query_row(
        "SELECT payload_json FROM event_log WHERE event_type=?1 AND json_extract(payload_json,'$.agentRunId')=?2 ORDER BY rowid LIMIT 1",
        params![ORIGIN_EVENT,f.run_id],|r|r.get(0)).unwrap();
    let template: serde_json::Value = serde_json::from_str(&template).unwrap();
    let parent = template["parentNativeSessionId"].as_str().unwrap();
    let input = json!({"prompt":"Core observed private native task"});
    let digest = crate::command::canonical_json_digest(&input).unwrap();
    let user = ActorRef::User {
        user_id: crate::current_user::CURRENT_USER_ID.into(),
    };
    let stop = AtomicBool::new(false);
    let body = format!("PRIVATE_AGENT_BODY{}完整末尾😀", "中文😀".repeat(10_000));
    for kind in [AdapterKind::ClaudeCodeCli, AdapterKind::QoderCli] {
        let home = f
            .directory
            .join(format!("legacy-evidence-{}", kind.as_str()));
        let project = home.join("projects").join("opaque");
        std::fs::create_dir_all(&project).unwrap();
        let sub = project.join(parent).join("subagents");
        std::fs::create_dir_all(&sub).unwrap();
        let records = [
            json!({"type":"assistant","uuid":"call-record","sessionId":parent,
            "message":{"content":[{"type":"tool_use","id":"core-native-tool","name":"Agent","input":input}]}}),
            json!({"type":"user","uuid":"result-record","parentUuid":"call-record","sessionId":parent,
                "toolUseResult":{"agentId":"native-agent-only"},"message":{"content":[{"type":"tool_result","tool_use_id":"core-native-tool","content":"started"}]}}),
        ];
        std::fs::write(
            project.join(format!("{parent}.jsonl")),
            records
                .iter()
                .map(serde_json::Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )
        .unwrap();
        std::fs::write(sub.join("agent-native-agent-only.jsonl"),json!({"type":"assistant","uuid":"own-agent-answer","agentId":"native-agent-only","timestamp":"2026-10-19T00:00:00Z","message":{"id":"own-agent-answer","role":"assistant","content":[{"type":"text","text":body}]}}).to_string()+"\n").unwrap();
        std::fs::write(
            sub.join("agent-native-agent-only.meta.json"),
            json!({"toolUseId":"core-native-tool","agentType":"Explore"}).to_string(),
        )
        .unwrap();
        f.database
            .connection()
            .execute_batch("SAVEPOINT legacy_agent_core_fixture")
            .unwrap();
        f.database
            .connection()
            .execute(
                "UPDATE agent_run SET runtime_adapter_kind=?1 WHERE id=?2",
                params![kind.as_str(), f.run_id],
            )
            .unwrap();
        let evidence = Uuid::new_v4().to_string();
        let payload = json!({"toolCallId":"core-native-tool","rawInputDigest":digest,"status":"in_progress","kind":"other"});
        f.database.connection().execute(
            "INSERT INTO agent_run_execution_evidence(id,agent_run_id,execution_epoch,sequence,event_type,kind,phase,payload_preview_json,content_byte_count,is_truncated,occurred_at,revision,change_sequence,updated_at)
             VALUES(?1,?2,?3,100009,'runtime.action','command','started',?4,?5,0,'2026-10-19T00:00:00Z',1,100009,'2026-10-19T00:00:00Z')",
            params![evidence,f.run_id,f.execution_epoch,payload.to_string(),payload.to_string().len() as i64]).unwrap();
        let mut origin = template.clone();
        origin["adapterKind"] = json!(kind.as_str());
        origin["homePath"] = json!(home.to_str().unwrap());
        f.database.connection().execute(
            "INSERT INTO event_log(event_id,event_type,payload_json,camp_id,entity_type,entity_id,actor_type,actor_id,source_agent_run_id,execution_epoch,created_at)
             SELECT ?1,event_type,?2,camp_id,entity_type,entity_id,actor_type,?3,source_agent_run_id,execution_epoch,created_at FROM event_log
             WHERE event_type=?4 AND json_extract(payload_json,'$.agentRunId')=?5 ORDER BY rowid LIMIT 1",
            params![Uuid::new_v4().to_string(),origin.to_string(),format!("runtime-adapter:{}",kind.as_str()),ORIGIN_EVENT,f.run_id]).unwrap();
        let reference = NativeAgentReference {
            thread_id: f.camp_id.clone(),
            agent_run_id: f.run_id.clone(),
            evidence_id: evidence.clone(),
        };
        let prepare = |actor: &ActorRef, reference: &NativeAgentReference| {
            NativeAgentService::prepare(
                &f.database,
                &ManagedBlobStore::new(&f.directory),
                actor,
                reference,
            )
        };
        let changes = f.database.connection().total_changes();
        let plan = prepare(&user, &reference).unwrap();
        let (reader, read, snapshot) =
            crate::runtime_subagent_session::reader::tests::assert_agent_workers(&plan, &body);
        assert!(!format!("{plan:?}").contains("legacy-evidence"));
        let (target, documents) = plan.open(&stop).unwrap();
        assert_eq!(target.agent_id, "native-agent-only");
        assert_eq!(target.parent_tool_use_id, "core-native-tool");
        assert_eq!(target.adapter_kind, kind);
        assert!(documents.documents.len() >= 2);
        assert!(!format!("{target:?}").contains("PRIVATE_AGENT_BODY"));
        NativeAgentService::revalidate(
            &f.database,
            &ManagedBlobStore::new(&f.directory),
            &user,
            &target,
        )
        .unwrap();
        assert!(plan.open(&AtomicBool::new(true)).is_err());
        for actor in [
            ActorRef::System {
                component_id: "ui".into(),
            },
            ActorRef::Agent {
                agent_id: "agent_1".into(),
                source_agent_run_id: f.run_id.clone(),
            },
            ActorRef::User {
                user_id: "other".into(),
            },
        ] {
            assert!(prepare(&actor, &reference).is_err());
        }
        for input in [
            NativeAgentReference {
                thread_id: "other".into(),
                ..reference.clone()
            },
            NativeAgentReference {
                agent_run_id: "other".into(),
                ..reference.clone()
            },
            NativeAgentReference {
                evidence_id: "other".into(),
                ..reference.clone()
            },
        ] {
            assert!(prepare(&user, &input).is_err());
        }
        for key in [
            "agentId",
            "nativeSessionId",
            "path",
            "home",
            "adapterKind",
            "toolCallId",
            "inputDigest",
        ] {
            let mut value =
                json!({"threadId":f.camp_id,"agentRunId":f.run_id,"evidenceId":evidence});
            value[key] = json!("override");
            assert!(serde_json::from_value::<NativeAgentReference>(value).is_err());
        }
        assert_eq!(
            f.database.connection().total_changes(),
            changes,
            "prepare/open/revalidation are read only"
        );
        f.database
            .connection()
            .execute(
                "UPDATE agent_run SET execution_epoch=execution_epoch+1 WHERE id=?1",
                [&f.run_id],
            )
            .unwrap();
        let history_plan = prepare(&user, &reference).unwrap();
        let historical = history_plan.open(&stop).unwrap().0;
        assert_eq!(
            reader
                .poll_agent(&user, &read.read_id, &history_plan)
                .unwrap()
                .state,
            "ready"
        );
        let window = reader
            .page_agent(
                &user,
                crate::runtime_subagent_session::reader::PageRead {
                    read_id: read.read_id.clone(),
                    snapshot_reference: snapshot,
                    cursor: None,
                    max_bytes: None,
                },
                &history_plan,
            )
            .unwrap();
        assert!(!window.live);
        assert_eq!(historical.agent_id, target.agent_id);
        assert!(!historical.live);
        NativeAgentService::revalidate(
            &f.database,
            &ManagedBlobStore::new(&f.directory),
            &user,
            &target,
        )
        .unwrap();
        f.database.connection().execute("UPDATE agent_run_execution_evidence SET payload_preview_json=json_set(payload_preview_json,'$.rawInputDigest',?1) WHERE id=?2",params!["0".repeat(64),evidence]).unwrap();
        assert!(prepare(&user, &reference).unwrap().open(&stop).is_err());
        assert!(
            reader
                .poll_agent(&user, &read.read_id, &prepare(&user, &reference).unwrap())
                .is_err()
        );
        reader.close(&user, &read.read_id).unwrap();
        assert!(
            NativeAgentService::revalidate(
                &f.database,
                &ManagedBlobStore::new(&f.directory),
                &user,
                &target
            )
            .is_err()
        );
        f.database
            .connection()
            .execute_batch(
                "ROLLBACK TO legacy_agent_core_fixture; RELEASE legacy_agent_core_fixture",
            )
            .unwrap();
    }
}
