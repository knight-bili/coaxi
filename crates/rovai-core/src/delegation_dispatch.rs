//! The six approved operations share exact CLI lease fences. Filesystem workers
//! and status waits never retain the Core DB mutex or global invocation gate.
use std::{borrow::BorrowMut, future::Future, path::Path};

use anyhow::{Context, Result, ensure};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::{Mutex, MutexGuard};

use crate::{
    builtin_tool_runtime::{AuthorizedBuiltinToolInvocation, BuiltinToolLeaseRegistry},
    builtin_tool_transport::{
        BUILTIN_TOOL_IPC_PROTOCOL_VERSION, BuiltinToolAuth, BuiltinToolIpcRequest,
        BuiltinToolIpcRequestBody,
    },
    command::CommandExecution,
    db::Database,
    delegation_contract::{
        self as contract, CancelParams, HistoryParams, ReadParams, ResumeParams, StartParams,
        StatusParams,
    },
    delegation_model_command::{self as model, Invocation},
    delegation_model_query as query,
    managed_blob::ManagedBlobStore,
    team_tool::{AuthenticatedTeamToolRun, TeamToolService},
};

pub(crate) struct Outcome {
    pub operation: &'static str,
    pub payload: Value,
    pub replayed: bool,
}
impl Outcome {
    fn command(operation: &'static str, execution: CommandExecution) -> Self {
        // Approved business refusals are TaskReports, including stored refusals.
        Self {
            operation,
            payload: execution.result.payload,
            replayed: execution.replayed,
        }
    }
    fn query(operation: &'static str, payload: Value) -> Self {
        Self {
            operation,
            payload,
            replayed: false,
        }
    }
}

fn parse<T: DeserializeOwned>(input: &Value) -> Result<T> {
    serde_json::from_value(input.clone()).context("builtin_tool.invalid_input")
}

// No Debug/Serialize, no authority accepted from JSON. The immutable CLI auth
// tuple is retained so same-Run lease rotation during file IO still fences it.
struct ActiveInvocation<'a> {
    auth: &'a BuiltinToolAuth,
    authorized: AuthorizedBuiltinToolInvocation,
    source: AuthenticatedTeamToolRun,
    tool_call: String,
}
impl<'a> ActiveInvocation<'a> {
    fn new(
        auth: &'a BuiltinToolAuth,
        authorized: AuthorizedBuiltinToolInvocation,
        db: &Database,
        request_id: &str,
    ) -> Result<Self> {
        let call = format!(
            "agent-run:{}:builtin-cli:{request_id}",
            authorized.agent_run_id
        );
        let source = TeamToolService::default().authenticate_attested_binding(
            db,
            &authorized.native_binding.native_binding_id,
            &authorized.native_binding.binding_credential,
            &call,
            &authorized.agent_run_id,
            authorized.execution_epoch,
        )?;
        Ok(Self {
            auth,
            authorized,
            source,
            tool_call: call,
        })
    }
    fn invocation<'s, T>(&'s self, input: &'s T) -> Invocation<'s, T> {
        Invocation {
            binding: &self.authorized.native_binding,
            source: &self.source,
            tool_call: &self.tool_call,
            input,
        }
    }
    async fn guard<'s>(&self, leases: &'s BuiltinToolLeaseRegistry) -> Result<MutexGuard<'s, ()>> {
        let gate = leases.invocation_guard().await;
        // Each new bind generates a fresh lease id/token/generation. Exact
        // authentication under the same gate used by retirement pins this lease
        // until the short DB transaction completes; Native Binding alone cannot.
        leases
            .authenticate(self.auth)
            .await
            .map_err(|_| anyhow::anyhow!("builtin_tool.run_not_bound"))?;
        Ok(gate)
    }
    async fn commit_start<B: BorrowMut<Database>>(
        &self,
        database: &Mutex<B>,
        leases: &BuiltinToolLeaseRegistry,
        input: &StartParams,
        observed: &crate::delegation_start::ObservedStart,
    ) -> Result<CommandExecution> {
        let _gate = self.guard(leases).await?;
        let mut db = database.lock().await;
        observed.commit(
            BorrowMut::<Database>::borrow_mut(&mut *db),
            &self.invocation(input),
        )
    }
    async fn commit_resume<B: BorrowMut<Database>>(
        &self,
        database: &Mutex<B>,
        leases: &BuiltinToolLeaseRegistry,
        input: &ResumeParams,
        prepared: &crate::delegation_resume::PreparedResume,
        directory: &crate::delegation_resume::DirectoryObservation,
    ) -> Result<CommandExecution> {
        let _gate = self.guard(leases).await?;
        let mut db = database.lock().await;
        model::resume(
            BorrowMut::<Database>::borrow_mut(&mut *db),
            &self.invocation(input),
            prepared,
            directory,
        )
    }
}

