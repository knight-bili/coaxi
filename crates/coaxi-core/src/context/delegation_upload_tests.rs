// Existing Core owner; control/terminal rows are fixtures, not authenticated takeover or HTTP proof.
fn assert_child_upload_gate(fixture: &mut Fixture, conversation: &str) {
    use crate::{
        draft_client::DraftClient,
        local_attachment_source::{LocalAttachmentKind, LocalAttachmentSourceRef},
        web_upload::{self, NativeUploadPreparation, UploadIntent, UploadTarget},
    };
    let client = DraftClient::default();
    let command = Uuid::new_v4().to_string();
    let target = UploadTarget::DelegationChild {
        conversation_id: conversation.into(),
        composer_scope: None,
    };
    let missing = fixture
        .directory
        .join("unavailable-child-source.txt")
        .display()
        .to_string();
    let params = NativeUploadPreparation {
        command_id: command.clone(),
        camp_id: fixture.camp_id.clone(),
        expected_revision: 0,
        target: target.clone(),
        source_path: missing.clone(),
        display_name: "unavailable-child-source.txt".into(),
        media_type: None,
    };
    assert_eq!(
        web_upload::preflight_child_native(&fixture.database, &client, &params)
            .unwrap_err()
            .to_string(),
        "delegation.not_user_controlled"
    );
    let intent = UploadIntent {
        target,
        command_id: command.clone(),
        camp_id: fixture.camp_id.clone(),
        expected_revision: 0,
        display_name: params.display_name.clone(),
        source_kind: LocalAttachmentKind::File,
        byte_size: 0,
        sha256: "0".repeat(64),
    };
    let source = LocalAttachmentSourceRef {
        id: command,
        source_path: missing,
        display_name: params.display_name,
        kind: LocalAttachmentKind::File,
        media_type: None,
        observed_byte_size: Some(0),
    };
    let denied = web_upload::bind(
        &mut fixture.database,
        &fixture.directory,
        &client,
        intent.clone(),
        source,
    )
    .unwrap();
    assert_eq!(denied.result.code, "delegation.not_user_controlled");
    assert_eq!(
        denied.result.status,
        crate::command::CommandResultStatus::Rejected
    );
    assert!(
        web_upload::reconcile(&fixture.database, &client, intent)
            .unwrap()
            .unwrap()
            .replayed
    );
}

