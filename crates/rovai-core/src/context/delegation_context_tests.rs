// Extends the existing materialization owner; fixture SQL is not authenticated admission.
fn context_child(fixture: &mut Fixture, task: &str, call: &str) -> (String, String, i64) {
    let parent=fixture.run_id.clone();
    let epoch=fixture.execution_epoch;
    context_child_of(fixture,&parent,epoch,task,call)
}
fn context_child_of(fixture: &mut Fixture, parent: &str, parent_epoch: i64, task: &str, call: &str) -> (String, String, i64) {
    let workspace=json!({"executionRoot":fixture.directory,"access":"read_only","isolation":"shared"});
    context_child_in(fixture,parent,parent_epoch,task,call,&workspace)
}
fn context_child_in(fixture: &mut Fixture, parent: &str, parent_epoch: i64, task: &str, call: &str, workspace: &Value) -> (String, String, i64) {
    let blob = ManagedBlobStore::new(&fixture.directory)
        .put_bytes(
            &mut fixture.database,
            task.as_bytes(),
            "text/plain",
            "sensitive",
        )
        .unwrap();
    let (binding,generation):(String,i64)=fixture.database.connection().query_row("SELECT c.native_binding_id,c.native_binding_generation FROM agent_run r JOIN conversation c ON c.id=r.conversation_id WHERE r.id=?1",[parent],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    let target = crate::delegation_contract::RuntimeTarget::try_from("codex".to_string()).unwrap();
    let preset = json!({});
    // Fixture-selected target; the production factory resolves a fresh Registry snapshot.
    let config: String = fixture.database.connection().query_row("SELECT effective_config_json FROM agent_run WHERE id=?1",[parent],|r|r.get(0)).unwrap();
    let value: Value = serde_json::from_str(&config).unwrap();
    let runtime: crate::agent_profile::FrozenAgentRuntimeConfig = serde_json::from_value(value["runtime"].clone()).unwrap();
    let model = if runtime.model.source == "runtime_default" { crate::agent_profile::ModelSelection::RuntimeDefault }
        else { crate::agent_profile::ModelSelection::Explicit {model_id:runtime.model.model_id.clone(),options:runtime.model.options.clone(),dsh_source:runtime.model.dsh_source} };
    let configuration = crate::agent_profile::MemberRuntimeConfiguration { adapter_kind:runtime.adapter_kind, model, permissions:runtime.permissions.clone() };
    let typed_workspace = serde_json::from_value(workspace.clone()).unwrap();
    let plan = crate::delegation_factory::ChildRunPlan {
        task,target:&target,installation_id:&runtime.installation_id,configuration:&configuration,workspace:&typed_workspace,
        policy:crate::delegation_policy::FrozenPolicy::new(true,8,crate::delegation_policy::Role::Child).unwrap(),
    };
    let tx = fixture.database.connection_mut().transaction().unwrap();
    let registered = crate::delegation_factory::register_initial(
        &tx,
        &crate::delegation::ParentInvocation {
            run_id: parent,epoch: parent_epoch,binding_id: &binding,binding_generation: generation,tool_call_id: call,
        },
        &crate::delegation::FrozenRequest {
            task_blob_id: &blob.id,runtime: &target,working_dir: None,workspace: &workspace,runtime_preset: &preset,
        },
        &plan,
        "2026-10-17T00:00:00Z",
    ).unwrap();
    let run: String = tx
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [&registered.identity.id],
            |r| r.get(0),
        )
        .unwrap();
    tx.commit().unwrap();
    let epoch = claim_context_child(fixture, &run);
    (registered.identity.id, run, epoch)
}
fn claim_context_child(fixture: &mut Fixture, run: &str) -> i64 {
    let candidate = ExecutionRuntimeService::default()
        .list_dispatchable_agent_runs(&fixture.database, 8)
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.agent_run_id == run)
        .expect("child dispatch candidate");
    let workspace_json: String = fixture.database.connection().query_row("SELECT workspace_json FROM agent_run WHERE id=?1",[run],|r|r.get(0)).unwrap();
    let workspace: AgentRunWorkspace = serde_json::from_str(&workspace_json).unwrap();
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
                    agent_run_id: run.to_string(),
                    expected_version: candidate.version,
                    lease_owner: "context-child-fixture".into(),
                    lease_seconds: 60,
                    workspace: Some(workspace),
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
    epoch
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

