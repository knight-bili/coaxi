// Same source filesystem owner, including real source->worker->page. Channels
// hold workers at exact boundaries; no scheduler sleeps or external Runtime.
use super::*;
use std::sync::mpsc;

fn request(root: &ObservedNativeSessionTarget) -> StartRead {
    StartRead {
        read_id: uuid::Uuid::new_v4().to_string(),
        reference: NativeChildSessionReference {
            thread_id: root.thread_id.clone(),
            agent_run_id: root.agent_run_id.clone(),
            evidence_id: root.evidence_id.clone(),
            receiver_index: 0,
        },
        descendant: None,
    }
}
fn empty_snapshot(target: &ObservedNativeSessionTarget, children: &[&str]) -> NativeSnapshot {
    NativeSnapshot::new(
        &ActorRef::User {
            user_id: CURRENT_USER_ID.into(),
        },
        target,
        content::NativeContent {
            turns: vec![],
            context_tokens: None,
            session_stats: None,
            summary: None,
            snapshot_digest: "controlled-source".into(),
            notes: Default::default(),
            verified_children: children.iter().map(|v| (*v).into()).collect(),
        },
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn settled(registry: &Registry) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let permits = runtime.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(3),
            registry
                .workers
                .clone()
                .acquire_many_owned(MAX_WORKERS as u32),
        )
        .await
        .unwrap()
        .unwrap()
    });
    drop(permits);
}
fn page(request: &StartRead, snapshot: String) -> PageRead {
    PageRead {
        read_id: request.read_id.clone(),
        snapshot_reference: snapshot,
        cursor: None,
        max_bytes: None,
    }
}

fn fallback() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-10-17T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

