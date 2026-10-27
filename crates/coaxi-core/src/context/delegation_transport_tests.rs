fn assert_legacy_delegation_transport_denied(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    run: &str,
) {
    assert!(!f.database.connection().query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='delegation')",[],|r|r.get::<_,bool>(0)).unwrap());
    let epoch: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT execution_epoch FROM agent_run WHERE id=?1",
            [run],
            |r| r.get(0),
        )
        .unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let leases = crate::builtin_tool_runtime::BuiltinToolLeaseRegistry::default();
    let config = crate::builtin_tool_runtime::BuiltinToolProcessConfig::create(
        &std::env::current_exe().unwrap(),
        &crate::builtin_tool_transport::LocalIpcEndpoint::UnixSocket {
            path: f
                .directory
                .join("legacy-denial-fixture.sock")
                .to_string_lossy()
                .into_owned(),
        },
        &f.directory,
    )
    .unwrap();
    let auth = rt
        .block_on(leases.bind(&config, run, epoch, binding, &f.directory))
        .unwrap();
    let before = f.database.connection().total_changes();
    rt.block_on(async {
        let db = tokio::sync::Mutex::new(&mut f.database);
        for (op, input) in [
            (
                "delegation.start",
                json!({"agent_type":"codex","task":"旧会话不能自动补权"}),
            ),
            ("delegation.status", json!({"task_ids":["unknown"]})),
            ("delegation.cancel", json!({"task_id":"unknown"})),
            ("delegation.resume", json!({"task_id":"unknown"})),
            ("delegation.read", json!({"task_id":"unknown"})),
            ("delegation.history", json!({"task_id":"unknown"})),
        ] {
            let request = delegation_cli_request(&auth, op, input);
            let crate::builtin_tool_transport::BuiltinToolIpcResponse::Envelope { envelope } =
                crate::delegation_dispatch::invoke_ipc(
                    &db,
                    &leases,
                    &f.directory,
                    &request,
                    std::future::pending(),
                )
                .await
            else {
                panic!("expected explicit legacy denial")
            };
            assert_eq!(
                envelope.error.as_ref().unwrap().code,
                "delegation.access_denied"
            );
            crate::builtin_tool_cli_output::project_envelope(envelope).unwrap();
        }
    });
    assert_eq!(f.database.connection().total_changes(), before);
    rt.block_on(leases.unregister(config.process_id()));
}

