use std::io::Write;

use serde_json::json;

use super::*;

struct SourceFixture(PathBuf);
impl SourceFixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("rovai-native-child-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
    fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for SourceFixture {
    fn drop(&mut self) {
        assert_eq!(
            self.0.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("rovai-native-child-")
        );
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn target(adapter: &str) -> ObservedNativeSessionTarget {
    ObservedNativeSessionTarget {
        thread_id: "thread-fixture".into(),
        conversation_id: "conversation-fixture".into(),
        agent_run_id: "run-fixture".into(),
        execution_epoch: 1,
        evidence_id: "evidence-fixture".into(),
        adapter_kind: adapter.into(),
        installation_id: "installation-fixture".into(),
        binding_id: "binding-fixture".into(),
        binding_generation: 1,
        parent_native_session_id: "parent".into(),
        child_native_session_id: "child".into(),
        status: "running".into(),
        relationship: "runtime_reported_receiver".into(),
        live: true,
        recorded_home: None,
    }
}

fn header(id: &str, parent: Option<&str>, history: Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&json!({"type":"session_meta","ordinal":0,
        "payload":{"id":id,"parent_thread_id":parent,"history_base":history}}))
    .unwrap();
    bytes.push(b'\n');
    bytes
}

// One filesystem owner, no SQLite, Runtime process, credentials, network or
// provider. Actor/binding correlation belongs to the existing Context owner.
#[test]
fn lineage_sources_keep_exact_rollouts_and_lossless_stable_chunks() {
    let f = SourceFixture::new();
    let codex = target("codex-cli");
    let name = "sessions/2026/10/18/rollout-2026-10-18T12-00-00-child.jsonl";
    let mut bytes = header("child", Some("parent"), Value::Null);
    let long = format!(
        "头{}中{}尾",
        "甲".repeat(CHUNK_BYTES),
        "乙".repeat(CHUNK_BYTES)
    );
    bytes.extend(serde_json::to_vec(&json!({"type":"response_item","ordinal":1,
        "payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":long}]}})).unwrap());
    bytes.push(b'\n');
    let path = f.write(name, &bytes);
    f.write(
        "sessions/contains-child-but-not-a-rollout.jsonl",
        header("child", Some("parent"), Value::Null),
    );
    let mut documents = NativeDocuments::open(&f.0, &codex).unwrap();
    assert_eq!(documents.documents.len(), 1);
    let document = &mut documents.documents[0];
    assert_eq!(document.kind, DocumentKind::CodexRollout);
    assert_eq!(document.snapshot_bytes, bytes.len() as u64);
    assert_eq!(
        document.snapshot_digest,
        format!("{:x}", Sha256::digest(&bytes))
    );
    let mut rebuilt = Vec::new();
    for index in 0..document.chunk_count() {
        rebuilt.extend(document.read_chunk(index).unwrap());
    }
    assert_eq!(
        rebuilt, bytes,
        "long UTF-8 records are not preview-truncated"
    );
    assert!(document.read_chunk(document.chunk_count()).is_err());
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"new native activity\n")
        .unwrap();
    let mut after_append = Vec::new();
    for index in 0..document.chunk_count() {
        after_append.extend(document.read_chunk(index).unwrap());
    }
    assert_eq!(
        after_append, bytes,
        "a snapshot does not shift after append"
    );
    let mut changed = bytes.clone();
    changed[CHUNK_BYTES + 20] ^= 1;
    fs::write(&path, &changed).unwrap();
    assert!(
        document.read_chunk(1).is_err(),
        "same-length edits fail by digest"
    );
    fs::write(&path, b"short replacement").unwrap();
    assert!(document.read_chunk(1).is_err());
    drop(documents);
    f.write(name, header("foreign", Some("parent"), Value::Null));
    assert!(NativeDocuments::open(&f.0, &codex).is_err());
    f.write(name, header("child", Some("foreign-parent"), Value::Null));
    assert!(NativeDocuments::open(&f.0, &codex).is_err());
    let mut nested = header("child", None, Value::Null);
    let mut value: Value = serde_json::from_slice(&nested).unwrap();
    value["payload"]["source"] = json!({"subagent":{"thread_spawn":{"parent_thread_id":"parent"}}});
    nested = serde_json::to_vec(&value).unwrap();
    f.write(name, nested);
    assert!(
        NativeDocuments::open(&f.0, &codex).is_ok(),
        "older structured lineage is supported"
    );
    f.write(
        "sessions/2026/10/18/rollout-2026-10-18T11-00-00-parent.jsonl",
        header("parent", None, Value::Null),
    );
    let mut fork: Value =
        serde_json::from_slice(&header("child", Some("parent"), Value::Null)).unwrap();
    fork["payload"]["forked_from_id"] = json!("parent");
    fork["payload"]["forked_from_ordinal_exclusive"] = json!(4);
    fork["payload"]["subagent_history_start_ordinal"] = json!(8);
    f.write(name, serde_json::to_vec(&fork).unwrap());
    let documents = NativeDocuments::open(&f.0, &codex).unwrap();
    assert_eq!(documents.documents.len(), 2);
    assert_eq!(documents.documents[0].seed_start_ordinal, Some(8));
    assert_eq!(
        documents.documents[0].history_base,
        Some(HistoryLink {
            document_index: 1,
            end_ordinal_exclusive: 4
        })
    );
    assert_eq!(
        documents.documents[0].own_header.as_ref().unwrap()["payload"]["id"],
        "child"
    );
    fork["payload"]["history_base"] = json!({"thread_id":"child","end_ordinal_exclusive":1});
    // The explicit base wins over the older fork pointer, even when that exposes
    // an invalid cycle; silently falling back would show the wrong history.
    f.write(name, serde_json::to_vec(&fork).unwrap());
    assert!(NativeDocuments::open(&f.0, &codex).is_err());
    drop(documents);
    f.write(name, header("child", Some("parent"), Value::Null));
    let later = "sessions/2026/10/18/rollout-2026-10-18T12-01-00-child_revert-id.jsonl";
    f.write(
        &later,
        header(
            "child",
            Some("parent"),
            json!({"thread_id":"child","end_ordinal_exclusive":2}),
        ),
    );
    let documents = NativeDocuments::open(&f.0, &codex).unwrap();
    assert_eq!(documents.documents.len(), 2);
    assert_eq!(
        documents.documents[0].history_base,
        Some(HistoryLink {
            document_index: 1,
            end_ordinal_exclusive: 2
        })
    );
    drop(documents);
    f.write(
        &later,
        header(
            "child",
            Some("parent"),
            json!({"thread_id":"revert-id","end_ordinal_exclusive":2}),
        ),
    );
    assert!(
        NativeDocuments::open(&f.0, &codex).is_err(),
        "cycles do not return a partial history"
    );
    f.write(
        &later,
        header(
            "child",
            Some("parent"),
            json!({"thread_id":"missing-rollout","end_ordinal_exclusive":2}),
        ),
    );
    assert!(NativeDocuments::open(&f.0, &codex).is_err());
    let duplicate = f.write(
        "archived_sessions/rollout-2026-10-18T12-00-00-child.jsonl",
        header("child", Some("parent"), Value::Null),
    );
    f.write(
        &later,
        header(
            "child",
            Some("parent"),
            json!({"thread_id":"child","end_ordinal_exclusive":2}),
        ),
    );
    assert!(
        NativeDocuments::open(&f.0, &codex).is_err(),
        "duplicate exact rollout identities are ambiguous"
    );
    fs::remove_file(duplicate).unwrap();

    let grok = target("grok-build");
    let update = json!({"params":{"sessionId":"parent","update":{"sessionUpdate":"subagent_spawned",
        "subagent_id":"short-id","parent_session_id":"parent","child_session_id":"child"}}});
    let parent = f.write(
        "sessions/group-a/parent/updates.jsonl",
        serde_json::to_vec(&update).unwrap(),
    );
    let child_bytes = b"{\"params\":{\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"child body\"}}}}\n";
    f.write("sessions/group-b/child/updates.jsonl", child_bytes);
    f.write(
        "sessions/group-b/child/chat_history.jsonl",
        b"{\"role\":\"tool\",\"content\":\"original answer\"}\n",
    );
    let mut documents = NativeDocuments::open(&f.0, &grok).unwrap();
    assert_eq!(documents.documents.len(), 2);
    assert_eq!(documents.documents[1].kind, DocumentKind::GrokChatHistory);
    assert_eq!(documents.documents[0].read_chunk(0).unwrap(), child_bytes);
    drop(documents);
    fs::write(&parent, b"{}").unwrap();
    assert!(
        NativeDocuments::open(&f.0, &grok).is_err(),
        "mere sibling location is not lineage"
    );
    f.write(
        "sessions/group-a/parent/subagents/short-id/meta.json",
        b"{\"child_session_id\":\"child\"}",
    );
    assert!(
        NativeDocuments::open(&f.0, &grok).is_ok(),
        "parent disk metadata may prove the exact child"
    );
    f.write(
        "sessions/group-a/parent/subagents/short-id/meta.json",
        b"{\"child_session_id\":\"child\",\"parent_session_id\":\"foreign-parent\"}",
    );
    assert!(NativeDocuments::open(&f.0, &grok).is_err());
    f.write(
        "sessions/group-a/parent/subagents/short-id/meta.json",
        b"{\"subagent_id\":\"child\"}",
    );
    assert!(
        NativeDocuments::open(&f.0, &grok).is_err(),
        "short ids are never Session fallback"
    );
    assert!(NativeDocuments::open(Path::new("relative"), &grok).is_err());
    let mut bad = grok.clone();
    bad.child_native_session_id = "../child".into();
    assert!(NativeDocuments::open(&f.0, &bad).is_err());
    assert!(open_contained(&f.0, &f.0.join("sessions/../outside")).is_err());
    crate::runtime_subagent_session::content::tests::assert_content_projection(&f.0,&grok);
    #[cfg(unix)]
    {
        let external = SourceFixture::new();
        external.write("outside", b"private unrelated bytes");
        std::os::unix::fs::symlink(external.0.join("outside"), f.0.join("sessions/link.jsonl"))
            .unwrap();
        assert!(open_contained(&f.0, &f.0.join("sessions/link.jsonl")).is_err());
    }
}
