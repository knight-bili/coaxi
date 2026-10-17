// Extends the existing legacy-input owner, using its same database and real
// old schemas. Runtime state/credentials remain the original Core fixtures.
fn queued_legacy_batch(fixture: &mut Fixture) -> (String, String) {
    let project = fixture.directory.join("legacy-queued-project");
    std::fs::create_dir_all(&project).unwrap();
    let created = CollaborationService::default()
        .create_test_camp_conversation(
            &mut fixture.database,
            &CommandEnvelope {
                command_id: Uuid::new_v4().to_string(),
                actor: ActorRef::User {
                    user_id: "test-user".into(),
                },
                camp_id: None,
                expected_versions: vec![],
                execution_epoch: None,
                payload: crate::collaboration::TestThreadConversationCommand {
                    project_path: project.display().to_string(),
                    project_binding_kind: crate::collaboration::ProjectBindingKind::Directory,
                    body: "旧版本排队输入".into(),
                    address: TestThreadMessageAddress::Default,
                    purpose: "旧版本恢复验证".into(),
                },
            },
        )
        .unwrap();
    assert_eq!(created.result.status, CommandResultStatus::Accepted);
    let camp = created.result.payload["threadId"]
        .as_str()
        .unwrap()
        .to_string();
    let run = created.result.payload["agentRunIds"][0]
        .as_str()
        .unwrap()
        .to_string();
    let view = ThreadAttachmentViewStore::for_test(&fixture.database).unwrap();
    view.ensure_empty_camp_ready(&mut fixture.database, &camp)
        .unwrap();
    let selection = crate::current_input_skill::SkillSelectionSnapshot {
        schema_version: 1,
        entries: vec![],
    };
    // Explicit historical data fixture: a valid retained v1 selection on the
    // claimed old input. No production trigger or contract is removed.
    fixture.database.connection().execute("UPDATE agent_run SET skill_selection_snapshot_json=?2,skill_selection_snapshot_digest=?3 WHERE id=?1",params![fixture.run_id,serde_json::to_string(&selection).unwrap(),selection.canonical_digest().unwrap()]).unwrap();
    (camp, run)
}

fn assert_legacy_delegation_upgrade(
    fixture: &mut Fixture,
    queued: &(String, String),
    old_payload: &str,
    old_digest: &str,
    charter_delivery_mode: CharterDeliveryMode,
) {
    fn frozen(database: &Database, id: &str) -> (String, String, String) {
        database.connection().query_row("SELECT skill_selection_snapshot_json,skill_selection_snapshot_digest,effective_config_json FROM agent_run WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap()
    }
    let first = frozen(&fixture.database, &fixture.run_id);
    let second = frozen(&fixture.database, &queued.1);
    assert_eq!(
        serde_json::from_str::<Value>(&first.0).unwrap()["schemaVersion"],
        1
    );
    assert_eq!(
        serde_json::from_str::<Value>(&second.0).unwrap()["schemaVersion"],
        2
    );
    assert_eq!(
        fixture
            .database
            .connection()
            .query_row(
                "SELECT status FROM agent_run WHERE id=?1",
                [&queued.1],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "queued"
    );
    // Reopen this populated old139 file through the entire production upgrade.
    let upgraded = Database::open(&fixture.directory).unwrap();
    assert!(
        matches!(crate::db::classify_database_contract(upgraded.connection()).unwrap(),crate::db::DatabaseContractClassification::Current(ref marker) if marker.contract_version=="v1.73" && marker.projection_schema_version==160)
    );
    drop(upgraded);
    fixture
        .database
        .connection()
        .execute("UPDATE delegation_settings SET enabled=1,depth_limit=8", [])
        .unwrap();
    assert_eq!(frozen(&fixture.database, &fixture.run_id), first);
    assert_eq!(frozen(&fixture.database, &queued.1), second);
    let store = ManagedBlobStore::new(&fixture.directory);
    let request = MaterializeContextRequest {
        agent_run_id: &fixture.run_id,
        execution_epoch: fixture.execution_epoch,
        charter_delivery_mode,
        max_payload_bytes: DEFAULT_MAX_CONTEXT_PAYLOAD_BYTES,
    };
    let ContextMaterialization::Ready(replay) = ContextService
        .materialize(&mut fixture.database, &store, &request)
        .unwrap()
    else {
        panic!("old1 replay")
    };
    assert_eq!(replay.rendered_payload, old_payload);
    assert_eq!(replay.rendered_payload_digest, old_digest);
    let expected_version = fixture
        .database
        .connection()
        .query_row(
            "SELECT version FROM agent_run WHERE id=?1",
            [&queued.1],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    let claimed = ExecutionRuntimeService::default()
        .claim_agent_run(
            &mut fixture.database,
            &CommandEnvelope {
                command_id: Uuid::new_v4().to_string(),
                actor: ActorRef::System {
                    component_id: "agent-run-scheduler".into(),
                },
                camp_id: Some(queued.0.clone()),
                expected_versions: vec![],
                execution_epoch: None,
                payload: ClaimAgentRunCommand {
                    agent_run_id: queued.1.clone(),
                    expected_version,
                    lease_owner: "legacy-test-scheduler".into(),
                    lease_seconds: 60,
                    workspace: None,
                    starting_git_observation: None,
                },
            },
        )
        .unwrap();
    assert_eq!(
        claimed.result.status,
        CommandResultStatus::Accepted,
        "{:?}",
        claimed.result
    );
    let epoch = claimed.result.payload["executionEpoch"].as_i64().unwrap();
    TeamToolService::default()
        .prepare_binding_credential(&mut fixture.database, &queued.1, epoch, false)
        .unwrap();
    let request = MaterializeContextRequest {
        agent_run_id: &queued.1,
        execution_epoch: epoch,
        ..request
    };
    let ContextMaterialization::Ready(ready) = ContextService
        .materialize(&mut fixture.database, &store, &request)
        .unwrap()
    else {
        panic!("old2 queued materialization")
    };
    assert_eq!(
        fixture
            .database
            .connection()
            .query_row(
                "SELECT context_manifest_version FROM context_manifest WHERE id=?1",
                [&ready.manifest_id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        32
    );
    assert_eq!(frozen(&fixture.database, &fixture.run_id), first);
    assert_eq!(frozen(&fixture.database, &queued.1), second);
    assert!(!ready.rendered_payload.contains("DELEGATION_GUIDANCE"));
}
