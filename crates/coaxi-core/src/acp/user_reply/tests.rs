use super::*;
use std::{
    future::Future,
    pin::Pin,
    task::{Context as TaskContext, Poll},
};

struct MemoryWriter {
    bytes: Arc<std::sync::Mutex<Vec<u8>>>,
    stop_after: Option<usize>,
    fail_flush: bool,
}
impl AsyncWrite for MemoryWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut bytes = self.bytes.lock().unwrap();
        if self.stop_after.is_some_and(|n| bytes.len() >= n) {
            return Poll::Pending;
        }
        let count = self
            .stop_after
            .map_or(buf.len(), |n| buf.len().min(n - bytes.len()));
        bytes.extend_from_slice(&buf[..count]);
        Poll::Ready(Ok(count))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(if self.fail_flush {
            Err(std::io::Error::other("fixture failed flush"))
        } else {
            Ok(())
        })
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

fn route() -> (
    Target,
    AcpRuntimeOwner,
    RwLock<HashMap<String, AcpSessionRoute>>,
) {
    let target = Target {
        host: "host-one".into(),
        run: "run-one".into(),
        epoch: 1,
        session: "session-one".into(),
        prompt: "prompt-one".into(),
        delivery: "delivery-one".into(),
    };
    let owner = AcpRuntimeOwner {
        agent_run_id: target.run.clone(),
        execution_epoch: target.epoch,
    };
    let routes = RwLock::new(HashMap::from([(
        target.session.clone(),
        AcpSessionRoute {
            owner: owner.clone(),
            phase: AcpSessionPhase::PromptActive(AcpActivePrompt {
                prompt_id: target.prompt.clone(),
                delivery_id: target.delivery.clone(),
                acceptance_emitted: false,
                prompt_activity_observed: false,
                kimi_compaction_lifecycle: KimiCompactionLifecycle::Idle,
                zcode_input_id: None,
            }),
            sequence: 1,
            client_terminal_create_cancelled: false,
        },
    )]));
    (target, owner, routes)
}

// Lowest native writer owner: Tokio memory I/O only. Existing protocol tests do
// not own serialized late-response fencing and the old Cursor process owner is Unix-only.
#[tokio::test]
async fn native_user_reply_rechecks_the_exact_prompt_at_the_writer_and_poisoned_frames_never_continue()
 {
    let (target, owner, routes) = route();
    let alive = AtomicBool::new(true);
    let violated = AtomicBool::new(false);
    let bytes = Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = Mutex::new(Box::new(MemoryWriter {
        bytes: bytes.clone(),
        stop_after: None,
        fail_flush: false,
    }));
    let message =
        json!({"jsonrpc":"2.0","id":" rpc preserved ","result":{"answers":{"选择？":"保持原文"}}});
    for key in ["host", "run", "epoch", "session", "prompt", "delivery"] {
        let mut wrong = target.clone();
        match key {
            "host" => wrong.host.push('x'),
            "run" => wrong.run.push('x'),
            "epoch" => wrong.epoch += 1,
            "session" => wrong.session.push('x'),
            "prompt" => wrong.prompt.push('x'),
            _ => wrong.delivery.push('x'),
        }
        assert_eq!(
            write(
                &writer,
                &routes,
                &alive,
                &violated,
                "host-one",
                &owner,
                &wrong,
                message.clone(),
                Duration::from_secs(1)
            )
            .await,
            Outcome::Fenced,
            "{key}"
        );
        assert!(bytes.lock().unwrap().is_empty());
    }
    // Explicitly poll while another writer owns the mutex, then replace the route.
    let held = writer.lock().await;
    let pending = write(
        &writer,
        &routes,
        &alive,
        &violated,
        "host-one",
        &owner,
        &target,
        message.clone(),
        Duration::from_secs(1),
    );
    tokio::pin!(pending);
    std::future::poll_fn(|cx| {
        assert!(pending.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    routes
        .write()
        .await
        .get_mut(&target.session)
        .unwrap()
        .owner
        .execution_epoch = 2;
    drop(held);
    assert_eq!(pending.await, Outcome::Fenced);
    assert!(bytes.lock().unwrap().is_empty());
    routes.write().await.get_mut(&target.session).unwrap().owner = owner.clone();
    // Completed prompts remain valid evidence, but must never accept a User reply.
    let phase = routes.read().await[&target.session].phase.clone();
    if let AcpSessionPhase::PromptActive(prompt) = &phase {
        routes.write().await.get_mut(&target.session).unwrap().phase =
            AcpSessionPhase::PromptCompleted(prompt.clone());
    }
    assert_eq!(
        write(
            &writer,
            &routes,
            &alive,
            &violated,
            "host-one",
            &owner,
            &target,
            message.clone(),
            Duration::from_secs(1)
        )
        .await,
        Outcome::Fenced
    );
    routes.write().await.get_mut(&target.session).unwrap().phase = phase;
    assert_eq!(
        write(
            &writer,
            &routes,
            &alive,
            &violated,
            "host-one",
            &owner,
            &target,
            message.clone(),
            Duration::from_secs(1)
        )
        .await,
        Outcome::Sent
    );
    let frame = bytes.lock().unwrap().clone();
    assert_eq!(frame.last(), Some(&b'\n'));
    assert_eq!(frame.iter().filter(|b| **b == b'\n').count(), 1);
    assert_eq!(serde_json::from_slice::<Value>(&frame).unwrap(), message);
    bytes.lock().unwrap().clear();
    for (limit, flush_error) in [(Some(9), false), (None, true)] {
        let alive = AtomicBool::new(true);
        let violated = AtomicBool::new(false);
        let writer = Mutex::new(Box::new(MemoryWriter {
            bytes: bytes.clone(),
            stop_after: limit,
            fail_flush: flush_error,
        }));
        assert_eq!(
            write(
                &writer,
                &routes,
                &alive,
                &violated,
                "host-one",
                &owner,
                &target,
                message.clone(),
                Duration::from_millis(10)
            )
            .await,
            Outcome::Unknown
        );
        assert!(alive.load(Ordering::Acquire) && violated.load(Ordering::Acquire));
        let count = bytes.lock().unwrap().len();
        assert!(count > 0);
        assert_eq!(
            write(
                &writer,
                &routes,
                &alive,
                &violated,
                "host-one",
                &owner,
                &target,
                message.clone(),
                Duration::from_secs(1)
            )
            .await,
            Outcome::Fenced
        );
        assert_eq!(bytes.lock().unwrap().len(), count);
        bytes.lock().unwrap().clear();
    }
}
