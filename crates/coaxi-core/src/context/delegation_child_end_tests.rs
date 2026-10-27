// Reuses the existing owner and database. Factory and provider signals remain fixtures.
fn child_end_send(f: &mut Fixture, conversation: &str, body: &str) -> Value {
    let client = crate::draft_client::DraftClient::default();
    let draft = crate::delegation_user::read_draft(
        &f.database,
        &queue_actor(),
        &client,
        &f.camp_id,
        conversation,
        None,
    )
    .unwrap();
    let revision = serde_json::to_value(draft).unwrap()["revision"]
        .as_i64()
        .unwrap();
    let envelope = crate::application::delegation_input::send_envelope(
        &f.database, &client,
        json!({"commandId":Uuid::new_v4().to_string(),"command":{"conversationId":conversation,"expectedRevision":revision,"body":body}}),
    ).unwrap();
    let result = crate::delegation_user::send(&mut f.database, &envelope).unwrap();
    assert_eq!(
        result.result.status,
        crate::command::CommandResultStatus::Accepted,
        "{:?}",
        result.result
    );
    result.result.payload
}
fn child_end_snapshot(f: &Fixture, conversation: &str) -> String {
    f.database.connection().query_row("SELECT json_object('ended',c.ended_at,'version',c.version,'sequence',c.last_message_sequence,
      'messages',(SELECT count(*) FROM conversation_message WHERE conversation_id=c.id),
      'drafts',(SELECT json_group_array(json_object('client',client_id,'scope',scope_key,'revision',revision,'attachments',source_attachments_json,'quotes',quotes_json)) FROM single_chat_composer_draft WHERE conversation_id=c.id),
      'pending',(SELECT json_group_array(json_object('id',id,'state',state,'revision',revision,'body',body)) FROM single_chat_pending_input WHERE conversation_id=c.id),
      'edits',(SELECT count(*) FROM single_chat_pending_input_edit_session WHERE conversation_id=c.id),
      'events',(SELECT count(*) FROM event_log)) FROM conversation c WHERE c.id=?1",[conversation],|r|r.get(0)).unwrap()
}
fn assert_child_user_end(f: &mut Fixture, parent: &str, epoch: i64) {
    use crate::{application::delegation_control as api, delegation_user as user};
    let (id, original, original_epoch) =
        context_child_of(f, parent, epoch, "User可结束的子会话", "user-end-root");
    let (_, unrelated, _) = context_child_of(
        f,
        parent,
        epoch,
        "另一个仍在工作的分支",
        "user-end-unrelated",
    );
    let conversation: String = f
        .database
        .connection()
        .query_row(
            "SELECT conversation_id FROM agent_run WHERE id=?1",
            [&original],
            |r| r.get(0),
        )
        .unwrap();
    let request = || json!({"commandId":Uuid::new_v4().to_string(),"command":{"conversationId":conversation}});
    let not_owned =
        api::handle(&mut f.database, "delegations.conversation.end", request()).unwrap();
    assert_eq!(not_owned.result["status"], "rejected");
    api::handle(
        &mut f.database,
        "delegations.takeover",
        control_request(&id, "expectedControlVersion", 1),
    )
    .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &original, original_epoch)
        .unwrap();
    let first = child_end_send(f, &conversation, "第一轮用户工作");
    let first_run = first["agentRunId"].as_str().unwrap();
    let first_epoch = claim_context_child(f, first_run);
    let (_, background, background_epoch) = context_child_of(
        f,
        first_run,
        first_epoch,
        "完成后保留后台",
        "user-end-background-parent",
    );
    let (_, leaf, _) = context_child_of(
        f,
        &background,
        background_epoch,
        "经过终态中间节点的后台",
        "user-end-background-leaf",
    );
    let (independent, independent_run, independent_epoch) = context_child_of(
        f,
        first_run,
        first_epoch,
        "单独接管后保留",
        "user-end-independent",
    );
    api::handle(
        &mut f.database,
        "delegations.takeover",
        control_request(&independent, "expectedControlVersion", 1),
    )
    .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &independent_run, independent_epoch)
        .unwrap();
    let independent_conversation: String = f
        .database
        .connection()
        .query_row(
            "SELECT conversation_id FROM agent_run WHERE id=?1",
            [&independent_run],
            |r| r.get(0),
        )
        .unwrap();
    let independent_user =
        child_end_send(f, &independent_conversation, "独立接管后的工作")["agentRunId"]
            .as_str()
            .unwrap()
            .to_string();
    lifecycle_succeed(f, &background, background_epoch);
    lifecycle_succeed(f, first_run, first_epoch);
    assert_eq!(lifecycle_status(f, &leaf), "running");
    let second = child_end_send(f, &conversation, "第二轮用户工作");
    let second_run = second["agentRunId"].as_str().unwrap();
    claim_context_child(f, second_run);
    let queued = child_end_send(f, &conversation, "结束时应停止排队");
    let pending = queued["pendingInputId"].as_str().unwrap();
    let edit=crate::application::delegation_input::edit_envelope(&f.database,&crate::draft_client::DraftClient::default(),json!({
        "commandId":Uuid::new_v4().to_string(),"command":{"conversationId":conversation,"pendingInputId":pending,"expectedRevision":1,"editToken":null,"action":{"type":"begin"}}
    })).unwrap();
    let editing = user::edit_pending_input(&mut f.database, &edit).unwrap();
    assert_eq!(
        editing.result.status,
        crate::command::CommandResultStatus::Applied,
        "{:?}",
        editing.result
    );
    let input = request();
    for forged in [
        "threadId",
        "campId",
        "actor",
        "draftClient",
        "sourcePath",
        "delegationId",
    ] {
        let mut invalid = input.clone();
        invalid["command"][forged] = json!("forged");
        assert!(api::end_envelope(&f.database, invalid).is_err(), "{forged}");
    }
    let mut wrong = api::end_envelope(&f.database, request()).unwrap();
    wrong.camp_id = Some("other-thread".into());
    assert_eq!(
        user::end_conversation(&mut f.database, &wrong)
            .unwrap()
            .result
            .status,
        crate::command::CommandResultStatus::Rejected
    );
    let mut wrong = api::end_envelope(&f.database, request()).unwrap();
    wrong.actor = ActorRef::System {
        component_id: "fixture".into(),
    };
    assert_eq!(
        user::end_conversation(&mut f.database, &wrong)
            .unwrap()
            .result
            .code,
        "delegation.user_required"
    );
    let legacy = lifecycle_command(
        f,
        queue_actor(),
        crate::single_chat::EndSingleChatCommand {
            camp_id: f.camp_id.clone(),
            conversation_id: conversation.clone(),
        },
    );
    assert_eq!(
        crate::single_chat::SingleChatService::default()
            .end(&mut f.database, &legacy)
            .unwrap()
            .result
            .code,
        "single_chat.not_active"
    );
    let old_private: String = f
        .database
        .connection()
        .query_row(
            "SELECT id FROM conversation WHERE kind='single_chat' LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        api::end_envelope(
            &f.database,
            json!({"commandId":Uuid::new_v4().to_string(),"command":{"conversationId":old_private}})
        )
        .is_err()
    );
    let before = child_end_snapshot(f, &conversation);
    let rows = control_rows(f);
    let control = crate::delegation::control(f.database.connection(), &id).unwrap();
    f.database.connection().execute_batch(&format!("CREATE TEMP TRIGGER reject_child_end BEFORE UPDATE OF state ON single_chat_pending_input WHEN OLD.id='{pending}' AND NEW.state='cancelled' BEGIN SELECT RAISE(ABORT,'injected child end rollback'); END;")).unwrap();
    let error = api::handle(
        &mut f.database,
        "delegations.conversation.end",
        input.clone(),
    )
    .err()
    .unwrap();
    assert!(format!("{error:#}").contains("injected child end rollback"));
    assert_eq!(child_end_snapshot(f, &conversation), before);
    assert_eq!(control_rows(f), rows);
    assert_eq!(
        api::reconcile(&f.database, "delegations.conversation.end", input.clone()).unwrap()["state"],
        "unknown"
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER reject_child_end;")
        .unwrap();
    let closed = api::handle(
        &mut f.database,
        "delegations.conversation.end",
        input.clone(),
    )
    .unwrap();
    assert_eq!(closed.result["code"], "delegation.conversation_ended");
    assert_eq!(
        closed.changed,
        Some((f.camp_id.clone(), conversation.clone()))
    );
    assert_eq!(lifecycle_status(f, second_run), "cancelled");
    assert_eq!(lifecycle_status(f, &leaf), "cancelled");
    assert_eq!(lifecycle_status(f, first_run), "succeeded");
    assert_eq!(lifecycle_status(f, &original), "cancelled");
    let after = control_rows(f);
    for protected in [&independent_user, &unrelated] {
        assert_eq!(
            rows.iter().find(|r| &r.0 == protected),
            after.iter().find(|r| &r.0 == protected)
        );
    }
    let now = crate::delegation::control(f.database.connection(), &id).unwrap();
    assert_eq!(
        (control.version, control.takeover_message_sequence),
        (now.version, now.takeover_message_sequence)
    );
    let history: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT count(*) FROM conversation_message WHERE conversation_id=?1",
            [&conversation],
            |r| r.get(0),
        )
        .unwrap();
    assert!(history >= 4);
    let cleared:(i64,i64,i64)=f.database.connection().query_row("SELECT (SELECT count(*) FROM single_chat_composer_draft WHERE conversation_id=?1),(SELECT count(*) FROM single_chat_pending_input_edit_session WHERE conversation_id=?1),(SELECT count(*) FROM single_chat_pending_input WHERE conversation_id=?1 AND state IN ('queued','needs_repair'))",[&conversation],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(cleared, (0, 0, 0));
    let changes = f.database.connection().total_changes();
    let replay = api::handle(
        &mut f.database,
        "delegations.conversation.end",
        input.clone(),
    )
    .unwrap();
    assert_eq!(replay.result, closed.result);
    assert!(replay.changed.is_none());
    assert_eq!(
        api::reconcile(&f.database, "delegations.conversation.end", input).unwrap()["result"],
        closed.result
    );
    assert_eq!(changes, f.database.connection().total_changes());
    let repeated = api::handle(&mut f.database, "delegations.conversation.end", request()).unwrap();
    assert_eq!(repeated.result["code"], "delegation.conversation_ended");
    assert_eq!(
        repeated.result["payload"]["cancelledAgentRunId"],
        Value::Null
    );
    let rejected=crate::application::delegation_input::send_envelope(&f.database,&crate::draft_client::DraftClient::default(),json!({"commandId":Uuid::new_v4().to_string(),"command":{"conversationId":conversation,"expectedRevision":0,"body":"结束后不得发送"}})).unwrap();
    assert_eq!(
        user::send(&mut f.database, &rejected)
            .unwrap()
            .result
            .status,
        crate::command::CommandResultStatus::Rejected
    );
    assert!(
        !user::ready_pending_inputs(&f.database)
            .unwrap()
            .iter()
            .any(|p| p.0.conversation_id == conversation)
    );
}