/// `B` is Database in Core and a borrowed Database in the existing integration
/// owner. Tests reuse that owner's store, rather than create another fixture DB.
pub(crate) async fn invoke<B, D>(
    database: &Mutex<B>,
    leases: &BuiltinToolLeaseRegistry,
    data_dir: &Path,
    request: &BuiltinToolIpcRequest,
    disconnected: D,
) -> Result<Outcome>
where
    B: BorrowMut<Database>,
    D: Future<Output = ()>,
{
    ensure!(
        request.ipc_protocol_version == BUILTIN_TOOL_IPC_PROTOCOL_VERSION,
        "builtin_tool.ipc_protocol_mismatch"
    );
    let BuiltinToolIpcRequestBody::Invoke {
        request_id,
        operation,
        input,
    } = &request.body;
    ensure!(
        uuid::Uuid::parse_str(request_id).is_ok(),
        "builtin_tool.invalid_request_id"
    );
    let operation =
        contract::canonical_operation(operation).context("builtin_tool.unknown_operation")?;
    // Decode before any work or receipt lookup. A malformed request cannot be
    // silently repaired; original aliases map to the same canonical identity.
    enum Input {
        Start(StartParams),
        Status(StatusParams),
        Cancel(CancelParams),
        Resume(ResumeParams),
        Read(ReadParams),
        History(HistoryParams),
    }
    let input_typed = match operation {
        "delegation.start" => Input::Start(parse(input)?),
        "delegation.status" => Input::Status(parse(input)?),
        "delegation.cancel" => Input::Cancel(parse(input)?),
        "delegation.resume" => Input::Resume(parse(input)?),
        "delegation.read" => Input::Read(parse(input)?),
        "delegation.history" => Input::History(parse(input)?),
        _ => unreachable!(),
    };
    let active = {
        let _gate = leases.invocation_guard().await;
        let mut db = database.lock().await;
        let db = BorrowMut::<Database>::borrow_mut(&mut *db);
        if matches!(
            input_typed,
            Input::Start(_) | Input::Cancel(_) | Input::Resume(_)
        ) {
            let proof = leases
                .authenticate_receipt(&request.auth)
                .await
                .map_err(|_| anyhow::anyhow!("builtin_tool.run_not_bound"))?;
            if let Some(recorded) = proof.replay_cli(db, operation, request_id, input)? {
                return Ok(Outcome::command(operation, recorded));
            }
        }
        let authorized = leases
            .authenticate(&request.auth)
            .await
            .map_err(|_| anyhow::anyhow!("builtin_tool.run_not_bound"))?;
        ActiveInvocation::new(&request.auth, authorized, db, request_id)?
    };
    match input_typed {
        Input::Start(input) => {
            let prepared = {
                let _gate = active.guard(leases).await?;
                let mut db = database.lock().await;
                crate::delegation_start::prepare(
                    BorrowMut::<Database>::borrow_mut(&mut *db),
                    &ManagedBlobStore::new(data_dir),
                    &active.invocation(&input),
                )?
            };
            let observed = tokio::task::spawn_blocking(move || prepared.observe())
                .await
                .context("delegation.worker_failed")??;
            let execution = active
                .commit_start(database, leases, &input, &observed)
                .await?;
            // A committed receipt wins over failure to remove its durable file
            // intent. GC recovery owns any retained intent after worker failure.
            let cleanup_result = execution.clone();
            if tokio::task::spawn_blocking(move || observed.finish_after(&cleanup_result))
                .await
                .map_or(true, |r| r.is_err())
            {
                eprintln!("delegation task intent cleanup deferred to Core recovery");
            }
            Ok(Outcome::command(operation, execution))
        }
        Input::Resume(input) => {
            let prepared = {
                let _gate = active.guard(leases).await?;
                let mut db = database.lock().await;
                crate::delegation_resume::prepare(
                    BorrowMut::<Database>::borrow_mut(&mut *db),
                    &active.invocation(&input),
                )?
            };
            let (prepared, directory) = tokio::task::spawn_blocking(move || {
                let directory = prepared.observe_directory();
                (prepared, directory)
            })
            .await
            .context("delegation.worker_failed")?;
            let execution = active
                .commit_resume(database, leases, &input, &prepared, &directory)
                .await?;
            Ok(Outcome::command(operation, execution))
        }
        Input::Status(input) => {
            let notify = {
                let mut db = database.lock().await;
                BorrowMut::<Database>::borrow_mut(&mut *db)
                    .execution_wake
                    .delegation
                    .clone()
            };
            let interrupted = async {
                tokio::select! {
                    _ = disconnected => {},
                    _ = leases.wait_until_retired(&request.auth) => {},
                }
            };
            let result = query::wait_for_status(
                notify,
                &input,
                || async {
                    let _gate = active.guard(leases).await?;
                    let mut db = database.lock().await;
                    query::status_snapshot(
                        BorrowMut::<Database>::borrow_mut(&mut *db),
                        &active.invocation(&input),
                    )
                },
                interrupted,
            )
            .await;
            if result.is_err() && leases.authenticate(&request.auth).await.is_err() {
                anyhow::bail!("builtin_tool.run_not_bound");
            }
            Ok(Outcome::query(operation, result?))
        }
        Input::Cancel(input) => {
            let _gate = active.guard(leases).await?;
            let mut db = database.lock().await;
            Ok(Outcome::command(
                operation,
                model::cancel(
                    BorrowMut::<Database>::borrow_mut(&mut *db),
                    &active.invocation(&input),
                )?,
            ))
        }
        Input::Read(input) => {
            let _gate = active.guard(leases).await?;
            let mut db = database.lock().await;
            Ok(Outcome::query(
                operation,
                query::read(
                    BorrowMut::<Database>::borrow_mut(&mut *db),
                    &active.invocation(&input),
                )?,
            ))
        }
        Input::History(input) => {
            let _gate = active.guard(leases).await?;
            let mut db = database.lock().await;
            Ok(Outcome::query(
                operation,
                query::history(
                    BorrowMut::<Database>::borrow_mut(&mut *db),
                    &active.invocation(&input),
                )?,
            ))
        }
    }
}