// These assertions own the projection seam, not authenticated User takeover or a real Runtime.
fn assert_child_native_projections(fixture: &Fixture, run: &str, epoch: i64) -> PreparedMcpProjection {
    use crate::current_input_skill::{CurrentInputSkillOmissionReason, CurrentInputSkillResolutionOutcome};
    let mut snapshot = load_run_snapshot(&fixture.database, run, epoch).unwrap().unwrap();
    let runtime: crate::agent_profile::FrozenAgentRuntimeConfig =
        serde_json::from_value(snapshot.effective_config["runtime"].clone()).unwrap();
    let root = crate::managed_skills::managed_skills_root(&fixture.directory).unwrap();
    let mut entries = Vec::new();
    for (index,(name,base)) in [
        ("cli-operations", root.clone()),
        ("memory-stewardship", root.clone()),
        ("cli-operations", fixture.directory.join("user-skills")),
    ].into_iter().enumerate() {
        let dir = base.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("SKILL.md");
        std::fs::write(&path,format!("---\nname: {name}\ndescription: Test native Skill\n---\nIndependent test content\n")).unwrap();
        let canonical = path.canonicalize().unwrap();
        let native = crate::native_skills::read_native_skill(&path,&canonical,"user",runtime.adapter_kind).unwrap();
        // A lexical alias must not bypass the official source check.
        let selected_path = if index==0 { dir.join("..").join(name).join("SKILL.md") } else { path };
        entries.push(json!({"skillId":native.id,
            "nameAtSend":name,"firstSegmentIndex":index,"firstMessageIndex":0,
            "source":"native","sourcePath":selected_path}));
    }
    snapshot.skill_selection_snapshot = serde_json::from_value(json!({"schemaVersion":3,"entries":entries})).unwrap();
    snapshot.skill_selection_snapshot_digest = snapshot.skill_selection_snapshot.canonical_digest().unwrap();
    let selected = snapshot.skill_selection_snapshot.canonical_json_and_digest().unwrap();
    let exposure = SkillExposureSnapshot::default();
    let exposure = PreparedSkillExposure {
        digest:canonical_json_digest(&serde_json::to_value(&exposure).unwrap()).unwrap(), snapshot:exposure,
    };
    let packs = prepare_run_skill_packs(fixture.database.connection(), &snapshot).unwrap();
    let resolved = resolve_run_skills(fixture.database.connection(),&snapshot,&exposure,runtime.adapter_kind,packs.as_ref()).unwrap();
    assert_eq!(resolved.links.len(),1, "custom same-name Skill must survive");
    assert!(resolved.links[0].path.contains("user-skills"));
    for entry in &resolved.resolution.entries[..2] {
        assert_eq!(entry.outcome,CurrentInputSkillResolutionOutcome::Unavailable);
        assert_eq!(entry.reason,Some(CurrentInputSkillOmissionReason::PolicyRestricted));
        assert!(entry.path.is_none());
    }
    assert_eq!(selected,snapshot.skill_selection_snapshot.canonical_json_and_digest().unwrap());
    let frozen = resolution_from_frozen(&serde_json::to_value(&resolved.resolution).unwrap(),
        &snapshot.skill_selection_snapshot,&snapshot.skill_selection_snapshot_digest).unwrap();
    assert_eq!(frozen.digest,resolved.digest);
    assert_eq!(frozen.links,resolved.links);
    assert_eq!(serde_json::to_value(&frozen.resolution).unwrap(),serde_json::to_value(&resolved.resolution).unwrap());
    snapshot.effective_config.as_object_mut().unwrap().remove("delegationPolicy");
    assert!(resolve_run_skills(fixture.database.connection(),&snapshot,&exposure,runtime.adapter_kind,packs.as_ref()).is_err());
    snapshot.invocation_kind = "single_chat".into();
    let legacy = resolve_run_skills(fixture.database.connection(),&snapshot,&exposure,runtime.adapter_kind,packs.as_ref()).unwrap();
    assert_eq!(legacy.links.len(),3,"legacy parent Skill selection must remain unchanged");

    let config_store = McpConfigStore::new(fixture.directory.join("delegation-home/.rovai/mcp.json"));
    let known = ["agent_1".to_string()].into_iter().collect();
    let mut config = Box::new(config_store.get(&known).unwrap());
    for name in ["delegation-selected","delegation-unassigned"] {
        let mut servers=serde_json::Map::new();
        servers.insert(name.to_string(),json!({"command":"node","args":[format!("{name}.js")]}));
        let McpMutationResult::Ok { config:created,.. } = config_store.create(CreateMcpServerParams {
            expected_config_digest:config.config_digest,
            definition_json:json!({"mcpServers":servers}).to_string(),
        },&known).unwrap() else { panic!("create one child MCP fixture server") };
        config=created;
        let id = config.servers.iter().find(|s|s.name==name).unwrap().server_id.clone();
        let McpMutationResult::Ok { config:enabled,.. } = config_store.set_enabled(SetMcpServerEnabledParams {
            expected_config_digest:config.config_digest,server_id:id.clone(),enabled:true,acknowledge_high_risk:false,
        },&known).unwrap() else { panic!("enable MCP fixture") };
        config=enabled;
        if name=="delegation-selected" {
            let McpMutationResult::Ok { config:assigned,.. } = config_store.set_assignment(SetMcpAssignmentParams {
                expected_config_digest:config.config_digest,server_id:id,agent_id:"agent_1".into(),assigned:true,acknowledge_high_risk:false,
            },&known).unwrap() else { panic!("assign MCP fixture") };
            config=assigned;
        }
    }
    let root = std::path::PathBuf::from(snapshot.workspace["executionRoot"].as_str().unwrap());
    let service = McpProjectionService::new(&fixture.directory);
    let request = McpProjectionRequest {agent_run_id:run,execution_epoch:epoch,agent_id:"agent_1",
        adapter_kind:runtime.adapter_kind,reported_runtime_version:runtime.reported_version.as_deref(),execution_root:&root};
    let prepared = service.prepare(&fixture.database,&config_store,&request).unwrap();
    assert_eq!(prepared.servers.keys().map(String::as_str).collect::<Vec<_>>(),vec!["delegation-selected"]);
    let canonical:Value=serde_json::from_slice(&std::fs::read(&prepared.canonical_path).unwrap()).unwrap();
    assert!(canonical["servers"].get("delegation-selected").is_some());
    assert!(canonical["servers"].get("delegation-unassigned").is_none());
    let other_root=fixture.directory.join("wrong-child-root");
    std::fs::create_dir_all(&other_root).unwrap();
    for wrong in [
        McpProjectionRequest {agent_id:"another-member",..request.clone()},
        McpProjectionRequest {adapter_kind:AdapterKind::ClaudeCodeCli,..request.clone()},
        McpProjectionRequest {reported_runtime_version:Some("wrong-version"),..request.clone()},
        McpProjectionRequest {execution_root:&other_root,..request.clone()},
        McpProjectionRequest {execution_epoch:epoch+1,..request.clone()},
    ] {
        assert!(service.prepare(&fixture.database,&config_store,&wrong).is_err(),
            "cached MCP must not bypass child ownership/target validation");
    }
    let selected_id=config.servers.iter().find(|s|s.name=="delegation-selected").unwrap().server_id.clone();
    assert!(matches!(config_store.set_enabled(SetMcpServerEnabledParams {
        expected_config_digest:config.config_digest,server_id:selected_id,enabled:false,acknowledge_high_risk:false,
    },&known).unwrap(),McpMutationResult::Ok{..}));
    let replay=service.prepare(&fixture.database,&config_store,&request).unwrap();
    assert_eq!(prepared.snapshot,replay.snapshot);
    assert_eq!(prepared.projection_digest,replay.projection_digest);
    assert!(crate::local_attachment_source::load_agent_run_source_attachments(&fixture.database,run,epoch).unwrap().is_empty());
    prepared
}

