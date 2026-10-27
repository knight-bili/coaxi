// Extends the existing Core owner. Terminal/takeover fixtures do not represent live processes.
fn queue_actor() -> ActorRef {
    ActorRef::User {
        user_id: crate::current_user::CURRENT_USER_ID.into(),
    }
}
fn queue_draft(f: &Fixture, c: &crate::draft_client::DraftClient, conversation: &str) -> Value {
    serde_json::to_value(
        crate::delegation_user::read_draft(
            &f.database,
            &queue_actor(),
            c,
            &f.camp_id,
            conversation,
            None,
        )
        .unwrap(),
    )
    .unwrap()
}
fn queue_read(
    f: &Fixture,
    c: &crate::draft_client::DraftClient,
    conversation: &str,
) -> crate::single_chat::SingleChatPendingInputsView {
    let view = crate::delegation_user::read_pending_inputs(
        &f.database,
        &queue_actor(),
        c,
        &f.camp_id,
        conversation,
        None,
    )
    .unwrap();
    let api = crate::application::delegation_input::read(
        &f.database,
        c,
        json!({"conversationId":conversation}),
    )
    .unwrap();
    assert_eq!(api, serde_json::to_value(&view).unwrap());
    assert!(!serde_json::to_string(&api).unwrap().contains("sourcePath"));
    view
}
fn queue_send(
    f: &mut Fixture,
    c: &crate::draft_client::DraftClient,
    conversation: &str,
    body: &str,
) -> String {
    let revision = queue_draft(f, c, conversation)["revision"]
        .as_i64()
        .unwrap();
    let request = json!({"commandId":Uuid::new_v4().to_string(),"command":{
        "conversationId":conversation,"expectedRevision":revision,"body":body}});
    use crate::application::delegation_input as api;
    let command = api::send_envelope(&f.database, c, request.clone()).unwrap();
    assert_eq!(command.camp_id.as_deref(), Some(f.camp_id.as_str()));
    assert_eq!(command.payload.draft_client, *c);
    for field in [
        "threadId",
        "draftClient",
        "sourcePath",
        "actor",
        "attachments",
    ] {
        let mut forged = request.clone();
        forged["command"][field] = json!("forged");
        assert!(
            api::send_envelope(&f.database, c, forged).is_err(),
            "{field}"
        );
    }
    let before = f.database.connection().total_changes();
    assert_eq!(
        api::reconcile(&f.database, c, "delegations.message.send", request.clone()).unwrap()
            ["state"],
        "unknown"
    );
    assert_eq!(before, f.database.connection().total_changes());
    let result = crate::delegation_user::send(&mut f.database, &command).unwrap();
    assert_eq!(
        result.result.status,
        crate::command::CommandResultStatus::Accepted,
        "{:?}",
        result.result
    );
    let id = result.result.payload["pendingInputId"]
        .as_str()
        .expect("input must queue while a User Run is active")
        .to_string();
    let changes: i64 = f
        .database
        .connection()
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    assert!(
        crate::delegation_user::send(&mut f.database, &command)
            .unwrap()
            .replayed
    );
    assert_eq!(
        changes,
        f.database
            .connection()
            .query_row("SELECT total_changes()", [], |r| r.get::<_, i64>(0))
            .unwrap()
    );
    let stored =
        api::reconcile(&f.database, c, "delegations.message.send", request.clone()).unwrap();
    assert_eq!(stored["state"], "recorded");
    assert_eq!(
        stored["result"],
        serde_json::to_value(&result.result).unwrap()
    );
    let mut changed = request;
    changed["command"]["body"] = json!("different input");
    // Gateway returns the first submission even when retry payload bytes change.
    assert_eq!(api::reconcile(&f.database, c, "delegations.message.send", changed).unwrap(), stored);
    assert_eq!(changes,f.database.connection().query_row("SELECT total_changes()", [], |r|r.get::<_,i64>(0)).unwrap());
    id
}
fn queue_edit_command(
    f: &Fixture,
    c: &crate::draft_client::DraftClient,
    conversation: &str,
    id: &str,
    revision: i64,
    token: Option<&str>,
    action: crate::single_chat::SingleChatPendingInputEditAction,
) -> CommandEnvelope<crate::delegation_user::EditDelegationPendingInputCommand> {
    let request = json!({"commandId":Uuid::new_v4().to_string(),"command":{
        "conversationId":conversation,"pendingInputId":id,"expectedRevision":revision,
        "editToken":token,"action":action}});
    let envelope =
        crate::application::delegation_input::edit_envelope(&f.database, c, request.clone())
            .unwrap();
    for field in ["threadId", "draftClient", "sourcePath", "actor"] {
        let mut forged = request.clone();
        forged["command"][field] = json!("forged");
        assert!(
            crate::application::delegation_input::edit_envelope(&f.database, c, forged).is_err()
        );
    }
    envelope
}
fn queue_edit(
    f: &mut Fixture,
    command: &CommandEnvelope<crate::delegation_user::EditDelegationPendingInputCommand>,
) -> crate::command::CommandExecution {
    let sources = crate::delegation_user::edit_source_inputs(&f.database, command).unwrap();
    let prepared = std::sync::Arc::new(
        crate::private_source_observation::PreparedSources::capture(sources).unwrap(),
    );
    let result =
        crate::delegation_user::edit_with_prepared_sources(&mut f.database, command, prepared)
            .unwrap();
    let mut input = serde_json::to_value(&command.payload.0).unwrap();
    input.as_object_mut().unwrap().remove("threadId");
    input.as_object_mut().unwrap().remove("draftClient");
    let request = json!({"commandId":command.command_id,"command":input});
    let before = f.database.connection().total_changes();
    use crate::application::delegation_input as api;
    let stored = api::reconcile(
        &f.database,
        &command.payload.0.draft_client,
        "delegations.pendingInputs.edit",
        request.clone(),
    )
    .unwrap();
    assert_eq!(stored["state"], "recorded");
    assert_eq!(
        stored["result"],
        serde_json::to_value(&result.result).unwrap()
    );
    let other = crate::draft_client::DraftClient::verified_web(&"c".repeat(64)).unwrap();
    assert!(api::reconcile(
        &f.database,
        &other,
        "delegations.pendingInputs.edit",
        request
    )
    .is_err());
    assert_eq!(before, f.database.connection().total_changes());
    result
}
fn queue_publish(
    f: &mut Fixture,
    command: crate::delegation_user::PublishDelegationPendingInput,
) -> crate::command::CommandExecution {
    let envelope = CommandEnvelope {
        command_id: Uuid::new_v4().to_string(),
        actor: ActorRef::User {
            user_id: command.0.user_id.clone(),
        },
        camp_id: Some(command.0.camp_id.clone()),
        expected_versions: vec![],
        execution_epoch: None,
        payload: command,
    };
    let sources = crate::delegation_user::publish_source_inputs(&f.database, &envelope).unwrap();
    let prepared = std::sync::Arc::new(
        crate::private_source_observation::PreparedSources::capture(sources).unwrap(),
    );
    crate::delegation_user::publish_with_prepared_sources(&mut f.database, &envelope, prepared)
        .unwrap()
}
fn queue_terminal(f: &Fixture, run: &str) {
    f.database.connection().execute("UPDATE agent_run SET status='cancelled',ended_at='2026-10-17T00:02:00Z',cancel_requested_at='2026-10-17T00:02:00Z',cancel_reason_code='user_request',cancel_acknowledged_at='2026-10-17T00:02:00Z',runtime_recovery_required=0 WHERE id=?1",[run]).unwrap();
}
fn queue_upload(
    f: &mut Fixture,
    conversation: &str,
    name: &str,
) -> crate::local_attachment_source::LocalAttachmentSourceRef {
    let client = crate::draft_client::DraftClient::default();
    let path = f.directory.join(name);
    std::fs::write(&path, b"child queue bytes").unwrap();
    let revision = queue_draft(f, &client, conversation)["revision"]
        .as_i64()
        .unwrap();
    let observed = crate::web_upload::observe_native(
        &client,
        crate::web_upload::NativeUploadPreparation {
            command_id: Uuid::new_v4().to_string(),
            camp_id: f.camp_id.clone(),
            expected_revision: revision,
            target: crate::web_upload::UploadTarget::DelegationChild {
                conversation_id: conversation.into(),
                composer_scope: None,
            },
            source_path: path.display().to_string(),
            display_name: name.into(),
            media_type: Some("text/plain".into()),
        },
    )
    .unwrap();
    let intent = serde_json::from_value(observed["intent"].clone()).unwrap();
    let source =
        serde_json::from_value::<crate::local_attachment_source::LocalAttachmentSourceRef>(
            observed["source"].clone(),
        )
        .unwrap();
    assert_eq!(
        crate::web_upload::bind(
            &mut f.database,
            &f.directory,
            &client,
            intent,
            source.clone()
        )
        .unwrap()
        .result
        .status,
        crate::command::CommandResultStatus::Applied
    );
    source
}
fn assert_child_user_queue(f: &mut Fixture, conversation: &str, active: &str) {
    use crate::{
        delegation_user as child,
        single_chat::{PublishSingleChatPendingInput, SingleChatPendingInputEditAction as Action},
    };
    let native = crate::draft_client::DraftClient::default();
    for command in ["delegations.message.send","delegations.pending_input.edit","delegations.pending_input.publish"] {
        let wake = crate::execution_wake::ExecutionWake::default();
        wake.command_committed(command);
        let mut notified = std::pin::pin!(wake.runs.notified());
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(std::future::Future::poll(notified.as_mut(), &mut context).is_ready(), "{command} must wake dispatch");
    }
    let web = crate::draft_client::DraftClient::verified_web(&"b".repeat(64)).unwrap();
    let first = queue_upload(f, conversation, "queue-first.txt");
    let second = queue_upload(f, conversation, "queue-second.txt");
    let (assistant,body):(String,String)=f.database.connection().query_row("SELECT id,body FROM conversation_message WHERE conversation_id=?1 AND author_type='agent' AND sequence=4",[conversation],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    let revision = queue_draft(f, &native, conversation)["revision"]
        .as_i64()
        .unwrap();
    let quote = child_material_command(
        f,
        &native,
        conversation,
        revision,
        json!({"type":"quote","action":{"type":"add","selection":{"messageId":assistant,"bodyAtSelection":body,"startScalar":0,"endScalar":body.chars().count(),"text":body}}}),
    );
    assert_eq!(
        child::mutate_draft(&mut f.database, &quote)
            .unwrap()
            .result
            .status,
        crate::command::CommandResultStatus::Applied
    );
    let a = queue_send(f, &native, conversation, "第一份排队输入");
    let b = queue_send(f, &web, conversation, "第二份排队输入");
    let view = queue_read(f, &native, conversation);
    assert!(view.execution_active);
    assert_eq!(
        view.items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        [a.as_str(), b.as_str()]
    );
    assert_eq!(view.items[0].attachments.len(), 2);
    assert_eq!(view.items[0].quotes.len(), 1);
    assert!(child::ready_pending_inputs(&f.database).unwrap().is_empty());
    assert!(crate::single_chat::ready_pending_inputs(&f.database)
        .unwrap()
        .iter()
        .all(|i| i.conversation_id != conversation));
    let mut command = queue_edit_command(f, &native, conversation, &a, 1, None, Action::Begin);
    let legacy = CommandEnvelope {
        command_id: Uuid::new_v4().to_string(),
        actor: command.actor.clone(),
        camp_id: command.camp_id.clone(),
        expected_versions: vec![],
        execution_epoch: None,
        payload: command.payload.0.clone(),
    };
    assert_eq!(
        crate::single_chat::SingleChatService::default()
            .edit_pending_input(&mut f.database, &legacy)
            .unwrap()
            .result
            .code,
        "single_chat.not_active"
    );
    let opened = queue_edit(f, &command);
    let old_token = opened.result.payload["editToken"]
        .as_str()
        .unwrap()
        .to_string();
    let frozen_pending_upload = assert_pending_native_upload(f, conversation, &a, &old_token);
    assert!(queue_read(f, &web, conversation).edit_session.is_none());
    command = queue_edit_command(f, &web, conversation, &a, 1, None, Action::Takeover);
    let reopened = queue_edit(f, &command);
    let token = reopened.result.payload["editToken"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(token, old_token);
    let uploaded_pending = assert_pending_web_upload(
        f,
        conversation,
        &a,
        &b,
        &old_token,
        &token,
        frozen_pending_upload,
    );
    let stale = queue_edit_command(
        f,
        &native,
        conversation,
        &a,
        1,
        Some(&old_token),
        Action::Save {
            body: "旧编辑不得覆盖".into(),
        },
    );
    assert!(child::edit_source_inputs(&f.database, &stale)
        .unwrap()
        .is_empty());
    assert_eq!(
        queue_edit(f, &stale).result.code,
        "single_chat.pending_input_edit_fenced"
    );
    let reorder = queue_edit_command(
        f,
        &web,
        conversation,
        &a,
        1,
        Some(&token),
        Action::ReorderAttachments {
            attachment_ref_ids: vec![
                second.id.clone(),
                first.id.clone(),
                uploaded_pending.clone(),
            ],
        },
    );
    assert_eq!(
        queue_edit(f, &reorder).result.status,
        crate::command::CommandResultStatus::Applied
    );
    let invalid = queue_edit_command(
        f,
        &web,
        conversation,
        &a,
        1,
        Some(&token),
        Action::ReorderAttachments {
            attachment_ref_ids: vec![first.id.clone(), first.id.clone()],
        },
    );
    assert_eq!(
        queue_edit(f, &invalid).result.code,
        "attachment_order_changed"
    );
    let save = queue_edit_command(
        f,
        &web,
        conversation,
        &a,
        1,
        Some(&token),
        Action::Save {
            body: "保存后的第一份输入".into(),
        },
    );
    assert_eq!(
        queue_edit(f, &save).result.status,
        crate::command::CommandResultStatus::Applied
    );
    let saved_upload =
        crate::local_attachment_source::LocalAttachmentOwnerLocator::DelegationChildPending {
            camp_id: f.camp_id.clone(),
            conversation_id: conversation.into(),
            pending_input_id: a.clone(),
            attachment_ref_id: uploaded_pending.clone(),
        };
    assert!(
        crate::local_attachment_source::load_source_attachment_for_client(
            &f.database,
            &saved_upload,
            &native
        )
        .unwrap()
        .is_some()
    );
    assert!(
        crate::local_attachment_source::load_source_attachment_for_client(
            &f.database,
            &pending_edit_locator(f, conversation, &a, &token, &uploaded_pending),
            &web
        )
        .unwrap()
        .is_none()
    );
    let begin = queue_edit_command(f, &web, conversation, &a, 2, None, Action::Begin);
    let begun = queue_edit(f, &begin);
    let lease = begun.result.payload["editToken"].as_str().unwrap();
    crate::single_chat::recover_pending_edit_sessions(&f.database).unwrap();
    let stale = queue_edit_command(
        f,
        &web,
        conversation,
        &a,
        2,
        Some(lease),
        Action::Save {
            body: "重启前租约不可提交".into(),
        },
    );
    assert_eq!(
        queue_edit(f, &stale).result.code,
        "single_chat.pending_input_edit_fenced"
    );
    let takeover = queue_edit_command(f, &web, conversation, &a, 2, None, Action::Takeover);
    let owned = queue_edit(f, &takeover);
    let cancel = queue_edit_command(
        f,
        &web,
        conversation,
        &a,
        2,
        owned.result.payload["editToken"].as_str(),
        Action::Cancel,
    );
    assert_eq!(
        queue_edit(f, &cancel).result.status,
        crate::command::CommandResultStatus::Applied
    );
    queue_terminal(f, active);
    f.database
        .connection()
        .execute(
            "UPDATE agent_run SET cancel_acknowledged_at=NULL WHERE id=?1",
            [active],
        )
        .unwrap();
    assert!(child::ready_pending_inputs(&f.database).unwrap().is_empty());
    let waiting = child::PublishDelegationPendingInput(PublishSingleChatPendingInput {
        camp_id: f.camp_id.clone(),
        conversation_id: conversation.into(),
        pending_input_id: a.clone(),
        expected_revision: 2,
        user_id: crate::current_user::CURRENT_USER_ID.into(),
    });
    assert_eq!(
        queue_publish(f, waiting).result.code,
        "single_chat.pending_input_not_ready"
    );
    let epoch: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT execution_epoch FROM agent_run WHERE id=?1",
            [active],
            |r| r.get(0),
        )
        .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, active, epoch)
        .unwrap();
    let ready = child::ready_pending_inputs(&f.database).unwrap();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].0.pending_input_id, a);
    let wrong = child::PublishDelegationPendingInput(PublishSingleChatPendingInput {
        camp_id: f.camp_id.clone(),
        conversation_id: conversation.into(),
        pending_input_id: b.clone(),
        expected_revision: 1,
        user_id: crate::current_user::CURRENT_USER_ID.into(),
    });
    assert_eq!(
        queue_publish(f, wrong).result.code,
        "single_chat.pending_input_not_ready"
    );
    let published = queue_publish(f, ready[0].clone());
    assert_eq!(
        published.result.status,
        crate::command::CommandResultStatus::Accepted,
        "{:?}",
        published.result
    );
    let run_a = published.result.payload["agentRunId"].as_str().unwrap();
    let message = published.result.payload["conversationMessageId"]
        .as_str()
        .unwrap();
    let (saved, sources, quotes): (String, String, String) = f
        .database
        .connection()
        .query_row(
            "SELECT body,source_attachments_json,quotes_json FROM conversation_message WHERE id=?1",
            [message],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(saved, "保存后的第一份输入");
    let refs = crate::local_attachment_source::parse_source_attachments(&sources).unwrap();
    assert_eq!(
        refs.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        [
            second.id.as_str(),
            first.id.as_str(),
            uploaded_pending.as_str()
        ]
    );
    assert_eq!(
        crate::message_quote::parse_quotes(&quotes).unwrap().len(),
        1
    );
    assert!(child::ready_pending_inputs(&f.database).unwrap().is_empty());
    queue_terminal(f, run_a);
    let ready = child::ready_pending_inputs(&f.database).unwrap();
    assert_eq!(ready[0].0.pending_input_id, b);
    let published = queue_publish(f, ready[0].clone());
    assert_eq!(
        published.result.status,
        crate::command::CommandResultStatus::Accepted
    );
    let run_b = published.result.payload["agentRunId"].as_str().unwrap();

    queue_upload(f, conversation, "queue-return.txt");
    let c = queue_send(f, &native, conversation, "回移时必须保留的材料");
    let target_before = queue_draft(f, &web, conversation);
    let revision = target_before["revision"].as_i64().unwrap();
    let returning = queue_edit_command(
        f,
        &web,
        conversation,
        &c,
        1,
        None,
        Action::ReturnToComposer {
            expected_draft_revision: revision,
            target_body: Some("浏览器已有完整输入".into()),
        },
    );
    f.database.connection().execute_batch("CREATE TEMP TRIGGER reject_child_queue_return BEFORE INSERT ON single_chat_retained_input BEGIN SELECT RAISE(ABORT,'injected queue return failure'); END;").unwrap();
    let prepared = std::sync::Arc::new(
        crate::private_source_observation::PreparedSources::capture(
            child::edit_source_inputs(&f.database, &returning).unwrap(),
        )
        .unwrap(),
    );
    assert!(
        child::edit_with_prepared_sources(&mut f.database, &returning, prepared.clone())
            .unwrap_err()
            .to_string()
            .contains("injected queue return failure")
    );
    assert_eq!(queue_draft(f, &web, conversation), target_before);
    assert!(crate::command::DomainCommandGateway
        .replay_if_recorded(&f.database, &returning)
        .unwrap()
        .is_none());
    assert_eq!(
        f.database
            .connection()
            .query_row(
                "SELECT state FROM single_chat_pending_input WHERE id=?1",
                [&c],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "queued"
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER reject_child_queue_return;")
        .unwrap();
    let returned =
        child::edit_with_prepared_sources(&mut f.database, &returning, prepared).unwrap();
    assert_eq!(returned.result.payload["placement"], "retained");
    assert_eq!(
        returned.result.payload["retainedInput"]["body"],
        "回移时必须保留的材料"
    );
    assert_eq!(queue_draft(f, &web, conversation), target_before);
    let retained = returned.result.payload["retainedInput"]["id"]
        .as_str()
        .unwrap();
    assert_eq!(
        f.database
            .connection()
            .query_row(
                "SELECT client_id FROM single_chat_retained_input WHERE id=?1",
                [retained],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        web.id()
    );

    let changed = queue_upload(f, conversation, "queue-changed.txt");
    let d = queue_send(f, &native, conversation, "等待修复的队首");
    let e = queue_send(f, &web, conversation, "后面的输入不能越过队首");
    queue_terminal(f, run_b);
    std::fs::write(
        &changed.source_path,
        vec![b'x'; changed.observed_byte_size.unwrap() as usize],
    )
    .unwrap();
    let ready = child::ready_pending_inputs(&f.database).unwrap();
    assert_eq!(ready[0].0.pending_input_id, d);
    assert_eq!(
        queue_publish(f, ready[0].clone()).result.code,
        "single_chat.attachment_source_changed"
    );
    assert!(child::ready_pending_inputs(&f.database).unwrap().is_empty());
    let view = queue_read(f, &native, conversation);
    assert_eq!(view.items[0].state, "needs_repair");
    assert_eq!(view.items[1].id, e);
    let begin = queue_edit_command(f, &native, conversation, &d, 1, None, Action::Begin);
    let opened = queue_edit(f, &begin);
    let token = opened.result.payload["editToken"].as_str().unwrap();
    let remove = queue_edit_command(
        f,
        &native,
        conversation,
        &d,
        1,
        Some(token),
        Action::RemoveAttachment {
            attachment_ref_id: changed.id,
        },
    );
    assert_eq!(
        queue_edit(f, &remove).result.status,
        crate::command::CommandResultStatus::Applied
    );
    let save = queue_edit_command(
        f,
        &native,
        conversation,
        &d,
        1,
        Some(token),
        Action::Save {
            body: "已经修复的队首".into(),
        },
    );
    assert_eq!(
        queue_edit(f, &save).result.status,
        crate::command::CommandResultStatus::Applied
    );
    let ready = child::ready_pending_inputs(&f.database).unwrap();
    assert_eq!(ready[0].0.pending_input_id, d);
    let published = queue_publish(f, ready[0].clone());
    assert_eq!(
        published.result.status,
        crate::command::CommandResultStatus::Accepted
    );
    queue_terminal(f, published.result.payload["agentRunId"].as_str().unwrap());
    let ready = child::ready_pending_inputs(&f.database).unwrap();
    assert_eq!(ready[0].0.pending_input_id, e);
    assert_eq!(
        queue_publish(f, ready[0].clone()).result.status,
        crate::command::CommandResultStatus::Accepted
    );
    assert!(queue_read(f, &native, conversation).items.is_empty());
}
