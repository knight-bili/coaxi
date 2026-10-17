// Existing Core owner. File/DB operations are real; control and terminal setup remain fixtures.
fn pending_edit_locator(
    f: &Fixture,
    conversation: &str,
    pending: &str,
    token: &str,
    source: &str,
) -> crate::local_attachment_source::LocalAttachmentOwnerLocator {
    crate::local_attachment_source::LocalAttachmentOwnerLocator::DelegationChildPendingEdit {
        camp_id: f.camp_id.clone(),
        conversation_id: conversation.into(),
        pending_input_id: pending.into(),
        edit_token: token.into(),
        composer_scope: None,
        attachment_ref_id: source.into(),
    }
}
fn observe_pending_native(
    f: &Fixture,
    conversation: &str,
    pending: &str,
    token: &str,
    path: &std::path::Path,
) -> (
    crate::web_upload::UploadIntent,
    crate::local_attachment_source::LocalAttachmentSourceRef,
) {
    let native = crate::draft_client::DraftClient::default();
    let params = crate::web_upload::NativeUploadPreparation {
        command_id: Uuid::new_v4().to_string(),
        camp_id: f.camp_id.clone(),
        expected_revision: 1,
        target: crate::web_upload::UploadTarget::DelegationChildPending {
            conversation_id: conversation.into(),
            pending_input_id: pending.into(),
            edit_token: token.into(),
            composer_scope: None,
        },
        source_path: path.display().to_string(),
        display_name: path.file_name().unwrap().to_str().unwrap().into(),
        media_type: None,
    };
    crate::web_upload::preflight_child_native(&f.database, &native, &params).unwrap();
    let observed = crate::web_upload::observe_native(&native, params).unwrap();
    (
        serde_json::from_value(observed["intent"].clone()).unwrap(),
        serde_json::from_value(observed["source"].clone()).unwrap(),
    )
}
fn assert_pending_native_upload(
    f: &mut Fixture,
    conversation: &str,
    pending: &str,
    token: &str,
) -> (
    crate::web_upload::UploadIntent,
    crate::local_attachment_source::LocalAttachmentSourceRef,
) {
    use crate::{
        local_attachment_source as source, single_chat::SingleChatPendingInputEditAction as Action,
        web_upload,
    };
    let native = crate::draft_client::DraftClient::default();
    let web = crate::draft_client::DraftClient::verified_web(&"b".repeat(64)).unwrap();
    let directory = f.directory.join("pending-native-directory");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(
        directory.join("readme.txt"),
        b"native child pending directory",
    )
    .unwrap();
    let (intent, item) = observe_pending_native(f, conversation, pending, token, &directory);
    assert_eq!(item.kind, source::LocalAttachmentKind::Directory);
    assert_eq!(
        web_upload::bind(
            &mut f.database,
            &f.directory,
            &native,
            intent.clone(),
            item.clone()
        )
        .unwrap()
        .result
        .status,
        crate::command::CommandResultStatus::Applied
    );
    let locator = pending_edit_locator(f, conversation, pending, token, &item.id);
    assert_eq!(
        source::load_source_attachment_for_client(&f.database, &locator, &native).unwrap(),
        Some(item.clone())
    );
    assert!(
        source::load_source_attachment_for_client(&f.database, &locator, &web)
            .unwrap()
            .is_none()
    );
    let store = crate::camp_attachment::ThreadAttachmentStore::new(&f.directory);
    assert_eq!(
        crate::camp_attachment::attachment_location(&f.database, &store, &locator, &native)
            .unwrap(),
        Some(directory)
    );
    let saved = source::LocalAttachmentOwnerLocator::DelegationChildPending {
        camp_id: f.camp_id.clone(),
        conversation_id: conversation.into(),
        pending_input_id: pending.into(),
        attachment_ref_id: item.id.clone(),
    };
    assert!(
        source::load_source_attachment_for_client(&f.database, &saved, &native)
            .unwrap()
            .is_none()
    );
    let legacy = source::LocalAttachmentOwnerLocator::SingleChatPendingEdit {
        camp_id: f.camp_id.clone(),
        conversation_id: conversation.into(),
        pending_input_id: pending.into(),
        edit_token: token.into(),
        attachment_ref_id: item.id.clone(),
    };
    assert!(
        source::load_source_attachment_for_client(&f.database, &legacy, &native)
            .unwrap()
            .is_none()
    );
    let snapshot = web_upload::snapshot(&f.database, &f.directory, &native, &intent).unwrap();
    assert_eq!(
        snapshot["pendingInputs"]["editSession"]["workingAttachments"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        snapshot["pendingInputs"]["items"][0]["attachments"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(!serde_json::to_string(&snapshot)
        .unwrap()
        .contains("sourcePath"));
    let remove = queue_edit_command(
        f,
        &native,
        conversation,
        pending,
        1,
        Some(token),
        Action::RemoveAttachment {
            attachment_ref_id: item.id.clone(),
        },
    );
    assert_eq!(
        queue_edit(f, &remove).result.status,
        crate::command::CommandResultStatus::Applied
    );
    let changes: i64 = f
        .database
        .connection()
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    assert!(
        web_upload::bind(&mut f.database, &f.directory, &native, intent, item)
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
    assert!(
        source::load_source_attachment_for_client(&f.database, &locator, &native)
            .unwrap()
            .is_none()
    );

    let path = f.directory.join("pending-native-unbound.txt");
    std::fs::write(&path, b"frozen old pending upload").unwrap();
    let frozen = observe_pending_native(f, conversation, pending, token, &path);
    crate::single_chat::recover_pending_edit_sessions(&f.database).unwrap();
    let blocked = crate::web_upload::NativeUploadPreparation {
        command_id: Uuid::new_v4().to_string(),
        camp_id: f.camp_id.clone(),
        expected_revision: 1,
        target: frozen.0.target.clone(),
        source_path: f
            .directory
            .join("must-not-read-pending.txt")
            .display()
            .to_string(),
        display_name: "must-not-read-pending.txt".into(),
        media_type: None,
    };
    assert_eq!(
        web_upload::preflight_child_native(&f.database, &native, &blocked)
            .unwrap_err()
            .to_string(),
        "single_chat.pending_input_edit_fenced"
    );
    // Recovery remains readable by its original editor; new byte binding needs a fresh edit lease.
    let saved_ref = queue_read(f, &native, conversation).items[0].attachments[0]
        .id
        .clone();
    assert!(source::load_source_attachment_for_client(
        &f.database,
        &pending_edit_locator(f, conversation, pending, token, &saved_ref),
        &native
    )
    .unwrap()
    .is_some());
    frozen
}
fn assert_pending_web_upload(
    f: &mut Fixture,
    conversation: &str,
    pending: &str,
    other_pending: &str,
    old_token: &str,
    token: &str,
    frozen: (
        crate::web_upload::UploadIntent,
        crate::local_attachment_source::LocalAttachmentSourceRef,
    ),
) -> String {
    use crate::{local_attachment_source as source, web_upload};
    let native = crate::draft_client::DraftClient::default();
    let web = crate::draft_client::DraftClient::verified_web(&"b".repeat(64)).unwrap();
    std::fs::remove_file(&frozen.1.source_path).unwrap();
    let stale = web_upload::bind(
        &mut f.database,
        &f.directory,
        &native,
        frozen.0.clone(),
        frozen.1,
    )
    .unwrap();
    assert!(!stale.replayed);
    assert_eq!(stale.result.code, "single_chat.pending_input_edit_fenced");
    assert!(
        web_upload::reconcile(&f.database, &native, frozen.0)
            .unwrap()
            .unwrap()
            .replayed
    );

    let path = f.directory.join("pending-web-spool.txt");
    std::fs::write(&path, b"web child pending bytes").unwrap();
    let observation = crate::local_attachment_snapshot::observe_private_source(&path).unwrap();
    let mut item =
        source::observe_source_attachment(&path, "pending-web-spool.txt", Some("text/plain"))
            .unwrap();
    item.id = Uuid::new_v4().to_string();
    let intent = web_upload::UploadIntent {
        command_id: item.id.clone(),
        camp_id: f.camp_id.clone(),
        expected_revision: 1,
        target: web_upload::UploadTarget::DelegationChildPending {
            conversation_id: conversation.into(),
            pending_input_id: pending.into(),
            edit_token: token.into(),
            composer_scope: None,
        },
        display_name: item.display_name.clone(),
        source_kind: source::LocalAttachmentKind::File,
        byte_size: observation.byte_size,
        sha256: observation.content_digest,
    };
    assert_eq!(
        web_upload::bind(
            &mut f.database,
            &f.directory,
            &web,
            intent.clone(),
            item.clone()
        )
        .unwrap()
        .result
        .status,
        crate::command::CommandResultStatus::Applied
    );
    let locator = pending_edit_locator(f, conversation, pending, token, &item.id);
    assert_eq!(
        source::load_source_attachment_for_client(&f.database, &locator, &web).unwrap(),
        Some(item.clone())
    );
    let store = crate::camp_attachment::ThreadAttachmentStore::new(&f.directory);
    assert!(
        crate::camp_attachment::attachment_location(&f.database, &store, &locator, &native)
            .unwrap()
            .is_none()
    );
    let stale = pending_edit_locator(f, conversation, pending, old_token, &item.id);
    assert!(
        crate::camp_attachment::attachment_location(&f.database, &store, &stale, &web)
            .unwrap()
            .is_none()
    );
    let wrong = pending_edit_locator(f, conversation, other_pending, token, &item.id);
    assert!(
        crate::camp_attachment::attachment_location(&f.database, &store, &wrong, &web)
            .unwrap()
            .is_none()
    );
    let mut wrong = locator.clone();
    if let source::LocalAttachmentOwnerLocator::DelegationChildPendingEdit {
        composer_scope, ..
    } = &mut wrong
    {
        *composer_scope = Some(
            crate::single_chat_composer_scope::SingleChatComposerScope::Canvas {
                board_id: Uuid::new_v4().to_string(),
                node_id: Uuid::new_v4().to_string(),
            },
        );
    }
    assert!(
        crate::camp_attachment::attachment_location(&f.database, &store, &wrong, &web)
            .unwrap()
            .is_none()
    );
    let saved = source::LocalAttachmentOwnerLocator::DelegationChildPending {
        camp_id: f.camp_id.clone(),
        conversation_id: conversation.into(),
        pending_input_id: pending.into(),
        attachment_ref_id: item.id.clone(),
    };
    assert!(
        source::load_source_attachment_for_client(&f.database, &saved, &web)
            .unwrap()
            .is_none()
    );
    let snapshot = web_upload::snapshot(&f.database, &f.directory, &web, &intent).unwrap();
    assert_eq!(
        snapshot["pendingInputs"]["editSession"]["workingAttachments"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(!serde_json::to_string(&snapshot)
        .unwrap()
        .contains("sourcePath"));
    let changes: i64 = f
        .database
        .connection()
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    assert!(
        web_upload::bind(&mut f.database, &f.directory, &web, intent, item.clone())
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
    let saved_ref = queue_read(f, &web, conversation).items[0].attachments[0]
        .id
        .clone();
    let saved = source::LocalAttachmentOwnerLocator::DelegationChildPending {
        camp_id: f.camp_id.clone(),
        conversation_id: conversation.into(),
        pending_input_id: pending.into(),
        attachment_ref_id: saved_ref,
    };
    assert!(
        source::load_source_attachment_for_client(&f.database, &saved, &native)
            .unwrap()
            .is_some()
    );
    item.id
}