fn assert_delegation_materialization(fixture: &mut Fixture, legacy_private_run: &str) {
    assert_user_delegation_old_database(fixture);
    let old_manifest: String = fixture
        .database
        .connection()
        .query_row(
            "SELECT rendered_payload_blob_id FROM context_manifest WHERE agent_run_id=?1",
            [legacy_private_run],
            |r| r.get(0),
        )
        .unwrap();
    crate::db::delegation::migrate_connection(fixture.database.connection()).unwrap();
    assert_native_interaction_migration(fixture);
    fixture.database.connection().execute_batch("UPDATE delegation_settings SET enabled=1,depth_limit=8 WHERE singleton=1;").unwrap();
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
    let mcp = assert_child_native_projections(fixture, &run, epoch);
    let skill_snapshot = SkillExposureSnapshot::default();
    let skill_exposure = PreparedSkillExposure {
        digest: canonical_json_digest(&serde_json::to_value(&skill_snapshot).unwrap()).unwrap(),
        snapshot: skill_snapshot,
    };
    let store = ManagedBlobStore::new(&fixture.directory);
    let request = MaterializeContextRequest {
        agent_run_id: &run,
        execution_epoch: epoch,
        charter_delivery_mode: CharterDeliveryMode::FirstPayload,
        max_payload_bytes: DEFAULT_MAX_CONTEXT_PAYLOAD_BYTES,
    };
    let mut inherited = skill_exposure.clone();
    inherited.snapshot.skills.push(crate::skill_projection::SkillExposureEntry {
        skill_id:"parent-platform-skill".into(),name:"cli-operations".into(),revision_id:"parent-revision".into(),
        content_digest:"sha256:parent".into(),group_key:"codex".into(),delivered_via_group_key:None,
        status:"ready".into(),entry_path:None,reason_code:None,conflict_statuses:Vec::new(),
    });
    inherited.digest=canonical_json_digest(&serde_json::to_value(&inherited.snapshot).unwrap()).unwrap();
    assert!(ContextService.materialize_with_exposures(&mut fixture.database,&store,&inherited,&mcp,&request).is_err(),
        "child cannot inherit a parent/legacy project projection");
    assert_eq!(fixture.database.connection().query_row("SELECT count(*) FROM context_manifest WHERE agent_run_id=?1",[&run],|r|r.get::<_,i64>(0)).unwrap(),0);
    let ContextMaterialization::Ready(first) = ContextService
        .materialize_with_exposures(&mut fixture.database, &store, &skill_exposure, &mcp, &request)
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
    assert_child_user_materials(fixture, &id, &run, legacy_private_run);

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

    // Construct the public role directly in this formatter fixture; actual Runtime
    // authentication and native acknowledgements retain their explicit fixture boundary.
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
