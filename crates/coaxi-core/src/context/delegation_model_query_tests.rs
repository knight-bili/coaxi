// Reuses the original Context owner and candidate database. Core commands and
// filesystem observations are real; process lease/terminal/cleanup are fixtures.
fn query_read(
    f: &Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    input: Value,
) -> anyhow::Result<Value> {
    let input = serde_json::from_value::<crate::delegation_contract::ReadParams>(input)?;
    crate::delegation_model_query::read(
        &f.database,
        &crate::delegation_model_command::Invocation {
            binding,
            source,
            tool_call: "query-read",
            input: &input,
        },
    )
}
fn query_history(
    f: &Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    input: Value,
) -> anyhow::Result<Value> {
    let input = serde_json::from_value::<crate::delegation_contract::HistoryParams>(input)?;
    crate::delegation_model_query::history(
        &f.database,
        &crate::delegation_model_command::Invocation {
            binding,
            source,
            tool_call: "query-history",
            input: &input,
        },
    )
}
fn assert_delegation_model_queries(
    f: &mut Fixture,
    binding: &crate::team_tool::BuiltinToolBindingCredential,
    source: &crate::team_tool::AuthenticatedTeamToolRun,
    request: &crate::delegation::FrozenRequest<'_>,
    plan: &crate::delegation_factory::ChildRunPlan<'_>,
    task: &str,
    original_run: &str,
    full: &str,
) {
    use crate::{
        delegation_contract::{CancelParams, StatusParams},
        delegation_model_command::{self as model, Invocation},
        delegation_model_query as query,
    };
    let before = f.database.connection().total_changes();
    let first = query_read(
        f,
        binding,
        source,
        json!({"task_id":task,"limit_bytes":65536}),
    )
    .unwrap();
    assert!(first["text"].as_str().unwrap().len() <= 65536);
    let fixed = first["result_ref"].clone();
    let mut joined = first["text"].as_str().unwrap().to_owned();
    let original_cursor = first["next_cursor"].as_str().unwrap().to_owned();
    let mut cursor = first.get("next_cursor").cloned();
    while let Some(next) = cursor {
        let page = query_read(
            f,
            binding,
            source,
            json!({"task_id":task,"cursor":next,"limit_bytes":65536}),
        )
        .unwrap();
        assert_eq!(page["result_ref"], fixed);
        assert!(page["text"].as_str().unwrap().len() <= 65536);
        joined.push_str(page["text"].as_str().unwrap());
        cursor = page.get("next_cursor").cloned();
    }
    assert_eq!(joined, full);
    assert!(
        query_read(f, binding, source, json!({"task_id":task,"limit_bytes":1}))
            .unwrap_err()
            .to_string()
            .contains("page_limit_too_small")
    );
    assert_eq!(
        query_read(f, binding, source, json!({"task_id":task,"limit_bytes":4})).unwrap()["text"],
        "中"
    );
    let mut bad = original_cursor.clone();
    bad.push('!');
    assert!(query_read(f, binding, source, json!({"task_id":task,"cursor":bad})).is_err());
    let latest = query_history(f, binding, source, json!({"task_id":task,"limit":1})).unwrap();
    assert_eq!(latest["messages"][0]["author_type"], "assistant");
    assert_eq!(latest["messages"][0]["body"], full);
    let history_cursor = latest["next_cursor"].as_str().unwrap().to_owned();
    let prior = query_history(
        f,
        binding,
        source,
        json!({"task_id":task,"before_cursor":history_cursor,"limit":1}),
    )
    .unwrap();
    assert_eq!(prior["messages"][0]["author_type"], "delegation_parent");
    assert_eq!(prior["messages"][0]["body"], plan.task);
    assert!(prior.get("next_cursor").is_none());
    assert!(
        query_read(
            f,
            binding,
            source,
            json!({"task_id":task,"cursor":history_cursor})
        )
        .is_err()
    );
    assert!(
        query_history(
            f,
            binding,
            source,
            json!({"task_id":task,"before_cursor":original_cursor})
        )
        .is_err()
    );
    assert_eq!(f.database.connection().total_changes(), before);
    // Explicit isolated corruption with a before snapshot: a saved cursor must
    // not join pieces from two versions of a supposedly fixed result.
    let final_id = fixed["message_id"].as_str().unwrap();
    let saved: String = f
        .database
        .connection()
        .query_row(
            "SELECT body FROM conversation_message WHERE id=?1",
            [final_id],
            |r| r.get(0),
        )
        .unwrap();
    f.database
        .connection()
        .execute(
            "UPDATE conversation_message SET body='changed fixture result' WHERE id=?1",
            [final_id],
        )
        .unwrap();
    assert!(
        query_read(
            f,
            binding,
            source,
            json!({"task_id":task,"cursor":original_cursor})
        )
        .unwrap_err()
        .to_string()
        .contains("result_changed")
    );
    f.database
        .connection()
        .execute(
            "UPDATE conversation_message SET body=?1 WHERE id=?2",
            params![saved, final_id],
        )
        .unwrap();
    assert_eq!(
        query_read(
            f,
            binding,
            source,
            json!({"task_id":task,"cursor":original_cursor})
        )
        .unwrap()["result_ref"],
        fixed
    );
    let self_id:String=f.database.connection().query_row("SELECT id FROM delegation WHERE child_conversation_id=(SELECT conversation_id FROM agent_run WHERE id=?1)",[&source.agent_run_id],|r|r.get(0)).unwrap();
    assert!(
        query_history(f, binding, source, json!({"task_id":self_id})).unwrap()["messages"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        query_read(f, binding, source, json!({"task_id":self_id}))
            .unwrap_err()
            .to_string()
            .contains("result_unavailable")
    );
    let parent_id:String=f.database.connection().query_row("SELECT parent.id FROM delegation own JOIN delegation parent ON parent.child_conversation_id=own.parent_conversation_id WHERE own.id=?1",[&self_id],|r|r.get(0)).unwrap();
    for id in ["missing-query", parent_id.as_str()] {
        let expected = json!({"task_id":id,"status":"unknown"});
        assert_eq!(
            query_read(
                f,
                binding,
                source,
                json!({"task_id":id,"cursor":original_cursor})
            )
            .unwrap(),
            expected
        );
        assert_eq!(
            query_history(f, binding, source, json!({"task_id":id})).unwrap(),
            expected
        );
    }
    let waiting = model_start(f, binding, source, request, plan, "query-waiting-task").unwrap();
    let waiting_id = waiting.result.payload["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        query_read(f, binding, source, json!({"task_id":waiting_id}))
            .unwrap_err()
            .to_string()
            .contains("result_unavailable")
    );
    assert!(
        query_read(
            f,
            binding,
            source,
            json!({"task_id":waiting_id,"cursor":original_cursor})
        )
        .is_err()
    );
    assert!(
        query_history(
            f,
            binding,
            source,
            json!({"task_id":waiting_id,"before_cursor":history_cursor})
        )
        .is_err()
    );

    // User takeover fixes the parent's ceiling. Actual User send/claim gives
    // the child's own reader an exclusive CURRENT_INPUT bound instead.
    crate::application::delegation_control::handle(
        &mut f.database,
        "delegations.takeover",
        control_request(task, "expectedControlVersion", 1),
    )
    .unwrap();
    let epoch: i64 = f
        .database
        .connection()
        .query_row(
            "SELECT execution_epoch FROM agent_run WHERE id=?1",
            [original_run],
            |r| r.get(0),
        )
        .unwrap();
    ExecutionRuntimeService::default()
        .record_runtime_cleanup_completed(&f.database, original_run, epoch)
        .unwrap();
    let conversation = fixed["conversation_id"].as_str().unwrap();
    let path = f.directory.join("query-private-attachment.txt");
    std::fs::write(&path, b"private user fixture").unwrap();
    let attachment = crate::local_attachment_source::LocalAttachmentSourceRef {
        id: Uuid::new_v4().to_string(),
        source_path: path.canonicalize().unwrap().to_str().unwrap().to_owned(),
        display_name: "private.txt".into(),
        kind: crate::local_attachment_source::LocalAttachmentKind::File,
        media_type: Some("text/plain".into()),
        observed_byte_size: Some(20),
    };
    let prepared =
        crate::private_source_observation::PreparedSources::capture(vec![attachment.clone()])
            .unwrap();
    let tx = f.database.connection_mut().transaction().unwrap();
    assert!(
        crate::private_source_observation::capture_sources(
            &tx,
            conversation,
            std::slice::from_ref(&attachment),
            &prepared
        )
        .unwrap()
        .is_none()
    );
    tx.execute("INSERT INTO single_chat_composer_draft(conversation_id,client_id,scope_key,revision,source_attachments_json,updated_at) VALUES(?1,?2,'',1,?3,'2026-10-17T00:00:00Z')",params![conversation,crate::draft_client::DraftClient::default().id(),crate::local_attachment_source::serialize_source_attachments(&[attachment.clone()]).unwrap()]).unwrap();
    tx.commit().unwrap();
    let sent = child_end_send(f, conversation, "用户接管后的私密正文");
    let user_run = sent["agentRunId"].as_str().unwrap();
    let user_epoch = claim_context_child(f, user_run);
    let user_binding = TeamToolService::default()
        .prepare_binding_credential(&mut f.database, user_run, user_epoch, false)
        .unwrap();
    let user_source = crate::team_tool::AuthenticatedTeamToolRun {
        agent_run_id: user_run.into(),
        execution_epoch: user_epoch,
        ..source.clone()
    };
    let own = query_history(f, &user_binding, &user_source, json!({"task_id":task})).unwrap();
    assert_eq!(own["messages"].as_array().unwrap().len(), 2);
    assert!(!own.to_string().contains("用户接管后的私密正文"));
    assert_eq!(
        query_read(f, &user_binding, &user_source, json!({"task_id":task})).unwrap()["result_ref"],
        fixed
    );
    // Parent cursor cannot be used through own-child authority even though it
    // refers to the same persisted result message.
    assert!(
        query_read(
            f,
            &user_binding,
            &user_source,
            json!({"task_id":task,"cursor":original_cursor})
        )
        .is_err()
    );
    let hidden = query_history(f, binding, source, json!({"task_id":task})).unwrap();
    assert_eq!(hidden["messages"].as_array().unwrap().len(), 2);
    assert!(!hidden.to_string().contains("用户接管后的私密正文"));
    lifecycle_succeed(f, user_run, user_epoch);
    let sent = child_end_send(f, conversation, "当前用户输入不进入自己的历史");
    let next_run = sent["agentRunId"].as_str().unwrap();
    let next_epoch = claim_context_child(f, next_run);
    let next_binding = TeamToolService::default()
        .prepare_binding_credential(&mut f.database, next_run, next_epoch, false)
        .unwrap();
    let next_source = crate::team_tool::AuthenticatedTeamToolRun {
        agent_run_id: next_run.into(),
        execution_epoch: next_epoch,
        ..source.clone()
    };
    let own = query_history(f, &next_binding, &next_source, json!({"task_id":task})).unwrap();
    let messages = own["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[2]["author_type"], "user");
    assert_eq!(
        messages[2]["attachments"],
        json!([{"id":attachment.id,"name":"private.txt","kind":"file","size_bytes":20}])
    );
    assert!(
        !own.to_string().contains("source_path")
            && !own.to_string().contains(attachment.source_path.as_str())
    );
    assert!(!own.to_string().contains("当前用户输入不进入自己的历史"));
    assert_eq!(
        query_read(f, &next_binding, &next_source, json!({"task_id":task})).unwrap()["text"],
        "只属于本子会话的完成结果"
    );
    let parent_history = query_history(
        f,
        binding,
        source,
        json!({"task_id":task,"before_cursor":history_cursor}),
    )
    .unwrap();
    assert_eq!(parent_history["messages"], prior["messages"]);
    assert_eq!(
        query_read(
            f,
            binding,
            source,
            json!({"task_id":task,"cursor":original_cursor})
        )
        .unwrap()["result_ref"],
        fixed
    );
    assert_eq!(
        query_history(f, &next_binding, &next_source, json!({"task_id":parent_id})).unwrap(),
        json!({"task_id":parent_id,"status":"unknown"})
    );
    lifecycle_succeed(f, next_run, next_epoch);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    let status_input: StatusParams = serde_json::from_value(
        json!({"task_ids":[waiting_id,task,waiting_id,parent_id,"missing-query"]}),
    )
    .unwrap();
    let poll = runtime
        .block_on(query::wait_for_status(
            f.database.execution_wake.delegation.clone(),
            &status_input,
            || {
                std::future::ready(query::status_snapshot(
                    &f.database,
                    &Invocation {
                        binding,
                        source,
                        tool_call: "status-poll",
                        input: &status_input,
                    },
                ))
            },
            std::future::pending(),
        ))
        .unwrap();
    assert_eq!(
        poll["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["task_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            waiting_id.as_str(),
            task,
            waiting_id.as_str(),
            parent_id.as_str(),
            "missing-query"
        ]
    );
    assert_eq!(poll["tasks"][0]["status"], "running");
    assert_eq!(poll["tasks"][1]["status"], "completed");
    assert_eq!(poll["tasks"][2], poll["tasks"][0]);
    assert_eq!(
        poll["tasks"][3],
        json!({"task_id":parent_id,"status":"unknown"})
    );
    assert!(
        query_read(
            f,
            binding,
            source,
            json!({"task_id":task,"cursor":poll["cursor"]})
        )
        .is_err()
    );
    let wrong: StatusParams =
        serde_json::from_value(json!({"task_ids":[task],"after_cursor":poll["cursor"]})).unwrap();
    assert!(
        query::status_snapshot(
            &f.database,
            &Invocation {
                binding,
                source,
                tool_call: "wrong-status-set",
                input: &wrong
            }
        )
        .is_err()
    );

    // A commit after the first snapshot and before the first await is not lost.
    let wait_input: StatusParams =
        serde_json::from_value(json!({"task_ids":[waiting_id],"wait_ms":0})).unwrap();
    let stop = CancelParams {
        task_id: waiting_id.clone().try_into().unwrap(),
    };
    let notify = f.database.execution_wake.delegation.clone();
    runtime.block_on(async {
        let database = tokio::sync::Mutex::new(&mut f.database);
        let reads = std::cell::Cell::new(0);
        let wait = query::wait_for_status(
            notify,
            &wait_input,
            || async {
                let mut db = database.lock().await;
                reads.set(reads.get() + 1);
                let snapshot = query::status_snapshot(
                    &db,
                    &Invocation {
                        binding,
                        source,
                        tool_call: "status-race",
                        input: &wait_input,
                    },
                )?;
                if reads.get() == 1 {
                    model::cancel(
                        &mut db,
                        &Invocation {
                            binding,
                            source,
                            tool_call: "status-race-stop",
                            input: &stop,
                        },
                    )?;
                }
                Ok(snapshot)
            },
            std::future::pending(),
        );
        let value = tokio::time::timeout(std::time::Duration::from_secs(1), wait)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reads.get(), 2);
        assert_eq!(value["tasks"][0]["status"], "canceled");
    });

    let simultaneous =
        model_start(f, binding, source, request, plan, "status-two-waiters").unwrap();
    let simultaneous_id = simultaneous.result.payload["task_id"].as_str().unwrap();
    let input: StatusParams =
        serde_json::from_value(json!({"task_ids":[simultaneous_id],"wait_ms":0})).unwrap();
    let stop = CancelParams {
        task_id: simultaneous_id.to_owned().try_into().unwrap(),
    };
    let notify = f.database.execution_wake.delegation.clone();
    runtime.block_on(async {
        let database = tokio::sync::Mutex::new(&mut f.database);
        let arrived = tokio::sync::Notify::new();
        let reads = std::cell::Cell::new(0);
        let snapshot = || async {
            let db = database.lock().await;
            let value = query::status_snapshot(
                &db,
                &Invocation {
                    binding,
                    source,
                    tool_call: "two-status",
                    input: &input,
                },
            )?;
            reads.set(reads.get() + 1);
            if reads.get() == 2 {
                arrived.notify_one();
            }
            Ok(value)
        };
        let a = query::wait_for_status(notify.clone(), &input, snapshot, std::future::pending());
        let b = query::wait_for_status(notify, &input, snapshot, std::future::pending());
        let cancel = async {
            arrived.notified().await;
            let mut db = database.lock().await;
            model::cancel(
                &mut db,
                &Invocation {
                    binding,
                    source,
                    tool_call: "stop-two-status",
                    input: &stop,
                },
            )
            .unwrap();
        };
        let (a, b, ()) = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            tokio::join!(a, b, cancel)
        })
        .await
        .unwrap();
        assert_eq!(a.unwrap()["tasks"][0]["status"], "canceled");
        assert_eq!(b.unwrap()["tasks"][0]["status"], "canceled");
    });

    let path = f.directory.join("status-block-directory");
    std::fs::create_dir_all(&path).unwrap();
    let external =
        crate::delegation_contract::WorkingDirectory::try_from(path.to_str().unwrap().to_owned())
            .unwrap();
    let external_request = crate::delegation::FrozenRequest {
        working_dir: Some(&external),
        ..*request
    };
    let blocked = model_start(
        f,
        binding,
        source,
        &external_request,
        plan,
        "status-blocked",
    )
    .unwrap();
    let blocked_id = blocked.result.payload["task_id"].as_str().unwrap();
    let input: StatusParams =
        serde_json::from_value(json!({"task_ids":[blocked_id],"wait_ms":0})).unwrap();
    let cursor = runtime
        .block_on(query::wait_for_status(
            f.database.execution_wake.delegation.clone(),
            &input,
            || {
                std::future::ready(query::status_snapshot(
                    &f.database,
                    &Invocation {
                        binding,
                        source,
                        tool_call: "block-status",
                        input: &input,
                    },
                ))
            },
            std::future::pending(),
        ))
        .unwrap();
    assert_eq!(
        cursor["tasks"][0]["blocked_on"],
        blocked.result.payload["blocked_on"]
    );
    let input: StatusParams = serde_json::from_value(
        json!({"task_ids":[blocked_id],"wait_ms":15,"after_cursor":cursor["cursor"]}),
    )
    .unwrap();
    let reads = std::cell::Cell::new(0);
    let notify = f.database.execution_wake.delegation.clone();
    runtime.block_on(async {
        let start = tokio::time::Instant::now();
        let value = query::wait_for_status(
            notify,
            &input,
            || {
                reads.set(reads.get() + 1);
                std::future::ready(query::status_snapshot(
                    &f.database,
                    &Invocation {
                        binding,
                        source,
                        tool_call: "same-block-wait",
                        input: &input,
                    },
                ))
            },
            std::future::pending(),
        )
        .await
        .unwrap();
        assert_eq!(start.elapsed(), std::time::Duration::from_millis(15));
        assert_eq!(reads.get(), 2);
        assert_eq!(value, cursor);
    });
    // Positive waits cap at60s and disconnect is read-only, never a stop command.
    let idle = model_start(f, binding, source, request, plan, "status-idle").unwrap();
    let idle_id = idle.result.payload["task_id"].as_str().unwrap();
    let input: StatusParams =
        serde_json::from_value(json!({"task_ids":[idle_id],"wait_ms":90000})).unwrap();
    runtime.block_on(async {
        let start = tokio::time::Instant::now();
        let value = query::wait_for_status(
            f.database.execution_wake.delegation.clone(),
            &input,
            || {
                std::future::ready(query::status_snapshot(
                    &f.database,
                    &Invocation {
                        binding,
                        source,
                        tool_call: "bounded-wait",
                        input: &input,
                    },
                ))
            },
            std::future::pending(),
        )
        .await
        .unwrap();
        assert_eq!(start.elapsed(), std::time::Duration::from_secs(60));
        assert_eq!(value["tasks"][0]["status"], "running");
    });
    let before = f.database.connection().total_changes();
    runtime.block_on(async {
        let value = query::wait_for_status(
            f.database.execution_wake.delegation.clone(),
            &input,
            || {
                std::future::ready(query::status_snapshot(
                    &f.database,
                    &Invocation {
                        binding,
                        source,
                        tool_call: "disconnect-wait",
                        input: &input,
                    },
                ))
            },
            std::future::ready(()),
        )
        .await;
        assert!(
            value
                .unwrap_err()
                .to_string()
                .contains("status_disconnected")
        );
    });
    assert_eq!(f.database.connection().total_changes(), before);
    let still:String=f.database.connection().query_row("SELECT r.status FROM delegation_attempt a JOIN agent_run r ON r.id=a.agent_run_id WHERE a.delegation_id=?1",[idle_id],|r|r.get(0)).unwrap();
    assert_eq!(still, "queued");

    // Actual Core parent cancellation wakes the child parent's waiter and revokes
    // its authority. It does not let a stopped source keep reading current state.
    let parent = model_start(f, binding, source, request, plan, "status-source-parent").unwrap();
    let parent_task = parent.result.payload["task_id"].as_str().unwrap();
    let parent_run: String = f
        .database
        .connection()
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [parent_task],
            |r| r.get(0),
        )
        .unwrap();
    let parent_epoch = claim_context_child(f, &parent_run);
    let parent_binding = TeamToolService::default()
        .prepare_binding_credential(&mut f.database, &parent_run, parent_epoch, false)
        .unwrap();
    let parent_source = crate::team_tool::AuthenticatedTeamToolRun {
        agent_run_id: parent_run,
        execution_epoch: parent_epoch,
        ..source.clone()
    };
    let leaf = model_start(
        f,
        &parent_binding,
        &parent_source,
        request,
        plan,
        "status-source-leaf",
    )
    .unwrap();
    let leaf_id = leaf.result.payload["task_id"].as_str().unwrap();
    let input: StatusParams =
        serde_json::from_value(json!({"task_ids":[leaf_id],"wait_ms":0})).unwrap();
    let stop = CancelParams {
        task_id: parent_task.to_owned().try_into().unwrap(),
    };
    let notify = f.database.execution_wake.delegation.clone();
    runtime.block_on(async {
        let database = tokio::sync::Mutex::new(&mut f.database);
        let reads = std::cell::Cell::new(0);
        let wait = query::wait_for_status(
            notify,
            &input,
            || async {
                let mut db = database.lock().await;
                let snap = query::status_snapshot(
                    &db,
                    &Invocation {
                        binding: &parent_binding,
                        source: &parent_source,
                        tool_call: "parent-stop-wait",
                        input: &input,
                    },
                )?;
                reads.set(reads.get() + 1);
                if reads.get() == 1 {
                    model::cancel(
                        &mut db,
                        &Invocation {
                            binding,
                            source,
                            tool_call: "stop-source-parent",
                            input: &stop,
                        },
                    )?;
                }
                Ok(snap)
            },
            std::future::pending(),
        );
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), wait)
                .await
                .unwrap()
                .is_err()
        );
    });
}
