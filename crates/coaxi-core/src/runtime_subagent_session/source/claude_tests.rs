// Extends the original filesystem owner and its owned temporary directory.
// This is not a Core grant, producer event, real Runtime or complete parser test.
fn assert_legacy_claude_agent_documents(f: &SourceFixture) {
    use super::claude::{ClaudeAgentLink, open_agent};
    use std::sync::atomic::AtomicBool;
    let home = f.0.join("claude-config");
    let parent_path = "claude-config/projects/opaque-project/legacy-parent.jsonl";
    let agent_path =
        "claude-config/projects/opaque-project/legacy-parent/subagents/agent-abc123.jsonl";
    let link = ClaudeAgentLink {
        parent_session_id: "legacy-parent",
        parent_tool_use_id: "toolu_1",
        agent_id: "abc123",
    };
    let parent = |tool: &str, agent: &str| {
        [
            json!({"type":"assistant","sessionId":"legacy-parent","message":{"content":[
                {"type":"tool_use","id":"toolu_1","name":tool,"input":{"prompt":"do work"}}]}}),
            // A sparse earlier result does not fabricate a link or prevent a
            // later explicitly structured acknowledgement from proving it.
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"pending"}]}}),
            json!({"type":"user","sessionId":"legacy-parent","toolUseResult":{"status":"async_launched","agentId":agent},
                "message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"launched"}]}}),
        ].iter().map(Value::to_string).collect::<Vec<_>>().join("\n")
    };
    let full = "完整甲乙丙".repeat(50000);
    // Neither parentUuid (a message pointer) nor sessionId (which can be shared
    // or separate) turns the short agent id into an independent Session grant.
    let child = format!(
        "{}\n",
        json!({"type":"assistant","agentId":"abc123",
        "parentUuid":"message-chain-only","sessionId":"separate-session-not-authority",
        "message":{"content":[{"type":"text","text":full}]}})
    );
    f.write(parent_path, parent("Agent", "abc123"));
    f.write(agent_path, child.as_bytes());
    let stop = AtomicBool::new(false);
    let read = || open_agent(&home, &link, &stop);
    let mut documents = read().unwrap();
    let digest = crate::command::canonical_json_digest(&json!({"prompt":"do work"})).unwrap();
    let observed = super::legacy::open_observed(
        crate::agent_profile::AdapterKind::ClaudeCodeCli,
        &home,
        "legacy-parent",
        "toolu_1",
        &digest,
        &stop,
    )
    .unwrap()
    .unwrap();
    assert_eq!(observed.agent_id, "abc123");
    assert_eq!(observed.documents.documents.len(), 2);
    assert!(
        super::legacy::open_observed(
            crate::agent_profile::AdapterKind::ClaudeCodeCli,
            &home,
            "legacy-parent",
            "toolu_1",
            &"0".repeat(64),
            &stop
        )
        .is_err()
    );
    assert!(
        super::legacy::open_observed(
            crate::agent_profile::AdapterKind::ClaudeCodeCli,
            &home,
            "legacy-parent",
            "other-tool",
            &digest,
            &stop
        )
        .unwrap()
        .is_none()
    );
    crate::runtime_subagent_session::content::claude::assert_original_cases();
    assert_eq!(documents.documents.len(), 2);
    assert_eq!(documents.documents[0].kind, DocumentKind::ClaudeParent);
    assert_eq!(documents.documents[1].kind, DocumentKind::ClaudeAgent);
    let transcript = &mut documents.documents[1];
    assert!(transcript.chunk_count() > 1);
    let actual: Vec<_> = (0..transcript.chunk_count())
        .flat_map(|i| transcript.read_chunk(i).unwrap())
        .collect();
    assert_eq!(actual, child.as_bytes());
    let digest = transcript.snapshot_digest.clone();
    assert_eq!(read().unwrap().documents[1].snapshot_digest, digest);
    let fallback = "2026-10-19T00:00:00Z".parse().unwrap();
    let project = |documents: &mut NativeDocuments, stop: &AtomicBool| {
        crate::runtime_subagent_session::content::claude_read::read_agent(
            documents,
            fallback,
            stop,
            &mut |_, _| None,
            &std::collections::BTreeMap::new(),
            &mut |_, _| None,
        )
    };
    let content = project(&mut documents, &stop).unwrap();
    let actual: String = content
        .turns
        .iter()
        .flat_map(|turn| turn.blocks.iter())
        .filter_map(|block| {
            if let crate::runtime_subagent_session::content::model::ContentBlock::Text { text } =
                block
            {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        actual, full,
        "original full Claude projection retains all verified UTF8"
    );
    assert_eq!(content.summary.as_ref().unwrap().started_at, fallback);
    assert_eq!(
        content.snapshot_digest,
        project(&mut read().unwrap(), &stop)
            .unwrap()
            .snapshot_digest
    );
    assert!(
        content.verified_children.is_empty(),
        "agent ids are not Session grants"
    );
    assert!(project(&mut documents, &AtomicBool::new(true)).is_err());
    drop(documents);
    f.write(parent_path, parent("Task", "abc123"));
    assert!(read().is_ok());
    f.write(parent_path, parent("Read", "abc123"));
    assert!(read().is_err());
    f.write(parent_path, parent("Agent", "another-agent"));
    assert!(read().is_err());
    let wrong_parent = parent("Agent", "abc123").replace(
        "\"sessionId\":\"legacy-parent\"",
        "\"sessionId\":\"other-parent\"",
    );
    f.write(parent_path, wrong_parent);
    assert!(read().is_err());
    let no_ack = parent("Agent", "abc123").lines().next().unwrap().to_owned();
    f.write(parent_path, no_ack);
    assert!(read().is_err());
    f.write(
        parent_path,
        format!(
            "{}\n{}",
            parent("Agent", "abc123"),
            parent("Agent", "other-agent")
        ),
    );
    assert!(
        read().is_err(),
        "contradictory acknowledgements cannot be chosen by recency"
    );
    f.write(parent_path, parent("Agent", "abc123"));
    f.write(agent_path, b"{\"agentId\":\"other-agent\"}");
    assert!(read().is_err());
    f.write(agent_path, child.as_bytes());
    let duplicate = f.write(
        "claude-config/projects/other-project/legacy-parent.jsonl",
        parent("Agent", "abc123"),
    );
    assert!(read().is_err());
    fs::remove_file(duplicate).unwrap();
    let mut changed = read().unwrap();
    let replacement = child.replace("完整", "改动");
    assert_eq!(replacement.len(), child.len());
    f.write(agent_path, replacement);
    assert!(changed.documents[1].read_chunk(0).is_err());
    let bad = ClaudeAgentLink {
        agent_id: "../other",
        ..link
    };
    assert!(open_agent(&home, &bad, &stop).is_err());
    assert!(
        open_agent(Path::new("missing-relative"), &link, &AtomicBool::new(true))
            .err()
            .unwrap()
            .to_string()
            .contains("read_cancelled")
    );
    #[cfg(unix)]
    {
        let outside = f.write("outside-legacy-agent", child.as_bytes());
        fs::remove_file(f.0.join(agent_path)).unwrap();
        std::os::unix::fs::symlink(outside, f.0.join(agent_path)).unwrap();
        assert!(read().is_err());
    }
}
