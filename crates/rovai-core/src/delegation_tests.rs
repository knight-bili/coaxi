use super::*;
use crate::{
    collaboration::{
        CollaborationService, CreateThreadCommand, ExecutionRequest, TestThreadMessageAddress,
        TestThreadMessageCommand,
    },
    command::{ActorRef, CommandEnvelope},
    managed_blob::ManagedBlobStore,
    runtime::{AgentRunWorkspace, ClaimAgentRunCommand, ExecutionRuntimeService},
    team_tool::TeamToolService,
};
use serde_json::json;
use std::{cell::Cell, io::Cursor};

const TASK: &str = "complete independent task";

fn user<T>(camp_id: Option<String>, payload: T) -> CommandEnvelope<T> {
    CommandEnvelope {
        command_id: Uuid::new_v4().to_string(),
        actor: ActorRef::User {
            user_id: "identity-test".into(),
        },
        camp_id,
        expected_versions: Vec::new(),
        execution_epoch: None,
        payload,
    }
}

fn rows(connection: &Connection, table: &str) -> Vec<Vec<rusqlite::types::Value>> {
    let mut statement = connection
        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
        .unwrap();
    statement
        .query_map([], |r| {
            (0..r.as_ref().column_count()).map(|i| r.get(i)).collect()
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

pub(crate) fn child(
    tx: &Transaction<'_>,
    identity: &Identity,
    source_run: &str,
    owner_override: Option<&str>,
) -> Result<String> {
    child_with_task(tx, identity, source_run, owner_override, TASK, None)
}

pub(crate) fn child_with_task(
    tx: &Transaction<'_>,
    identity: &Identity,
    source_run: &str,
    owner_override: Option<&str>,
    task: &str,
    workspace: Option<&serde_json::Value>,
) -> Result<String> {
    let at = "2026-10-17T00:00:00Z";
    tx.execute("INSERT INTO conversation(id,camp_id,agent_id,kind,created_at,updated_at,last_message_sequence) VALUES(?1,?2,?3,'delegation_child',?4,?4,1)",params![identity.child_conversation_id,identity.camp_id,owner_override.unwrap_or(&identity.owner_agent_id),at])?;
    let message = Uuid::new_v4().to_string();
    tx.execute("INSERT INTO conversation_message(id,conversation_id,sequence,author_type,author_id,body,created_at) VALUES(?1,?2,1,'system','delegation-parent',?3,?4)",params![message,identity.child_conversation_id,task,at])?;
    let turn = Uuid::new_v4().to_string();
    let budget =
        crate::execution_budget::freeze_camp_turn_execution_budget(None, chrono::Utc::now(), 1)?;
    tx.execute("INSERT INTO camp_turn(id,camp_id,trigger_type,trigger_id,status,kind,created_at,updated_at,
        execution_budget_schema_version,execution_budget_accepted_at,execution_budget_deadline_at,
        execution_budget_elapsed_seconds,execution_budget_max_agent_run_responsibilities,
        execution_budget_max_accepted_a2a,execution_budget_root_agent_run_responsibilities)
        VALUES(?1,?2,'conversation_message',?3,'running','delegation',?4,?4,?5,?6,?7,?8,?9,?10,?11)",params![turn,identity.camp_id,message,at,budget.schema_version,budget.accepted_at,budget.deadline_at,budget.elapsed_seconds,budget.max_agent_run_responsibilities,budget.max_accepted_a2a,budget.root_agent_run_responsibilities])?;
    let source_config: String = tx.query_row(
        "SELECT effective_config_json FROM agent_run WHERE id=?1",
        [source_run],
        |r| r.get(0),
    )?;
    let mut config: serde_json::Value = serde_json::from_str(&source_config)?;
    let base_runtime: crate::agent_profile::FrozenAgentRuntimeConfig =
        serde_json::from_value(config["runtime"].clone())?;
    let policy = crate::delegation_policy::FrozenPolicy::new(
        true,
        8,
        crate::delegation_policy::Role::Child,
    )?;
    let runtime = if crate::delegation_policy::FrozenPolicy::from_config(&config)?
        .is_some_and(|p| p.role == crate::delegation_policy::Role::Child)
    {
        base_runtime
    } else {
        policy.freeze_resolved_runtime(&base_runtime)?
    };
    config["runtime"] = serde_json::to_value(&runtime)?;
    config["delegationPolicy"] = serde_json::to_value(policy)?;
    config.as_object_mut().unwrap().remove("configDigest");
    config["configDigest"] = serde_json::json!(crate::command::canonical_json_digest(&config)?);
    let workspace_json = if let Some(workspace) = workspace {
        Some(serde_json::to_string(workspace)?)
    } else {
        tx.query_row(
            "SELECT workspace_json FROM agent_run WHERE id=?1",
            [source_run],
            |r| r.get::<_, Option<String>>(0),
        )?
    };
    // Copy a valid Core-owned fixture row; this is not Runtime configuration admission or launch.
    let mut statement =
        tx.prepare("SELECT name FROM pragma_table_info('agent_run') ORDER BY cid")?;
    let columns = statement
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let expressions = columns
        .iter()
        .map(|column| match column.as_str() {
            "id" | "responsibility_key" | "idempotency_key" => "?1".to_string(),
            "conversation_id" | "destination_conversation_id" => "?2".into(),
            "camp_turn_id" => "?3".into(),
            "trigger_conversation_message_id" => "?4".into(),
            "invocation_kind" => "'delegation'".into(),
            "operation_policy" => "'delegation_child_v1'".into(),
            "response_delivery" => "'conversation_message'".into(),
            "status" => "'queued'".into(),
            "execution_epoch" | "runtime_recovery_required" => "0".into(),
            "initial_camp_context_through_sequence" => "0".into(),
            "initial_conversation_context_through_sequence" => "1".into(),
            "effective_config_json" => "?6".into(),
            "runtime_binding_compatibility_digest" => "?7".into(),
            "runtime_native_session_compatibility_key" => "?8".into(),
            "workspace_json" => "?9".into(),
            "version" | "operation_policy_version" => "1".into(),
            "task_id"
            | "camp_id"
            | "anchor_message_id"
            | "current_public_tail_sequence"
            | "trigger_camp_message_id"
            | "trigger_message_delivery_id"
            | "started_at"
            | "ended_at"
            | "wait_reason"
            | "wait_deadline_at"
            | "execution_lease_owner"
            | "execution_lease_expires_at"
            | "cancel_requested_at"
            | "cancel_reason_code"
            | "cancel_acknowledged_at"
            | "final_conversation_message_id"
            | "final_camp_message_id"
            | "workspace_preparing_at" => "NULL".into(),
            _ => format!("\"{column}\""),
        })
        .collect::<Vec<_>>();
    let run = Uuid::new_v4().to_string();
    tx.execute(
        &format!(
            "INSERT INTO agent_run({}) SELECT {} FROM agent_run WHERE id=?5",
            columns.join(","),
            expressions.join(",")
        ),
        params![
            run,
            identity.child_conversation_id,
            turn,
            message,
            source_run,
            serde_json::to_string(&config)?,
            runtime.binding_compatibility_digest,
            runtime.native_session_compatibility_key,
            workspace_json,
        ],
    )?;
    Ok(run)
}

#[test]
fn identity_graph_preserves_core_rows_and_fences_scope_replay_depth_and_takeover() {
    let mut database = crate::test_support::seeded_runtime_database_owned();
    let directory = database.directory().to_path_buf();
    let workspace = directory.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let service = CollaborationService::default();
    let created = service
        .create_camp(
            &mut database,
            &user(
                None,
                CreateThreadCommand::for_test_with_members(
                    workspace.display().to_string(),
                    &["agent_1", "agent_2"],
                    "agent_1",
                ),
            ),
        )
        .unwrap();
    let camp = created.result.payload["threadId"]
        .as_str()
        .unwrap()
        .to_string();
    let sent = service
        .send_test_camp_message(
            &mut database,
            &user(
                Some(camp.clone()),
                TestThreadMessageCommand {
                    camp_id: camp.clone(),
                    draft_revision: None,
                    body: "parent for delegation identity storage".into(),
                    prepared_attachment_ids: Vec::new(),
                    address: TestThreadMessageAddress::Explicit {
                        agent_ids: vec!["agent_1".into()],
                    },
                    reply_to_camp_message_id: None,
                    execution: Some(ExecutionRequest {
                        task_id: None,
                        purpose: "identity fixture".into(),
                        completion_role: "required".into(),
                        budget: None,
                    }),
                },
            ),
        )
        .unwrap();
    let run = sent.result.payload["agentRunIds"][0]
        .as_str()
        .unwrap()
        .to_string();
    let runtime = ExecutionRuntimeService::default();
    let candidate = runtime
        .list_dispatchable_agent_runs(&database, 10)
        .unwrap()
        .into_iter()
        .find(|r| r.agent_run_id == run)
        .unwrap();
    let claim = runtime
        .claim_agent_run(
            &mut database,
            &CommandEnvelope {
                command_id: Uuid::new_v4().to_string(),
                actor: ActorRef::System {
                    component_id: "agent-run-scheduler".into(),
                },
                camp_id: Some(camp.clone()),
                expected_versions: Vec::new(),
                execution_epoch: None,
                payload: ClaimAgentRunCommand {
                    agent_run_id: run.clone(),
                    expected_version: candidate.version,
                    lease_owner: "identity-fixture".into(),
                    lease_seconds: 60,
                    workspace: Some(AgentRunWorkspace::runtime_managed_path(
                        workspace.display().to_string(),
                    )),
                    starting_git_observation: None,
                },
            },
        )
        .unwrap();
    let epoch = claim.result.payload["executionEpoch"].as_i64().unwrap();
    TeamToolService::default()
        .prepare_binding_credential(&mut database, &run, epoch, false)
        .unwrap();
    let (parent_conversation,binding,generation):(String,String,i64)=database.connection().query_row("SELECT c.id,c.native_binding_id,c.native_binding_generation FROM agent_run r JOIN conversation c ON c.id=r.conversation_id WHERE r.id=?1",[&run],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    let blob_store = ManagedBlobStore::new(&directory);
    let task = blob_store
        .put_reader(
            &mut database,
            &mut Cursor::new(TASK.as_bytes()),
            "text/plain",
            "sensitive",
        )
        .unwrap();
    let changed_task = blob_store
        .put_reader(
            &mut database,
            &mut Cursor::new(b"different task"),
            "text/plain",
            "sensitive",
        )
        .unwrap();
    let connection = database.connection();
    let tables = [
        "conversation",
        "camp_turn",
        "agent_run",
        "event_log",
        "managed_blob",
        "runtime_input_delivery",
        "context_manifest",
        "native_session_bootstrap_evidence",
        "agent_run_input",
        "single_chat_composer_draft",
        "message_delivery",
    ];
    let before = tables
        .iter()
        .map(|t| rows(connection, t))
        .collect::<Vec<_>>();
    let public_before: i64 = connection
        .query_row(
            "SELECT count(*) FROM conversation WHERE kind='camp_member'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    crate::db::delegation::checks::source(connection);
    connection
        .execute_batch(
            "PRAGMA foreign_keys=OFF; CREATE TABLE delegation_control(collision INTEGER);",
        )
        .unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    assert!(crate::db::delegation::install_identity_schema(&tx).is_err());
    tx.rollback().unwrap();
    for (table, expected) in tables.iter().zip(&before) {
        assert_eq!(&rows(connection, table), expected, "rollback {table}");
    }
    assert!(
        !connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='delegation_settings')",
                [],
                |r| r.get::<_, bool>(0)
            )
            .unwrap()
    );
    connection
        .execute_batch("DROP TABLE delegation_control;")
        .unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    crate::db::delegation::install_identity_schema(&tx).unwrap();
    tx.commit().unwrap();
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    assert!(crate::db::delegation::identity_schema_matches(connection).unwrap());
    crate::db::delegation::checks::target(connection);
    let tx = connection.unchecked_transaction().unwrap();
    tx.execute_batch("DROP TRIGGER delegation_owner_insert; CREATE TRIGGER delegation_owner_insert BEFORE INSERT ON delegation BEGIN SELECT 1; END;").unwrap();
    assert!(
        !crate::db::delegation::identity_schema_matches(&tx).unwrap(),
        "same trigger name must not admit a forged guard"
    );
    tx.rollback().unwrap();
    for (table, expected) in tables.iter().zip(&before) {
        assert_eq!(&rows(connection, table), expected, "preserved {table}");
    }
    assert!(
        connection
            .query_row("PRAGMA foreign_keys", [], |r| r.get::<_, bool>(0))
            .unwrap()
    );
    connection
        .execute(
            "UPDATE delegation_settings SET enabled=1,depth_limit=8 WHERE singleton=1",
            [],
        )
        .unwrap();
    let target = RuntimeTarget::try_from("codex".to_string()).unwrap();
    let frozen_workspace = json!({"executionRoot":workspace});
    let preset = json!({"fixture":"no Runtime launched"});
    let request = FrozenRequest {
        task_blob_id: &task.id,
        runtime: &target,
        working_dir: None,
        workspace: &frozen_workspace,
        runtime_preset: &preset,
    };
    let parent = ParentInvocation {
        run_id: &run,
        epoch,
        binding_id: &binding,
        binding_generation: generation,
        tool_call_id: "tool-original",
    };
    let factory_calls = Cell::new(0);
    let tx = connection.unchecked_transaction().unwrap();
    let registered = register(&tx, &parent, &request, "2026-10-17T00:00:00Z", |tx, id| {
        factory_calls.set(factory_calls.get() + 1);
        child(tx, id, &run, None)
    })
    .unwrap();
    tx.commit().unwrap();
    assert!(!registered.replayed);
    let first = registered.identity;
    assert_eq!(first.depth, 1);
    assert_eq!(first.root_conversation_id, parent_conversation);
    assert!(
        find_for_parent(connection, &first.id, "another-parent")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        find_for_parent(connection, &first.id, &parent_conversation).unwrap(),
        Some(first.clone())
    );
    let count: i64 = connection
        .query_row("SELECT count(*) FROM agent_profile", [], |r| r.get(0))
        .unwrap();
    connection
        .execute("UPDATE delegation_settings SET enabled=0", [])
        .unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    let replay = register(&tx, &parent, &request, "different timestamp", |_, _| {
        panic!("replay must not create a child")
    })
    .unwrap();
    tx.commit().unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.identity, first);
    assert_eq!(factory_calls.get(), 1);
    let tx = connection.unchecked_transaction().unwrap();
    let changed = FrozenRequest {
        task_blob_id: &changed_task.id,
        ..request
    };
    assert!(
        register(&tx, &parent, &changed, "now", |_, _| panic!(
            "conflict must not create"
        ))
        .is_err()
    );
    tx.rollback().unwrap();
    let fresh = ParentInvocation {
        tool_call_id: "disabled-new",
        ..parent
    };
    let tx = connection.unchecked_transaction().unwrap();
    assert!(
        register(&tx, &fresh, &request, "now", |_, _| panic!(
            "disabled must not create"
        ))
        .is_err()
    );
    tx.rollback().unwrap();
    connection
        .execute("UPDATE delegation_settings SET enabled=1", [])
        .unwrap();
    for wrong in [
        ParentInvocation {
            epoch: epoch + 1,
            tool_call_id: "stale-epoch",
            ..parent
        },
        ParentInvocation {
            binding_id: "wrong-binding",
            tool_call_id: "stale-binding",
            ..parent
        },
    ] {
        let tx = connection.unchecked_transaction().unwrap();
        assert!(
            register(&tx, &wrong, &request, "now", |_, _| panic!(
                "stale must not create"
            ))
            .is_err()
        );
        tx.rollback().unwrap();
    }
    let tx = connection.unchecked_transaction().unwrap();
    let wrong_owner = ParentInvocation {
        tool_call_id: "wrong-owner",
        ..parent
    };
    assert!(
        register(&tx, &wrong_owner, &request, "now", |tx, id| child(
            tx,
            id,
            &run,
            Some("agent_2")
        ))
        .is_err()
    );
    tx.rollback().unwrap();
    for sql in [
        "UPDATE delegation SET depth=7",
        "UPDATE delegation_attempt SET trigger_tool_call_id='other'",
        "UPDATE conversation SET agent_id='agent_2' WHERE kind='delegation_child'",
        "UPDATE agent_run SET operation_policy='camp_member_v1' WHERE invocation_kind='delegation'",
    ] {
        assert!(connection.execute(sql, []).is_err(), "{sql}");
    }
    let tx = connection.unchecked_transaction().unwrap();
    let mismatched_task = ParentInvocation {
        tool_call_id: "wrong-initial-message",
        ..parent
    };
    assert!(register(&tx,&mismatched_task,&request,"now",|tx,id|{
        let child_run=child(tx,id,&run,None)?;
        tx.execute("UPDATE conversation_message SET body='not the complete original task' WHERE conversation_id=?1",[&id.child_conversation_id])?;
        Ok(child_run)
    }).unwrap_err().to_string().contains("original task"));
    tx.rollback().unwrap();
    assert!(
        connection
            .execute("DELETE FROM managed_blob WHERE id=?1", [&task.id])
            .is_err()
    );

    // Each new parent is derived from persisted child ownership, never caller-supplied root/depth.
    let mut last = first.clone();
    for expected_depth in 2..=8 {
        let child_run: String = connection
            .query_row(
                "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
                [&last.id],
                |r| r.get(0),
            )
            .unwrap();
        let child_binding = Uuid::new_v4().to_string();
        connection.execute("UPDATE conversation SET native_binding_id=?1,native_binding_generation=1 WHERE id=?2",params![child_binding,last.child_conversation_id]).unwrap();
        connection
            .execute(
                "UPDATE agent_run SET status='running',execution_epoch=1 WHERE id=?1",
                [&child_run],
            )
            .unwrap();
        let nested = ParentInvocation {
            run_id: &child_run,
            epoch: 1,
            binding_id: &child_binding,
            binding_generation: 1,
            tool_call_id: "nested-tool",
        };
        let tx = connection.unchecked_transaction().unwrap();
        let next = register(&tx, &nested, &request, "now", |tx, id| {
            child(tx, id, &run, None)
        })
        .unwrap();
        tx.commit().unwrap();
        assert_eq!(next.identity.depth, expected_depth);
        assert_eq!(
            next.identity.parent_delegation_id.as_deref(),
            Some(last.id.as_str())
        );
        assert_eq!(next.identity.root_conversation_id, parent_conversation);
        last = next.identity;
    }
    let last_run: String = connection
        .query_row(
            "SELECT agent_run_id FROM delegation_attempt WHERE delegation_id=?1",
            [&last.id],
            |r| r.get(0),
        )
        .unwrap();
    connection.execute("UPDATE conversation SET native_binding_id='depth-eight-binding',native_binding_generation=1 WHERE id=?1",[&last.child_conversation_id]).unwrap();
    connection
        .execute(
            "UPDATE agent_run SET status='running',execution_epoch=1 WHERE id=?1",
            [&last_run],
        )
        .unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    let ninth = ParentInvocation {
        run_id: &last_run,
        epoch: 1,
        binding_id: "depth-eight-binding",
        binding_generation: 1,
        tool_call_id: "ninth-tool",
    };
    assert!(
        register(&tx, &ninth, &request, "now", |_, _| panic!(
            "depth nine must not create"
        ))
        .is_err()
    );
    tx.rollback().unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM agent_profile", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        count
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM conversation WHERE kind='camp_member'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        public_before
    );
    assert!(
        connection
            .execute(
                "UPDATE conversation SET native_binding_id=?1 WHERE id=?2",
                params![binding, first.child_conversation_id]
            )
            .is_err()
    );

    let tx = connection.unchecked_transaction().unwrap();
    assert!(
        record_takeover_boundary(&tx, &first.id, 1, "takeover-command", "now").is_err(),
        "must first close the old output route"
    );
    tx.rollback().unwrap();
    connection.execute("UPDATE agent_run SET status='cancelled',ended_at='now' WHERE invocation_kind='delegation'",[]).unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    let taken = record_takeover_boundary(&tx, &first.id, 1, "takeover-command", "now").unwrap();
    tx.commit().unwrap();
    assert_eq!(taken.takeover_message_sequence, Some(1));
    connection.execute("INSERT INTO conversation_message(id,conversation_id,sequence,author_type,author_id,body,created_at) VALUES('later-user',?1,2,'user','User','later private material','now')",[&first.child_conversation_id]).unwrap();
    connection
        .execute(
            "UPDATE conversation SET last_message_sequence=2 WHERE id=?1",
            [&first.child_conversation_id],
        )
        .unwrap();
    let tx = connection.unchecked_transaction().unwrap();
    assert_eq!(
        record_takeover_boundary(&tx, &first.id, 1, "takeover-command", "another time").unwrap(),
        taken
    );
    assert!(record_takeover_boundary(&tx, &first.id, 1, "different-command", "now").is_err());
    tx.rollback().unwrap();
    assert!(connection.execute("UPDATE delegation_control SET control_version=control_version+1,takeover_message_sequence=2 WHERE delegation_id=?1",[&first.id]).is_err());
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
    let path = directory.join("rovai.sqlite");
    database.close();
    // Reopen raw storage only: the complete startup migration/format admission is not registered yet.
    let reopened = Connection::open(path).unwrap();
    assert_eq!(
        find_for_parent(&reopened, &first.id, &parent_conversation).unwrap(),
        Some(first.clone())
    );
    assert_eq!(control(&reopened, &first.id).unwrap(), taken);
}
