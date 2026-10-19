// Extends the same owned filesystem fixture; no new Rust owner/DB/process.
fn assert_qoder_agent_documents(f: &SourceFixture) {
    use super::qoder::{QoderAgentLink, open_agent};
    use std::sync::atomic::AtomicBool;
    let home = f.0.join("qoder-captured");
    let parent_path = "qoder-captured/projects/opaque/parent-q.jsonl";
    let directory = "qoder-captured/projects/opaque/parent-q/subagents";
    let meta_path = format!("{directory}/agent-short-q.meta.json");
    let child_path = format!("{directory}/agent-short-q.jsonl");
    let link = QoderAgentLink {
        parent_session_id: "parent-q",
        parent_tool_use_id: "call-q",
        agent_id: "short-q",
    };
    let lines = |records: &[Value]| {
        records
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };
    let root = json!({"type":"user","uuid":"u","parentUuid":null,"sessionId":"parent-q",
        "message":{"content":"do work"}});
    let call = json!({"type":"assistant","uuid":"a","parentUuid":"u","sessionId":"parent-q",
        "message":{"content":[{"type":"tool_use","id":"call-q","name":"Agent","input":{"prompt":"work"}}]}});
    let result = json!({"type":"user","uuid":"r","parentUuid":"a","sourceToolAssistantUUID":"a",
        "sessionId":"parent-q","message":{"content":[{"type":"tool_result","tool_use_id":"call-q","content":"background task started"}]}});
    let base = vec![root.clone(), call.clone(), result.clone()];
    let stop = AtomicBool::new(false);
    let write_parent = |records: &[Value]| {
        f.write(parent_path, lines(records));
    };
    let write_meta = |id: &str| {
        f.write(
            &meta_path,
            serde_json::to_vec_pretty(&json!({"toolUseId":id,"agentType":"Explore"})).unwrap(),
        );
    };
    let read = || open_agent(&home, &link, &stop);
    write_parent(&base);
    write_meta("call-q");
    let text = "完整独立代理材料".repeat(20000);
    let child = lines(&[
        json!({"type":"assistant","agentId":"short-q","sessionId":"not-a-session-grant",
        "message":{"content":[{"type":"text","text":text}]}}),
    ]);
    f.write(&child_path, child.as_bytes());
    let task_a = format!("{directory}/task-a.json");
    let task_z = format!("{directory}/task-z.json");
    f.write(
        &task_z,
        json!({"parentToolUseId":"call-q","status":"FAILED","result":"later file"}).to_string(),
    );
    let task = json!({"parentToolUseId":"call-q","status":"COMPLETED","result":"saved result"})
        .to_string();
    f.write(&task_a, &task);
    f.write(
        &format!("{directory}/task-0.json"),
        json!({"parentToolUseId":"unrelated","result":"foreign"}).to_string(),
    );
    let mut documents = read().unwrap();
    assert_eq!(
        documents
            .documents
            .iter()
            .map(|d| d.kind)
            .collect::<Vec<_>>(),
        vec![
            DocumentKind::QoderParent,
            DocumentKind::QoderAgentMetadata,
            DocumentKind::QoderAgent,
            DocumentKind::QoderTask
        ]
    );
    assert_eq!(
        documents.documents[3].source_label.as_deref(),
        Some("task-a.json")
    );
    let agent = &mut documents.documents[2];
    assert!(agent.chunk_count() > 1);
    let actual: Vec<_> = (0..agent.chunk_count())
        .flat_map(|i| agent.read_chunk(i).unwrap())
        .collect();
    assert_eq!(
        actual,
        child.as_bytes(),
        "complete UTF8, no preview truncation"
    );
    assert_eq!(
        read().unwrap().documents[2].snapshot_digest,
        agent.snapshot_digest
    );
    f.write(&task_a, task.replace("saved result", "edited value"));
    assert!(documents.documents[3].read_chunk(0).is_err());
    drop(documents);
    fs::remove_file(f.0.join(&task_a)).unwrap();
    fs::remove_file(f.0.join(&task_z)).unwrap();
    assert_eq!(
        read().unwrap().documents.len(),
        3,
        "task files are advisory, not authority"
    );
    let mut rewound = base.clone();
    rewound.push(json!({"type":"active-leaf","sessionId":"parent-q","leafUuid":"u","explicit":true,"rewound":true}));
    write_parent(&rewound);
    assert!(
        read().is_err(),
        "a rewound Agent call cannot grant its old file"
    );
    rewound.last_mut().unwrap()["leafUuid"] = Value::Null;
    write_parent(&rewound);
    assert!(read().is_err(), "cleared branch stays empty");
    let mut stale = vec![root.clone(), json!({"type":"active-leaf","leafUuid":null})];
    stale.extend([call.clone(), result.clone()]);
    write_parent(&stale);
    assert!(read().is_ok(), "stale clear cannot hide new activity");
    let parallel = vec![
        root.clone(),
        call.clone(),
        result.clone(),
        json!({"type":"user","uuid":"other-result","parentUuid":"a","sourceToolAssistantUUID":"a",
            "message":{"content":[{"type":"tool_result","tool_use_id":"other","content":"other"}]}}),
        json!({"type":"assistant","uuid":"tail","parentUuid":"other-result","message":{"content":"done"}}),
    ];
    write_parent(&parallel);
    assert!(read().is_ok(), "original parallel sibling result recovery");
    let mut compacted = base.clone();
    compacted.extend([
        json!({"type":"system","uuid":"boundary","parentUuid":null,"logicalParentUuid":"r","subtype":"compact_boundary"}),
        json!({"type":"assistant","uuid":"post","parentUuid":"boundary","message":{"content":"after"}}),
        json!({"type":"active-leaf","leafUuid":"post"})]);
    write_parent(&compacted);
    assert!(
        read().is_ok(),
        "logical parent preserves pre-compaction call"
    );
    let mut sidechain = base.clone();
    sidechain[1]["isSidechain"] = json!(true);
    write_parent(&sidechain);
    assert!(read().is_err());
    let mut wrong = base.clone();
    wrong[2]["sessionId"] = json!("other-parent");
    write_parent(&wrong);
    assert!(read().is_err());
    let mut dangling = base.clone();
    dangling[2]["parentUuid"] = json!("missing");
    write_parent(&dangling);
    assert!(
        read().is_err(),
        "display fallback cannot prove file relationship"
    );
    let mut cycle = base.clone();
    cycle[1]["parentUuid"] = json!("r");
    write_parent(&cycle);
    assert!(read().is_err());
    let mut conflict = base.clone();
    let mut duplicate = call.clone();
    duplicate["message"]["content"][0]["name"] = json!("Read");
    conflict.push(duplicate);
    write_parent(&conflict);
    assert!(read().is_err());
    let mut malformed = base.clone();
    malformed.push(json!({"type":"active-leaf"}));
    write_parent(&malformed);
    assert!(read().is_err());
    write_parent(&base);
    write_meta("other-call");
    assert!(read().is_err());
    write_meta("call-q");
    fs::remove_file(f.0.join(&meta_path)).unwrap();
    let mut claude_ack = base.clone();
    claude_ack[2]["toolUseResult"] = json!({"agentId":"short-q"});
    write_parent(&claude_ack);
    assert!(
        read().is_err(),
        "Claude acknowledgement cannot replace Qoder meta"
    );
    write_meta("call-q");
    write_parent(&base);
    f.write(&child_path, lines(&[json!({"agentId":"another-agent"})]));
    assert!(read().is_err());
    f.write(&child_path, child.as_bytes());
    f.write(parent_path, format!("{}{{\"type\":", lines(&base)));
    assert!(read().is_err());
    write_parent(&base);
    let duplicate = f.write(
        "qoder-captured/projects/another/parent-q.jsonl",
        lines(&base),
    );
    assert!(read().is_err());
    fs::remove_file(duplicate).unwrap();
    assert!(read().is_ok());
    let bad = QoderAgentLink {
        agent_id: "../escape",
        ..link
    };
    assert!(open_agent(&home, &bad, &stop).is_err());
    assert!(
        open_agent(Path::new("relative"), &link, &AtomicBool::new(true))
            .err()
            .unwrap()
            .to_string()
            .contains("read_cancelled")
    );
}
