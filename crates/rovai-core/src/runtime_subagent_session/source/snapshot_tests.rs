fn assert_complete_source_discovery_and_windows(f: &SourceFixture) {
    use std::sync::atomic::AtomicBool;
    let stop = AtomicBool::new(false);
    let padding = "长来源😀".repeat(100_000);
    let mut grok = target("grok-build");
    let home = f.0.join("snapshot-grok");
    grok.recorded_home = Some(home.clone());
    let parent = f.write(
        "snapshot-grok/sessions/group/parent/updates.jsonl",
        serde_json::to_vec(&json!({
        "padding":padding,"params":{"update":{"sessionUpdate":"subagent_spawned",
            "child_session_id":"child","parent_session_id":"parent"}}}))
        .unwrap(),
    );
    f.write("snapshot-grok/sessions/group/child/updates.jsonl", b"{\"params\":{\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"complete child\"}}}}\n");
    assert!(fs::metadata(&parent).unwrap().len() > HEADER_BYTES);
    assert!(
        NativeDocuments::open_cancellable(&home, &grok, &stop).is_ok(),
        "complete parent records cannot be discarded at the old header limit"
    );
    fs::write(&parent, b"{}\n").unwrap();
    let metadata = f.write(
        "snapshot-grok/sessions/group/parent/subagents/agent/meta.json",
        serde_json::to_vec(&json!({
        "padding":padding,"child_session_id":"child","parent_session_id":"parent"}))
        .unwrap(),
    );
    assert!(fs::metadata(&metadata).unwrap().len() > HEADER_BYTES);
    assert!(NativeDocuments::open_recorded_cancellable(&grok, &stop).is_ok());
    let fallback = chrono::DateTime::parse_from_rfc3339("2026-10-18T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let content = crate::runtime_subagent_session::content::read(&grok, fallback, &stop).unwrap();
    let actor = crate::command::ActorRef::User {
        user_id: crate::current_user::CURRENT_USER_ID.into(),
    };
    let snapshot = crate::runtime_subagent_session::snapshot::NativeSnapshot::new(
        &actor, &grok, content, &stop,
    )
    .unwrap();
    assert!(
        snapshot
            .window(&actor, &grok, &snapshot.reference().unwrap(), None, 32768)
            .unwrap()
            .fragment
            .contains("complete child")
    );
    fs::write(&metadata, serde_json::to_vec(&json!({"padding":padding,"child_session_id":"child","parent_session_id":"wrong-parent"})).unwrap()).unwrap();
    assert!(NativeDocuments::open_cancellable(&home, &grok, &stop).is_err());

    let mut codex = target("codex-cli");
    let codex_home = f.0.join("snapshot-codex");
    codex.recorded_home = Some(codex_home.clone());
    let mut bytes = header("child", Some("parent"), Value::Null);
    bytes.extend(
        serde_json::to_vec(
            &json!({"type":"response_item", "payload":{"type":"message", "role":"assistant",
        "content":[{"type":"output_text","text":"complete codex child"}]}}),
        )
        .unwrap(),
    );
    bytes.push(b'\n');
    let path = f.write(
        "snapshot-codex/sessions/2026/10/18/rollout-2026-10-18T12-00-00-child.jsonl",
        bytes,
    );
    assert!(NativeDocuments::open_recorded_cancellable(&codex, &stop).is_ok());
    let content = crate::runtime_subagent_session::content::read(&codex, fallback, &stop).unwrap();
    let snapshot = crate::runtime_subagent_session::snapshot::NativeSnapshot::new(
        &actor, &codex, content, &stop,
    )
    .unwrap();
    assert!(
        snapshot
            .window(&actor, &codex, &snapshot.reference().unwrap(), None, 32768)
            .unwrap()
            .fragment
            .contains("complete codex child")
    );
    stop.store(true, std::sync::atomic::Ordering::Release);
    for (home, target) in [(&home, &grok), (&codex_home, &codex)] {
        let error = NativeDocuments::open_cancellable(home, target, &stop)
            .err()
            .unwrap();
        assert!(error.to_string().contains("read_cancelled"));
        assert!(
            crate::runtime_subagent_session::content::read(target, fallback, &stop)
                .err()
                .unwrap()
                .to_string()
                .contains("read_cancelled")
        );
    }
    assert!(entries_with_stop(&codex_home, &codex_home.join("sessions"), 4, Some(&stop)).is_err());
    assert!(
        NativeDocument::open_with_cancel(
            &codex_home,
            &path,
            DocumentKind::CodexRollout,
            Some(&stop)
        )
        .is_err()
    );
    assert!(
        NativeDocuments::open_cancellable(&f.0.join("missing"), &codex, &stop)
            .err()
            .unwrap()
            .to_string()
            .contains("read_cancelled")
    );
    crate::runtime_subagent_session::snapshot::tests::assert_signed_windows(&codex);
}