/// Public IPC adapter. It runs before the generic active-only path so a retired
/// proof can recover a recorded mutation, and it bypasses the lease result cache
/// for scoped reads/waits. No raw error text is copied into the envelope.
pub(crate) async fn invoke_ipc<B, D>(
    database: &Mutex<B>,
    leases: &BuiltinToolLeaseRegistry,
    data_dir: &Path,
    request: &BuiltinToolIpcRequest,
    disconnected: D,
) -> crate::builtin_tool_transport::BuiltinToolIpcResponse
where
    B: BorrowMut<Database>,
    D: Future<Output = ()>,
{
    use crate::builtin_tool_transport::{
        BuiltinToolError, BuiltinToolInvocationEnvelope, BuiltinToolIpcResponse,
        recovery_for_operation_error,
    };
    let BuiltinToolIpcRequestBody::Invoke {
        request_id,
        operation,
        input,
    } = &request.body;
    if request.ipc_protocol_version != BUILTIN_TOOL_IPC_PROTOCOL_VERSION {
        return BuiltinToolIpcResponse::ipc_error(
            "builtin_tool.unsupported_ipc_version",
            "Built-in Tool IPC protocol version is unsupported",
        );
    }
    let Some(operation) = contract::canonical_operation(operation) else {
        return BuiltinToolIpcResponse::ipc_error(
            "builtin_tool.unknown_operation",
            "Requested operation is unavailable",
        );
    };
    if uuid::Uuid::parse_str(request_id).is_err() {
        return BuiltinToolIpcResponse::ipc_error(
            "builtin_tool.invalid_request_id",
            "requestId must be a UUID",
        );
    }
    let result = if crate::team_tool_catalog::validate_builtin_tool_input(operation, input).is_err()
    {
        Err(anyhow::anyhow!("builtin_tool.invalid_input"))
    } else {
        invoke(database, leases, data_dir, request, disconnected).await
    };
    let envelope = match result {
        Ok(outcome) => {
            if crate::delegation_catalog::validate_output(outcome.operation, &outcome.payload)
                .is_err()
            {
                return BuiltinToolIpcResponse::Envelope {
                    envelope: BuiltinToolInvocationEnvelope::rejected(
                        operation,
                        request_id,
                        BuiltinToolError {
                            code: "builtin_tool.output_contract_mismatch".into(),
                            message: "Core result could not be safely projected".into(),
                            recovery: recovery_for_operation_error(
                                operation,
                                "builtin_tool.output_contract_mismatch",
                            ),
                            details: None,
                        },
                    )
                    .expect("known operation and UUID"),
                };
            }
            BuiltinToolInvocationEnvelope::success(outcome.operation, request_id, outcome.payload)
        }
        Err(error) => {
            let (code, message) = public_error(&error);
            BuiltinToolInvocationEnvelope::rejected(
                operation,
                request_id,
                BuiltinToolError {
                    code: code.into(),
                    message: message.into(),
                    recovery: recovery_for_operation_error(operation, code),
                    details: None,
                },
            )
        }
    };
    match envelope {
        Ok(envelope) => BuiltinToolIpcResponse::Envelope { envelope },
        Err(_) => BuiltinToolIpcResponse::ipc_error(
            "builtin_tool.internal_error",
            "Core could not form the operation response",
        ),
    }
}
fn public_error(error: &anyhow::Error) -> (&'static str, &'static str) {
    if matches!(
        error.downcast_ref::<crate::command::CommandGatewayError>(),
        Some(crate::command::CommandGatewayError::IdempotencyConflict { .. })
    ) {
        return (
            "builtin_tool.idempotency_conflict",
            "The original request identity was reused with different input",
        );
    }
    if error.chain().any(|cause| cause.is::<rusqlite::Error>()) {
        return (
            "builtin_tool.internal_error",
            "Core could not complete the delegation request",
        );
    }
    match error.to_string().as_str() {
        "builtin_tool.invalid_input" => (
            "builtin_tool.invalid_input",
            "Command input does not match the accepted arguments",
        ),
        "builtin_tool.run_not_bound" => (
            "builtin_tool.run_not_bound",
            "The exact Run lease is no longer active",
        ),
        "delegation.cursor_invalid" => (
            "delegation.cursor_invalid",
            "The cursor does not belong to this scoped request; request a fresh page",
        ),
        "delegation.result_unavailable" => (
            "delegation.result_unavailable",
            "The task has no readable fixed final result yet",
        ),
        "delegation.result_changed" => (
            "delegation.result_changed",
            "The original fixed result no longer matches its saved identity",
        ),
        "delegation.page_limit_too_small" => (
            "delegation.page_limit_too_small",
            "Increase limit_bytes to include the next complete UTF-8 character",
        ),
        "delegation.status_disconnected" => (
            "delegation.status_disconnected",
            "Waiting ended; the delegated work was not canceled",
        ),
        code if code.starts_with("delegation.")
            && code
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_') =>
        {
            (
                "delegation.access_denied",
                "The current frozen Run has no authority for this delegation operation",
            )
        }
        _ if error
            .downcast_ref::<crate::team_tool::TeamToolInvocationError>()
            .is_some() =>
        {
            (
                "builtin_tool.run_not_bound",
                "The native Run binding is no longer active",
            )
        }
        _ => (
            "builtin_tool.internal_error",
            "Core could not complete the delegation request",
        ),
    }
}

