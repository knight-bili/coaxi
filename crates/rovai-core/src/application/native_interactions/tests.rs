//! Coupled seam owner: existing Core fixture + real managed test ACP process.
//! Codec/CAS/write races remain owned by the smaller existing owners.
use super::*;
use crate::{
    agent_profile::{
        AdapterPermissionConfig, CreateAdapterInstallationCommand, InstallationSource,
        MemberRuntimeConfiguration, ModelSelection,
    },
    context::{ContextMaterialization, MaterializeContextRequest},
    delegation_contract::RuntimeTarget,
    delegation_factory::{self as factory, ChildRunPlan},
    delegation_policy::{FrozenPolicy, Role},
    runtime::ClaimAgentRunCommand,
};

fn command<P>(camp: Option<&str>, payload: P) -> CommandEnvelope<P> {
    CommandEnvelope {
        command_id: uuid::Uuid::new_v4().to_string(),
        actor: ActorRef::User {
            user_id: CURRENT_USER_ID.into(),
        },
        camp_id: camp.map(str::to_owned),
        expected_versions: vec![],
        execution_epoch: None,
        payload,
    }
}

fn claim(
    db: &mut Database,
    camp: &str,
    run: &str,
    workspace: &crate::runtime::AgentRunWorkspace,
) -> i64 {
    let version = ExecutionRuntimeService::default()
        .list_dispatchable_agent_runs(db, 8)
        .unwrap()
        .into_iter()
        .find(|r| r.agent_run_id == run)
        .expect("dispatchable fixture Run")
        .version;
    let mut request = command(
        Some(camp),
        ClaimAgentRunCommand {
            agent_run_id: run.into(),
            expected_version: version,
            lease_owner: "native-seam-fixture".into(),
            lease_seconds: 120,
            workspace: Some(workspace.clone()),
            starting_git_observation: None,
        },
    );
    request.actor = ActorRef::System {
        component_id: "agent-run-scheduler".into(),
    };
    let result = ExecutionRuntimeService::default()
        .claim_agent_run(db, &request)
        .unwrap();
    assert_eq!(
        result.result.status,
        CommandResultStatus::Accepted,
        "{:?}",
        result.result
    );
    result.result.payload["executionEpoch"].as_i64().unwrap()
}

async fn request(core: &Arc<Core>, method: &str, params: Value) -> Value {
    core.handle(&serde_json::from_value(json!({"id":1,"method":method,"params":params})).unwrap())
        .await
        .unwrap()
}

