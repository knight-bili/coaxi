// Extends the original policy owner; no new Rust test or Core database.
fn assert_frozen_policy_versions() {
    use crate::delegation_policy::{FrozenPolicy, Role};
    for version in [0, 1, 2, 3, 4] {
        for operation in [
            "delegation.start",
            "delegate_to_agent",
            "delegation.status",
            "get_delegation_status",
            "delegation.cancel",
            "cancel_delegation",
            "delegation.resume",
            "resume_delegation",
            "delegation.read",
            "delegation.history",
        ] {
            assert_eq!(
                single_chat_operation_is_allowed(version, operation),
                version == 3
            );
        }
        assert!(!single_chat_operation_is_allowed(version, "memory.write"));
        assert_eq!(
            single_chat_operation_is_allowed(version, "thread.read"),
            (1..=3).contains(&version)
        );
        assert_eq!(
            frozen_single_chat_policy_is_supported(SINGLE_CHAT_OPERATION_POLICY, version, "{}")
                .unwrap(),
            matches!(version, 1 | 2)
        );
        for role in [Role::PublicParent, Role::PrivateParent, Role::Child] {
            let config =
                json!({"delegationPolicy":FrozenPolicy::new(false,8,role).unwrap()}).to_string();
            assert_eq!(
                frozen_single_chat_policy_is_supported(
                    SINGLE_CHAT_OPERATION_POLICY,
                    version,
                    &config
                )
                .unwrap(),
                version == 3 && role == Role::PrivateParent
            );
        }
    }
    for invalid in [
        "null",
        "{}",
        r#"{"schemaVersion":2,"enabled":true,"depthLimit":1,"role":"private_parent"}"#,
    ] {
        let value: Value = serde_json::from_str(invalid).unwrap();
        assert!(
            frozen_single_chat_policy_is_supported(
                SINGLE_CHAT_OPERATION_POLICY,
                3,
                &json!({"delegationPolicy":value}).to_string()
            )
            .is_err()
        );
    }
}

