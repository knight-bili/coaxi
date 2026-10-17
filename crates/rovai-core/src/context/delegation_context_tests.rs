// Extends the existing materialization owner; fixture SQL is not authenticated admission.
fn context_child(fixture: &mut Fixture, task: &str, call: &str) -> (String, String, i64) {
    let blob = ManagedBlobStore::new(&fixture.directory)
        .put_bytes(
            &mut fixture.database,
            task.as_bytes(),
            "text/plain",
            "sensitive",
        )
        .unwrap();
    let (binding,generation):(String,i64)=fixture.database.connection().query_row("SELECT c.native_binding_id,c.native_binding_generation FROM agent_run r JOIN conversation c ON c.id=r.conversation_id WHERE r.id=?1",[&fixture.run_id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    let target = crate::delegation_contract::RuntimeTarget::try_from("codex".to_string()).unwrap();
    let workspace =
        json!({"executionRoot":fixture.directory,"access":"read_only","isolation":"shared"});
    let preset = json!({});
    let tx = fixture.database.connection_mut().transaction().unwrap();
    let registered = crate::delegation::register(
        &tx,
        &crate::delegation::ParentInvocation {
            run_id: &fixture.run_id,
            epoch: fixture.execution_epoch,
            binding_id: &binding,
            binding_generation: generation,
            tool_call_id: call,
        },
        &crate::delegation::FrozenRequest {
            task_blob_id: &blob.id,
            runtime: &target,
            working_dir: None,
            workspace: &workspace,
            runtime_preset: &preset,
        },
        "2026-10-17T00:00:00Z",
        |tx, identity| {
            crate::delegation::tests::child_with_task(
                tx,
                identity,
                &fixture.run_id,
                None,
                task,
                Some(&workspace),
            )
        },
    )
    .unwrap();
    let run: String = tx
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [&registered.identity.id],
            |r| r.get(0),
        )
        .unwrap();
    tx.commit().unwrap();
    let candidate = ExecutionRuntimeService::default()
        .list_dispatchable_agent_runs(&fixture.database, 8)
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.agent_run_id == run)
        .expect("child dispatch candidate");
    let claimed = ExecutionRuntimeService::default()
        .claim_agent_run(
            &mut fixture.database,
            &CommandEnvelope {
                command_id: Uuid::new_v4().to_string(),
                actor: ActorRef::System {
                    component_id: "agent-run-scheduler".into(),
                },
                camp_id: Some(fixture.camp_id.clone()),
                expected_versions: Vec::new(),
                execution_epoch: None,
                payload: ClaimAgentRunCommand {
                    agent_run_id: run.clone(),
                    expected_version: candidate.version,
                    lease_owner: "context-child-fixture".into(),
                    lease_seconds: 60,
                    workspace: Some(AgentRunWorkspace {
                        execution_root: fixture.directory.display().to_string(),
                        access: "read_only".into(),
                        isolation: "shared".into(),
                    }),
                    starting_git_observation: None,
                },
            },
        )
        .unwrap();
    assert_eq!(
        claimed.result.status,
        crate::command::CommandResultStatus::Accepted,
        "{:?}",
        claimed.result
    );
    let epoch = claimed.result.payload["executionEpoch"].as_i64().unwrap();
    TeamToolService::default()
        .prepare_binding_credential(&mut fixture.database, &run, epoch, false)
        .unwrap();
    (registered.identity.id, run, epoch)
}

fn context_section(payload: &str, name: &str) -> Value {
    let start = format!("[{name}]\n");
    let end = format!("\n[/{name}]");
    serde_json::from_str(
        payload
            .split_once(&start)
            .unwrap()
            .1
            .split_once(&end)
            .unwrap()
            .0,
    )
    .unwrap()
}