async fn wait_items(
    core: &Arc<Core>,
    conversation: &str,
    count: usize,
    status: &str,
) -> Vec<Value> {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let page = request(
                core,
                "runtime.interactions.list",
                json!({"conversationId":conversation}),
            )
            .await;
            let items = page["items"].as_array().unwrap();
            let matched: Vec<_> = items
                .iter()
                .filter(|v| v["status"] == status)
                .cloned()
                .collect();
            if matched.len() == count {
                return matched;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("native callback state must converge")
}

#[tokio::test]
async fn native_child_questions_cross_the_core_reader_and_exact_user_reply_then_close_on_host_exit()
{
    let root = std::env::temp_dir().join(format!("rovai-native-seam-{}", uuid::Uuid::new_v4()));
    let workspace_path = root.join("workspace");
    std::fs::create_dir_all(&workspace_path).unwrap();
    let script = root.join("peer.mjs");
    // Keep fixture-only protocol changes out of Rust recompilation. The peer is
    // copied into this test's exact root, independent of the command's cwd.
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/fixtures/native-interaction-acp-runtime.mjs"),
        &script,
    )
    .unwrap();
    #[cfg(windows)]
    let executable = {
        let node = crate::runtime_discovery::resolve_active_command_path("node")
            .expect("Node fixture dependency");
        let path = root.join("peer.cmd");
        std::fs::write(
            &path,
            format!(
                "@echo off\r\n\"{}\" \"{}\" %*\r\n",
                dunce::simplified(&node).display(),
                script.display()
            ),
        )
        .unwrap();
        path
    };
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script.clone()
    };
    // A Windows command shim's interpreter/content identity participates in the
    // frozen Runtime. Register real file identity, without inventing probe health.
    let verify_path = executable.clone();
    let verified = tokio::task::spawn_blocking(move || {
        crate::agent_profile::DiscoveredRuntimeEntry {
            adapter_kind: AdapterKind::GrokBuild,
            executable_path: verify_path.to_string_lossy().into_owned(),
            source: InstallationSource::Custom,
            executable_fingerprint: crate::agent_runtime_adapter::executable_fingerprint(
                &verify_path,
            )?,
            entrypoint_locator_identity: None,
        }
        .verify()
    })
    .await
    .unwrap()
    .unwrap();
    let mut core = super::super::tests::runtime_resolution_test_core(&root).unwrap();
    let (incoming, mut receiver) = mpsc::unbounded_channel();
    core.grok_build = AcpCliRuntimeAdapter::new(
        AdapterKind::GrokBuild,
        incoming,
        core.data_dir.join("runtime/grok-build"),
        core.runtime_fleet.clone(),
        CompactionDetectorPolicy::Disabled,
    )
    .unwrap();
    let core = Arc::new(core);
    let enabled = request(
        &core,
        "delegations.settings.update",
        json!({
            "commandId":uuid::Uuid::new_v4().to_string(),
            "command":{"expectedVersion":1,"patch":{"enabled":true}}
        }),
    )
    .await;
    assert_eq!(enabled["code"], "delegation.settings_updated");
    let workspace = crate::runtime::AgentRunWorkspace::runtime_managed_path(
        workspace_path.to_string_lossy().into_owned(),
    );
    let (camp, child, run, epoch, credential, execution) = {
        let mut db = core.database.lock().await;
        crate::agent_profile::configure_test_runtime(&db, &["agent_1"]);
        let camp_result = CollaborationService::default()
            .create_camp(
                &mut db,
                &command(
                    None,
                    CreateThreadCommand::for_test(workspace.execution_root.clone()),
                ),
            )
            .unwrap();
        let camp = camp_result.result.payload["threadId"]
            .as_str()
            .unwrap()
            .to_owned();
        let opened = SingleChatService::default()
            .open(
                &mut db,
                &command(
                    Some(&camp),
                    OpenSingleChatCommand {
                        draft_client: Default::default(),
                        camp_id: camp.clone(),
                        agent_id: "agent_1".into(),
                    },
                ),
            )
            .unwrap();
        let parent_conversation = opened.result.payload["conversationId"].as_str().unwrap();
        let revision = db
            .connection()
            .query_row(
                "SELECT revision FROM single_chat_composer_draft WHERE conversation_id=?1",
                [parent_conversation],
                |r| r.get(0),
            )
            .unwrap();
        let sent = SingleChatService::default()
            .send(
                &mut db,
                &command(
                    Some(&camp),
                    SendSingleChatMessageCommand {
                        composer_scope: None,
                        draft_client: Default::default(),
                        camp_id: camp.clone(),
                        conversation_id: parent_conversation.into(),
                        body: "受控测试父任务".into(),
                        draft_revision: revision,
                    },
                ),
            )
            .unwrap();
        let parent = sent.result.payload["agentRunId"].as_str().unwrap();
        let parent_epoch = claim(&mut db, &camp, parent, &workspace);
        let parent_credential = TeamToolService::default()
            .prepare_binding_credential(&mut db, parent, parent_epoch, false)
            .unwrap();
        let installed = AgentProfileService::default()
            .create_installation(
                &mut db,
                &command(
                    None,
                    CreateAdapterInstallationCommand {
                        adapter_kind: AdapterKind::GrokBuild,
                        executable_path: executable.to_string_lossy().into_owned(),
                        command_name: "controlled-grok".into(),
                        source: InstallationSource::Custom,
                        auth_scope: "native-seam".into(),
                    },
                ),
            )
            .unwrap();
        let installation = installed.result.payload["installationId"].as_str().unwrap();
        assert_eq!(
            AgentProfileService::default()
                .commit_discovered_runtime_entry(&mut db, verified, Some(installation))
                .unwrap(),
            installation
        );
        let target = RuntimeTarget::try_from("grok".to_owned()).unwrap();
        let task = "仅运行隔离假队员，验证原生提问往返。";
        let blob = ManagedBlobStore::new(&core.data_dir)
            .put_bytes(&mut db, task.as_bytes(), "text/plain", "sensitive")
            .unwrap();
        let configuration = MemberRuntimeConfiguration {
            adapter_kind: AdapterKind::GrokBuild,
            model: ModelSelection::RuntimeDefault,
            permissions: AdapterPermissionConfig {
                adapter_kind: AdapterKind::GrokBuild,
                schema_version: 1,
                values: crate::agent_runtime_adapter::AgentRuntimeAdapterRegistry::default()
                    .member_permission_defaults(AdapterKind::GrokBuild),
            },
        };
        let tx = db.connection_mut().transaction().unwrap();
        let created = factory::register_initial(
            &tx,
            &crate::delegation::ParentInvocation {
                run_id: parent,
                epoch: parent_epoch,
                binding_id: &parent_credential.native_binding_id,
                binding_generation: parent_credential.native_binding_generation,
                tool_call_id: "native-seam-child",
            },
            &crate::delegation::FrozenRequest {
                task_blob_id: &blob.id,
                runtime: &target,
                working_dir: None,
                workspace: &serde_json::to_value(&workspace).unwrap(),
                runtime_preset: &json!({}),
            },
            &ChildRunPlan {
                task,
                target: &target,
                installation_id: installation,
                configuration: &configuration,
                workspace: &workspace,
                policy: FrozenPolicy::new(true, 8, Role::Child).unwrap(),
            },
            &chrono::Utc::now().to_rfc3339(),
        )
        .unwrap();
        let run: String = tx
            .query_row(
                "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
                [&created.identity.id],
                |r| r.get(0),
            )
            .unwrap();
        tx.commit().unwrap();
        let epoch = claim(&mut db, &camp, &run, &workspace);
        let credential = TeamToolService::default()
            .prepare_binding_credential(&mut db, &run, epoch, false)
            .unwrap();
        let execution = ExecutionRuntimeService::default()
            .load_agent_run_execution(&db, &run, epoch)
            .unwrap()
            .unwrap();
        (
            camp,
            created.identity.child_conversation_id,
            run,
            epoch,
            credential,
            execution,
        )
    };
    let tools = BuiltinToolProcessConfig::create(
        &std::env::current_exe().unwrap(),
        &builtin_tool_endpoint(),
        &core.data_dir.join("runtime"),
    )
    .unwrap();
    eprintln!(
        "[native-interaction-fixture] isolated_root={}",
        root.display()
    );
    let runtime = core
        .grok_build
        .ensure_agent_run_runtime(
            &run,
            epoch,
            &camp,
            &execution.agent_id,
            &credential.native_binding_id,
            &workspace,
            execution.permission_semantics,
            &execution.runtime,
            &tools,
            &BTreeMap::new(),
            "sha256:fixture-projection",
            &workspace_path,
            "sha256:fixture-runtime",
        )
        .await
        .unwrap_or_else(|error| {
            while let Ok(message) = receiver.try_recv() {
                eprintln!("[controlled-acp-startup] {message:?}");
            }
            panic!("controlled ACP startup failed: {error:#}");
        });
    let session = runtime
        .start_or_resume_session_with_native_rules(
            None,
            runtime.session_capabilities().await,
            &execution.runtime.model.source,
            &execution.runtime.model.model_id,
            &execution.runtime.model.options,
            &BTreeMap::new(),
            Some("Controlled ACP fixture; no provider or external tools."),
        )
        .await
        .unwrap();
    let delivery = {
        let mut db = core.database.lock().await;
        let mut bind = command(
            Some(&camp),
            crate::runtime::BindNativeSessionCommand {
                conversation_id: child.clone(),
                agent_run_id: run.clone(),
                expected_conversation_version: credential.conversation_version,
                expected_execution_epoch: epoch,
                previous_adapter_installation_id: credential.previous_adapter_installation_id,
                previous_native_session_id: credential.previous_native_session_id,
                previous_binding_compatibility_digest: credential
                    .previous_binding_compatibility_digest,
                proposed_binding_id: Some(credential.native_binding_id),
                adapter_installation_id: execution.runtime.installation_id,
                native_session_id: session,
                binding_compatibility_digest: execution.runtime.binding_compatibility_digest,
            },
        );
        bind.actor = ActorRef::System {
            component_id: "runtime-adapter:native-fixture".into(),
        };
        assert_eq!(
            ExecutionRuntimeService::default()
                .bind_native_session(&mut db, &bind)
                .unwrap()
                .result
                .status,
            CommandResultStatus::Applied
        );
        let ContextMaterialization::Ready(context) = ContextService
            .materialize(
                &mut db,
                &ManagedBlobStore::new(&core.data_dir),
                &MaterializeContextRequest {
                    agent_run_id: &run,
                    execution_epoch: epoch,
                    charter_delivery_mode: CharterDeliveryMode::FirstPayload,
                    max_payload_bytes: crate::context::DEFAULT_MAX_CONTEXT_PAYLOAD_BYTES,
                },
            )
            .unwrap()
        else {
            panic!("context unavailable")
        };
        let delivery = ContextService
            .prepare_input_delivery_for_context(&mut db, &run, epoch, &context)
            .unwrap();
        assert!(
            ContextService
                .begin_runtime_input_dispatch(&mut db, &delivery.id, &run, epoch)
                .unwrap()
        );
        (delivery.id, context.runtime_payload)
    };
    let (stop, shutdown) = oneshot::channel();
    let listener = tokio::spawn(process_acp_events(
        core.clone(),
        receiver,
        core.output.clone(),
        shutdown,
    ));
    runtime
        .start_prompt(&delivery.0, &delivery.1)
        .await
        .unwrap();
    let pending = wait_items(&core, &child, 2, "pending").await;
    let mut decisions = vec![];
    for item in pending {
        let view = request(
            &core,
            "runtime.interactions.get",
            json!({"interactionId":item["id"]}),
        )
        .await;
        assert_eq!(view["conversationId"], child);
        assert!(view.get("nativeSessionId").is_none());
        let answer = if view["kind"] == "question" {
            json!({"kind":"question","answers":[{"questionId":"q0","labels":["接口"]}],"declined":false})
        } else {
            json!({"kind":"plan_approval","decision":"approve","feedback":null})
        };
        let params = json!({"commandId":uuid::Uuid::new_v4().to_string(),"command":{
            "interactionId":item["id"],"expectedVersion":item["version"],"answer":answer}});
        let result = request(&core, "runtime.interactions.resolve", params.clone()).await;
        assert_eq!(result["code"], "interaction.resolved", "{result:?}");
        decisions.push((params, result));
    }
    wait_items(&core, &child, 2, "sent").await;
    wait_items(&core, &child, 1, "pending").await;
    let trace = std::fs::read_to_string(root.join("wire.jsonl")).unwrap();
    let replies: Vec<Value> = trace
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 2);
    assert_eq!(
        replies
            .iter()
            .find(|r| r["id"] == " question rpc ")
            .unwrap()["result"],
        json!({"outcome":"accepted","answers":{"先做哪一项？":"接口"},"partial_answers":{}})
    );
    assert_eq!(
        replies.iter().find(|r| r["id"] == "plan-rpc").unwrap()["result"],
        json!({"outcome":"approved","feedback":""})
    );
    // An exact command retry returns its durable receipt without a second native write.
    for (params, expected) in decisions {
        assert_eq!(
            request(&core, "runtime.interactions.resolve", params).await,
            expected
        );
    }
    // The controlled peer exits itself while the third request is outstanding.
    // Require the production Exited callback, rather than the polling fallback.
    runtime
        .respond(json!("fixture-exit"), json!({}))
        .await
        .unwrap();
    let closed = wait_items(&core, &child, 1, "closed").await;
    let view = request(
        &core,
        "runtime.interactions.get",
        json!({"interactionId":closed[0]["id"]}),
    )
    .await;
    assert_eq!(view["closeReason"], "native_host_exited");
    core.grok_build.shutdown_all().await;
    assert_eq!(
        std::fs::read_to_string(root.join("wire.jsonl")).unwrap(),
        trace
    );
    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(5), listener)
        .await
        .unwrap()
        .unwrap();
    core.agent_run_tasks.lock().await.shutdown().await;
    drop(runtime);
    drop(core);
    // Only this test-owned, resolved temporary root is removed.
    assert!(root.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(root).unwrap();
}
