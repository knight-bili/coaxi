//! Exact live-prompt response at the serialized native writer. No User/JSON constructor.
use super::*;

#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub host: String,
    pub run: String,
    pub epoch: i64,
    pub session: String,
    pub prompt: String,
    pub delivery: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Sent,
    Fenced,
    Unknown,
}

pub(super) fn route_matches(
    routes: &HashMap<String, AcpSessionRoute>,
    owner: &AcpRuntimeOwner,
    target: &Target,
) -> bool {
    owner.agent_run_id == target.run
        && owner.execution_epoch == target.epoch
        && routes.get(&target.session).is_some_and(|route| {
            &route.owner == owner
                && matches!(&route.phase,AcpSessionPhase::PromptActive(prompt)
                if prompt.prompt_id==target.prompt && prompt.delivery_id==target.delivery)
        })
}

#[allow(clippy::too_many_arguments)]
struct IncompleteWrite<'a> {
    violated: &'a AtomicBool,
    complete: bool,
}
impl Drop for IncompleteWrite<'_> {
    fn drop(&mut self) {
        if !self.complete {
            self.violated.store(true, Ordering::Release);
        }
    }
}

pub(super) async fn write<W: AsyncWrite + Unpin + Send + ?Sized>(
    stdin: &Mutex<Box<W>>,
    routes: &RwLock<HashMap<String, AcpSessionRoute>>,
    alive: &AtomicBool,
    violated: &AtomicBool,
    host: &str,
    owner: &AcpRuntimeOwner,
    target: &Target,
    message: Value,
    budget: Duration,
) -> Outcome {
    let deadline = tokio::time::Instant::now() + budget;
    let outcome = tokio::time::timeout_at(deadline, async {
        // Match only after acquiring the same writer used by every Host frame.
        // Holding the route read guard until flush prevents detach/rebind from
        // crossing this write. No SQLite or ingress lock is held here.
        let mut writer = stdin.lock().await;
        let routes = routes.read().await;
        let live = alive.load(Ordering::Acquire)
            && !violated.load(Ordering::Acquire)
            && target.host == host
            && route_matches(&routes, owner, target);
        if !live {
            return Ok::<_, anyhow::Error>(Outcome::Fenced);
        }
        let mut bytes = serde_json::to_vec(&message)?;
        bytes.push(b'\n');
        // Drop before writer/routes on error or cancellation, so a queued writer
        // cannot append a new frame after a partially written response.
        let mut writing = IncompleteWrite {
            violated,
            complete: false,
        };
        writer.write_all(&bytes).await?;
        writer.flush().await?;
        writing.complete = true;
        Ok(Outcome::Sent)
    })
    .await;
    match outcome {
        Ok(Ok(outcome)) => outcome,
        _ => {
            // Includes a blocked writer, partial write, failed flush and timeout.
            // The Runtime retires this Host before another frame can be sent.
            violated.store(true, Ordering::Release);
            // Poison is not process-exit evidence. Keep the live process visible
            // to its existing cleanup owner until shutdown/reap actually runs.
            Outcome::Unknown
        }
    }
}

#[cfg(all(test, feature = "extended-tests"))]
mod tests;
