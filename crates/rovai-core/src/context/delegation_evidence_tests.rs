// Same original Context owner/store. Provider observations are explicit fixtures,
// while the dispatcher, lifecycle, canonical read model and receipts are real.
fn delegation_activities(f: &Fixture, run: &str) -> Vec<(String, Value)> {
    let ids = f
        .database
        .connection()
        .prepare(
            "SELECT id FROM agent_run_execution_evidence WHERE agent_run_id=?1 ORDER BY sequence",
        )
        .unwrap()
        .query_map([run], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    ids.into_iter()
        .map(|id| {
            let payload = crate::execution_evidence::ExecutionEvidenceService
                .read_full_payload(
                    &f.database,
                    &ManagedBlobStore::new(&f.directory),
                    &f.camp_id,
                    &id,
                )
                .unwrap();
            (id, payload)
        })
        .collect()
}
fn delegation_activity(
    f: &Fixture,
    run: &str,
    op: &str,
    request: &crate::builtin_tool_transport::BuiltinToolIpcRequest,
) -> (String, Value) {
    let crate::builtin_tool_transport::BuiltinToolIpcRequestBody::Invoke { request_id, .. } =
        &request.body;
    delegation_activities(f, run)
        .into_iter()
        .find(|(_, p)| p["coreRequestId"] == *request_id && p["canonicalTool"] == op)
        .unwrap()
}
fn assert_delegation_evidence(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
) {
    use crate::{
        builtin_tool_runtime::{BuiltinToolLeaseRegistry, BuiltinToolProcessConfig},
        builtin_tool_transport::{BuiltinToolIpcRequestBody, LocalIpcEndpoint},
        execution_evidence::ExecutionEvidenceService,
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let leases = BuiltinToolLeaseRegistry::default();
    let config = BuiltinToolProcessConfig::create(
        &std::env::current_exe().unwrap(),
        &LocalIpcEndpoint::UnixSocket {
            path: f
                .directory
                .join("activity-fixture.sock")
                .to_string_lossy()
                .into_owned(),
        },
        &f.directory,
    )
    .unwrap();
    let auth = rt
        .block_on(leases.bind(
            &config,
            &source.agent_run_id,
            source.execution_epoch,
            binding,
            &f.directory,
        ))
        .unwrap();
    let start = delegation_cli_request(
        &auth,
        "delegation.start",
        json!({"agent_type":"codex","task":"private-evidence-task-do-not-copy"}),
    );
    let initial = delegation_dispatch_call(&rt, f, &leases, &start);
    let task = initial.payload["task_id"].as_str().unwrap().to_owned();
    let (activity_id, activity) =
        delegation_activity(f, &source.agent_run_id, "delegation.start", &start);
    assert_eq!(activity["status"], "completed");
    assert!(activity["coreEnvelope"].get("result").is_none());
    assert_eq!(
        activity["agentOutputDigest"],
        crate::command::canonical_json_digest(&initial.payload).unwrap()
    );
    assert!(
        !activity
            .to_string()
            .contains("private-evidence-task-do-not-copy")
    );
    let receipt = activity["receiptId"].as_str().unwrap();
    assert!(f.database.connection().query_row("SELECT EXISTS(SELECT 1 FROM event_log WHERE command_id=?1 AND event_type='command.result')",[receipt],|r|r.get::<_,bool>(0)).unwrap());
    let stored:(i64,i64)=f.database.connection().query_row("SELECT revision,(SELECT COUNT(*) FROM agent_run_execution_evidence WHERE agent_run_id=?2) FROM agent_run_execution_evidence WHERE id=?1",params![activity_id,source.agent_run_id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(stored.0, 2);
    let changes = f.database.connection().total_changes();
    assert_eq!(
        delegation_dispatch_call(&rt, f, &leases, &start).payload,
        initial.payload
    );
    assert_eq!(f.database.connection().total_changes(), changes);

    let shell = format!("fixture-shell-{}", Uuid::new_v4());
    let command = format!("rovai delegation status --task-id {task}");
    ExecutionEvidenceService
        .record_runtime_event(
            &mut f.database,
            &ManagedBlobStore::new(&f.directory),
            &source.agent_run_id,
            source.execution_epoch,
            "runtime.action",
            &json!({"toolCallId":shell,"kind":"execute","input":command,"status":"started"}),
        )
        .unwrap()
        .unwrap();
    let query = delegation_cli_request(&auth, "delegation.status", json!({"task_ids":[&task]}));
    let status = delegation_dispatch_call(&rt, f, &leases, &query);
    let native=ExecutionEvidenceService.record_runtime_event(&mut f.database,&ManagedBlobStore::new(&f.directory),&source.agent_run_id,source.execution_epoch,"runtime.action",&json!({"toolCallId":shell,"kind":"execute","input":command,"status":"completed","exitCode":0,"output":serde_json::to_string(&status.payload).unwrap()})).unwrap().unwrap();
    let native_id = native.id.clone();
    let page = crate::execution_window::read_page(
        &mut f.database,
        &f.camp_id,
        &source.agent_run_id,
        None,
        96,
    )
    .unwrap();
    let carrier = page.evidence.iter().find(|e| e.id == native_id).unwrap();
    assert_eq!(
        carrier.payload["executionWindowBuiltinOperation"],
        "delegation.status"
    );
    let cancel = delegation_cli_request(&auth, "delegation.cancel", json!({"task_id":&task}));
    delegation_dispatch_call(&rt, f, &leases, &cancel);
    let fresh = delegation_dispatch_call(&rt, f, &leases, &query);
    assert_eq!(fresh.payload["tasks"][0]["status"], "canceled");
    let BuiltinToolIpcRequestBody::Invoke { request_id, .. } = &query.body;
    let rows = delegation_activities(f, &source.agent_run_id)
        .into_iter()
        .filter(|(_, p)| p["coreEnvelope"]["requestId"] == *request_id)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].0, rows[1].0);
    fn saved_output(p: &Value) -> Value {
        p["output"]
            .as_str()
            .map(|text| serde_json::from_str(text).unwrap())
            .unwrap_or_else(|| p["output"].clone())
    }
    assert_eq!(saved_output(&rows[0].1)["tasks"][0]["status"], "running");
    assert_eq!(saved_output(&rows[1].1)["tasks"][0]["status"], "canceled");
    assert_eq!(
        rows[0].1["agentOutputDigest"],
        crate::command::canonical_json_digest(&status.payload).unwrap()
    );

    // Starting evidence failure is before task dispatch. Finishing evidence
    // failure is after commit and must not replace the known task result.
    f.database.connection().execute_batch("CREATE TEMP TRIGGER fail_delegation_activity_start BEFORE INSERT ON agent_run_execution_evidence BEGIN SELECT RAISE(ABORT,'private-start-storage-fixture'); END;").unwrap();
    let counts = factory_counts(f);
    let failed = delegation_cli_request(
        &auth,
        "delegation.start",
        json!({"agent_type":"codex","task":"must-not-dispatch"}),
    );
    let failed_result = rt.block_on(crate::delegation_dispatch::invoke_ipc(
        &tokio::sync::Mutex::new(&mut f.database),
        &leases,
        &f.directory,
        &failed,
        std::future::pending(),
    ));
    let crate::builtin_tool_transport::BuiltinToolIpcResponse::Envelope { envelope } =
        failed_result
    else {
        panic!("expected safe storage error")
    };
    assert_eq!(
        envelope.error.as_ref().unwrap().code,
        "builtin_tool.internal_error"
    );
    assert!(
        !serde_json::to_string(&envelope)
            .unwrap()
            .contains("private-start-storage-fixture")
    );
    assert_eq!(factory_counts(f), counts);
    f.database.connection().execute_batch("DROP TRIGGER fail_delegation_activity_start; CREATE TEMP TRIGGER fail_delegation_activity_receipt BEFORE INSERT ON event_log WHEN NEW.event_type='command.result' AND NEW.command_type='delegation.start' BEGIN SELECT RAISE(ABORT,'private-receipt-storage-fixture'); END;").unwrap();
    let retry = delegation_cli_request(
        &auth,
        "delegation.start",
        json!({"agent_type":"codex","task":"same-input-after-precommit-fault"}),
    );
    let before = factory_counts(f);
    assert!(
        rt.block_on(crate::delegation_dispatch::invoke(
            &tokio::sync::Mutex::new(&mut f.database),
            &leases,
            &f.directory,
            &retry,
            std::future::pending()
        ))
        .is_err()
    );
    assert_eq!(factory_counts(f), before);
    f.database
        .connection()
        .execute_batch("DROP TRIGGER fail_delegation_activity_receipt;")
        .unwrap();
    let retried = delegation_dispatch_call(&rt, f, &leases, &retry);
    assert_eq!(retried.payload["status"], "running");
    let BuiltinToolIpcRequestBody::Invoke { request_id, .. } = &retry.body;
    let attempts = delegation_activities(f, &source.agent_run_id)
        .into_iter()
        .filter(|(_, p)| p["coreRequestId"] == *request_id)
        .collect::<Vec<_>>();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].1["status"], "failed");
    assert_eq!(attempts[1].1["status"], "completed");
    assert!(
        attempts
            .iter()
            .all(|(_, p)| p.pointer("/_rovaiLifecycle/terminalConflict") != Some(&json!(true)))
    );
    delegation_dispatch_call(
        &rt,
        f,
        &leases,
        &delegation_cli_request(
            &auth,
            "delegation.cancel",
            json!({"task_id":retried.payload["task_id"]}),
        ),
    );
    f.database.connection().execute_batch("CREATE TEMP TRIGGER fail_delegation_activity_end BEFORE UPDATE ON agent_run_execution_evidence BEGIN SELECT RAISE(ABORT,'private-end-storage-fixture'); END;").unwrap();
    let uncertain = delegation_cli_request(
        &auth,
        "delegation.start",
        json!({"agent_type":"codex","task":"committed-receipt-must-win"}),
    );
    let committed = delegation_dispatch_call(&rt, f, &leases, &uncertain);
    assert_eq!(committed.payload["status"], "running");
    let unfinished = delegation_activity(f, &source.agent_run_id, "delegation.start", &uncertain);
    assert_eq!(unfinished.1["status"], "started");
    f.database
        .connection()
        .execute_batch("DROP TRIGGER fail_delegation_activity_end;")
        .unwrap();
    let changes = f.database.connection().total_changes();
    assert_eq!(
        delegation_dispatch_call(&rt, f, &leases, &uncertain).payload,
        committed.payload
    );
    assert_eq!(f.database.connection().total_changes(), changes);
    assert_eq!(
        delegation_activity(f, &source.agent_run_id, "delegation.start", &uncertain),
        unfinished
    );
    assert_delegation_observation_recovery(
        f,
        &rt,
        &leases,
        &auth,
        source,
        &uncertain,
        &committed,
        &unfinished,
    );
    delegation_dispatch_call(
        &rt,
        f,
        &leases,
        &delegation_cli_request(
            &auth,
            "delegation.cancel",
            json!({"task_id":committed.payload["task_id"]}),
        ),
    );

    // Finish an already started status observation after a real parent-command
    // cancel fences its independent source Run; it cannot authorize new work.
    let workspace: Value = serde_json::from_str(
        &f.database
            .connection()
            .query_row(
                "SELECT workspace_json FROM agent_run WHERE id=?1",
                [&source.agent_run_id],
                |r| r.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    let (child, run, epoch) = context_child_in(
        f,
        &source.agent_run_id,
        source.execution_epoch,
        "fenced activity source",
        "evidence-fenced-source",
        &workspace,
    );
    let child_binding = TeamToolService::default()
        .prepare_binding_credential(&mut f.database, &run, epoch, false)
        .unwrap();
    let fenced_auth = rt
        .block_on(
            leases.bind(
                &BuiltinToolProcessConfig::create(
                    &std::env::current_exe().unwrap(),
                    &LocalIpcEndpoint::UnixSocket {
                        path: f
                            .directory
                            .join("fenced-activity-fixture.sock")
                            .to_string_lossy()
                            .into_owned(),
                    },
                    &f.directory,
                )
                .unwrap(),
                &run,
                epoch,
                &child_binding,
                &f.directory,
            ),
        )
        .unwrap();
    let snap = delegation_cli_request(
        &fenced_auth,
        "delegation.status",
        json!({"task_ids":["unknown"]}),
    );
    let snap = delegation_dispatch_call(&rt, f, &leases, &snap);
    let wait = delegation_cli_request(
        &fenced_auth,
        "delegation.status",
        json!({"task_ids":["unknown"],"wait_ms":0,"after_cursor":snap.payload["cursor"]}),
    );
    f.database
        .connection()
        .execute_batch(&format!(
        "CREATE TEMP TRIGGER lose_fenced_activity_end BEFORE UPDATE ON agent_run_execution_evidence
         WHEN OLD.agent_run_id='{run}' BEGIN SELECT RAISE(ABORT,'lost fenced result'); END;"
    ))
        .unwrap();
    rt.block_on(async {
        let db = tokio::sync::Mutex::new(&mut f.database);
        let future = crate::delegation_dispatch::invoke(
            &db,
            &leases,
            &f.directory,
            &wait,
            std::future::pending(),
        );
        tokio::pin!(future);
        assert_delegation_wait_is_pending(future.as_mut()).await;
        {
            let mut db = db.lock().await;
            crate::delegation_model_command::cancel(
                &mut db,
                &crate::delegation_model_command::Invocation {
                    binding,
                    source,
                    tool_call: "cancel-evidence-source",
                    input: &crate::delegation_contract::CancelParams {
                        task_id: child.clone().try_into().unwrap(),
                    },
                },
            )
            .unwrap();
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), future)
                .await
                .unwrap()
                .is_err()
        );
    });
    let BuiltinToolIpcRequestBody::Invoke { request_id, .. } = &wait.body;
    f.database
        .connection()
        .execute_batch("DROP TRIGGER lose_fenced_activity_end;")
        .unwrap();
    let closed = delegation_activities(f, &run)
        .into_iter()
        .find(|(_, p)| p["coreRequestId"] == *request_id)
        .unwrap();
    assert_eq!(closed.1["status"], "started");
    f.database
        .connection()
        .execute(
            "UPDATE agent_run SET execution_epoch=execution_epoch+1 WHERE id=?1",
            [&run],
        )
        .unwrap();
    let before_late = f.database.connection().total_changes();
    let mut late = closed.1.clone();
    late["status"] = json!("failed");
    assert!(
        ExecutionEvidenceService
            .record_builtin_tool_result(
                &mut f.database,
                &ManagedBlobStore::new(&f.directory),
                &run,
                epoch,
                &late
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(f.database.connection().total_changes(), before_late);
    let mut cursor = crate::delegation_dispatch::recovery::Cursor::capture(&f.database).unwrap();
    let count = factory_counts(f);
    while !cursor.done() {
        assert_eq!(
            cursor.next(&mut f.database, &f.directory).unwrap().failures,
            0
        );
    }
    let recovered = delegation_activities(f, &run)
        .into_iter()
        .find(|(id, _)| id == &closed.0)
        .unwrap()
        .1;
    assert_eq!(recovered["status"], "failed");
    assert_eq!(recovered["errorCode"], "builtin_tool.result_unconfirmed");
    assert_eq!(factory_counts(f), count);
    let count = factory_counts(f);
    let denied = delegation_cli_request(
        &fenced_auth,
        "delegation.start",
        json!({"agent_type":"codex","task":"fenced-cannot-dispatch"}),
    );
    let result = rt.block_on(crate::delegation_dispatch::invoke(
        &tokio::sync::Mutex::new(&mut f.database),
        &leases,
        &f.directory,
        &denied,
        std::future::pending(),
    ));
    assert!(result.is_err());
    assert_eq!(factory_counts(f), count);
    rt.block_on(leases.unregister(&fenced_auth.process_id));
    rt.block_on(leases.unregister(config.process_id()));
}

fn assert_delegation_observation_recovery(
    f: &mut Fixture,
    rt: &tokio::runtime::Runtime,
    leases: &crate::builtin_tool_runtime::BuiltinToolLeaseRegistry,
    auth: &crate::builtin_tool_transport::BuiltinToolAuth,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    request: &crate::builtin_tool_transport::BuiltinToolIpcRequest,
    result: &crate::delegation_dispatch::Outcome,
    unfinished: &(String, Value),
) {
    use crate::{
        delegation_dispatch::recovery::Cursor, execution_evidence::ExecutionEvidenceService,
    };
    let store = ManagedBlobStore::new(&f.directory);
    // Same request proof, different actual attempt: the committed receipt must
    // never be borrowed without its atomic attempt association.
    let mut other = unfinished.1.clone();
    other["toolCallId"] = json!(Uuid::new_v4().to_string());
    let orphan = ExecutionEvidenceService
        .record_builtin_tool_started(
            &mut f.database,
            &store,
            &source.agent_run_id,
            source.execution_epoch,
            &other,
        )
        .unwrap()
        .unwrap()
        .id
        .clone();
    other["toolCallId"] = json!(Uuid::new_v4().to_string());
    let gone = ExecutionEvidenceService
        .record_builtin_tool_started(
            &mut f.database,
            &store,
            &source.agent_run_id,
            source.execution_epoch,
            &other,
        )
        .unwrap()
        .unwrap()
        .id
        .clone();
    let gone_rowid: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT rowid FROM agent_run_execution_evidence WHERE id=?1",
            [&gone],
            |r| r.get(0),
        )
        .unwrap();
    let mut cursor = Cursor::capture(&f.database).unwrap();
    f.database
        .connection()
        .execute(
            "DELETE FROM canonical_runtime_activity WHERE EXISTS
        (SELECT 1 FROM json_each(source_evidence_ids_json) WHERE value=?1)",
            [&gone],
        )
        .unwrap();
    f.database
        .connection()
        .execute(
            "DELETE FROM agent_run_execution_evidence WHERE id=?1",
            [&gone],
        )
        .unwrap();
    f.database.connection().execute_batch("CREATE TEMP TRIGGER lose_later_activity_end BEFORE UPDATE ON agent_run_execution_evidence BEGIN SELECT RAISE(ABORT,'lost later result'); END;").unwrap();
    let later = delegation_cli_request(
        auth,
        "delegation.status",
        json!({"task_ids":[result.payload["task_id"]]}),
    );
    delegation_dispatch_call(rt, f, leases, &later);
    let later_id = delegation_activity(f, &source.agent_run_id, "delegation.status", &later).0;
    let later_rowid: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT rowid FROM agent_run_execution_evidence WHERE id=?1",
            [&later_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        later_rowid, gone_rowid,
        "SQLite reused the deleted rowid; recovery must pin UUIDs"
    );
    f.database
        .connection()
        .execute_batch("DROP TRIGGER lose_later_activity_end;")
        .unwrap();
    f.database
        .connection()
        .execute_batch(&format!(
            "CREATE TEMP TRIGGER lose_reconciliation BEFORE UPDATE ON agent_run_execution_evidence
         WHEN OLD.id='{}' BEGIN SELECT RAISE(ABORT,'reconciliation write fault'); END;",
            unfinished.0
        ))
        .unwrap();
    let counts = factory_counts(f);
    let batch = cursor.next(&mut f.database, &f.directory).unwrap();
    assert_eq!(batch.failures, 1);
    assert!(batch.updated.iter().any(|e| e.id == orphan));
    assert_eq!(
        delegation_activity(f, &source.agent_run_id, "delegation.status", &later).1["status"],
        "started"
    );
    assert_eq!(factory_counts(f), counts);
    f.database
        .connection()
        .execute_batch("DROP TRIGGER lose_reconciliation;")
        .unwrap();
    // A new real SQLite connection consumes the durable proof. Native identity
    // remains the owner's fixture; this is not an App/provider restart.
    f.database = Database::open(&f.directory).unwrap();
    let mut retry = Cursor::capture(&f.database).unwrap();
    while !retry.done() {
        assert_eq!(
            retry.next(&mut f.database, &f.directory).unwrap().failures,
            0
        );
    }
    // request ID alone now has two observations; choose the original exact row.
    let saved = delegation_activities(f, &source.agent_run_id)
        .into_iter()
        .find(|(id, _)| id == &unfinished.0)
        .unwrap()
        .1;
    assert_eq!(saved["status"], "completed");
    let crate::builtin_tool_transport::BuiltinToolIpcRequestBody::Invoke { request_id, .. } =
        &request.body;
    assert_eq!(saved["coreEnvelope"]["requestId"], *request_id);
    assert_eq!(saved["coreReconciliation"], "receipt_confirmed");
    assert_eq!(
        saved["agentOutputDigest"],
        crate::command::canonical_json_digest(&result.payload).unwrap()
    );
    let other = delegation_activities(f, &source.agent_run_id)
        .into_iter()
        .find(|(id, _)| id == &orphan)
        .unwrap()
        .1;
    assert_eq!(other["errorCode"], "builtin_tool.result_unconfirmed");
    assert_eq!(
        delegation_activity(f, &source.agent_run_id, "delegation.status", &later).1["errorCode"],
        "builtin_tool.result_unconfirmed"
    );
    assert!(!saved.to_string().contains("committed-receipt-must-win"));
    assert_eq!(factory_counts(f), counts);
    let changes = f.database.connection().total_changes();
    let mut again = Cursor::capture(&f.database).unwrap();
    while !again.done() {
        assert!(
            again
                .next(&mut f.database, &f.directory)
                .unwrap()
                .updated
                .is_empty()
        );
    }
    assert_eq!(f.database.connection().total_changes(), changes);
}