pub(crate) async fn connection_ended(reader: &mut (impl tokio::io::AsyncRead + Unpin)) {
    use tokio::io::AsyncReadExt;
    let mut probe = [0u8; 1];
    // One request per connection. EOF, IO error or extra protocol bytes ends
    // this wait; it never becomes authority to stop the underlying task.
    let _ = reader.read(&mut probe).await;
}

#[cfg(all(test, feature = "slow-tests"))]
pub(crate) mod tests {
    use super::*;
    // Reuse the production prepare/observe/commit methods, inserting the lease
    // rotation at an exact stage boundary instead of relying on worker timing.
    pub(crate) async fn assert_observed_start_fenced<B: BorrowMut<Database>>(
        database: &Mutex<B>,
        leases: &BuiltinToolLeaseRegistry,
        config: &crate::builtin_tool_runtime::BuiltinToolProcessConfig,
        data_dir: &Path,
        request: &BuiltinToolIpcRequest,
    ) -> BuiltinToolAuth {
        let BuiltinToolIpcRequestBody::Invoke {
            request_id, input, ..
        } = &request.body;
        let input: StartParams = parse(input).unwrap();
        let active = {
            let _gate = leases.invocation_guard().await;
            let authorized = leases.authenticate(&request.auth).await.unwrap();
            let mut db = database.lock().await;
            ActiveInvocation::new(
                &request.auth,
                authorized,
                BorrowMut::<Database>::borrow_mut(&mut *db),
                request_id,
            )
            .unwrap()
        };
        let (prepared, changes) = {
            let _gate = active.guard(leases).await.unwrap();
            let mut db = database.lock().await;
            let db = BorrowMut::<Database>::borrow_mut(&mut *db);
            (
                crate::delegation_start::prepare(
                    db,
                    &ManagedBlobStore::new(data_dir),
                    &active.invocation(&input),
                )
                .unwrap(),
                db.connection().total_changes(),
            )
        };
        let observed = tokio::task::spawn_blocking(move || prepared.observe())
            .await
            .unwrap()
            .unwrap();
        let auth = leases
            .bind(
                config,
                &active.source.agent_run_id,
                active.source.execution_epoch,
                &active.authorized.native_binding,
                data_dir,
            )
            .await
            .unwrap();
        assert_eq!(
            active
                .commit_start(database, leases, &input, &observed)
                .await
                .err()
                .unwrap()
                .to_string(),
            "builtin_tool.run_not_bound"
        );
        let mut db = database.lock().await;
        assert_eq!(
            BorrowMut::<Database>::borrow_mut(&mut *db)
                .connection()
                .total_changes(),
            changes
        );
        auth
    }

