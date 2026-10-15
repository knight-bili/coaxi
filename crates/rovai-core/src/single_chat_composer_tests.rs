use super::*;
use crate::{
    collaboration::{CollaborationService, CreateThreadCommand},
    command::CommandResultStatus,
    local_attachment_source::observe_source_attachment,
    message_quote::QuoteSelection,
    single_chat::{OpenSingleChatCommand, SendSingleChatMessageCommand, SingleChatService},
    test_support::OwnedTestDatabase,
};

struct Fixture {
    database: OwnedTestDatabase,
    thread: String,
    conversation: String,
    scopes: Vec<SingleChatComposerScope>,
}

fn envelope<T>(thread: Option<&str>, payload: T) -> CommandEnvelope<T> {
    CommandEnvelope {
        command_id: Uuid::new_v4().to_string(),
        actor: ActorRef::User {
            user_id: crate::current_user::CURRENT_USER_ID.into(),
        },
        camp_id: thread.map(str::to_owned),
        expected_versions: vec![],
        execution_epoch: None,
        payload,
    }
}

fn fixture() -> Fixture {
    let mut database = crate::test_support::seeded_runtime_database_owned();
    let workspace = database.directory().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let created = CollaborationService::default()
        .create_camp(
            &mut database,
            &envelope(
                None,
                CreateThreadCommand::for_test(workspace.to_string_lossy().into()),
            ),
        )
        .unwrap();
    let thread = created.result.payload["threadId"]
        .as_str()
        .unwrap()
        .to_owned();
    let opened = SingleChatService::default()
        .open(
            &mut database,
            &envelope(
                Some(&thread),
                OpenSingleChatCommand {
                    draft_client: Default::default(),
                    camp_id: thread.clone(),
                    agent_id: "agent_1".into(),
                },
            ),
        )
        .unwrap();
    let conversation = opened.result.payload["conversationId"]
        .as_str()
        .unwrap()
        .to_owned();
    let board = Uuid::new_v4().to_string();
    database
        .connection()
        .execute(
            "INSERT INTO canvas_board(id,revision,created_at,updated_at) VALUES(?1,1,'now','now')",
            [&board],
        )
        .unwrap();
    let mut scopes = Vec::new();
    for _ in 0..3 {
        let node = Uuid::new_v4().to_string();
        database.connection().execute("INSERT INTO canvas_node(id,board_id,kind,thread_id,conversation_id,conversations_json,collapsed,grid_columns,grid_rows,x,y,width,height,created_at,updated_at)
            VALUES(?1,?2,'conversation',?3,?4,'[]',0,0,0,0,0,224,132,'now','now')",params![node,board,thread,conversation]).unwrap();
        scopes.push(SingleChatComposerScope::Canvas {
            board_id: board.clone(),
            node_id: node,
        });
    }
    Fixture {
        database,
        thread,
        conversation,
        scopes,
    }
}

fn command(
    f: &Fixture,
    index: usize,
    revision: i64,
    action: PrivateComposerAction,
) -> CommandEnvelope<MutatePrivateComposerCommand> {
    envelope(
        Some(&f.thread),
        MutatePrivateComposerCommand {
            draft_client: Default::default(),
            camp_id: f.thread.clone(),
            conversation_id: f.conversation.clone(),
            composer_scope: Some(f.scopes[index].clone()),
            expected_draft_revision: revision,
            action,
        },
    )
}

fn apply(
    f: &mut Fixture,
    index: usize,
    revision: i64,
    action: PrivateComposerAction,
) -> crate::command::StoredCommandResult {
    let input = command(f, index, revision, action);
    let result = mutate(&mut f.database, &input).unwrap().result;
    assert_eq!(result.status, CommandResultStatus::Applied, "{result:?}");
    result
}

fn read(f: &Fixture, index: usize) -> ScopedPrivateDraftView {
    read_draft(
        &f.database,
        &DraftClient::default(),
        &f.thread,
        &f.conversation,
        Some(&f.scopes[index]),
    )
    .unwrap()
}

fn add_default_source(f: &mut Fixture, name: &str) {
    let file = f.database.directory().join(name);
    std::fs::write(&file, name.as_bytes()).unwrap();
    let source = observe_source_attachment(&file, name, Some("text/plain")).unwrap();
    let revision = SingleChatService::default()
        .snapshot(&f.database, &f.conversation)
        .unwrap()
        .unwrap()
        .draft
        .revision;
    SingleChatService::default()
        .add_source_attachment(&mut f.database, &f.conversation, revision, source)
        .unwrap();
}

fn default_revision(f: &Fixture) -> i64 {
    SingleChatService::default()
        .snapshot(&f.database, &f.conversation)
        .unwrap()
        .unwrap()
        .draft
        .revision
}

#[test]
fn commands_keep_complete_inputs_separate_preserve_receipts_and_rollback_both_sides() {
    let mut f = fixture();
    let before = f.database.connection().total_changes();
    assert_eq!(read(&f, 0).draft.revision, 0);
    assert_eq!(read(&f, 1).draft.revision, 0);
    assert_eq!(
        f.database.connection().total_changes(),
        before,
        "read must not register an input"
    );
    let sent = SingleChatService::default()
        .send(
            &mut f.database,
            &envelope(
                Some(&f.thread),
                SendSingleChatMessageCommand {
                    composer_scope: None,
                    draft_client: Default::default(),
                    camp_id: f.thread.clone(),
                    conversation_id: f.conversation.clone(),
                    body: "测试文本".into(),
                    draft_revision: 0,
                },
            ),
        )
        .unwrap();
    let message = sent.result.payload["conversationMessageId"]
        .as_str()
        .unwrap()
        .to_owned();
    apply(&mut f, 0, 0, PrivateComposerAction::RegisterScope);
    let quote = apply(
        &mut f,
        0,
        0,
        PrivateComposerAction::Quote {
            action: QuoteAction::Add {
                selection: QuoteSelection {
                    message_id: message,
                    body_at_selection: "测试文本".into(),
                    start_scalar: 0,
                    end_scalar: 4,
                    text: "测试文本".into(),
                    current_user_display_name: None,
                },
            },
        },
    );
    let quote_id = quote.payload["draft"]["quotes"][0]["quoteId"]
        .as_str()
        .unwrap()
        .to_owned();
    let invalid = command(
        &f,
        1,
        0,
        PrivateComposerAction::Quote {
            action: QuoteAction::Remove {
                quote_id: quote_id.clone(),
            },
        },
    );
    assert_eq!(
        mutate(&mut f.database, &invalid).unwrap().result.status,
        CommandResultStatus::Rejected
    );
    assert_eq!(
        f.database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM single_chat_composer_scope",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1,
        "failed first write rolls registration back"
    );
    apply(
        &mut f,
        0,
        1,
        PrivateComposerAction::Quote {
            action: QuoteAction::Remove {
                quote_id: quote_id.clone(),
            },
        },
    );
    apply(
        &mut f,
        0,
        2,
        PrivateComposerAction::Quote {
            action: QuoteAction::Restore {
                quote_id: quote_id.clone(),
            },
        },
    );
    assert_eq!(read(&f, 0).draft.quotes[0].quote_id, quote_id);
    assert!(read(&f, 1).draft.quotes.is_empty());
    assert!(
        SingleChatService::default()
            .snapshot(&f.database, &f.conversation)
            .unwrap()
            .unwrap()
            .draft
            .quotes
            .is_empty()
    );
    add_default_source(&mut f, "first.md");
    add_default_source(&mut f, "second.md");
    let source_revision = default_revision(&f);
    let active_body = "长".repeat(100_001);
    let adopted = apply(
        &mut f,
        0,
        3,
        PrivateComposerAction::AdoptLegacyMaterials {
            expected_source_revision: source_revision,
            target_body: active_body.clone(),
        },
    );
    let saved = adopted.payload["retainedInput"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(adopted.payload["retainedInput"]["body"], "");
    assert_eq!(
        adopted.payload["retainedInput"]["attachments"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(read(&f, 0).draft.quotes[0].quote_id, quote_id);
    assert_eq!(read(&f, 0).draft.revision, 3);
    assert!(
        SingleChatService::default()
            .snapshot(&f.database, &f.conversation)
            .unwrap()
            .unwrap()
            .draft
            .attachments
            .is_empty()
    );
    let swap = command(
        &f,
        0,
        3,
        PrivateComposerAction::SwapRetained {
            retained_input_id: saved.clone(),
            expected_retained_revision: 1,
            current_body: active_body.clone(),
        },
    );
    let key = PrivateComposerKey::for_scope(Some(&f.scopes[0])).unwrap();
    let previous = format!(
        "{:?}{:?}",
        DraftMaterials::load(
            f.database.connection(),
            &f.conversation,
            &DraftClient::default(),
            &key
        )
        .unwrap(),
        load_retained(
            f.database.connection(),
            &f.conversation,
            &DraftClient::default(),
            key.sql_key(),
            &saved
        )
        .unwrap()
    );
    f.database.connection().execute_batch("CREATE TEMP TRIGGER reject_composer_receipt BEFORE INSERT ON event_log WHEN NEW.command_type='single_chat.composer_draft.mutate' AND NEW.event_type='command.result' BEGIN SELECT RAISE(ABORT,'composer receipt rollback fixture'); END;").unwrap();
    assert!(mutate(&mut f.database, &swap).is_err());
    assert_eq!(
        format!(
            "{:?}{:?}",
            DraftMaterials::load(
                f.database.connection(),
                &f.conversation,
                &DraftClient::default(),
                &key
            )
            .unwrap(),
            load_retained(
                f.database.connection(),
                &f.conversation,
                &DraftClient::default(),
                key.sql_key(),
                &saved
            )
            .unwrap()
        ),
        previous
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER reject_composer_receipt")
        .unwrap();
    let swapped = mutate(&mut f.database, &swap).unwrap();
    assert!(!swapped.replayed);
    assert_eq!(swapped.result.payload["body"], "");
    assert!(swapped.result.payload["retainedInput"]["body"].as_str() == Some(active_body.as_str()));
    assert_eq!(
        swapped.result.payload["retainedInput"]["quotes"][0]["quoteId"],
        quote_id
    );
    let replay = mutate(&mut f.database, &swap).unwrap();
    assert!(replay.replayed);
    assert_eq!(
        serde_json::to_value(replay.result).unwrap(),
        serde_json::to_value(swapped.result).unwrap()
    );
    assert_eq!(read(&f, 0).draft.revision, 4);
    let refs = read(&f, 0)
        .draft
        .attachments
        .iter()
        .map(|a| a.id.clone())
        .rev()
        .collect::<Vec<_>>();
    apply(
        &mut f,
        0,
        4,
        PrivateComposerAction::ReorderAttachments {
            attachment_ref_ids: refs.clone(),
        },
    );
    assert_eq!(
        read(&f, 0)
            .draft
            .attachments
            .iter()
            .map(|a| a.id.clone())
            .collect::<Vec<_>>(),
        refs
    );
    apply(
        &mut f,
        0,
        5,
        PrivateComposerAction::RemoveAttachment {
            attachment_ref_id: refs[0].clone(),
        },
    );
    let foreign = command(
        &f,
        1,
        0,
        PrivateComposerAction::RemoveAttachment {
            attachment_ref_id: refs[1].clone(),
        },
    );
    assert_eq!(
        mutate(&mut f.database, &foreign).unwrap().result.status,
        CommandResultStatus::Rejected
    );
    let foreign = command(
        &f,
        1,
        0,
        PrivateComposerAction::DiscardRetained {
            retained_input_id: saved.clone(),
            expected_retained_revision: 2,
        },
    );
    assert_eq!(
        mutate(&mut f.database, &foreign).unwrap().result.status,
        CommandResultStatus::Rejected
    );
    let mut foreign = swap.clone();
    foreign.payload.draft_client = DraftClient::verified_web(&"b".repeat(64)).unwrap();
    assert!(
        mutate(&mut f.database, &foreign).is_err(),
        "receipt must not cross verified editors"
    );
    assert!(
        read_draft(
            &f.database,
            &foreign.payload.draft_client,
            &f.thread,
            &f.conversation,
            Some(&f.scopes[0])
        )
        .unwrap()
        .draft
        .attachments
        .is_empty()
    );
    apply(
        &mut f,
        0,
        6,
        PrivateComposerAction::DiscardRetained {
            retained_input_id: saved,
            expected_retained_revision: 2,
        },
    );
    add_default_source(&mut f, "third.md");
    let source_revision = default_revision(&f);
    let first = apply(
        &mut f,
        1,
        0,
        PrivateComposerAction::AdoptLegacyMaterials {
            expected_source_revision: source_revision,
            target_body: "乙卡原正文".into(),
        },
    );
    assert_eq!(first.payload["draft"]["revision"], 0);
    assert_eq!(
        first.payload["retainedInput"]["attachments"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    add_default_source(&mut f, "fourth.md");
    let source_revision = default_revision(&f);
    let empty = apply(
        &mut f,
        2,
        0,
        PrivateComposerAction::AdoptLegacyMaterials {
            expected_source_revision: source_revision,
            target_body: "".into(),
        },
    );
    assert_eq!(
        empty.payload["draft"]["attachments"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(empty.payload.get("retainedInput").is_none());
    let mut spoof = serde_json::to_value(&foreign.payload).unwrap();
    spoof["draftClient"] = json!("a".repeat(64));
    assert!(serde_json::from_value::<MutatePrivateComposerCommand>(spoof).is_err());
}

#[test]
fn recovery_pages_are_read_only_context_bound_bounded_and_incomplete_after_changes() {
    let mut f = fixture();
    apply(&mut f, 0, 0, PrivateComposerAction::RegisterScope);
    apply(&mut f, 1, 0, PrivateComposerAction::RegisterScope);
    let mut input = PrivateRecoveryParams {
        camp_id: f.thread.clone(),
        conversation_id: f.conversation.clone(),
        cursor: None,
        limit: Some(1),
    };
    let before = f.database.connection().total_changes();
    let first = recovery(&f.database, &DraftClient::default(), &input).unwrap();
    assert_eq!(f.database.connection().total_changes(), before);
    assert!(!first.complete);
    assert_eq!(first.items.len(), 1);
    input.cursor = first.next_cursor.clone();
    let other = DraftClient::verified_web(&"c".repeat(64)).unwrap();
    assert!(recovery(&f.database, &other, &input).is_err());
    let original = input.cursor.clone().unwrap();
    let mut bad = original.as_bytes().to_vec();
    bad[0] = if bad[0] == b'A' { b'B' } else { b'A' };
    input.cursor = Some(String::from_utf8(bad).unwrap());
    assert!(recovery(&f.database, &DraftClient::default(), &input).is_err());
    input.cursor = Some(original);
    apply(&mut f, 2, 0, PrivateComposerAction::RegisterScope);
    let mut pages = 0;
    while input.cursor.is_some() {
        let page = recovery(&f.database, &DraftClient::default(), &input).unwrap();
        assert!(
            !page.complete,
            "changed first-page digest must survive every later page"
        );
        input.cursor = page.next_cursor;
        pages += 1;
        assert!(pages < 8);
    }
    input.limit = Some(100);
    let fresh = recovery(&f.database, &DraftClient::default(), &input).unwrap();
    assert!(fresh.complete);
    assert_eq!(fresh.items.len(), 4);
    let SingleChatComposerScope::Canvas { node_id, .. } = &f.scopes[0];
    f.database
        .connection()
        .execute("DELETE FROM canvas_node WHERE id=?1", [node_id])
        .unwrap();
    let orphan = recovery(&f.database, &DraftClient::default(), &input).unwrap();
    assert!(orphan.items.iter().any(|item|matches!(item,PrivateRecoveryItem::Scope {scope} if scope.composer_scope.as_ref()==Some(&f.scopes[0]) && scope.orphaned)));
    let tx = f.database.connection().unchecked_transaction().unwrap();
    for index in 0..101 {
        tx.execute("INSERT INTO single_chat_retained_input(id,conversation_id,client_id,body,created_at,updated_at) VALUES(?1,?2,'desktop','whole input','now','now')",params![format!("bulk-{index:03}"),f.conversation]).unwrap();
    }
    tx.commit().unwrap();
    input.limit = Some(usize::MAX);
    let bounded = recovery(&f.database, &DraftClient::default(), &input).unwrap();
    assert_eq!(bounded.items.len(), 100);
    assert!(!bounded.complete);
    assert!(bounded.next_cursor.is_some());
    let before = f.database.connection().total_changes();
    let foreign = recovery(&f.database, &other, &input).unwrap();
    assert!(foreign.complete && foreign.items.is_empty());
    assert_eq!(f.database.connection().total_changes(), before);
    assert_eq!(
        f.database
            .connection()
            .query_row("SELECT COUNT(*) FROM agent_run", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn send_consumes_only_the_registered_scope_and_keeps_one_private_fifo_and_legacy_shape() {
    let mut f = fixture();
    let legacy = json!({"threadId":f.thread,"conversationId":f.conversation,"body":"legacy input","draftRevision":0});
    let decoded: SendSingleChatMessageCommand = serde_json::from_value(legacy.clone()).unwrap();
    assert!(decoded.composer_scope.is_none());
    assert_eq!(
        serde_json::to_value(decoded).unwrap(),
        legacy,
        "default scope must not change the old serialized request"
    );
    add_default_source(&mut f, "alpha.md");
    let source_revision = default_revision(&f);
    apply(
        &mut f,
        0,
        0,
        PrivateComposerAction::AdoptLegacyMaterials {
            expected_source_revision: source_revision,
            target_body: "".into(),
        },
    );
    let alpha = read(&f, 0).draft.attachments[0].id.clone();
    add_default_source(&mut f, "beta.md");
    let source_revision = default_revision(&f);
    apply(
        &mut f,
        1,
        0,
        PrivateComposerAction::AdoptLegacyMaterials {
            expected_source_revision: source_revision,
            target_body: "".into(),
        },
    );
    let beta = read(&f, 1).draft.attachments[0].id.clone();
    add_default_source(&mut f, "default.md");
    let default_revision = default_revision(&f);
    let unregistered = envelope(
        Some(&f.thread),
        SendSingleChatMessageCommand {
            draft_client: Default::default(),
            composer_scope: Some(f.scopes[2].clone()),
            camp_id: f.thread.clone(),
            conversation_id: f.conversation.clone(),
            body: "丙卡问题".into(),
            draft_revision: 0,
        },
    );
    let rejected = SingleChatService::default()
        .send(&mut f.database, &unregistered)
        .unwrap();
    assert_eq!(rejected.result.status, CommandResultStatus::Rejected);
    assert_eq!(rejected.result.code, "single_chat.scope_unregistered");
    let input = envelope(
        Some(&f.thread),
        SendSingleChatMessageCommand {
            draft_client: Default::default(),
            composer_scope: Some(f.scopes[1].clone()),
            camp_id: f.thread.clone(),
            conversation_id: f.conversation.clone(),
            body: "乙卡问题".into(),
            draft_revision: 1,
        },
    );
    let sent = SingleChatService::default()
        .send(&mut f.database, &input)
        .unwrap();
    assert_eq!(sent.result.status, CommandResultStatus::Accepted);
    let run = sent.result.payload["agentRunId"].as_str().unwrap();
    let sources =
        crate::local_attachment_source::load_agent_run_source_attachments(&f.database, run, 0)
            .unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].id, beta);
    assert_eq!(read(&f, 1).draft.revision, 2);
    assert!(read(&f, 1).draft.attachments.is_empty());
    assert_eq!(read(&f, 0).draft.attachments[0].id, alpha);
    assert_eq!(
        SingleChatService::default()
            .snapshot(&f.database, &f.conversation)
            .unwrap()
            .unwrap()
            .draft
            .revision,
        default_revision
    );
    assert_eq!(
        SingleChatService::default()
            .snapshot(&f.database, &f.conversation)
            .unwrap()
            .unwrap()
            .draft
            .attachments
            .len(),
        1
    );
    let message = sent.result.payload["conversationMessageId"]
        .as_str()
        .unwrap()
        .to_owned();
    apply(
        &mut f,
        0,
        1,
        PrivateComposerAction::Quote {
            action: QuoteAction::Add {
                selection: QuoteSelection {
                    message_id: message,
                    body_at_selection: "乙卡问题".into(),
                    start_scalar: 0,
                    end_scalar: 4,
                    text: "乙卡问题".into(),
                    current_user_display_name: None,
                },
            },
        },
    );
    let quote = read(&f, 0).draft.quotes[0].clone();
    let mut queued = envelope(
        Some(&f.thread),
        SendSingleChatMessageCommand {
            draft_client: Default::default(),
            composer_scope: Some(f.scopes[0].clone()),
            camp_id: f.thread.clone(),
            conversation_id: f.conversation.clone(),
            body: "".into(),
            draft_revision: 2,
        },
    );
    let rejected = SingleChatService::default()
        .send(&mut f.database, &queued)
        .unwrap();
    assert_eq!(rejected.result.status, CommandResultStatus::Rejected);
    assert_eq!(rejected.result.code, "quote.question_required");
    assert_eq!(read(&f, 0).draft.revision, 2);
    queued.payload.body = "甲卡补充".into();
    queued.command_id = Uuid::new_v4().to_string();
    let accepted = SingleChatService::default()
        .send(&mut f.database, &queued)
        .unwrap();
    assert_eq!(accepted.result.status, CommandResultStatus::Accepted);
    assert_eq!(accepted.result.code, "single_chat.pending_input_queued");
    let replay = SingleChatService::default()
        .send(&mut f.database, &queued)
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(
        serde_json::to_value(replay.result).unwrap(),
        serde_json::to_value(accepted.result).unwrap()
    );
    let snapshot = SingleChatService::default()
        .snapshot(&f.database, &f.conversation)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.pending_inputs.items.len(), 1);
    assert_eq!(snapshot.pending_inputs.items[0].attachments[0].id, alpha);
    assert_eq!(
        snapshot.pending_inputs.items[0].quotes[0].quote_id,
        quote.quote_id
    );
    assert!(read(&f, 0).draft.attachments.is_empty());
    assert!(read(&f, 0).draft.quotes.is_empty());
    assert_eq!(snapshot.draft.revision, default_revision);
    assert_eq!(
        snapshot.agent_runs.len(),
        1,
        "scope cards do not create parallel execution lanes"
    );
    verify_application_routes(&mut f, &snapshot.pending_inputs.items[0]);
    assert_eq!(
        serde_json::to_value(&quote.model_projection())
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        3
    );
}

fn verify_application_routes(
    f: &mut Fixture,
    pending: &crate::single_chat::SingleChatPendingInputView,
) {
    use crate::application::private_composer::{handle, reconcile};
    use crate::single_chat::{EditSingleChatPendingInputCommand, SingleChatPendingInputEditAction};
    let client = DraftClient::default();
    SingleChatService::default()
        .edit_pending_input(
            &mut f.database,
            &envelope(
                Some(&f.thread),
                EditSingleChatPendingInputCommand {
                    draft_client: client.clone(),
                    camp_id: f.thread.clone(),
                    conversation_id: f.conversation.clone(),
                    pending_input_id: pending.id.clone(),
                    expected_revision: pending.revision,
                    edit_token: None,
                    action: SingleChatPendingInputEditAction::Begin,
                },
            ),
        )
        .unwrap();
    f.database.connection().execute("UPDATE single_chat_pending_input_edit_session SET working_body='默认位置未保存正文' WHERE conversation_id=?1", [&f.conversation]).unwrap();
    let legacy = handle(
        &mut f.database,
        &client,
        "singleChat.get",
        json!({"conversationId":f.conversation}),
    )
    .unwrap();
    assert_eq!(legacy.result["draft"].as_object().unwrap().len(), 4);
    assert!(
        legacy.result["pendingInputs"]
            .get("editingElsewhere")
            .is_none()
    );
    let token = legacy.result["pendingInputs"]["editSession"]["editToken"]
        .as_str()
        .unwrap()
        .to_owned();
    let default_view = handle(
        &mut f.database,
        &client,
        "singleChat.get",
        json!({"conversationId":f.conversation,"composerScope":null}),
    )
    .unwrap();
    assert_eq!(default_view.result["draft"]["scopeVersion"], 1);
    assert_eq!(default_view.result["draft"]["scopeKey"], "");
    assert_eq!(
        default_view.result["pendingInputs"]["editingElsewhere"],
        false
    );
    let alpha = handle(
        &mut f.database,
        &client,
        "singleChat.get",
        json!({"conversationId":f.conversation,"composerScope":f.scopes[0]}),
    )
    .unwrap();
    assert!(alpha.result["pendingInputs"]["editSession"].is_null());
    assert_eq!(alpha.result["pendingInputs"]["editingElsewhere"], true);
    assert!(
        !serde_json::to_string(&alpha.result)
            .unwrap()
            .contains(&token)
    );
    assert!(
        !serde_json::to_string(&alpha.result)
            .unwrap()
            .contains("默认位置未保存正文")
    );
    assert!(
        alpha.result["draft"]["attachments"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let beta_key = PrivateComposerKey::for_scope(Some(&f.scopes[1])).unwrap();
    f.database.connection().execute("UPDATE single_chat_pending_input_edit_session SET scope_key=?2 WHERE conversation_id=?1", params![f.conversation,beta_key.sql_key()]).unwrap();
    let default_hidden = handle(
        &mut f.database,
        &client,
        "singleChat.get",
        json!({"conversationId":f.conversation}),
    )
    .unwrap();
    assert!(default_hidden.result["pendingInputs"]["editSession"].is_null());
    let beta = handle(
        &mut f.database,
        &client,
        "singleChat.get",
        json!({"conversationId":f.conversation,"composerScope":f.scopes[1]}),
    )
    .unwrap();
    assert_eq!(
        beta.result["pendingInputs"]["editSession"]["editToken"],
        token
    );
    assert_eq!(
        beta.result["pendingInputs"]["editSession"]["workingQuotes"][0]["quoteId"],
        pending.quotes[0].quote_id
    );
    assert_eq!(beta.result["pendingInputs"]["editingElsewhere"], false);
    let web = DraftClient::verified_web(&"6".repeat(64)).unwrap();
    let mutation = json!({"commandId":Uuid::new_v4().to_string(),"command":{
        "threadId":f.thread,"conversationId":f.conversation,"composerScope":f.scopes[2],
        "expectedDraftRevision":0,"action":{"type":"register_scope"}}});
    assert_eq!(
        reconcile(&f.database, &web, mutation.clone()).unwrap()["state"],
        "unknown"
    );
    let first = handle(
        &mut f.database,
        &web,
        "singleChat.composerDraft.mutate",
        mutation.clone(),
    )
    .unwrap();
    assert_eq!(first.result["status"], "applied");
    assert_eq!(
        first.changed,
        Some((f.thread.clone(), f.conversation.clone()))
    );
    let before: i64 = f
        .database
        .connection()
        .query_row("SELECT COUNT(*) FROM event_log", [], |row| row.get(0))
        .unwrap();
    let stored = reconcile(&f.database, &web, mutation.clone()).unwrap();
    assert_eq!(stored["state"], "recorded");
    assert_eq!(stored["result"], first.result);
    let again = handle(
        &mut f.database,
        &web,
        "singleChat.composerDraft.mutate",
        mutation.clone(),
    )
    .unwrap();
    assert_eq!(again.result, first.result);
    assert!(again.changed.is_none());
    assert!(reconcile(&f.database, &client, mutation.clone()).is_err());
    let after: i64 = f
        .database
        .connection()
        .query_row("SELECT COUNT(*) FROM event_log", [], |row| row.get(0))
        .unwrap();
    assert_eq!(before, after);
    let recovery = handle(
        &mut f.database,
        &web,
        "singleChat.composerDraft.recovery",
        json!({"threadId":f.thread,"conversationId":f.conversation}),
    )
    .unwrap();
    assert_eq!(recovery.result["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        recovery.result["items"][0]["scope"]["composerScope"],
        serde_json::to_value(&f.scopes[2]).unwrap()
    );
    let mut spoof = mutation.clone();
    spoof["command"]["draftClient"] = json!({"clientId":"desktop"});
    assert!(
        handle(
            &mut f.database,
            &web,
            "singleChat.composerDraft.mutate",
            spoof.clone()
        )
        .is_err()
    );
    assert!(reconcile(&f.database, &web, spoof).is_err());
    assert!(
        handle(
            &mut f.database,
            &web,
            "singleChat.get",
            json!({"conversationId":f.conversation,"scopeKey":beta_key.sql_key()})
        )
        .is_err()
    );
    assert!(
        handle(
            &mut f.database,
            &web,
            "singleChat.composerDraft.recovery",
            json!({"threadId":f.thread,"conversationId":f.conversation,"clientId":"desktop"})
        )
        .is_err()
    );
    let missing = handle(
        &mut f.database,
        &web,
        "singleChat.get",
        json!({"conversationId":"missing","composerScope":null}),
    )
    .unwrap();
    assert!(missing.result.is_null());
}