fn assert_delegation_materialization(fixture: &mut Fixture, legacy_private_run: &str) {
    let old_manifest: String = fixture
        .database
        .connection()
        .query_row(
            "SELECT rendered_payload_blob_id FROM context_manifest WHERE agent_run_id=?1",
            [legacy_private_run],
            |r| r.get(0),
        )
        .unwrap();
    fixture
        .database
        .connection()
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .unwrap();
    let tx = fixture.database.connection_mut().transaction().unwrap();
    crate::db::delegation::install_identity_schema(&tx).unwrap();
    tx.commit().unwrap();
    fixture.database.connection().execute_batch("PRAGMA foreign_keys=ON; UPDATE delegation_settings SET enabled=1,depth_limit=8 WHERE singleton=1;").unwrap();
    assert_eq!(
        fixture
            .database
            .connection()
            .query_row(
                "SELECT rendered_payload_blob_id FROM context_manifest WHERE agent_run_id=?1",
                [legacy_private_run],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        old_manifest
    );
    let task = "完整独立任务：只检查自己的输入。";
    let (id, run, epoch) = context_child(fixture, task, "context-child-initial");
    let store = ManagedBlobStore::new(&fixture.directory);
    let request = MaterializeContextRequest {
        agent_run_id: &run,
        execution_epoch: epoch,
        charter_delivery_mode: CharterDeliveryMode::FirstPayload,
        max_payload_bytes: DEFAULT_MAX_CONTEXT_PAYLOAD_BYTES,
    };
    let ContextMaterialization::Ready(first) = ContextService
        .materialize(&mut fixture.database, &store, &request)
        .unwrap()
    else {
        panic!("child Context not ready")
    };
    let facts = context_section(&first.rendered_payload, "RUN_FACTS");
    assert_eq!(
        facts,
        json!({"schemaVersion":10,"threadId":fixture.camp_id,"ownerAgentId":"agent_1","conversationId":facts["conversationId"],"delegationId":id,"executionRoot":fixture.directory.canonicalize().unwrap(),"workspaceAccess":"read_only","workspaceIsolation":"shared"})
    );
    assert_eq!(
        context_section(&first.rendered_payload, "CURRENT_INPUT"),
        json!({"id":context_section(&first.rendered_payload,"CURRENT_INPUT")["id"],"message":task,"source":"delegation_parent"})
    );
    assert_eq!(
        context_section(&first.rendered_payload, "DELEGATION_CONTEXT")["mode"],
        "initial"
    );
    let order = [
        "[ROVAI_ADDITIONAL_SKILLS]",
        "[ROVAI_SKILL_PACKS]",
        "[DELEGATION_CONTEXT]",
        "[RUN_FACTS]",
        "[CURRENT_INPUT]",
    ];
    for pair in order.windows(2) {
        assert!(
            first.rendered_payload.find(pair[0]).unwrap()
                < first.rendered_payload.find(pair[1]).unwrap()
        );
    }
    for forbidden in [
        "[COLLABORATION_STATE]",
        "[SELF_ACTIVE_TASKS]",
        "[SHARED_THREAD]",
        "[A2A_GUIDANCE]",
        "[WORKSPACE]",
        "[DELEGATION_GUIDANCE]",
        "[MEMORY_ENTRYPOINT]",
        "cli-operations",
        "memory-stewardship",
        "只检查当前单聊输入",
    ] {
        assert!(
            !first.runtime_payload.contains(forbidden),
            "leaked {forbidden}"
        );
    }
    assert_eq!(
        context_section(&first.runtime_payload, "MEMBER_IDENTITY")["teamRole"],
        "Delegated executor"
    );
    let (manifest_version,profile,facts_version,blob,history_count):(i64,i64,i64,String,i64)=fixture.database.connection().query_row(
        "SELECT context_manifest_version,context_delivery_profile_version,run_facts_schema_version,rendered_payload_blob_id,(SELECT count(*) FROM context_manifest_history_camp WHERE context_manifest_id=m.id) FROM context_manifest m WHERE id=?1",[&first.manifest_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap();
    assert_eq!(
        (manifest_version, profile, facts_version, history_count),
        (35, 11, 10, 0)
    );
    assert_eq!(
        store.read_text(&fixture.database, &blob).unwrap(),
        first.rendered_payload
    );
    fixture
        .database
        .connection()
        .execute("UPDATE delegation_settings SET enabled=0,depth_limit=1", [])
        .unwrap();
    let ContextMaterialization::Ready(replayed) = ContextService
        .materialize(
            &mut fixture.database,
            &ManagedBlobStore::new(&fixture.directory),
            &request,
        )
        .unwrap()
    else {
        panic!("frozen child replay")
    };
    assert_eq!(first.rendered_payload, replayed.rendered_payload);
    assert_eq!(
        first.runtime_payload, replayed.runtime_payload,
        "recovery cannot restore the parent identity"
    );
    assert_eq!(first.manifest_id, replayed.manifest_id);

    // Formatting owners for resume and User follow-up; no cancellation/admission proof.
    let mut snapshot = load_run_snapshot(&fixture.database, &run, epoch)
        .unwrap()
        .unwrap();
    let mut child = delegation_context::load(&fixture.database, &snapshot)
        .unwrap()
        .unwrap();
    let tx = fixture.database.connection_mut().transaction().unwrap();
    let resumed = Uuid::new_v4().to_string();
    let user = Uuid::new_v4().to_string();
    tx.execute("INSERT INTO conversation_message(id,conversation_id,sequence,author_type,author_id,body,created_at) VALUES(?1,?2,2,'system','delegation-parent',?3,'2026-10-17T00:00:01Z')",params![resumed,snapshot.conversation_id,include_str!("../../resources/delegation-resume.txt").trim()]).unwrap();
    tx.execute("INSERT INTO conversation_message(id,conversation_id,sequence,author_type,author_id,body,created_at) VALUES(?1,?2,3,'user','test-user','用户接管后的新工作','2026-10-17T00:00:02Z')",params![user,snapshot.conversation_id]).unwrap();
    tx.commit().unwrap();
    snapshot.trigger_conversation_message_id = Some(resumed.clone());
    snapshot.conversation_message_boundary_sequence = 3;
    child.mode = "resume";
    child.interruption = Some("执行中断，仅供参考".into());
    let resumed_input =
        delegation_context::current_input(&fixture.database, &snapshot, &child).unwrap();
    assert_eq!(
        resumed_input.payload,
        json!({"id":resumed,"source":"delegation_parent","message":include_str!("../../resources/delegation-resume.txt").trim(),"interruptionContext":"执行中断，仅供参考"})
    );
    assert!(!resumed_input.payload.to_string().contains(task));
    snapshot.trigger_conversation_message_id = Some(user.clone());
    child.mode = "user_followup";
    child.takeover_sequence = Some(2);
    let user_input =
        delegation_context::current_input(&fixture.database, &snapshot, &child).unwrap();
    assert_eq!(
        user_input.payload,
        json!({"id":user,"source":"user","message":"用户接管后的新工作"})
    );
    child.takeover_sequence = Some(3);
    assert!(delegation_context::current_input(&fixture.database, &snapshot, &child).is_err());

    fixture
        .database
        .connection()
        .execute("UPDATE delegation_settings SET enabled=1,depth_limit=8", [])
        .unwrap();
    let huge = "界".repeat(DEFAULT_MAX_CONTEXT_PAYLOAD_BYTES);
    let (_, huge_run, huge_epoch) = context_child(fixture, &huge, "context-child-overflow");
    let error = ContextService
        .materialize(
            &mut fixture.database,
            &store,
            &MaterializeContextRequest {
                agent_run_id: &huge_run,
                execution_epoch: huge_epoch,
                ..request
            },
        )
        .unwrap_err();
    assert!(error.is::<ContextPayloadTooLarge>(), "{error:#}");
    assert_eq!(
        fixture
            .database
            .connection()
            .query_row(
                "SELECT count(*) FROM context_manifest WHERE agent_run_id=?1",
                [&huge_run],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );

    // Construct the future public role directly in this formatter fixture; production
    // remains gated until the complete160/210 startup migration is registered.
    let mut parent = load_run_snapshot(&fixture.database, &fixture.run_id, fixture.execution_epoch)
        .unwrap()
        .unwrap();
    let policy = crate::delegation_policy::FrozenPolicy::new(
        true,
        8,
        crate::delegation_policy::Role::PublicParent,
    )
    .unwrap();
    parent.effective_config["delegationPolicy"] = serde_json::to_value(policy).unwrap();
    parent
        .effective_config
        .as_object_mut()
        .unwrap()
        .remove("configDigest");
    parent.effective_config["configDigest"] =
        json!(canonical_json_digest(&parent.effective_config).unwrap());
    fixture
        .database
        .connection()
        .execute(
            "UPDATE agent_run SET effective_config_json=?2 WHERE id=?1",
            params![
                fixture.run_id,
                serde_json::to_string(&parent.effective_config).unwrap()
            ],
        )
        .unwrap();
    // The old frozen snapshot remains immutable. Build the future-format fixture
    // from a complete before snapshot, rather than weakening that production guard.
    let tx = fixture.database.connection_mut().transaction().unwrap();
    let count: i64 = tx
        .query_row(
            "SELECT count(*) FROM agent_run_input WHERE agent_run_id=?1",
            [&fixture.run_id],
            |r| r.get(0),
        )
        .unwrap();
    if count > 0 {
        assert!(
            tx.execute(
                "UPDATE agent_run_input SET context_manifest_version=35 WHERE agent_run_id=?1",
                [&fixture.run_id]
            )
            .is_err()
        );
        tx.execute("CREATE TEMP TABLE format35_parent_input_fixture AS SELECT * FROM agent_run_input WHERE agent_run_id=?1",[&fixture.run_id]).unwrap();
        tx.execute(
            "DELETE FROM agent_run_input WHERE agent_run_id=?1",
            [&fixture.run_id],
        )
        .unwrap();
        tx.execute_batch("UPDATE format35_parent_input_fixture SET context_manifest_version=35; INSERT INTO agent_run_input SELECT * FROM format35_parent_input_fixture; DROP TABLE format35_parent_input_fixture;").unwrap();
    }
    tx.commit().unwrap();
    let ContextMaterialization::Ready(parent_ready) = ContextService
        .materialize(
            &mut fixture.database,
            &store,
            &MaterializeContextRequest {
                agent_run_id: &fixture.run_id,
                execution_epoch: fixture.execution_epoch,
                ..request
            },
        )
        .unwrap()
    else {
        panic!("parent35")
    };
    assert_eq!(
        context_section(&parent_ready.rendered_payload, "DELEGATION_GUIDANCE"),
        policy.guidance().unwrap()
    );
    assert!(
        parent_ready
            .rendered_payload
            .find("[/ROVAI_SKILL_PACKS]")
            .unwrap()
            < parent_ready
                .rendered_payload
                .find("[DELEGATION_GUIDANCE]")
                .unwrap()
    );
    assert_eq!(fixture.database.connection().query_row("SELECT context_manifest_version,context_delivery_profile_version,run_facts_schema_version FROM context_manifest WHERE id=?1",[&parent_ready.manifest_id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?))).unwrap(),(35,12,9));
    assert_eq!(
        fixture
            .database
            .connection()
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
}
