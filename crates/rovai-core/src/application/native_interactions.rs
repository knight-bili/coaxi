//! Trusted ACP callback bridge. SQLite owns business state; this task owns only
//! the original generation-local Runtime handle and prompt writer correlation.
use super::*;
use crate::{
    acp::user_reply::{Outcome, Target},
    runtime_interaction::storage::{self, NativeFrame},
};
use anyhow::ensure;
use rusqlite::params;

#[allow(clippy::too_many_arguments)]
pub(super) async fn capture(
    core: &Arc<Core>,
    runtime: Arc<AcpRuntime>,
    target: Target,
    rpc_id: Value,
    method: &str,
    sequence: u64,
    params: &Value,
) -> Result<()> {
    ensure!(
        runtime.user_reply_target_is_current(&target).await,
        "interaction.native_route_not_live"
    );
    let generation = core.planned_shutdown.generation().to_string();
    let captured = {
        let mut db = core.database.lock().await;
        (|| -> Result<String> {
            let (binding,binding_generation):(String,i64)=db.connection().query_row(
            "SELECT c.native_binding_id,c.native_binding_generation FROM agent_run r JOIN conversation c ON c.id=r.conversation_id
             WHERE r.id=?1 AND r.execution_epoch=?2 AND c.native_session_id=?3",
            params![target.run,target.epoch,target.session],|r|Ok((r.get(0)?,r.get(1)?)))?;
            storage::capture(
                &mut db,
                &NativeFrame {
                    run: target.run.clone(),
                    epoch: target.epoch,
                    input_delivery: target.delivery.clone(),
                    core_generation: generation.clone(),
                    host_instance: target.host.clone(),
                    sequence: i64::try_from(sequence)?,
                    rpc_id: rpc_id.clone(),
                    method: method.into(),
                    session: target.session.clone(),
                    binding,
                    binding_generation,
                },
                params,
            )
        })()
    };
    let id = match captured {
        Ok(id) => id,
        Err(error) => {
            // A rejected duplicate RPC must not leave an older pending answer
            // capable of writing to the now-rejected native call.
            let mut db = core.database.lock().await;
            let prior: Option<String> = db
                .connection()
                .query_row(
                    "SELECT id FROM runtime_user_interaction
                WHERE core_generation=?1 AND host_instance_id=?2 AND native_request_id_json=?3
                  AND status IN ('pending','resolved','dispatching')",
                    params![generation, target.host, serde_json::to_string(&rpc_id)?],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(prior) = prior {
                storage::close_request(
                    &mut db,
                    &prior,
                    &generation,
                    &target.host,
                    "native_request_rejected",
                )?;
            }
            drop(db);
            let outcome = runtime
                .respond_user_interaction(
                    &target,
                    rpc_id,
                    Err("Core could not admit this native User request"),
                )
                .await;
            if outcome == Outcome::Unknown {
                // Do not retain the incoming callback's route permit while reaping.
                core.agent_run_tasks.lock().await.spawn(async move {
                    runtime.retire_failed_user_reply().await;
                });
            }
            return Err(error);
        }
    };
    changed(core, &id).await;
    let weak = Arc::downgrade(core);
    core.agent_run_tasks.lock().await.spawn(async move {
        drive(weak, runtime, target, generation, id).await;
    });
    Ok(())
}

async fn changed(core: &Core, id: &str) {
    let owner = {
        let db = core.database.lock().await;
        storage::owner(db.connection(), id)
    };
    if let Ok(Some((thread, conversation))) = owner {
        emit(
            &core.output,
            "runtime.interaction.changed",
            json!({"threadId":thread,"conversationId":conversation,"interactionId":id}),
        );
    }
}

async fn drive(
    weak: std::sync::Weak<Core>,
    runtime: Arc<AcpRuntime>,
    target: Target,
    generation: String,
    id: String,
) {
    loop {
        let Some(core) = weak.upgrade() else {
            return;
        };
        let wake = core.execution_wake.delegation.clone();
        let notified = wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if !runtime.user_reply_target_is_current(&target).await {
            let result = {
                let mut db = core.database.lock().await;
                storage::close_request(
                    &mut db,
                    &id,
                    &generation,
                    &target.host,
                    "native_route_closed",
                )
            };
            if result.is_ok() {
                changed(&core, &id).await;
                return;
            }
        } else {
            let state = {
                let db = core.database.lock().await;
                db.connection().query_row("SELECT status,version,followup_feedback IS NOT NULL FROM runtime_user_interaction
                    WHERE id=?1 AND core_generation=?2 AND host_instance_id=?3",params![id,generation,target.host],
                    |r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,bool>(2)?))).optional()
            };
            match state {
                Ok(None) => return,
                Ok(Some((status, _, _))) if !matches!(status.as_str(), "pending" | "resolved") => {
                    changed(&core, &id).await;
                    return;
                }
                Ok(Some((status, version, false))) if status == "resolved" => {
                    let Some(mut permit) = core.planned_shutdown.enter_runtime_route().await else {
                        let mut db = core.database.lock().await;
                        let _ = storage::close_request(
                            &mut db,
                            &id,
                            &generation,
                            &target.host,
                            "core_routes_closed",
                        );
                        return;
                    };
                    let reply = {
                        let mut db = core.database.lock().await;
                        storage::begin_dispatch(&mut db, &id, version, &generation, &target.host)
                    };
                    match reply {
                        Ok(reply) => {
                            let outcome = runtime
                                .respond_user_interaction(&target, reply.rpc_id, Ok(reply.response))
                                .await;
                            permit.complete_callback();
                            if outcome == Outcome::Unknown {
                                runtime.retire_failed_user_reply().await;
                            }
                            drop(core);
                            // Persist retries never repeat the native write. A crash
                            // before this receipt remains dispatching -> unknown.
                            loop {
                                let Some(core) = weak.upgrade() else {
                                    return;
                                };
                                let result = {
                                    let mut db = core.database.lock().await;
                                    storage::finish_dispatch(
                                        &mut db,
                                        &id,
                                        &reply.token,
                                        outcome == Outcome::Sent,
                                    )
                                };
                                if result.is_ok() {
                                    changed(&core, &id).await;
                                    return;
                                }
                                drop(core);
                                tokio::time::sleep(Duration::from_secs(1)).await;
                            }
                        }
                        Err(error) if error.to_string() == "interaction.scope_not_live" => {
                            let mut db = core.database.lock().await;
                            let _ = storage::close_request(
                                &mut db,
                                &id,
                                &generation,
                                &target.host,
                                "native_dispatch_scope_closed",
                            );
                            return;
                        }
                        Err(_) => {} // Stale competing task or a retryable store failure; never write here.
                    }
                }
                _ => {} // Grok feedback waits for the original Core User-queue proof.
            }
        }
        drop(core);
        // Normal wake is durable command/Run state change. The bounded fallback
        // notices a lost transport even if its final notification could not queue.
        tokio::select! {_=notified=>{},_=tokio::time::sleep(Duration::from_secs(5))=>{}}
    }
}
