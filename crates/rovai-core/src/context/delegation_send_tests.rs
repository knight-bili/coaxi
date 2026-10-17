// Initial child/terminal/takeover rows are fixtures, not authenticated lifecycle proof.
fn assert_child_user_send(fixture: &mut Fixture, conversation: &str, original_run: &str) {
    use crate::{
        delegation_user::{self, SendDelegationMessageCommand},
        draft_client::DraftClient,
    };
    let client = DraftClient::verified_web(&"b".repeat(64)).unwrap();
    let camp = fixture.camp_id.clone();
    let actor = ActorRef::User {
        user_id: crate::current_user::CURRENT_USER_ID.into(),
    };
    let (assistant,body):(String,String)=fixture.database.connection().query_row("SELECT id,body FROM conversation_message WHERE conversation_id=?1 AND author_type='agent' AND sequence=4",[conversation],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    let quote = child_material_command(
        fixture,
        &client,
        conversation,
        1,
        json!({"type":"quote","action":{"type":"add","selection":{
        "messageId":assistant,"bodyAtSelection":body,"startScalar":0,"endScalar":body.chars().count(),"text":body}}}),
    );
    assert_eq!(
        delegation_user::mutate_draft(&mut fixture.database, &quote)
            .unwrap()
            .result
            .status,
        crate::command::CommandResultStatus::Applied
    );
    let read = |db: &crate::db::Database| {
        serde_json::to_value(
            delegation_user::read_draft(db, &actor, &client, &camp, conversation, None).unwrap(),
        )
        .unwrap()
    };
    let mut before = read(&fixture.database);
    assert_eq!(before["revision"], 2);
    assert_eq!(before["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(before["quotes"].as_array().unwrap().len(), 1);
    let revision = std::cell::Cell::new(2);
    let make = || CommandEnvelope {
        command_id: Uuid::new_v4().to_string(),
        actor: actor.clone(),
        camp_id: Some(camp.clone()),
        expected_versions: vec![],
        execution_epoch: None,
        payload: SendDelegationMessageCommand {
            draft_client: client.clone(),
            camp_id: camp.clone(),
            conversation_id: conversation.into(),
            composer_scope: None,
            expected_revision: revision.get(),
            body: "继续核对这份文件，不读取队长材料".into(),
        },
    };
    let mut command = make();
    fixture
        .database
        .connection()
        .execute(
            "UPDATE agent_run SET cancel_requested_at='2026-10-17T00:00:09Z',cancel_reason_code='user_takeover' WHERE id=?1",
            [original_run],
        )
        .unwrap();
    assert!(
        delegation_user::send_source_inputs(&fixture.database, &command)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        delegation_user::send(&mut fixture.database, &command)
            .unwrap()
            .result
            .code,
        "delegation.cleanup_pending"
    );
    assert_eq!(read(&fixture.database), before);
    let epoch: i64 = fixture
        .database
        .connection()
        .query_row(
            "SELECT execution_epoch FROM agent_run WHERE id=?1",
            [original_run],
            |r| r.get(0),
        )
        .unwrap();
    crate::runtime::ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&fixture.database, original_run, epoch)
        .unwrap();
    command = make();
    command.payload.expected_revision += 1;
    assert_eq!(
        delegation_user::send(&mut fixture.database, &command)
            .unwrap()
            .result
            .code,
        "single_chat.draft_changed"
    );
    command = make();
    command.payload.draft_client = DraftClient::default();
    assert_eq!(
        delegation_user::send(&mut fixture.database, &command)
            .unwrap()
            .result
            .status,
        crate::command::CommandResultStatus::Rejected
    );
    command = make();
    command.actor = ActorRef::System {
        component_id: "not-user".into(),
    };
    assert_eq!(
        delegation_user::send(&mut fixture.database, &command)
            .unwrap()
            .result
            .status,
        crate::command::CommandResultStatus::Rejected
    );
    let path = fixture.directory.join("web-child-spool.txt");
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, vec![b'!'; bytes.len()]).unwrap();
    command = make();
    assert_eq!(
        delegation_user::send(&mut fixture.database, &command)
            .unwrap()
            .result
            .code,
        "single_chat.attachment_source_changed"
    );
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(read(&fixture.database), before);

    // Restoring bytes alone is not reselection: preserve the source-change fence and explicitly rebind.
    let saved_source:String=fixture.database.connection().query_row("SELECT source_attachments_json FROM single_chat_composer_draft WHERE conversation_id=?1 AND client_id=?2 AND scope_key=''",params![conversation,client.id()],|r|r.get(0)).unwrap();
    let mut source = crate::local_attachment_source::parse_source_attachments(&saved_source)
        .unwrap()
        .remove(0);
    let remove = child_material_command(
        fixture,
        &client,
        conversation,
        2,
        json!({"type":"remove_attachment","attachmentRefId":source.id}),
    );
    assert_eq!(
        delegation_user::mutate_draft(&mut fixture.database, &remove)
            .unwrap()
            .result
            .status,
        crate::command::CommandResultStatus::Applied
    );
    source.id = Uuid::new_v4().to_string();
    let observed = crate::local_attachment_snapshot::observe_private_source(&path).unwrap();
    let intent = crate::web_upload::UploadIntent {
        command_id: source.id.clone(),
        camp_id: camp.clone(),
        expected_revision: 3,
        target: crate::web_upload::UploadTarget::DelegationChild {
            conversation_id: conversation.into(),
            composer_scope: None,
        },
        display_name: source.display_name.clone(),
        source_kind: source.kind,
        byte_size: observed.byte_size,
        sha256: observed.content_digest,
    };
    assert_eq!(
        crate::web_upload::bind(
            &mut fixture.database,
            &fixture.directory,
            &client,
            intent,
            source
        )
        .unwrap()
        .result
        .status,
        crate::command::CommandResultStatus::Applied
    );
    revision.set(4);
    before = read(&fixture.database);
    let selected: Option<String> = fixture
        .database
        .connection()
        .query_row(
            "SELECT selected_runtime_adapter_kind FROM agent_profile WHERE id='agent_1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    fixture
        .database
        .connection()
        .execute(
            "UPDATE agent_profile SET selected_runtime_adapter_kind='pi' WHERE id='agent_1'",
            [],
        )
        .unwrap();
    command = make();
    let prepared = std::sync::Arc::new(
        crate::private_source_observation::PreparedSources::capture(
            delegation_user::send_source_inputs(&fixture.database, &command).unwrap(),
        )
        .unwrap(),
    );
    let rows_before: i64 = fixture
        .database
        .connection()
        .query_row(
            "SELECT count(*) FROM conversation_message WHERE conversation_id=?1",
            [conversation],
            |r| r.get(0),
        )
        .unwrap();
    fixture.database.connection().execute_batch("CREATE TEMP TRIGGER reject_child_user_admission BEFORE INSERT ON agent_run WHEN NEW.invocation_kind='delegation_user' BEGIN SELECT RAISE(ABORT,'injected atomic admission failure'); END;").unwrap();
    let injected = delegation_user::send_with_prepared_sources(
        &mut fixture.database,
        &command,
        prepared.clone(),
    );
    assert!(injected
        .as_ref()
        .unwrap_err()
        .to_string()
        .contains("injected atomic admission failure"));
    assert!(crate::command::DomainCommandGateway
        .replay_if_recorded(&fixture.database, &command)
        .unwrap()
        .is_none());
    assert_eq!(read(&fixture.database), before);
    assert_eq!(
        fixture
            .database
            .connection()
            .query_row(
                "SELECT count(*) FROM conversation_message WHERE conversation_id=?1",
                [conversation],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        rows_before
    );
    fixture
        .database
        .connection()
        .execute_batch("DROP TRIGGER reject_child_user_admission;")
        .unwrap();
    let accepted =
        delegation_user::send_with_prepared_sources(&mut fixture.database, &command, prepared)
            .unwrap();
    assert_eq!(
        accepted.result.status,
        crate::command::CommandResultStatus::Accepted,
        "{:?}",
        accepted.result
    );
    assert_eq!(accepted.result.code, "delegation.reply_queued");
    fixture
        .database
        .connection()
        .execute(
            "UPDATE agent_profile SET selected_runtime_adapter_kind=?1 WHERE id='agent_1'",
            [selected],
        )
        .unwrap();
    let run = accepted.result.payload["agentRunId"].as_str().unwrap();
    let message = accepted.result.payload["conversationMessageId"]
        .as_str()
        .unwrap();
    let (invocation,policy,destination,task,public_boundary,config,workspace):(String,String,String,Option<String>,i64,String,String)=fixture.database.connection().query_row("SELECT invocation_kind,operation_policy,destination_conversation_id,task_id,initial_camp_context_through_sequence,effective_config_json,workspace_json FROM agent_run WHERE id=?1",[run],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).unwrap();
    assert_eq!(invocation, "delegation_user");
    assert_eq!(policy, "delegation_child_v1");
    assert_eq!(destination, conversation);
    assert!(task.is_none());
    assert_eq!(public_boundary, 0);
    let original:(String,String)=fixture.database.connection().query_row("SELECT r.effective_config_json,d.resolved_workspace_json FROM delegation d JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=1 JOIN agent_run r ON r.id=a.agent_run_id WHERE d.child_conversation_id=?1",[conversation],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    let config: Value = serde_json::from_str(&config).unwrap();
    let original_config: Value = serde_json::from_str(&original.0).unwrap();
    assert_eq!(
        config["delegationPolicy"],
        original_config["delegationPolicy"]
    );
    assert_eq!(
        config["runtime"]["adapterKind"],
        original_config["runtime"]["adapterKind"]
    );
    assert_eq!(
        config["runtime"]["permissions"],
        original_config["runtime"]["permissions"]
    );
    assert_eq!(
        serde_json::from_str::<Value>(&workspace).unwrap(),
        serde_json::from_str::<Value>(&original.1).unwrap()
    );
    assert_eq!(
        serde_json::to_value(
            crate::message_quote::load_quotes(
                fixture.database.connection(),
                crate::message_quote::QuoteStorage::PrivateMessage,
                message
            )
            .unwrap()
        )
        .unwrap(),
        before["quotes"]
    );
    let stored: (String, String) = fixture
        .database
        .connection()
        .query_row(
            "SELECT body,source_attachments_json FROM conversation_message WHERE id=?1",
            [message],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(stored.0, command.payload.body);
    assert_eq!(
        crate::local_attachment_source::parse_source_attachments(&stored.1)
            .unwrap()
            .len(),
        1
    );
    let after = read(&fixture.database);
    assert_eq!(after["revision"], 5);
    assert!(
        after["attachments"].as_array().unwrap().is_empty()
            && after["quotes"].as_array().unwrap().is_empty()
    );
    let claim = ExecutionRuntimeService::default()
        .claim_agent_run(
            &mut fixture.database,
            &CommandEnvelope {
                command_id: Uuid::new_v4().to_string(),
                actor: ActorRef::System {
                    component_id: "agent-run-scheduler".into(),
                },
                camp_id: Some(camp.clone()),
                expected_versions: vec![],
                execution_epoch: None,
                payload: ClaimAgentRunCommand {
                    agent_run_id: run.into(),
                    expected_version: 1,
                    lease_owner: "child-user-send-fixture".into(),
                    lease_seconds: 60,
                    workspace: Some(serde_json::from_str(&workspace).unwrap()),
                    starting_git_observation: None,
                },
            },
        )
        .unwrap();
    assert_eq!(
        claim.result.status,
        crate::command::CommandResultStatus::Accepted,
        "{:?}",
        claim.result
    );
    let epoch = claim.result.payload["executionEpoch"].as_i64().unwrap();
    let snapshot = load_run_snapshot(&fixture.database, run, epoch)
        .unwrap()
        .unwrap();
    let current = load_current_input(&fixture.database, &snapshot).unwrap();
    assert_eq!(current.payload["message"], command.payload.body);
    assert_eq!(
        serde_json::to_value(current.quotes).unwrap(),
        before["quotes"]
    );
    assert_eq!(
        crate::local_attachment_source::load_agent_run_source_attachments(
            &fixture.database,
            run,
            epoch
        )
        .unwrap()
        .len(),
        1
    );
    let changes: i64 = fixture
        .database
        .connection()
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(
        delegation_user::send(&mut fixture.database, &command)
            .unwrap()
            .replayed
    );
    assert_eq!(
        changes,
        fixture
            .database
            .connection()
            .query_row("SELECT total_changes()", [], |r| r.get::<_, i64>(0))
            .unwrap()
    );
    let busy = make();
    assert_eq!(
        delegation_user::send(&mut fixture.database, &busy)
            .unwrap()
            .result
            .code,
        "delegation.user_run_active"
    );
    assert_eq!(
        fixture
            .database
            .connection()
            .query_row(
                "SELECT count(*) FROM delegation_attempt WHERE agent_run_id=?1",
                [run],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );

    // Attachment-only input must pass the actual User admission and Context projection too.
    std::fs::write(&path, &bytes).unwrap();
    fixture.database.connection().execute("UPDATE agent_run SET status='cancelled',ended_at='2026-10-17T00:01:00Z',cancel_requested_at='2026-10-17T00:01:00Z',cancel_reason_code='user_takeover',cancel_acknowledged_at='2026-10-17T00:01:00Z' WHERE id=?1",[run]).unwrap();
    let mut source = crate::local_attachment_source::parse_source_attachments(&stored.1)
        .unwrap()
        .remove(0);
    source.id = Uuid::new_v4().to_string();
    let observed = crate::local_attachment_snapshot::observe_private_source(&path).unwrap();
    let intent = crate::web_upload::UploadIntent {
        command_id: source.id.clone(),
        camp_id: camp.clone(),
        expected_revision: 5,
        target: crate::web_upload::UploadTarget::DelegationChild {
            conversation_id: conversation.into(),
            composer_scope: None,
        },
        display_name: source.display_name.clone(),
        source_kind: source.kind,
        byte_size: observed.byte_size,
        sha256: observed.content_digest,
    };
    assert_eq!(
        crate::web_upload::bind(
            &mut fixture.database,
            &fixture.directory,
            &client,
            intent,
            source
        )
        .unwrap()
        .result
        .status,
        crate::command::CommandResultStatus::Applied
    );
    let mut only_file = make();
    only_file.payload.expected_revision = 6;
    only_file.payload.body.clear();
    let accepted = delegation_user::send(&mut fixture.database, &only_file).unwrap();
    assert_eq!(
        accepted.result.status,
        crate::command::CommandResultStatus::Accepted,
        "{:?}",
        accepted.result
    );
    let second_run = accepted.result.payload["agentRunId"].as_str().unwrap();
    let claim = ExecutionRuntimeService::default()
        .claim_agent_run(
            &mut fixture.database,
            &CommandEnvelope {
                command_id: Uuid::new_v4().to_string(),
                actor: ActorRef::System {
                    component_id: "agent-run-scheduler".into(),
                },
                camp_id: Some(camp.clone()),
                expected_versions: vec![],
                execution_epoch: None,
                payload: ClaimAgentRunCommand {
                    agent_run_id: second_run.into(),
                    expected_version: 1,
                    lease_owner: "child-file-send-fixture".into(),
                    lease_seconds: 60,
                    workspace: Some(serde_json::from_str(&workspace).unwrap()),
                    starting_git_observation: None,
                },
            },
        )
        .unwrap();
    assert_eq!(
        claim.result.status,
        crate::command::CommandResultStatus::Accepted,
        "{:?}",
        claim.result
    );
    let epoch = claim.result.payload["executionEpoch"].as_i64().unwrap();
    let snapshot = load_run_snapshot(&fixture.database, second_run, epoch)
        .unwrap()
        .unwrap();
    assert_eq!(
        load_current_input(&fixture.database, &snapshot)
            .unwrap()
            .payload["message"],
        ""
    );
    assert_eq!(
        crate::local_attachment_source::load_agent_run_source_attachments(
            &fixture.database,
            second_run,
            epoch
        )
        .unwrap()
        .len(),
        1
    );
}