fn assert_delegation_policy3(
    database: &mut Database,
    service: &SingleChatService,
    camp: &str,
    legacy_run: &str,
) {
    let legacy: String = database
        .connection()
        .query_row(
            "SELECT effective_config_json FROM agent_run WHERE id=?1",
            [legacy_run],
            |r| r.get(0),
        )
        .unwrap();
    let legacy_conversation: String = database
        .connection()
        .query_row(
            "SELECT conversation_id FROM agent_run WHERE id=?1",
            [legacy_run],
            |row| row.get(0),
        )
        .unwrap();
    let ended = service
        .end(
            database,
            &user_envelope(
                "policy3-close-legacy",
                Some(camp),
                EndSingleChatCommand {
                    camp_id: camp.into(),
                    conversation_id: legacy_conversation,
                },
            ),
        )
        .unwrap();
    assert_eq!(ended.result.status, CommandResultStatus::Applied);
    database
        .connection()
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .unwrap();
    let tx = database.connection().unchecked_transaction().unwrap();
    crate::db::delegation::install_identity_schema(&tx).unwrap();
    tx.commit().unwrap();
    database
        .connection()
        .execute_batch("PRAGMA foreign_keys=ON;")
        .unwrap();
    // Exact readiness bits in the existing isolated Core fixture, not proof of
    // the still-unregistered full startup migration or a real Runtime launch.
    database.connection().execute_batch("UPDATE coaxi_data_contract SET contract_version='v1.73',projection_schema_version=160; INSERT INTO schema_migration VALUES(210,datetime('now')); UPDATE delegation_settings SET enabled=1,depth_limit=8;").unwrap();
    assert_eq!(
        database
            .connection()
            .query_row(
                "SELECT effective_config_json FROM agent_run WHERE id=?1",
                [legacy_run],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        legacy
    );
    assert_eq!(
        authorize_builtin_operation(database, legacy_run, 1, "delegation.start", &json!({}))
            .unwrap()
            .unwrap()
            .code,
        "single_chat.operation_denied"
    );
    let (conversation, _) = open(service, database, camp, "single-chat-open-policy3");
    let sent = send(
        service,
        database,
        camp,
        &conversation,
        "single-chat-send-policy3",
    );
    assert_eq!(sent.result.status, CommandResultStatus::Accepted);
    let run = sent.result.payload["agentRunId"]
        .as_str()
        .unwrap()
        .to_string();
    let (version, config): (i64, String) = database
        .connection()
        .query_row(
            "SELECT operation_policy_version,effective_config_json FROM agent_run WHERE id=?1",
            [&run],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(version, 3);
    assert_eq!(
        serde_json::from_str::<Value>(&config).unwrap()["delegationPolicy"]["role"],
        "private_parent"
    );
    // Existing owner uses an explicit running-state fixture, not native input ACK.
    database.connection().execute("UPDATE agent_run SET status='running',execution_epoch=1,started_at=updated_at WHERE id=?1",[&run]).unwrap();
    for operation in [
        "thread.read",
        "single_chat.history",
        "mission.get",
        "delegation.start",
        "delegation.status",
        "delegation.cancel",
        "delegation.resume",
        "delegation.read",
        "delegation.history",
    ] {
        assert!(
            authorize_builtin_operation(database, &run, 1, operation, &json!({}))
                .unwrap()
                .is_none(),
            "{operation}"
        );
    }
    let writes = database.connection().total_changes();
    let history = service
        .history(
            database,
            &run,
            1,
            &SingleChatHistoryInput {
                limit: None,
                before_sequence: None,
            },
        )
        .unwrap();
    assert!(serde_json::to_value(history).unwrap().is_object());
    assert_eq!(database.connection().total_changes(), writes);
    let finish = |database: &mut Database, command_id: &str| {
        ExecutionRuntimeService::default()
            .succeed_agent_run(
                database,
                &CommandEnvelope {
                    command_id: command_id.into(),
                    actor: ActorRef::System {
                        component_id: "runtime-adapter:test".into(),
                    },
                    camp_id: Some(camp.into()),
                    expected_versions: vec![],
                    execution_epoch: None,
                    payload: SucceedAgentRunCommand {
                        agent_run_id: run.clone(),
                        expected_version: 1,
                        execution_epoch: 1,
                        native_turn_id: "policy3-fixture-turn".into(),
                        final_output: "private policy3 final".into(),
                        missing_send_recovery_candidate: None,
                        ending_git_observation: None,
                    },
                },
            )
            .unwrap()
    };
    // Existing output-route immutability already forbids changing the number.
    assert!(
        database
            .connection()
            .execute(
                "UPDATE agent_run SET operation_policy_version=2 WHERE id=?1",
                [&run]
            )
            .is_err()
    );
    assert!(
        database
            .connection()
            .execute(
                "UPDATE agent_run SET operation_policy_version=3 WHERE id=?1",
                [legacy_run]
            )
            .is_err()
    );
    let mut wrong_role: Value = serde_json::from_str(&config).unwrap();
    wrong_role["delegationPolicy"]["role"] = json!("public_parent");
    // Corrupt configuration alone cannot borrow a supported v3 number.
    for (i, bad_config) in [legacy.clone(), wrong_role.to_string()]
        .into_iter()
        .enumerate()
    {
        database
            .connection()
            .execute(
                "UPDATE agent_run SET effective_config_json=?2 WHERE id=?1",
                params![run, bad_config],
            )
            .unwrap();
        assert!(authorize_builtin_operation(database, &run, 1, "thread.read", &json!({})).is_err());
        assert!(
            service
                .history(
                    database,
                    &run,
                    1,
                    &SingleChatHistoryInput {
                        limit: None,
                        before_sequence: None
                    }
                )
                .is_err()
        );
        let denied = finish(database, &format!("policy3-invalid-role-{i}"));
        assert_eq!(denied.result.status, CommandResultStatus::Rejected);
        assert_eq!(denied.result.code, "agent_run.terminal_fenced");
    }
    database
        .connection()
        .execute(
            "UPDATE agent_run SET operation_policy_version=3,effective_config_json=?2 WHERE id=?1",
            params![run, config],
        )
        .unwrap();
    let completed = finish(database, "single-chat-policy3-success");
    assert_eq!(
        completed.result.status,
        CommandResultStatus::Applied,
        "{:?}",
        completed.result
    );
    assert_eq!(database.connection().query_row("SELECT count(*) FROM conversation_message WHERE source_agent_run_id=?1 AND conversation_id=?2",params![run,conversation],|r|r.get::<_,i64>(0)).unwrap(),1);
    assert_eq!(
        database
            .connection()
            .query_row(
                "SELECT count(*) FROM camp_message WHERE source_agent_run_id=?1",
                [&run],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}
