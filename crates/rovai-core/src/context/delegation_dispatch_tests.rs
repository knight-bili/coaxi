// All six calls use the shared dispatcher, original Context owner's Database,
// real files/Registry/Gateway and original native Session/input fixtures.
fn delegation_dispatch_business(f: &Fixture) -> Value {
    let runs=f.database.connection().prepare("SELECT id,conversation_id,status,execution_epoch,cancel_requested_at FROM agent_run ORDER BY id").unwrap().query_map([],|r|Ok(json!([r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,Option<String>>(4)?]))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
    let messages: i64 = f
        .database
        .connection()
        .query_row("SELECT COUNT(*) FROM conversation_message", [], |r| {
            r.get(0)
        })
        .unwrap();
    json!({"graph":factory_counts(f),"runs":runs,"messages":messages})
}

fn delegation_cli_request(
    auth: &crate::builtin_tool_transport::BuiltinToolAuth,
    operation: &str,
    input: Value,
) -> crate::builtin_tool_transport::BuiltinToolIpcRequest {
    crate::builtin_tool_transport::BuiltinToolIpcRequest {
        ipc_protocol_version: crate::builtin_tool_transport::BUILTIN_TOOL_IPC_PROTOCOL_VERSION,
        auth: auth.clone(),
        body: crate::builtin_tool_transport::BuiltinToolIpcRequestBody::Invoke {
            request_id: Uuid::new_v4().to_string(),
            operation: operation.into(),
            input,
        },
    }
}
fn delegation_dispatch_call(
    rt: &tokio::runtime::Runtime,
    f: &mut Fixture,
    leases: &crate::builtin_tool_runtime::BuiltinToolLeaseRegistry,
    request: &crate::builtin_tool_transport::BuiltinToolIpcRequest,
) -> crate::delegation_dispatch::Outcome {
    rt.block_on(crate::delegation_dispatch::invoke(
        &tokio::sync::Mutex::new(&mut f.database),
        leases,
        &f.directory,
        request,
        std::future::pending(),
    ))
    .unwrap()
}
async fn assert_delegation_wait_is_pending<T>(
    future: std::pin::Pin<&mut impl std::future::Future<Output = T>>,
) {
    let mut future = future;
    let polled = std::future::poll_fn(|cx| std::task::Poll::Ready(future.as_mut().poll(cx))).await;
    assert!(
        polled.is_pending(),
        "status waiter should release all locks and wait"
    );
}
fn assert_delegation_dispatch(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    completed: &str,
) {
    use crate::{
        builtin_tool_runtime::{BuiltinToolLeaseRegistry, BuiltinToolProcessConfig},
        builtin_tool_transport::BuiltinToolIpcRequestBody,
        delegation_dispatch::invoke,
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let leases = BuiltinToolLeaseRegistry::default();
    let config = BuiltinToolProcessConfig::create(
        &std::env::current_exe().unwrap(),
        &crate::builtin_tool_transport::LocalIpcEndpoint::UnixSocket {
            path: f
                .directory
                .join("dispatch-fixture.sock")
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
        "delegate_to_agent",
        json!({"agent_type":"codex","task":format!("统一入口完整任务 {}",Uuid::new_v4())}),
    );
    let initial = delegation_dispatch_call(&rt, f, &leases, &start);
    assert_eq!(initial.operation, "delegation.start");
    assert!(!initial.replayed);
    let task = initial.payload["task_id"].as_str().unwrap().to_owned();
    let changes = f.database.connection().total_changes();
    let mut canonical = start.clone();
    let BuiltinToolIpcRequestBody::Invoke { operation, .. } = &mut canonical.body;
    *operation = "delegation.start".into();
    let replay = delegation_dispatch_call(&rt, f, &leases, &canonical);
    assert!(replay.replayed);
    assert_eq!(replay.payload, initial.payload);
    assert_eq!(f.database.connection().total_changes(), changes);
    let business = delegation_dispatch_business(f);
    let read = delegation_cli_request(
        &auth,
        "delegation.read",
        json!({"task_id":completed,"limit_bytes":32}),
    );
    assert!(
        !delegation_dispatch_call(&rt, f, &leases, &read).payload["text"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    let history = delegation_cli_request(
        &auth,
        "delegation.history",
        json!({"task_id":completed,"limit":1}),
    );
    assert_eq!(
        delegation_dispatch_call(&rt, f, &leases, &history).payload["messages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let snapshot = delegation_cli_request(
        &auth,
        "get_delegation_status",
        json!({"task_ids":[&task, "not-owned", &task]}),
    );
    let status = delegation_dispatch_call(&rt, f, &leases, &snapshot);
    assert_eq!(status.payload["tasks"][0], status.payload["tasks"][2]);
    assert_eq!(
        status.payload["tasks"][1],
        json!({"task_id":"not-owned","status":"unknown"})
    );
    assert_eq!(delegation_dispatch_business(f), business);

    // Observation rows record start/end, but no business state changes and no
    // locks survive the wait. Disconnect never cancels the child.
    let wait = delegation_cli_request(
        &auth,
        "delegation.status",
        json!({"task_ids":[&task,"not-owned",&task],"wait_ms":0,"after_cursor":status.payload["cursor"]}),
    );
    rt.block_on(async {
        let db = tokio::sync::Mutex::new(&mut f.database);
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let future = invoke(&db, &leases, &f.directory, &wait, async {
            let _ = rx.await;
        });
        tokio::pin!(future);
        assert_delegation_wait_is_pending(future.as_mut()).await;
        assert!(db.try_lock().is_ok());
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), leases.invocation_guard())
                .await
                .is_ok()
        );
        tx.send(()).unwrap();
        assert_eq!(
            future.await.err().unwrap().to_string(),
            "delegation.status_disconnected"
        );
    });
    assert_eq!(delegation_dispatch_business(f), business);

    // Two waiters see one same-Run lease rotation without a business notification.
    let auth2 = rt.block_on(async {
        let db = tokio::sync::Mutex::new(&mut f.database);
        let a = invoke(&db, &leases, &f.directory, &wait, std::future::pending());
        let b = invoke(&db, &leases, &f.directory, &wait, std::future::pending());
        tokio::pin!(a, b);
        assert_delegation_wait_is_pending(a.as_mut()).await;
        assert_delegation_wait_is_pending(b.as_mut()).await;
        let auth2 = leases
            .bind(
                &config,
                &source.agent_run_id,
                source.execution_epoch,
                binding,
                &f.directory,
            )
            .await
            .unwrap();
        for result in [
            tokio::time::timeout(std::time::Duration::from_secs(1), a)
                .await
                .unwrap(),
            tokio::time::timeout(std::time::Duration::from_secs(1), b)
                .await
                .unwrap(),
        ] {
            assert_eq!(
                result.err().unwrap().to_string(),
                "builtin_tool.run_not_bound"
            );
        }
        auth2
    });
    assert_eq!(delegation_dispatch_business(f), business);
    assert_eq!(
        delegation_dispatch_call(&rt, f, &leases, &start).payload,
        initial.payload
    );
    let changes = f.database.connection().total_changes();
    rt.block_on(async {
        let db = tokio::sync::Mutex::new(&mut f.database);
        for request in [&read, &history, &snapshot] {
            assert_eq!(
                invoke(&db, &leases, &f.directory, request, std::future::pending())
                    .await
                    .err()
                    .unwrap()
                    .to_string(),
                "builtin_tool.run_not_bound"
            );
        }
        let mut unknown = start.clone();
        let BuiltinToolIpcRequestBody::Invoke { request_id, .. } = &mut unknown.body;
        *request_id = Uuid::new_v4().to_string();
        assert_eq!(
            invoke(&db, &leases, &f.directory, &unknown, std::future::pending())
                .await
                .err()
                .unwrap()
                .to_string(),
            "builtin_tool.run_not_bound"
        );
        let mut changed = start.clone();
        let BuiltinToolIpcRequestBody::Invoke { input, .. } = &mut changed.body;
        input["task"] = json!("不能拿旧回执发送新任务");
        assert!(matches!(
            invoke(&db, &leases, &f.directory, &changed, std::future::pending())
                .await
                .err()
                .unwrap()
                .downcast_ref::<crate::command::CommandGatewayError>(),
            Some(crate::command::CommandGatewayError::IdempotencyConflict { .. })
        ));
        for (op, input) in [
            (
                "delegation.start",
                json!({"agent_type":"codex","task":"bad","working_dir":null}),
            ),
            ("delegation.status", json!({"task_ids":[],"wait_ms":0})),
            (
                "delegation.cancel",
                json!({"task_id":&task,"parent":"forged"}),
            ),
            ("delegation.resume", json!({"task_id":&task,"reason":null})),
            (
                "delegation.read",
                json!({"task_id":completed,"limit_bytes":0}),
            ),
            (
                "delegation.history",
                json!({"task_id":completed,"limit":101}),
            ),
        ] {
            let request = delegation_cli_request(&auth2, op, input);
            assert_eq!(
                invoke(&db, &leases, &f.directory, &request, std::future::pending())
                    .await
                    .err()
                    .unwrap()
                    .to_string(),
                "builtin_tool.invalid_input"
            );
        }
    });
    assert_eq!(f.database.connection().total_changes(), changes);

    // Original native Session, Context and input acceptance are real Core
    // operations; Session/ACK/cleanup observations remain explicit fixtures.
    let run: String = f
        .database
        .connection()
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [&task],
            |r| r.get(0),
        )
        .unwrap();
    let epoch = claim_context_child(f, &run);
    assert_eq!(
        resume_bind(f, &run, epoch, &format!("dispatcher-session-{task}"))
            .result
            .status,
        crate::command::CommandResultStatus::Applied
    );
    let context = resume_context(f, &run, epoch);
    resume_accept_input(f, &run, epoch, &context, "dispatcher-input-ack-fixture");
    let cancel = delegation_cli_request(&auth2, "cancel_delegation", json!({"task_id":&task}));
    let canceled = delegation_dispatch_call(&rt, f, &leases, &cancel);
    assert_eq!(canceled.payload["status"], "canceled");
    let mut fresh_status = snapshot.clone();
    fresh_status.auth = auth2.clone();
    assert_eq!(
        delegation_dispatch_call(&rt, f, &leases, &fresh_status).payload["tasks"][0]["status"],
        "canceled"
    );
    let pending = delegation_cli_request(
        &auth2,
        "resume_delegation",
        json!({"task_id":&task,"reason":"保留原会话继续"}),
    );
    let denied = delegation_dispatch_call(&rt, f, &leases, &pending);
    assert_eq!(denied.payload["error_code"], "delegation.cleanup_pending");
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, &run, epoch)
        .unwrap();
    let race = delegation_cli_request(
        &auth2,
        "delegation.resume",
        json!({"task_id":&task,"reason":"目录检查后连接换代"}),
    );
    let counts = factory_counts(f);
    let auth3 = rt.block_on(
        crate::delegation_dispatch::tests::assert_observed_resume_fenced(
            &tokio::sync::Mutex::new(&mut f.database),
            &leases,
            &config,
            &f.directory,
            &race,
        ),
    );
    assert_eq!(factory_counts(f), counts);
    let accepted = delegation_cli_request(
        &auth3,
        "delegation.resume",
        json!({"task_id":&task,"reason":"保留原会话继续"}),
    );
    let resumed = delegation_dispatch_call(&rt, f, &leases, &accepted);
    assert_eq!(resumed.payload["task_id"], task);
    assert_eq!(resumed.payload["status"], "running");
    let attempts: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM delegation_attempt WHERE delegation_id=?1",
            [&task],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(attempts, 2);
    let changes = f.database.connection().total_changes();
    assert_eq!(
        delegation_dispatch_call(&rt, f, &leases, &pending).payload,
        denied.payload
    );
    assert_eq!(f.database.connection().total_changes(), changes);

    let cleanup = delegation_cli_request(&auth3, "delegation.cancel", json!({"task_id":&task}));
    delegation_dispatch_call(&rt, f, &leases, &cleanup);
    // An actual observed file preparation is fenced before original commit.
    let late = delegation_cli_request(
        &auth3,
        "delegation.start",
        json!({"agent_type":"codex","task":format!("写文件后换租约 {}",Uuid::new_v4())}),
    );
    let counts = factory_counts(f);
    rt.block_on(
        crate::delegation_dispatch::tests::assert_observed_start_fenced(
            &tokio::sync::Mutex::new(&mut f.database),
            &leases,
            &config,
            &f.directory,
            &late,
        ),
    );
    assert_eq!(factory_counts(f), counts);
    rt.block_on(leases.unregister(config.process_id()));
    let changes = f.database.connection().total_changes();
    for (request, payload) in [
        (&start, &initial.payload),
        (&cancel, &canceled.payload),
        (&accepted, &resumed.payload),
    ] {
        let recovered = delegation_dispatch_call(&rt, f, &leases, request);
        assert!(recovered.replayed);
        assert_eq!(&recovered.payload, payload);
    }
    assert_eq!(f.database.connection().total_changes(), changes);
    ManagedBlobStore::new(&f.directory)
        .recover_gc_state(&mut f.database)
        .unwrap();
}
