// Extends the existing Context owner/database. Host entry helpers, storage and
// User commands are real; native Runtime terminal/cleanup acknowledgments remain fixtures.
fn user_delegation_read(f: &Fixture, method: &str, input: Value) -> anyhow::Result<Value> {
    crate::application::delegation_queries::read(&f.database, method, input)
}
fn assert_user_delegation_queries(
    f: &Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    task: &str,
    full: &str,
) {
    use crate::{command::ActorRef, delegation_model_query::user};
    let before = f.database.connection().total_changes();
    let detail = user_delegation_read(f, "delegations.get", json!({"delegationId":task})).unwrap();
    assert_eq!(detail["delegationId"], task);
    assert_eq!(detail["status"], "succeeded");
    assert_eq!(detail["takenOver"], true);
    assert_eq!(detail["currentAttemptNo"], 1);
    let activity = &detail["activity"];
    let expected_runs: Vec<String> = f.database.connection().prepare(
        "SELECT id FROM agent_run WHERE conversation_id=?1 AND invocation_kind IN ('delegation','delegation_user') ORDER BY created_at,id"
    ).unwrap().query_map([detail["conversationId"].as_str().unwrap()], |row| row.get(0)).unwrap()
        .collect::<rusqlite::Result<_>>().unwrap();
    assert!(!expected_runs.is_empty());
    assert_eq!(
        activity["runs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|run| run["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        expected_runs
    );
    for evidence in activity["executionEvidence"].as_array().unwrap() {
        assert!(
            expected_runs
                .iter()
                .any(|run| Some(run.as_str()) == evidence["agentRunId"].as_str())
        );
    }
    for approval in activity["approvals"].as_array().unwrap() {
        assert!(
            expected_runs
                .iter()
                .any(|run| Some(run.as_str()) == approval["agentRunId"].as_str())
        );
        assert_eq!(approval["status"], "pending");
    }
    for forbidden in [
        "task",
        "text",
        "runtimePreset",
        "workingDir",
        "originBindingId",
        "bindingToken",
    ] {
        assert!(detail.get(forbidden).is_none(), "{forbidden}");
    }
    let parent = detail["parentConversationId"].as_str().unwrap();
    let page = user_delegation_read(
        f,
        "delegations.list",
        json!({"parentConversationId":parent}),
    )
    .unwrap();
    let full_list = user_delegation_read(
        f,
        "delegations.list",
        json!({"parentConversationId":parent,"limit":100}),
    )
    .unwrap();
    assert!(page["items"].as_array().unwrap().len() <= 20);
    let expected = full_list["items"].as_array().unwrap();
    assert!(expected.len() > 1);
    for item in expected {
        assert!(
            item.get("activity").is_none(),
            "list must not fetch child activity bodies"
        );
        if item["takenOver"] == false
            && matches!(
                item["status"].as_str(),
                Some("queued" | "running" | "waiting")
            )
        {
            assert_eq!(item["cleanupPending"], false);
        }
    }
    assert!(
        expected
            .iter()
            .all(|item| item["parentConversationId"] == parent)
    );
    let mut list = user_delegation_read(
        f,
        "delegations.list",
        json!({"parentConversationId":parent,"limit":1}),
    )
    .unwrap();
    let cursor = list["nextCursor"].as_str().unwrap().to_string();
    let mut gathered = Vec::new();
    loop {
        gathered.extend(list["items"].as_array().unwrap().iter().cloned());
        let Some(cursor) = list.get("nextCursor").cloned() else {
            break;
        };
        list = user_delegation_read(
            f,
            "delegations.list",
            json!({"parentConversationId":parent,"cursor":cursor,"limit":1}),
        )
        .unwrap();
    }
    assert_eq!(&gathered, expected);
    assert!(
        user_delegation_read(
            f,
            "delegations.list",
            json!({"parentConversationId":detail["conversationId"],"cursor":cursor})
        )
        .is_err()
    );
    let other = ActorRef::User {
        user_id: "another-host-user".into(),
    };
    assert!(
        user::list(
            &f.database,
            &other,
            &serde_json::from_value(json!({"parentConversationId":parent,"cursor":cursor}))
                .unwrap()
        )
        .is_err()
    );
    assert!(
        user_delegation_read(
            f,
            "delegations.list",
            json!({"parentConversationId":parent,"cursor":format!("{cursor}!")})
        )
        .is_err()
    );
    for limit in [json!(0), json!(101), json!(null), json!(true)] {
        assert!(
            user_delegation_read(
                f,
                "delegations.list",
                json!({"parentConversationId":parent,"limit":limit})
            )
            .is_err()
        );
    }
    let history = user_delegation_read(
        f,
        "delegations.history",
        json!({"task_id":task,"limit":100}),
    )
    .unwrap();
    let messages = history["messages"].as_array().unwrap();
    assert!(messages.len() >= 4);
    assert!(history.to_string().contains("用户接管后的私密正文"));
    assert!(history.to_string().contains("当前用户输入不进入自己的历史"));
    for forbidden in ["source_path", "observed_sha256", "bindingToken"] {
        assert!(!history.to_string().contains(forbidden));
    }
    let one =
        user_delegation_read(f, "delegations.history", json!({"task_id":task,"limit":1})).unwrap();
    let history_cursor = one["next_cursor"].as_str().unwrap();
    assert!(
        query_history(
            f,
            binding,
            source,
            json!({"task_id":task,"before_cursor":history_cursor})
        )
        .is_err()
    );
    let parent_history =
        query_history(f, binding, source, json!({"task_id":task,"limit":1})).unwrap();
    assert!(
        user_delegation_read(
            f,
            "delegations.history",
            json!({"task_id":task,"before_cursor":parent_history["next_cursor"]})
        )
        .is_err()
    );
    let mut read = user_delegation_read(
        f,
        "delegations.read",
        json!({"task_id":task,"limit_bytes":65536}),
    )
    .unwrap();
    let fixed = read["result_ref"].clone();
    let read_cursor = read["next_cursor"].as_str().unwrap();
    assert!(
        query_read(
            f,
            binding,
            source,
            json!({"task_id":task,"cursor":read_cursor})
        )
        .is_err()
    );
    assert!(
        user_delegation_read(
            f,
            "delegations.read",
            json!({"task_id":task,"cursor":history_cursor})
        )
        .is_err()
    );
    let mut combined = String::new();
    loop {
        assert_eq!(read["result_ref"], fixed);
        combined.push_str(read["text"].as_str().unwrap());
        let Some(cursor) = read.get("next_cursor").cloned() else {
            break;
        };
        read = user_delegation_read(
            f,
            "delegations.read",
            json!({"task_id":task,"cursor":cursor,"limit_bytes":65536}),
        )
        .unwrap();
    }
    assert_eq!(
        combined, full,
        "User result stays the original attempt final, not a later User Run"
    );
    let actor = ActorRef::System {
        component_id: "not-user".into(),
    };
    assert!(
        user::get(
            &f.database,
            &actor,
            &serde_json::from_value(json!({"delegationId":task})).unwrap()
        )
        .is_err()
    );
    for method in ["delegations.read", "delegations.history"] {
        assert_eq!(
            user_delegation_read(f, method, json!({"task_id":"missing-user-task"})).unwrap(),
            json!({"task_id":"missing-user-task","status":"unknown"})
        );
    }
    assert!(
        user_delegation_read(
            f,
            "delegations.get",
            json!({"delegationId":"missing-user-task"})
        )
        .is_err()
    );
    assert!(
        user_delegation_read(
            f,
            "delegations.get",
            json!({"delegationId":task,"actor":{"type":"user"}})
        )
        .is_err()
    );
    assert!(user_delegation_read(f, "delegations.settings.get", json!({"enabled":true})).is_err());
    assert_eq!(f.database.connection().total_changes(), before);
}
fn assert_user_queries_after_parent_stop(f: &Fixture, task: &str, parent_run: &str) {
    assert_eq!(lifecycle_status(f, parent_run), "cancelled");
    let before = f.database.connection().total_changes();
    let detail = user_delegation_read(f, "delegations.get", json!({"delegationId":task})).unwrap();
    assert!(
        user_delegation_read(
            f,
            "delegations.list",
            json!({"parentConversationId":detail["parentConversationId"]})
        )
        .unwrap()["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["delegationId"] == task)
    );
    assert!(!user_delegation_read(f,"delegations.history",json!({"task_id":task})).unwrap()["messages"].as_array().unwrap().is_empty());
    assert!(
        user_delegation_read(f, "delegations.read", json!({"task_id":task})).unwrap()["text"]
            .is_string()
    );
    assert_eq!(f.database.connection().total_changes(), before);
}
fn settings_request(version: i64, patch: Value) -> Value {
    json!({"commandId":Uuid::new_v4().to_string(),"command":{"expectedVersion":version,"patch":patch}})
}
fn assert_user_delegation_old_database(f: &mut Fixture) {
    let before = f.database.connection().total_changes();
    for (method, params) in [
        (
            "delegations.list",
            json!({"parentConversationId":"old-parent"}),
        ),
        ("delegations.get", json!({"delegationId":"old-task"})),
        ("delegations.history", json!({"task_id":"old-task"})),
        ("delegations.read", json!({"task_id":"old-task"})),
        ("delegations.settings.get", json!({})),
    ] {
        assert_eq!(
            user_delegation_read(f, method, params)
                .unwrap_err()
                .to_string(),
            "delegation.unavailable"
        );
    }
    assert_eq!(f.database.connection().total_changes(), before);
    let p = settings_request(1, json!({"enabled":true}));
    let e = crate::application::delegation_queries::update_settings(&mut f.database, p.clone())
        .unwrap();
    assert_eq!(e.result.code, "delegation.unavailable");
    let before = f.database.connection().total_changes();
    assert_eq!(
        crate::application::delegation_queries::update_settings(&mut f.database, p)
            .unwrap()
            .result,
        e.result
    );
    assert_eq!(f.database.connection().total_changes(), before);
    assert_eq!(f.database.connection().query_row("SELECT COUNT(*) FROM sqlite_schema WHERE name IN ('delegation','delegation_control','delegation_attempt','delegation_settings')",[],|r|r.get::<_,i64>(0)).unwrap(),0);
}
fn assert_user_delegation_settings(f: &mut Fixture) {
    use crate::{
        application::delegation_queries as api,
        command::{ActorRef, CommandEnvelope},
        delegation_settings as settings,
    };
    let original = user_delegation_read(f, "delegations.settings.get", json!({})).unwrap();
    let original_runs: Vec<(String, String)> = f
        .database
        .connection()
        .prepare("SELECT id,effective_config_json FROM agent_run ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let version = original["version"].as_i64().unwrap();
    let request = settings_request(
        version,
        json!({"perRuntimeDefaults":{"codex":{"mode_id":"fixture-mode","config_values":{"model":"fixture-model"}}},"depthLimit":8,"completedCacheMaxMb":0}),
    );
    let unknown = api::reconcile(&f.database, request.clone()).unwrap();
    assert_eq!(unknown["state"], "unknown");
    let result = api::update_settings(&mut f.database, request.clone()).unwrap();
    assert_eq!(result.result.code, "delegation.settings_updated");
    let changes = f.database.connection().total_changes();
    assert_eq!(
        api::update_settings(&mut f.database, request.clone())
            .unwrap()
            .result,
        result.result
    );
    assert_eq!(
        api::reconcile(&f.database, request.clone()).unwrap()["result"],
        serde_json::to_value(&result.result).unwrap()
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    let mut changed = request.clone();
    changed["command"]["patch"]["depthLimit"] = json!(7);
    assert!(api::reconcile(&f.database, changed.clone()).is_err());
    assert!(api::update_settings(&mut f.database, changed).is_err());
    let enabled = settings_request(version + 1, json!({"enabled":false}));
    let saved = api::update_settings(&mut f.database, enabled)
        .unwrap()
        .result
        .payload;
    assert_eq!(
        saved["perRuntimeDefaults"],
        result.result.payload["perRuntimeDefaults"]
    );
    assert_eq!(saved["completedCacheMaxMb"], 0);
    let stale = settings_request(version, json!({"enabled":true}));
    assert_eq!(
        api::update_settings(&mut f.database, stale)
            .unwrap()
            .result
            .code,
        "delegation.settings_changed"
    );
    for patch in [
        json!({"depthLimit":0}),
        json!({"completedCacheMaxMb":-1}),
        json!({"perRuntimeDefaults":{"codex":{"config_values":{"":"invalid"}}}}),
    ] {
        assert_eq!(
            api::update_settings(&mut f.database, settings_request(version + 2, patch))
                .unwrap()
                .result
                .code,
            "delegation.settings_invalid"
        );
    }
    for patch in [
        json!({"enabled":null}),
        json!({"version":999}),
        json!({"perRuntimeDefaults":{"not-a-runtime":{}}}),
    ] {
        let before = f.database.connection().total_changes();
        assert!(
            api::update_settings(&mut f.database, settings_request(version + 2, patch)).is_err()
        );
        assert_eq!(f.database.connection().total_changes(), before);
    }
    // A failed event or receipt rolls back the settings and its version together.
    for event in ["delegation.settings_changed", "command.result"] {
        let before = user_delegation_read(f, "delegations.settings.get", json!({})).unwrap();
        let event_count: i64 = f
            .database
            .connection()
            .query_row("SELECT COUNT(*) FROM event_log", [], |r| r.get(0))
            .unwrap();
        f.database.connection().execute_batch(&format!("CREATE TEMP TRIGGER fail_user_delegation_settings BEFORE INSERT ON event_log WHEN NEW.event_type='{event}' BEGIN SELECT RAISE(ABORT,'settings-storage-fixture'); END;")).unwrap();
        let fail = settings_request(before["version"].as_i64().unwrap(), json!({"enabled":true}));
        assert!(api::update_settings(&mut f.database, fail.clone()).is_err());
        assert_eq!(
            user_delegation_read(f, "delegations.settings.get", json!({})).unwrap(),
            before
        );
        assert_eq!(
            f.database
                .connection()
                .query_row("SELECT COUNT(*) FROM event_log", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            event_count
        );
        assert_eq!(
            api::reconcile(&f.database, fail.clone()).unwrap()["state"],
            "unknown"
        );
        f.database
            .connection()
            .execute_batch("DROP TRIGGER fail_user_delegation_settings")
            .unwrap();
        assert_eq!(
            api::update_settings(&mut f.database, fail)
                .unwrap()
                .result
                .code,
            "delegation.settings_updated"
        );
    }
    let e = CommandEnvelope {
        command_id: Uuid::new_v4().to_string(),
        actor: ActorRef::System {
            component_id: "not-user".into(),
        },
        camp_id: None,
        expected_versions: Vec::new(),
        execution_epoch: None,
        payload: serde_json::from_value::<settings::UpdateSettingsParams>(
            json!({"expectedVersion":1,"patch":{}}),
        )
        .unwrap(),
    };
    assert_eq!(
        settings::update_user(&mut f.database, &e)
            .unwrap()
            .result
            .code,
        "delegation.user_required"
    );
    assert!(settings::read_user(&f.database, &e.actor).is_err());
    let events: Vec<String> = f
        .database
        .connection()
        .prepare(
            "SELECT payload_json FROM event_log WHERE event_type='delegation.settings_changed'",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(!events.is_empty());
    for event in events {
        let event: Value = serde_json::from_str(&event).unwrap();
        assert_eq!(event.as_object().unwrap().len(), 1);
        assert!(event["version"].is_i64());
    }
    let current = user_delegation_read(f, "delegations.settings.get", json!({})).unwrap();
    let mut restore = original;
    restore.as_object_mut().unwrap().remove("version");
    api::update_settings(
        &mut f.database,
        settings_request(current["version"].as_i64().unwrap(), restore),
    )
    .unwrap();
    let after_runs: Vec<(String, String)> = f
        .database
        .connection()
        .prepare("SELECT id,effective_config_json FROM agent_run ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(
        original_runs, after_runs,
        "settings do not upgrade or rewrite existing frozen Runs"
    );
}