pub(in crate::runtime_subagent_session) fn assert_read_workers(root: &ObservedNativeSessionTarget) {
    let actor = ActorRef::User {
        user_id: CURRENT_USER_ID.into(),
    };
    let registry = Registry::new("core-generation".into());
    let first = request(root);
    registry
        .start(&actor, first.clone(), root, fallback())
        .unwrap();
    settled(&registry);
    let view = registry.poll(&actor, &first.read_id, root).unwrap();
    assert_eq!(view.state, "ready");
    assert_eq!(view.core_generation, "core-generation");
    assert!(
        registry.file_target(&actor, &first.read_id, root).is_err(),
        "old records without a captured working directory cannot acquire today's directory"
    );
    let mut captured = root.clone();
    captured.recorded_working_directory = root.recorded_home.clone();
    let file_registry = Registry::new("file-generation".into());
    let file_read = request(&captured);
    file_registry
        .start_with(file_read.clone(), &captured, &actor, |target, _| {
            Ok(empty_snapshot(&target, &[]))
        })
        .unwrap();
    settled(&file_registry);
    assert_eq!(
        file_registry
            .file_target(&actor, &file_read.read_id, &captured)
            .unwrap()
            .recorded_working_directory,
        captured.recorded_working_directory
    );
    let mut wrong_root = captured.clone();
    wrong_root.binding_generation += 1;
    assert!(
        file_registry
            .file_target(&actor, &file_read.read_id, &wrong_root)
            .is_err()
    );
    file_registry.close(&actor, &file_read.read_id).unwrap();
    assert!(
        file_registry
            .file_target(&actor, &file_read.read_id, &captured)
            .is_err()
    );
    let reference = view.snapshot_reference.unwrap();
    let body = registry
        .page(&actor, page(&first, reference.clone()), root)
        .unwrap();
    assert!(body.fragment.contains("complete codex child"));
    let projected: serde_json::Value = serde_json::from_str(&body.fragment).unwrap();
    assert_eq!(projected["turns"][0]["timestamp"], "2026-10-17T00:00:00Z");
    assert!(body.complete);
    assert_eq!(
        registry.reference(&actor, &first.read_id).unwrap(),
        first.reference
    );
    let replay = registry
        .start_with(first.clone(), root, &actor, |_, _| {
            panic!("a retry must not create a second worker")
        })
        .unwrap();
    assert_eq!(
        replay.snapshot_reference.as_deref(),
        Some(reference.as_str())
    );
    let mut changed = root.clone();
    changed.status = "completed".into();
    changed.live = false;
    let historical = registry
        .page(&actor, page(&first, reference.clone()), &changed)
        .unwrap();
    assert!(!historical.live);
    assert_eq!(historical.reported_status, "completed");
    changed.binding_generation += 1;
    assert!(registry.poll(&actor, &first.read_id, &changed).is_err());
    assert!(
        registry
            .page(&actor, page(&first, reference.clone()), &changed)
            .is_err()
    );
    assert!(
        registry
            .start_with(first.clone(), &changed, &actor, |_, _| panic!(
                "changed scope cannot start"
            ))
            .is_err()
    );
    for bad in [
        ActorRef::User {
            user_id: "other-user".into(),
        },
        ActorRef::Agent {
            agent_id: "agent".into(),
            source_agent_run_id: "run".into(),
        },
        ActorRef::System {
            component_id: "runtime".into(),
        },
    ] {
        assert!(
            registry
                .start(&bad, first.clone(), root, fallback())
                .is_err()
        );
        assert!(registry.reference(&bad, &first.read_id).is_err());
        assert!(registry.poll(&bad, &first.read_id, root).is_err());
        assert!(
            registry
                .page(&bad, page(&first, reference.clone()), root)
                .is_err()
        );
        assert!(registry.close(&bad, &first.read_id).is_err());
        assert!(registry.file_target(&bad, &first.read_id, root).is_err());
    }
    let mut invalid = first.clone();
    invalid.read_id = "arbitrary-source-path".into();
    assert!(registry.start(&actor, invalid, root, fallback()).is_err());
    for value in [
        serde_json::json!({"readId":first.read_id,"actor":{"type":"system"}}),
        serde_json::json!({"readId":first.read_id,"path":"private-source"}),
    ] {
        assert!(serde_json::from_value::<ReadId>(value).is_err());
    }
    assert!(serde_json::from_value::<StartRead>(serde_json::json!({"readId":first.read_id,"reference":{
        "threadId":"thread","agentRunId":"run","evidenceId":"evidence","receiverIndex":0,"nativeSessionId":"arbitrary"}})).is_err());
    assert!(
        serde_json::from_value::<PageRead>(
            serde_json::json!({"readId":first.read_id,"snapshotReference":reference,"maxBytes":-1})
        )
        .is_err()
    );

    // Close-before-start and late retries cannot resurrect that UUID during its
    // lease. A failed request with a new UUID can still be observed explicitly.
    let closed = request(root);
    registry.close(&actor, &closed.read_id).unwrap();
    registry.close(&actor, &closed.read_id).unwrap();
    assert!(
        registry
            .start(&actor, closed, root, fallback())
            .err()
            .unwrap()
            .to_string()
            .contains("read_closed")
    );
    let failed = request(root);
    registry
        .start_with(failed.clone(), root, &actor, |_, _| {
            anyhow::bail!("secret-source-path and tool body")
        })
        .unwrap();
    settled(&registry);
    let view = registry.poll(&actor, &failed.read_id, root).unwrap();
    assert_eq!(view.state, "failed");
    assert_eq!(view.code, Some("native_child.read_failed"));
    assert!(
        !serde_json::to_string(&view)
            .unwrap()
            .contains("secret-source")
    );

    // A descendant may outlive its parent viewer, but it retains the original
    // Core root authority. It never accepts a raw Session ID as a substitute.
    let parent = request(root);
    registry
        .start_with(parent.clone(), root, &actor, |target, _| {
            Ok(empty_snapshot(&target, &["grandchild"]))
        })
        .unwrap();
    settled(&registry);
    let reference = registry
        .poll(&actor, &parent.read_id, root)
        .unwrap()
        .snapshot_reference
        .unwrap();
    let body = registry
        .page(&actor, page(&parent, reference), root)
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&body.fragment).unwrap();
    let token = value["child_references"][0]["reference"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut child = request(root);
    child.descendant = Some(DescendantRead {
        parent_read_id: parent.read_id.clone(),
        descendant_reference: token,
    });
    registry
        .start_with(child.clone(), root, &actor, |target, _| {
            assert_eq!(target.child_native_session_id, "grandchild");
            assert_eq!(target.parent_native_session_id, "child");
            assert!(!target.live);
            assert_eq!(target.status, "unknown");
            Ok(empty_snapshot(&target, &[]))
        })
        .unwrap();
    settled(&registry);
    registry.close(&actor, &parent.read_id).unwrap();
    assert!(registry.poll(&actor, &child.read_id, root).is_ok());
    assert!(registry.poll(&actor, &child.read_id, &changed).is_err());
    registry
        .start_with(child.clone(), root, &actor, |_, _| {
            panic!("child retry does not require an open parent viewer")
        })
        .unwrap();
    let mut forged = request(root);
    forged.descendant = Some(DescendantRead {
        parent_read_id: child.read_id.clone(),
        descendant_reference: "grandchild".into(),
    });
    assert!(registry.start(&actor, forged, root, fallback()).is_err());
    registry.close(&actor, &first.read_id).unwrap();
    registry.close(&actor, &failed.read_id).unwrap();
    registry.close(&actor, &child.read_id).unwrap();

    // Closing four blocked workers removes their UI entries but no permit is
    // released until the real worker exits. Late results have nowhere to publish.
    let mut controls = Vec::new();
    let mut stops = Vec::new();
    let mut ids = Vec::new();
    for _ in 0..MAX_WORKERS {
        let (started, receive_start) = mpsc::sync_channel(0);
        let (release, blocked) = mpsc::channel();
        let request = request(root);
        ids.push(request.read_id.clone());
        let view = registry
            .start_with(request, root, &actor, move |target, stop| {
                started.send(stop).unwrap();
                blocked.recv().unwrap();
                // Deliberately return a late success, ignoring stop, to verify the
                // registry publication fence rather than relying on parser behavior.
                Ok(empty_snapshot(&target, &[]))
            })
            .unwrap();
        assert_eq!(view.state, "reading");
        assert!(
            registry
                .file_target(&actor, ids.last().unwrap(), root)
                .is_err()
        );
        stops.push(receive_start.recv_timeout(Duration::from_secs(3)).unwrap());
        controls.push(release);
    }
    for id in &ids {
        registry.close(&actor, id).unwrap();
    }
    assert!(stops.iter().all(|stop| stop.load(Ordering::Acquire)));
    assert_eq!(registry.workers.available_permits(), 0);
    assert!(
        registry
            .start(&actor, request(root), root, fallback())
            .err()
            .unwrap()
            .to_string()
            .contains("reader_busy")
    );
    for release in controls {
        release.send(()).unwrap();
    }
    settled(&registry);
    assert_eq!(registry.workers.available_permits(), MAX_WORKERS);
    for id in ids {
        assert!(registry.reference(&actor, &id).is_err());
    }

    // Completed snapshots are also bounded, expire on access, and release their
    // last registry reference on close. No timer or wall-clock waiting is needed.
    let cache = Registry::new("cache-generation".into());
    let mut cached = Vec::new();
    for _ in 0..MAX_READERS {
        let request = request(root);
        cache
            .start_with(request.clone(), root, &actor, |target, _| {
                Ok(empty_snapshot(&target, &[]))
            })
            .unwrap();
        settled(&cache);
        cached.push(request);
    }
    assert!(
        cache
            .start(&actor, request(root), root, fallback())
            .err()
            .unwrap()
            .to_string()
            .contains("reader_limit")
    );
    let entry = cache
        .entries()
        .unwrap()
        .get(&cached[0].read_id)
        .unwrap()
        .clone();
    let weak = match &entry.data().unwrap().state {
        State::Ready(snapshot) => Arc::downgrade(snapshot),
        _ => panic!("ready"),
    };
    entry.data().unwrap().last_used = Instant::now() - IDLE_LIMIT - Duration::from_secs(1);
    assert!(cache.reference(&actor, &cached[0].read_id).is_err());
    assert!(entry.stop.load(Ordering::Acquire));
    drop(entry);
    assert!(weak.upgrade().is_none());
    let after_expiry = request(root);
    cache
        .start_with(after_expiry.clone(), root, &actor, |target, _| {
            Ok(empty_snapshot(&target, &[]))
        })
        .unwrap();
    settled(&cache);
    cache.begin_shutdown();
    assert!(cache.entries().unwrap().is_empty());
    assert!(
        cache
            .start(&actor, request(root), root, fallback())
            .is_err()
    );
    assert!(cache.reference(&actor, &after_expiry.read_id).is_err());
    assert!(cache.close(&actor, &after_expiry.read_id).is_ok());
    let leases = Registry::new("leases".into());
    let kept = request(root);
    leases
        .start_with(kept.clone(), root, &actor, |target, _| {
            Ok(empty_snapshot(&target, &[]))
        })
        .unwrap();
    settled(&leases);
    for _ in 0..MAX_CLOSED {
        leases
            .close(&actor, &uuid::Uuid::new_v4().to_string())
            .unwrap();
    }
    // Saturation blocks new work, not a replay of an already accepted job.
    leases
        .start_with(kept.clone(), root, &actor, |_, _| {
            panic!("saturated retry must not reread")
        })
        .unwrap();
    assert!(
        leases
            .start(&actor, request(root), root, fallback())
            .is_err()
    );
    leases.close(&actor, &kept.read_id).unwrap();
    assert!(leases.closed.lock().unwrap().len() <= MAX_CLOSED + MAX_READERS);
    for at in leases.closed.lock().unwrap().values_mut() {
        *at = Instant::now() - IDLE_LIMIT - Duration::from_secs(1);
    }
    let renewed = request(root);
    leases
        .start_with(renewed.clone(), root, &actor, |target, _| {
            Ok(empty_snapshot(&target, &[]))
        })
        .unwrap();
    settled(&leases);
    leases.close(&actor, &renewed.read_id).unwrap();
    assert_eq!(
        failure(&anyhow::anyhow!("native_child.read_cancelled")),
        "native_child.read_cancelled"
    );
}
