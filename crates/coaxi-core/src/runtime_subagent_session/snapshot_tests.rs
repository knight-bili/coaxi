// Extends the existing source filesystem owner. No separate test process,
// database, model, or claim that a public transport has already been connected.
use super::*;
use serde_json::{Value, json};

pub(in crate::runtime_subagent_session) fn assert_signed_windows(
    target: &ObservedNativeSessionTarget,
) {
    let actor = ActorRef::User {
        user_id: CURRENT_USER_ID.into(),
    };
    let stop = AtomicBool::new(false);
    let long = format!("头{}尾\n\u{0}\\\"", "中文😀".repeat(20_000));
    let tool = json!({"nested":{"body":long,"array":[true, null, 42]}}).to_string();
    let turn: model::MessageTurn = serde_json::from_value(json!({
        "id":"turn-1", "role":"assistant", "timestamp":"2026-10-18T00:00:00Z",
        "blocks":[{"type":"text","text":long}, {"type":"thinking","text":"完整思考😀"},
            {"type":"tool_use","tool_use_id":"call-1","tool_name":"read",
                "input_preview":tool,"status":"completed","meta":{"nested":{"complete":long}}},
            {"type":"tool_result","tool_use_id":"call-1","output_preview":tool,"is_error":false,
                "agent_stats":{"child_session_id":"verified-grandchild","recovered_result":long}},
            {"type":"tool_result","tool_use_id":"call-2","output_preview":"unverified",
                "is_error":false,"agent_stats":{"child_session_id":"unverified-native-id"}}],
        "usage":{"input_tokens":11,"output_tokens":12,"cache_creation_input_tokens":13,"cache_read_input_tokens":14},
        "duration_ms":9000,"model":"source-model","completed_at":"2026-10-18T00:00:09Z",
        "agent_message_id":"original-message"
    })).unwrap();
    let stats: model::SessionStats = serde_json::from_value(json!({
        "total_usage":{"input_tokens":11,"output_tokens":12,"cache_creation_input_tokens":13,"cache_read_input_tokens":14},
        "total_tokens":50,"total_duration_ms":9000,"context_window_used_tokens":100,
        "context_window_max_tokens":200,"context_window_usage_percent":50.0
    })).unwrap();
    let summary = model::NativeTranscriptSummary {
        title: Some(long.clone()),
        model: Some("source-model".into()),
        git_branch: Some("feature/source".into()),
        started_at: turn.timestamp,
        ended_at: turn.completed_at,
        message_count: 1,
    };
    let expected_turns = serde_json::to_value([&turn]).unwrap();
    let expected_stats = serde_json::to_value(&stats).unwrap();
    let expected_summary = serde_json::to_value(&summary).unwrap();
    let content = || NativeContent {
        turns: vec![turn.clone()],
        context_tokens: Some(100),
        session_stats: Some(stats.clone()),
        summary: Some(summary.clone()),
        snapshot_digest: "full-source-digest".into(),
        notes: super::super::content::ParseNotes {
            malformed_records: 2,
            incomplete_tail: true,
            unavailable_children: 3,
            unavailable_advisories: 4,
        },
        verified_children: vec!["verified-grandchild".into(), "verified-grandchild".into()],
    };
    let snapshot = NativeSnapshot::new(&actor, target, content(), &stop).unwrap();
    let reference = snapshot.reference().unwrap();
    let mut cursor: Option<String> = None;
    let mut joined = String::new();
    loop {
        let page = snapshot
            .window(
                &actor,
                target,
                &reference,
                cursor.as_deref(),
                MAX_WINDOW_BYTES,
            )
            .unwrap();
        assert_eq!(page.offset, joined.len() as u64);
        assert_eq!(page.source_digest, "full-source-digest");
        assert_eq!(page.projection_digest, snapshot.digest);
        assert!(page.fragment.len() <= MAX_WINDOW_BYTES);
        assert!(serde_json::to_vec(&page).unwrap().len() <= 6 * MAX_WINDOW_BYTES + 4096);
        assert!(!page.fragment.is_empty());
        assert_eq!(page.complete, page.next_cursor.is_none());
        joined.push_str(&page.fragment);
        cursor = page.next_cursor;
        if page.complete {
            assert_eq!(page.total_bytes, joined.len() as u64);
            break;
        }
    }
    assert_eq!(
        format!("{:x}", Sha256::digest(joined.as_bytes())),
        snapshot.digest
    );
    let view: Value = serde_json::from_str(&joined).unwrap();
    assert_eq!(view["turns"], expected_turns);
    assert_eq!(view["session_stats"], expected_stats);
    assert_eq!(view["summary"], expected_summary);
    assert_eq!(view["context_tokens"], 100);
    assert_eq!(
        view["notes"],
        json!({"malformed_records":2,"incomplete_tail":true,
        "unavailable_children":3,"unavailable_advisories":4})
    );
    assert_eq!(view["child_references"].as_array().unwrap().len(), 1);
    let child_ref = view["child_references"][0]["reference"].as_str().unwrap();
    assert_eq!(
        view["child_references"][0]["source_id"],
        "verified-grandchild"
    );
    let child = snapshot.descendant(&actor, target, child_ref).unwrap();
    assert_eq!(
        child.parent_native_session_id,
        target.child_native_session_id
    );
    assert_eq!(child.child_native_session_id, "verified-grandchild");
    assert_eq!(child.status, "unknown");
    assert!(!child.live);
    assert_eq!(child.relationship, "source_verified_descendant");
    assert_eq!(child.recorded_home, target.recorded_home);
    assert_eq!(
        child.recorded_working_directory,
        target.recorded_working_directory
    );

    let first = snapshot
        .window(&actor, target, &reference, None, 32)
        .unwrap();
    let again = snapshot
        .window(&actor, target, &reference, None, 32)
        .unwrap();
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(again).unwrap()
    );
    let mut terminal = target.clone();
    terminal.status = "completed".into();
    terminal.live = false;
    let page = snapshot
        .window(&actor, &terminal, &reference, None, 32)
        .unwrap();
    assert_eq!(page.reported_status, "completed");
    assert!(!page.live);
    assert_eq!(
        snapshot
            .descendant(&actor, &terminal, child_ref)
            .unwrap()
            .status,
        "unknown"
    );
    for user in [
        ActorRef::User {
            user_id: "other-user".into(),
        },
        ActorRef::Agent {
            agent_id: "agent".into(),
            source_agent_run_id: "run".into(),
        },
        ActorRef::System {
            component_id: "host".into(),
        },
    ] {
        assert!(
            snapshot
                .window(&user, target, &reference, None, 32)
                .is_err()
        );
        assert!(snapshot.descendant(&user, target, child_ref).is_err());
        assert!(NativeSnapshot::new(&user, target, content(), &stop).is_err());
    }
    let mut changed = Vec::new();
    macro_rules! change {
        ($field:ident, $value:expr) => {{
            let mut value = target.clone();
            value.$field = $value;
            changed.push(value);
        }};
    }
    change!(thread_id, "another-thread".into());
    change!(conversation_id, "another-conversation".into());
    change!(agent_run_id, "another-run".into());
    change!(evidence_id, "another-evidence".into());
    change!(execution_epoch, target.execution_epoch + 1);
    change!(adapter_kind, "another-adapter".into());
    change!(installation_id, "another-installation".into());
    change!(binding_id, "another-binding".into());
    change!(binding_generation, target.binding_generation + 1);
    change!(parent_native_session_id, "another-parent".into());
    change!(child_native_session_id, "another-child".into());
    change!(
        recorded_home,
        Some(std::path::PathBuf::from("C:/different-home"))
    );
    change!(
        recorded_working_directory,
        Some(std::path::PathBuf::from("C:/different-project"))
    );
    for different in changed {
        assert!(
            snapshot
                .window(&actor, &different, &reference, None, 32)
                .is_err()
        );
        assert!(snapshot.descendant(&actor, &different, child_ref).is_err());
    }
    let other = NativeSnapshot::new(&actor, target, content(), &stop).unwrap();
    let cursor = snapshot
        .window(&actor, target, &reference, None, 32)
        .unwrap()
        .next_cursor
        .unwrap();
    assert!(
        other
            .window(&actor, target, &reference, Some(&cursor), 32)
            .is_err()
    );
    assert!(
        snapshot
            .window(
                &actor,
                target,
                &other.reference().unwrap(),
                Some(&cursor),
                32
            )
            .is_err()
    );
    assert!(
        other
            .window(
                &actor,
                target,
                &other.reference().unwrap(),
                Some(&cursor),
                32
            )
            .is_err()
    );
    assert!(other.descendant(&actor, target, child_ref).is_err());
    assert!(
        snapshot
            .window(&actor, target, child_ref, None, 32)
            .is_err()
    );
    assert!(
        snapshot
            .window(&actor, target, &reference, Some(&reference), 32)
            .is_err()
    );
    assert!(snapshot.descendant(&actor, target, &reference).is_err());
    for limit in [0, 3, MAX_WINDOW_BYTES + 1, usize::MAX] {
        assert!(
            snapshot
                .window(&actor, target, &reference, None, limit)
                .is_err()
        );
    }
    let mut tampered = reference.clone().into_bytes();
    tampered[10] = if tampered[10] == b'A' { b'B' } else { b'A' };
    for bad in [
        String::from_utf8(tampered).unwrap(),
        "x".repeat(4097),
        "x.y.z".into(),
    ] {
        assert!(snapshot.window(&actor, target, &bad, None, 32).is_err());
        assert!(
            snapshot
                .window(&actor, target, &reference, Some(&bad), 32)
                .is_err()
        );
        assert!(snapshot.descendant(&actor, target, &bad).is_err());
    }
    let non_boundary = snapshot
        .projection
        .char_indices()
        .find(|(_, c)| c.len_utf8() > 1)
        .unwrap()
        .0
        + 1;
    for offset in [non_boundary as u64, u64::MAX] {
        let bad = encode(&Token::Window {
            v: 1,
            id: snapshot.id.clone(),
            scope: snapshot.scope.clone(),
            digest: snapshot.digest.clone(),
            offset,
        })
        .unwrap();
        assert!(
            snapshot
                .window(&actor, target, &reference, Some(&bad), 32)
                .is_err()
        );
    }
    let bad = encode(&Token::Descendant {
        v: 1,
        id: snapshot.id.clone(),
        scope: snapshot.scope.clone(),
        index: 1,
    })
    .unwrap();
    assert!(snapshot.descendant(&actor, target, &bad).is_err());
    let body = URL_SAFE_NO_PAD
        .decode(reference.split_once('.').unwrap().0)
        .unwrap();
    let token: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(token.as_object().unwrap().len(), 5);
    assert!(token.get("path").is_none() && token.get("native_session_id").is_none());
    let mut extended = token;
    extended["native_session_id"] = "unverified-native-id".into();
    let payload = serde_json::to_vec(&extended).unwrap();
    let bad = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(&payload),
        URL_SAFE_NO_PAD.encode(ring::hmac::sign(&key().unwrap(), &payload).as_ref())
    );
    assert!(snapshot.window(&actor, target, &bad, None, 32).is_err());

    let mut invalid = content();
    invalid
        .verified_children
        .push(target.child_native_session_id.clone());
    assert!(NativeSnapshot::new(&actor, target, invalid, &stop).is_err());
    let mut invalid = content();
    invalid.verified_children.push("../escape".into());
    assert!(NativeSnapshot::new(&actor, target, invalid, &stop).is_err());
    stop.store(true, Ordering::Release);
    assert!(NativeSnapshot::new(&actor, target, content(), &stop).is_err());
    stop.store(false, Ordering::Release);
    let mut writer = ProjectionWriter {
        bytes: Vec::new(),
        digest: Sha256::new(),
        stop: &stop,
    };
    writer.write_all("保留第一块😀".as_bytes()).unwrap();
    let saved = writer.bytes.clone();
    stop.store(true, Ordering::Release);
    assert!(writer.write_all(long.as_bytes()).is_err());
    assert_eq!(writer.bytes, saved);

    // Four-byte windows still make progress across Unicode, JSON escapes and
    // empty collections. This small projection avoids thousands of hash calls.
    stop.store(false, Ordering::Release);
    let small = NativeSnapshot::new(
        &actor,
        target,
        NativeContent {
            turns: vec![],
            context_tokens: None,
            session_stats: None,
            summary: Some(model::NativeTranscriptSummary {
                title: Some("中文😀\\\"\n".into()),
                model: None,
                git_branch: None,
                started_at: turn.timestamp,
                ended_at: None,
                message_count: 0,
            }),
            snapshot_digest: "small".into(),
            notes: Default::default(),
            verified_children: vec![],
        },
        &stop,
    )
    .unwrap();
    let reference = small.reference().unwrap();
    let mut cursor = None;
    let mut joined = String::new();
    loop {
        let page = small
            .window(&actor, target, &reference, cursor.as_deref(), 4)
            .unwrap();
        assert!(!page.fragment.is_empty() && page.fragment.len() <= 4);
        joined.push_str(&page.fragment);
        cursor = page.next_cursor;
        if page.complete {
            break;
        }
    }
    assert_eq!(joined, small.projection);
    assert_eq!(
        serde_json::from_str::<Value>(&joined).unwrap()["turns"],
        json!([])
    );
}
