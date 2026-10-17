// Extends the existing SQLite materialization owner. SQL terminal/takeover setup is not lifecycle proof.
fn child_material_command(
    fixture: &Fixture,
    client: &crate::draft_client::DraftClient,
    conversation: &str,
    revision: i64,
    action: Value,
) -> CommandEnvelope<crate::delegation_user::MutateDelegationComposerCommand> {
    crate::application::delegation_composer::mutation_envelope(client,json!({
        "commandId":Uuid::new_v4().to_string(),
        "command":{"threadId":fixture.camp_id,"conversationId":conversation,
            "expectedDraftRevision":revision,"action":action},
    })).unwrap()
}

fn assert_child_user_materials(fixture: &mut Fixture, id: &str, run: &str, legacy_run: &str) {
    use crate::delegation_user;
    use crate::draft_client::DraftClient;
    use crate::local_attachment_source::{LocalAttachmentKind, LocalAttachmentSourceRef};
    use crate::single_chat_composer;
    let client=DraftClient::default();
    let conversation:String=fixture.database.connection().query_row(
        "SELECT child_conversation_id FROM delegation WHERE id=?1",[id],|r|r.get(0)).unwrap();
    let foreign:String=fixture.database.connection().query_row(
        "SELECT conversation_id FROM agent_run WHERE id=?1",[legacy_run],|r|r.get(0)).unwrap();
    let reader=json!({"conversationId":conversation});
    assert!(crate::application::delegation_composer::handle(&fixture.database,&client,"delegations.composer.get",reader.clone()).is_err());
    let before=child_material_command(fixture,&client,&conversation,0,json!({"type":"register_scope"}));
    let denied=delegation_user::mutate_draft(&mut fixture.database,&before).unwrap();
    assert_eq!(denied.result.status,crate::command::CommandResultStatus::Rejected);
    assert_eq!(denied.result.code,"delegation.not_user_controlled");
    assert_eq!(fixture.database.connection().query_row("SELECT count(*) FROM single_chat_composer_draft WHERE conversation_id=?1",[&conversation],|r|r.get::<_,i64>(0)).unwrap(),0);

    let path=fixture.directory.join("owned-child-material.txt");
    let content=b"independent child source";
    std::fs::write(&path,content).unwrap();
    let source=LocalAttachmentSourceRef {id:Uuid::new_v4().to_string(),source_path:path.canonicalize().unwrap().display().to_string(),
        display_name:"owned-child-material.txt".into(),kind:LocalAttachmentKind::File,media_type:Some("text/plain".into()),observed_byte_size:Some(content.len() as u64)};
    let sources=crate::private_source_observation::PreparedSources::capture(vec![source.clone()]).unwrap();
    let tx=fixture.database.connection_mut().transaction().unwrap();
    assert!(crate::private_source_observation::capture_sources(&tx,&conversation,std::slice::from_ref(&source),&sources).is_err());
    tx.rollback().unwrap();

    let assistant=Uuid::new_v4().to_string();
    let assistant_body="帮手自己的检查结果";
    let tx=fixture.database.connection_mut().transaction().unwrap();
    tx.execute("INSERT INTO conversation_message(id,conversation_id,sequence,author_type,author_id,body,source_agent_run_id,created_at) VALUES(?1,?2,4,'agent','agent_1',?3,?4,'2026-10-17T00:00:03Z')",params![assistant,conversation,assistant_body,run]).unwrap();
    tx.execute("UPDATE conversation SET last_message_sequence=4 WHERE id=?1",[&conversation]).unwrap();
    tx.execute("UPDATE agent_run SET status='cancelled',ended_at='2026-10-17T00:00:04Z' WHERE id=?1",[run]).unwrap();
    let takeover=crate::delegation::record_takeover_boundary(&tx,id,1,&Uuid::new_v4().to_string(),"2026-10-17T00:00:05Z").unwrap();
    assert_eq!(takeover.takeover_message_sequence,Some(4));
    tx.commit().unwrap();
    let tx=fixture.database.connection_mut().transaction().unwrap();
    assert!(crate::private_source_observation::capture_sources(&tx,&conversation,std::slice::from_ref(&source),&sources).unwrap().is_none());
    tx.commit().unwrap();
    let snapshot=crate::application::delegation_composer::handle(&fixture.database,&client,"delegations.composer.get",reader.clone()).unwrap();
    assert_eq!(snapshot["revision"],0);
    for actor in [ActorRef::System {component_id:"material-test".into()},ActorRef::Agent {agent_id:"agent_1".into(),source_agent_run_id:fixture.run_id.clone()}] {
        assert!(delegation_user::read_draft(&fixture.database,&actor,&client,&fixture.camp_id,&conversation,None).is_err());
        let mut command=child_material_command(fixture,&client,&conversation,0,json!({"type":"register_scope"}));
        command.actor=actor;
        command.execution_epoch=Some(fixture.execution_epoch);
        let result=delegation_user::mutate_draft(&mut fixture.database,&command);
        assert!(result.is_err() || result.unwrap().result.status==crate::command::CommandResultStatus::Rejected);
    }
    let mut wrong=child_material_command(fixture,&client,&conversation,0,json!({"type":"register_scope"}));
    wrong.payload.0.conversation_id=foreign.clone();
    assert_eq!(delegation_user::mutate_draft(&mut fixture.database,&wrong).unwrap().result.status,crate::command::CommandResultStatus::Rejected);
    let mut wrong_thread=child_material_command(fixture,&client,&conversation,0,json!({"type":"register_scope"}));
    wrong_thread.camp_id=None;
    let wrong_result=delegation_user::mutate_draft(&mut fixture.database,&wrong_thread);
    assert!(wrong_result.is_err() || wrong_result.unwrap().result.status==crate::command::CommandResultStatus::Rejected);
    let own=child_material_command(fixture,&client,&conversation,0,json!({"type":"register_scope"}));
    let old=CommandEnvelope {command_id:own.command_id,actor:own.actor,camp_id:own.camp_id,expected_versions:own.expected_versions,
        execution_epoch:own.execution_epoch,payload:own.payload.0};
    assert_eq!(single_chat_composer::mutate(&mut fixture.database,&old).unwrap().result.code,"single_chat.not_active");

    let quote=child_material_command(fixture,&client,&conversation,0,json!({"type":"quote","action":{"type":"add","selection":{
        "messageId":assistant,"bodyAtSelection":assistant_body,"startScalar":0,"endScalar":assistant_body.chars().count(),"text":assistant_body}}}));
    let quoted=delegation_user::mutate_draft(&mut fixture.database,&quote).unwrap();
    assert_eq!(quoted.result.status,crate::command::CommandResultStatus::Applied,"{:?}",quoted.result);
    let first=crate::application::delegation_composer::handle(&fixture.database,&client,"delegations.composer.get",reader.clone()).unwrap();
    assert_eq!(first["revision"],1);
    assert_eq!(first["quotes"][0]["source"]["scope"],"delegation_child");
    assert_eq!(first["quotes"][0]["source"]["conversationId"],conversation);
    assert_eq!(first["quotes"][0]["authorAtCapture"]["displayName"],format!("Codex CLI {}",id.chars().take(8).collect::<String>()));
    let quote_id=first["quotes"][0]["quoteId"].as_str().unwrap();
    let removed=child_material_command(fixture,&client,&conversation,1,json!({"type":"quote","action":{"type":"remove","quoteId":quote_id}}));
    assert_eq!(delegation_user::mutate_draft(&mut fixture.database,&removed).unwrap().result.status,crate::command::CommandResultStatus::Applied);
    let restored=child_material_command(fixture,&client,&conversation,2,json!({"type":"quote","action":{"type":"restore","quoteId":quote_id}}));
    assert_eq!(delegation_user::mutate_draft(&mut fixture.database,&restored).unwrap().result.status,crate::command::CommandResultStatus::Applied);
    let latest=crate::application::delegation_composer::handle(&fixture.database,&client,"delegations.composer.get",reader.clone()).unwrap();
    assert_eq!(latest["revision"],3);
    assert_eq!(latest["quotes"],first["quotes"]);
    let stale=child_material_command(fixture,&client,&conversation,0,json!({"type":"quote","action":{"type":"remove","quoteId":quote_id}}));
    assert_eq!(delegation_user::mutate_draft(&mut fixture.database,&stale).unwrap().result.code,"single_chat.draft_changed");
    let foreign_message:String=fixture.database.connection().query_row("SELECT trigger_conversation_message_id FROM agent_run WHERE id=?1",[legacy_run],|r|r.get(0)).unwrap();
    let bad_quote=child_material_command(fixture,&client,&conversation,3,json!({"type":"quote","action":{"type":"add","selection":{
        "messageId":foreign_message,"bodyAtSelection":"foreign","startScalar":0,"endScalar":7,"text":"foreign"}}}));
    assert_eq!(delegation_user::mutate_draft(&mut fixture.database,&bad_quote).unwrap().result.code,"quote.source_unavailable");

    let before_changes:i64=fixture.database.connection().query_row("SELECT total_changes()",[],|r|r.get(0)).unwrap();
    let replayed=delegation_user::mutate_draft(&mut fixture.database,&quote).unwrap();
    assert!(replayed.replayed);
    assert_eq!(replayed.result,quoted.result);
    let request=json!({"commandId":quote.command_id,"command":quote.payload});
    let reconciled=crate::application::delegation_composer::reconcile(&fixture.database,&client,request).unwrap();
    assert_eq!(reconciled["state"],"recorded");
    assert_eq!(reconciled["result"],serde_json::to_value(&quoted.result).unwrap());
    assert!(crate::application::delegation_composer::handle(&fixture.database,&client,"delegations.composer.recovery",reader.clone()).is_ok());
    assert_eq!(before_changes,fixture.database.connection().query_row("SELECT total_changes()",[],|r|r.get::<_,i64>(0)).unwrap());
    assert!(crate::application::delegation_composer::handle(&fixture.database,&client,"delegations.composer.get",json!({"conversationId":foreign})).is_err());
    assert!(crate::application::delegation_composer::handle(&fixture.database,&client,"delegations.composer.get",json!({"conversationId":conversation,"sourcePath":"forged"})).is_err());
    assert_eq!(crate::application::delegation_composer::handle(&fixture.database,&client,"delegations.composer.get",json!({"conversationId":conversation,"composerScope":null})).unwrap(),latest);

    // Browser materials share the same child owner, but not the Native editor's draft or retained input.
    let web=DraftClient::verified_web(&"a".repeat(64)).unwrap();
    assert_eq!(crate::application::delegation_composer::handle(&fixture.database,&web,"delegations.composer.get",reader.clone()).unwrap()["revision"],0);
    let second_path=fixture.directory.join("second-owned-child-material.txt");
    std::fs::write(&second_path,content).unwrap();
    let second_source=LocalAttachmentSourceRef {id:Uuid::new_v4().to_string(),source_path:second_path.canonicalize().unwrap().display().to_string(),..source.clone()};
    let refs=vec![source.clone(),second_source.clone()];
    let prepared=crate::private_source_observation::PreparedSources::capture(refs.clone()).unwrap();
    let tx=fixture.database.connection_mut().transaction().unwrap();
    assert!(crate::private_source_observation::capture_sources(&tx,&conversation,&refs,&prepared).unwrap().is_none());
    tx.execute("INSERT INTO single_chat_composer_draft(conversation_id,client_id,scope_key,revision,source_attachments_json,updated_at) VALUES(?1,?2,'',1,?3,'2026-10-17T00:00:06Z')",
        params![conversation,web.id(),crate::local_attachment_source::serialize_source_attachments(&refs).unwrap()]).unwrap();
    let retained=Uuid::new_v4().to_string();
    tx.execute("INSERT INTO single_chat_retained_input(id,conversation_id,client_id,scope_key,revision,body,source_attachments_json,quotes_json,quote_trash_json,created_at,updated_at) VALUES(?1,?2,?3,'',1,'网页保留正文','[]','[]','[]','2026-10-17T00:00:06Z','2026-10-17T00:00:06Z')",
        params![retained,conversation,web.id()]).unwrap();
    tx.commit().unwrap();
    let reorder=child_material_command(fixture,&web,&conversation,1,json!({"type":"reorder_attachments","attachmentRefIds":[second_source.id,source.id]}));
    assert_eq!(delegation_user::mutate_draft(&mut fixture.database,&reorder).unwrap().result.status,crate::command::CommandResultStatus::Applied);
    let key=crate::single_chat_composer_scope::PrivateComposerKey::default();
    let reordered=single_chat_composer::DraftMaterials::load(fixture.database.connection(),&conversation,&web,&key).unwrap();
    assert_eq!(reordered.sources().unwrap(),vec![second_source.clone(),source.clone()]);
    let remove=child_material_command(fixture,&web,&conversation,2,json!({"type":"remove_attachment","attachmentRefId":source.id}));
    assert_eq!(delegation_user::mutate_draft(&mut fixture.database,&remove).unwrap().result.status,crate::command::CommandResultStatus::Applied);
    let invalid_order=child_material_command(fixture,&web,&conversation,3,json!({"type":"reorder_attachments","attachmentRefIds":[second_source.id,second_source.id]}));
    assert_eq!(delegation_user::mutate_draft(&mut fixture.database,&invalid_order).unwrap().result.code,"attachment_order_invalid");
    let swap=child_material_command(fixture,&web,&conversation,3,json!({"type":"swap_retained","retainedInputId":retained,"expectedRetainedRevision":1,"currentBody":"当前用户正文"}));
    let swapped=delegation_user::mutate_draft(&mut fixture.database,&swap).unwrap();
    assert_eq!(swapped.result.status,crate::command::CommandResultStatus::Applied);
    assert_eq!(swapped.result.payload["body"],"网页保留正文");
    let saved:(String,String,i64)=fixture.database.connection().query_row("SELECT body,source_attachments_json,revision FROM single_chat_retained_input WHERE id=?1",[&retained],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(saved.0,"当前用户正文");
    assert_eq!(crate::local_attachment_source::parse_source_attachments(&saved.1).unwrap(),vec![second_source.clone()]);
    assert_eq!(saved.2,2);
    let cross_client=child_material_command(fixture,&client,&conversation,3,json!({"type":"discard_retained","retainedInputId":retained,"expectedRetainedRevision":2}));
    assert_eq!(delegation_user::mutate_draft(&mut fixture.database,&cross_client).unwrap().result.code,"single_chat.retained_not_found");
    let discard=child_material_command(fixture,&web,&conversation,4,json!({"type":"discard_retained","retainedInputId":retained,"expectedRetainedRevision":2}));
    assert_eq!(delegation_user::mutate_draft(&mut fixture.database,&discard).unwrap().result.status,crate::command::CommandResultStatus::Applied);
    let adopt=child_material_command(fixture,&web,&conversation,4,json!({"type":"adopt_legacy_materials","expectedSourceRevision":4,"targetBody":""}));
    assert_eq!(delegation_user::mutate_draft(&mut fixture.database,&adopt).unwrap().result.code,"single_chat.scope_must_differ");
    assert_eq!(crate::application::delegation_composer::handle(&fixture.database,&client,"delegations.composer.get",reader).unwrap(),latest);
}