fn assert_child_upload_sources(fixture: &mut Fixture, conversation: &str) {
    use crate::{
        draft_client::DraftClient,
        local_attachment_source::{
            self, LocalAttachmentKind, LocalAttachmentOwnerLocator, LocalAttachmentSourceRef,
        },
        web_upload::{self, NativeUploadPreparation, UploadIntent, UploadTarget},
    };
    let native = DraftClient::default();
    let revision:i64=fixture.database.connection().query_row("SELECT revision FROM single_chat_composer_draft WHERE conversation_id=?1 AND client_id='desktop' AND scope_key=''",[conversation],|r|r.get(0)).unwrap();
    let target = UploadTarget::DelegationChild {
        conversation_id: conversation.into(),
        composer_scope: None,
    };
    let path = fixture.directory.join("native-child-upload.txt");
    let content = b"original child upload";
    std::fs::write(&path, content).unwrap();
    let preparation = NativeUploadPreparation {
        command_id: Uuid::new_v4().to_string(),
        camp_id: fixture.camp_id.clone(),
        expected_revision: revision,
        target: target.clone(),
        source_path: path.display().to_string(),
        display_name: "native-child-upload.txt".into(),
        media_type: Some("text/plain".into()),
    };
    web_upload::preflight_child_native(&fixture.database, &native, &preparation).unwrap();
    let observed = web_upload::observe_native(&native, preparation).unwrap();
    let intent: UploadIntent = serde_json::from_value(observed["intent"].clone()).unwrap();
    let source: LocalAttachmentSourceRef =
        serde_json::from_value(observed["source"].clone()).unwrap();
    // Same-size content changes must roll back draft, observation and receipt together.
    std::fs::write(&path, vec![b'x'; content.len()]).unwrap();
    assert!(web_upload::bind(
        &mut fixture.database,
        &fixture.directory,
        &native,
        intent.clone(),
        source.clone()
    )
    .is_err());
    assert!(
        web_upload::reconcile(&fixture.database, &native, intent.clone())
            .unwrap()
            .is_none()
    );
    let key = crate::single_chat_composer_scope::PrivateComposerKey::default();
    assert_eq!(
        crate::single_chat_composer::DraftMaterials::load(
            fixture.database.connection(),
            conversation,
            &native,
            &key
        )
        .unwrap()
        .revision,
        revision
    );
    std::fs::write(&path, content).unwrap();
    let bound = web_upload::bind(
        &mut fixture.database,
        &fixture.directory,
        &native,
        intent.clone(),
        source.clone(),
    )
    .unwrap();
    assert_eq!(
        bound.result.status,
        crate::command::CommandResultStatus::Applied
    );
    let before: i64 = fixture
        .database
        .connection()
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    assert!(
        web_upload::bind(
            &mut fixture.database,
            &fixture.directory,
            &native,
            intent.clone(),
            source.clone()
        )
        .unwrap()
        .replayed
    );
    assert_eq!(
        before,
        fixture
            .database
            .connection()
            .query_row("SELECT total_changes()", [], |r| r.get::<_, i64>(0))
            .unwrap()
    );
    let snapshot =
        web_upload::snapshot(&fixture.database, &fixture.directory, &native, &intent).unwrap();
    assert_eq!(snapshot["revision"], revision + 1);
    let locator = LocalAttachmentOwnerLocator::DelegationChildComposer {
        camp_id: fixture.camp_id.clone(),
        conversation_id: conversation.into(),
        composer_scope: None,
        attachment_ref_id: source.id.clone(),
    };
    assert_eq!(
        local_attachment_source::load_source_attachment_for_client(
            &fixture.database,
            &locator,
            &native
        )
        .unwrap(),
        Some(source.clone())
    );
    let store = crate::camp_attachment::ThreadAttachmentStore::new(&fixture.directory);
    assert_eq!(
        crate::camp_attachment::attachment_location(&fixture.database, &store, &locator, &native)
            .unwrap(),
        Some(path.clone())
    );
    let foreign = DraftClient::verified_web(&"b".repeat(64)).unwrap();
    assert!(crate::camp_attachment::attachment_location(
        &fixture.database,
        &store,
        &locator,
        &foreign
    )
    .unwrap()
    .is_none());
    let legacy = LocalAttachmentOwnerLocator::SingleChatComposer {
        camp_id: fixture.camp_id.clone(),
        conversation_id: conversation.into(),
        attachment_ref_id: source.id.clone(),
    };
    assert!(local_attachment_source::load_source_attachment_for_client(
        &fixture.database,
        &legacy,
        &native
    )
    .unwrap()
    .is_none());
    let mut wrong = locator.clone();
    if let LocalAttachmentOwnerLocator::DelegationChildComposer {
        conversation_id, ..
    } = &mut wrong
    {
        *conversation_id = Uuid::new_v4().to_string();
    }
    assert!(crate::camp_attachment::attachment_location(
        &fixture.database,
        &store,
        &wrong,
        &native
    )
    .unwrap()
    .is_none());
    let mut old_target = intent.clone();
    old_target.command_id = Uuid::new_v4().to_string();
    old_target.target = UploadTarget::SingleChat {
        conversation_id: conversation.into(),
        composer_scope: None,
    };
    assert_eq!(
        web_upload::retry_without_source(&mut fixture.database, &native, old_target)
            .unwrap()
            .unwrap()
            .result
            .code,
        "single_chat.not_active"
    );

    let folder = fixture.directory.join("native-child-directory");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("readme.txt"), b"child folder").unwrap();
    let preparation = NativeUploadPreparation {
        command_id: Uuid::new_v4().to_string(),
        camp_id: fixture.camp_id.clone(),
        expected_revision: revision + 1,
        target: target.clone(),
        source_path: folder.display().to_string(),
        display_name: "native-child-directory".into(),
        media_type: None,
    };
    web_upload::preflight_child_native(&fixture.database, &native, &preparation).unwrap();
    let observed = web_upload::observe_native(&native, preparation).unwrap();
    let dir_intent: UploadIntent = serde_json::from_value(observed["intent"].clone()).unwrap();
    let dir_source: LocalAttachmentSourceRef =
        serde_json::from_value(observed["source"].clone()).unwrap();
    assert_eq!(dir_source.kind, LocalAttachmentKind::Directory);
    assert_eq!(
        web_upload::bind(
            &mut fixture.database,
            &fixture.directory,
            &native,
            dir_intent,
            dir_source
        )
        .unwrap()
        .result
        .status,
        crate::command::CommandResultStatus::Applied
    );

    // The Host spool is represented by a real file, not a browser-supplied local path.
    let web_path = fixture.directory.join("web-child-spool.txt");
    std::fs::write(&web_path, b"browser spool bytes").unwrap();
    let observation = crate::local_attachment_snapshot::observe_private_source(&web_path).unwrap();
    let web_intent = UploadIntent {
        command_id: Uuid::new_v4().to_string(),
        camp_id: fixture.camp_id.clone(),
        expected_revision: 0,
        target: target.clone(),
        display_name: "web-child-spool.txt".into(),
        source_kind: LocalAttachmentKind::File,
        byte_size: observation.byte_size,
        sha256: observation.content_digest,
    };
    let web_source = LocalAttachmentSourceRef {
        id: web_intent.command_id.clone(),
        source_path: web_path.display().to_string(),
        display_name: web_intent.display_name.clone(),
        kind: LocalAttachmentKind::File,
        media_type: None,
        observed_byte_size: Some(web_intent.byte_size),
    };
    assert_eq!(
        web_upload::bind(
            &mut fixture.database,
            &fixture.directory,
            &foreign,
            web_intent.clone(),
            web_source.clone()
        )
        .unwrap()
        .result
        .status,
        crate::command::CommandResultStatus::Applied
    );
    assert_eq!(
        web_upload::snapshot(&fixture.database, &fixture.directory, &foreign, &web_intent).unwrap()
            ["revision"],
        1
    );
    assert!(local_attachment_source::load_source_attachment_for_client(
        &fixture.database,
        &locator,
        &foreign
    )
    .unwrap()
    .is_none());
    let retained = Uuid::new_v4().to_string();
    fixture.database.connection().execute("INSERT INTO single_chat_retained_input(id,conversation_id,client_id,scope_key,revision,body,source_attachments_json,quotes_json,quote_trash_json,created_at,updated_at) VALUES(?1,?2,'desktop','',1,'保留草稿','[]','[]','[]','2026-10-17T00:00:07Z','2026-10-17T00:00:07Z')",params![retained,conversation]).unwrap();
    let swap = child_material_command(
        fixture,
        &native,
        conversation,
        revision + 2,
        json!({"type":"swap_retained","retainedInputId":retained,"expectedRetainedRevision":1,"currentBody":"上传后的正文"}),
    );
    assert_eq!(
        crate::delegation_user::mutate_draft(&mut fixture.database, &swap)
            .unwrap()
            .result
            .status,
        crate::command::CommandResultStatus::Applied
    );
    assert!(local_attachment_source::load_source_attachment_for_client(
        &fixture.database,
        &locator,
        &native
    )
    .unwrap()
    .is_none());
    let saved = LocalAttachmentOwnerLocator::DelegationChildRetained {
        camp_id: fixture.camp_id.clone(),
        conversation_id: conversation.into(),
        composer_scope: None,
        retained_input_id: retained,
        attachment_ref_id: source.id.clone(),
    };
    assert_eq!(
        local_attachment_source::load_source_attachment_for_client(
            &fixture.database,
            &saved,
            &native
        )
        .unwrap(),
        Some(source.clone())
    );
    assert!(local_attachment_source::load_source_attachment_for_client(
        &fixture.database,
        &saved,
        &foreign
    )
    .unwrap()
    .is_none());
    let message = Uuid::new_v4().to_string();
    fixture.database.connection().execute("INSERT INTO conversation_message(id,conversation_id,sequence,author_type,author_id,body,source_attachments_json,created_at) VALUES(?1,?2,5,'user','user','已发布的上传来源',?3,'2026-10-17T00:00:08Z')",params![message,conversation,local_attachment_source::serialize_source_attachments(std::slice::from_ref(&web_source)).unwrap()]).unwrap();
    fixture
        .database
        .connection()
        .execute(
            "UPDATE conversation SET last_message_sequence=5 WHERE id=?1",
            [conversation],
        )
        .unwrap();
    let published = LocalAttachmentOwnerLocator::DelegationChildMessage {
        camp_id: fixture.camp_id.clone(),
        conversation_id: conversation.into(),
        conversation_message_id: message.clone(),
        attachment_ref_id: web_source.id.clone(),
    };
    assert_eq!(
        local_attachment_source::load_source_attachment_for_client(
            &fixture.database,
            &published,
            &native
        )
        .unwrap(),
        Some(web_source)
    );
    assert!(fixture
        .database
        .connection()
        .execute(
            "UPDATE conversation_message SET source_attachments_json='[]' WHERE id=?1",
            [&message]
        )
        .is_err());
    assert_eq!(fixture.database.connection().execute("UPDATE conversation_message SET source_attachments_json=source_attachments_json WHERE id=?1",[&message]).unwrap(),1);
}