    pub(crate) async fn assert_observed_resume_fenced<B: BorrowMut<Database>>(
        database: &Mutex<B>,
        leases: &BuiltinToolLeaseRegistry,
        config: &crate::builtin_tool_runtime::BuiltinToolProcessConfig,
        data_dir: &Path,
        request: &BuiltinToolIpcRequest,
    ) -> BuiltinToolAuth {
        let BuiltinToolIpcRequestBody::Invoke {
            request_id, input, ..
        } = &request.body;
        let input: ResumeParams = parse(input).unwrap();
        let (active, prepared, changes) = {
            let _gate = leases.invocation_guard().await;
            let authorized = leases.authenticate(&request.auth).await.unwrap();
            let mut db = database.lock().await;
            let db = BorrowMut::<Database>::borrow_mut(&mut *db);
            let active = ActiveInvocation::new(&request.auth, authorized, db, request_id).unwrap();
            let prepared =
                crate::delegation_resume::prepare(db, &active.invocation(&input)).unwrap();
            (active, prepared, db.connection().total_changes())
        };
        let (prepared, directory) = tokio::task::spawn_blocking(move || {
            let directory = prepared.observe_directory();
            (prepared, directory)
        })
        .await
        .unwrap();
        let auth = leases
            .bind(
                config,
                &active.source.agent_run_id,
                active.source.execution_epoch,
                &active.authorized.native_binding,
                data_dir,
            )
            .await
            .unwrap();
        assert_eq!(
            active
                .commit_resume(database, leases, &input, &prepared, &directory)
                .await
                .err()
                .unwrap()
                .to_string(),
            "builtin_tool.run_not_bound"
        );
        let mut db = database.lock().await;
        assert_eq!(
            BorrowMut::<Database>::borrow_mut(&mut *db)
                .connection()
                .total_changes(),
            changes
        );
        auth
    }
}