fn assert_delegation_transport(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    completed: &str,
) {
    use crate::{
        builtin_tool_runtime::{BuiltinToolLeaseRegistry, BuiltinToolProcessConfig},
        builtin_tool_transport::{BuiltinToolIpcRequest, BuiltinToolIpcResponse, LocalIpcEndpoint},
        delegation_dispatch::invoke_ipc,
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let leases = BuiltinToolLeaseRegistry::default();
    #[cfg(windows)]
    let endpoint = LocalIpcEndpoint::WindowsNamedPipe {
        name: format!(
            r"\\.\pipe\coaxi-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ),
    };
    #[cfg(unix)]
    let endpoint = LocalIpcEndpoint::UnixSocket {
        path: f
            .directory
            .join("delegation-transport.sock")
            .to_string_lossy()
            .into_owned(),
    };
    let config = BuiltinToolProcessConfig::create(
        &std::env::current_exe().unwrap(),
        &endpoint,
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
        json!({"agent_type":"codex","task":"本地管道完整委派任务"}),
    );
    // A real OS IPC frame and the exact adapter used by Core. This is not a
    // running Host or an agent process; the existing Database is borrowed.
    let response = rt.block_on(async {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let mut listener = crate::platform::local_ipc::LocalIpcListener::bind(&endpoint).unwrap();
        let db = tokio::sync::Mutex::new(&mut f.database);
        let server = async {
            let stream = listener.accept().await.unwrap();
            let (reader, mut writer) = tokio::io::split(stream);
            let mut reader = BufReader::new(reader);
            let mut frame = String::new();
            reader.read_line(&mut frame).await.unwrap();
            let request: BuiltinToolIpcRequest = serde_json::from_str(&frame).unwrap();
            let response = invoke_ipc(
                &db,
                &leases,
                &f.directory,
                &request,
                crate::delegation_dispatch::connection_ended(&mut reader),
            )
            .await;
            writer
                .write_all(&serde_json::to_vec(&response).unwrap())
                .await
                .unwrap();
            writer.write_all(b"\n").await.unwrap();
            writer.shutdown().await.unwrap();
        };
        let client = async {
            let mut stream = crate::platform::local_ipc::LocalIpcClientStream::connect(&endpoint)
                .await
                .unwrap();
            stream
                .write_all(&serde_json::to_vec(&start).unwrap())
                .await
                .unwrap();
            stream.write_all(b"\n").await.unwrap();
            stream.flush().await.unwrap();
            let mut reply = String::new();
            BufReader::new(stream).read_line(&mut reply).await.unwrap();
            serde_json::from_str::<BuiltinToolIpcResponse>(&reply).unwrap()
        };
        let (_, response) = tokio::join!(server, client);
        response
    });
    let BuiltinToolIpcResponse::Envelope { envelope } = response else {
        panic!("expected successful IPC envelope")
    };
    assert!(envelope.ok, "{envelope:?}");
    let payload = crate::builtin_tool_cli_output::project_envelope(envelope).unwrap();
    let task = payload["task_id"].as_str().unwrap().to_owned();
    let before = f.database.connection().total_changes();
    rt.block_on(async {
        let db = tokio::sync::Mutex::new(&mut f.database);
        for (op, input) in [
            (
                "delegation.read",
                json!({"task_id":completed,"limit_bytes":32}),
            ),
            ("delegation.history", json!({"task_id":completed,"limit":1})),
            (
                "get_delegation_status",
                json!({"task_ids":[&task,"unknown",&task]}),
            ),
            ("resume_delegation", json!({"task_id":"unknown"})),
        ] {
            let request = delegation_cli_request(&auth, op, input);
            let BuiltinToolIpcResponse::Envelope { envelope } =
                invoke_ipc(&db, &leases, &f.directory, &request, std::future::pending()).await
            else {
                panic!("expected envelope")
            };
            assert!(envelope.ok, "{envelope:?}");
            crate::builtin_tool_cli_output::project_envelope(envelope).unwrap();
        }
    });
    // Queries record observations; resume-unknown also records its sole receipt.
    assert!(f.database.connection().total_changes() > before);
    let business = delegation_dispatch_business(f);
    let wait = delegation_cli_request(
        &auth,
        "delegation.status",
        json!({"task_ids":[&task],"wait_ms":0}),
    );
    rt.block_on(async {
        use tokio::io::{AsyncBufReadExt,AsyncWriteExt,BufReader};
        let mut listener=crate::platform::local_ipc::LocalIpcListener::bind(&endpoint).unwrap();
        let db=tokio::sync::Mutex::new(&mut f.database);
        let (ready_tx,ready_rx)=tokio::sync::oneshot::channel::<()>();
        let server=async {
            let stream=listener.accept().await.unwrap();
            let (reader,_writer)=tokio::io::split(stream);
            let mut reader=BufReader::new(reader);
            let mut frame=String::new();
            reader.read_line(&mut frame).await.unwrap();
            let request: BuiltinToolIpcRequest=serde_json::from_str(&frame).unwrap();
            let future=invoke_ipc(&db,&leases,&f.directory,&request,crate::delegation_dispatch::connection_ended(&mut reader));
            tokio::pin!(future);
            assert_delegation_wait_is_pending(future.as_mut()).await;
            assert!(db.try_lock().is_ok());
            ready_tx.send(()).unwrap();
            let response=tokio::time::timeout(std::time::Duration::from_secs(1),future).await.unwrap();
            let BuiltinToolIpcResponse::Envelope {envelope}=response else {panic!("expected disconnect envelope")};
            assert_eq!(envelope.error.as_ref().unwrap().code,"delegation.status_disconnected");
            assert_eq!(crate::builtin_tool_cli_output::project_envelope(envelope).unwrap()["error"]["recovery"],"stop");
        };
        let client=async {
            let mut stream=crate::platform::local_ipc::LocalIpcClientStream::connect(&endpoint).await.unwrap();
            stream.write_all(&serde_json::to_vec(&wait).unwrap()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            stream.flush().await.unwrap();
            ready_rx.await.unwrap();
            drop(stream);
        };
        tokio::join!(server,client);
    });
    assert_eq!(delegation_dispatch_business(f), business);
    let cancel = delegation_cli_request(&auth, "cancel_delegation", json!({"task_id":&task}));
    rt.block_on(async {
        let db = tokio::sync::Mutex::new(&mut f.database);
        let BuiltinToolIpcResponse::Envelope { envelope } =
            invoke_ipc(&db, &leases, &f.directory, &cancel, std::future::pending()).await
        else {
            panic!("expected cancellation")
        };
        assert!(envelope.ok);
        assert_eq!(
            crate::builtin_tool_cli_output::project_envelope(envelope).unwrap()["status"],
            "canceled"
        );
    });
    rt.block_on(leases.unregister(config.process_id()));
    let changes = f.database.connection().total_changes();
    rt.block_on(async {
        let db = tokio::sync::Mutex::new(&mut f.database);
        let BuiltinToolIpcResponse::Envelope { envelope } =
            invoke_ipc(&db, &leases, &f.directory, &start, std::future::pending()).await
        else {
            panic!("expected original receipt")
        };
        assert!(envelope.ok);
        assert_eq!(
            crate::builtin_tool_cli_output::project_envelope(envelope).unwrap(),
            payload
        );
        let BuiltinToolIpcResponse::Envelope { envelope } =
            invoke_ipc(&db, &leases, &f.directory, &wait, std::future::pending()).await
        else {
            panic!("expected refusal")
        };
        assert_eq!(envelope.error.unwrap().code, "builtin_tool.run_not_bound");
    });
    assert_eq!(f.database.connection().total_changes(), changes);
}
