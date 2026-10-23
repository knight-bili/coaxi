//! Receive a complete batch before binding. Client paths never cross this boundary.
use super::*;
use axum::extract::Multipart;
use rovai_core::draft_client::DraftClient;
use std::collections::{HashMap, HashSet};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileIntent {
    id: String,
    display_name: String,
    byte_size: u64,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Intent {
    operation: String,
    command_id: String,
    command: Value,
    #[serde(default)]
    keep_attachment_ids: Vec<String>,
    files: Vec<FileIntent>,
}
fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
fn validate(intent: &Intent) -> Result<()> {
    ensure!(
        matches!(
            intent.operation.as_str(),
            "missions.create" | "missions.update"
        ),
        "invalid_mission_upload"
    );
    ensure!(
        valid_id(&intent.command_id) && intent.files.len() + intent.keep_attachment_ids.len() <= 10,
        "invalid_mission_upload"
    );
    if intent.operation == "missions.create" {
        ensure!(
            intent.keep_attachment_ids.is_empty(),
            "invalid_mission_upload"
        )
    }
    let mut ids = HashSet::new();
    for file in &intent.files {
        ensure!(
            valid_id(&file.id)
                && ids.insert(&file.id)
                && file.byte_size <= uploads::MAX_BYTES as u64,
            "invalid_mission_upload"
        );
        ensure!(
            file.sha256.len() == 64 && file.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid_mission_upload"
        );
        ensure!(
            !file.display_name.trim().is_empty()
                && file.display_name.len() <= 1024
                && !file.display_name.contains(['\0', '/', '\\']),
            "invalid_mission_upload"
        );
    }
    Ok(())
}
fn public_receipt(operation: &str, receipt: Value) -> Value {
    if operation == "missions.create" {
        operations::Operation::MissionCreate.project(receipt)
    } else {
        operations::Operation::MissionUpdate.project(receipt)
    }
}
async fn recorded(
    state: &WebState,
    client: &DraftClient,
    intent: &Intent,
) -> Result<Option<Value>> {
    let reply=state.core.request_for_editor("commands.reconcile",json!({"operation":intent.operation,"params":{"commandId":intent.command_id,"command":intent.command}}),client.clone()).await?;
    ensure!(reply.error.is_none(), "mission_reconciliation_failed");
    let value = reply.result.context("mission receipt missing")?;
    Ok((value["state"] == "recorded")
        .then(|| public_receipt(&intent.operation, value["result"].clone())))
}

pub async fn submit(
    State(state): State<WebState>,
    Extension(session): Extension<Arc<Session>>,
    mut multipart: Multipart,
) -> Response {
    let Ok(permit) = state.uploads.clone().try_acquire_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS, "upload_capacity");
    };
    let client = DraftClient::verified_web(&session.client_id).expect("Host editor identity");
    let field = match multipart.next_field().await {
        Ok(Some(value)) => value,
        _ => return error(StatusCode::BAD_REQUEST, "invalid_mission_upload"),
    };
    if field.name() != Some("intent") {
        return error(StatusCode::BAD_REQUEST, "invalid_mission_upload");
    }
    let Ok(text) = field.text().await else {
        return error(StatusCode::BAD_REQUEST, "invalid_mission_upload");
    };
    if text.len() > 65536 {
        return error(StatusCode::BAD_REQUEST, "invalid_mission_upload");
    }
    let Ok(intent) = serde_json::from_str::<Intent>(&text) else {
        return error(StatusCode::BAD_REQUEST, "invalid_mission_upload");
    };
    if validate(&intent).is_err() {
        return error(StatusCode::BAD_REQUEST, "invalid_mission_upload");
    }
    match recorded(&state, &client, &intent).await {
        Ok(Some(receipt)) => return Json(json!({"result":receipt})).into_response(),
        Ok(None) => {}
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "mission_result_unknown"),
    }
    let mut files = HashMap::new();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(value)) => value,
            Ok(None) => break,
            Err(_) => return error(StatusCode::BAD_REQUEST, "invalid_mission_upload"),
        };
        let Some(id) = field
            .name()
            .and_then(|name| name.strip_prefix("file:"))
            .map(str::to_owned)
        else {
            return error(StatusCode::BAD_REQUEST, "invalid_mission_upload");
        };
        let Some(expected) = intent.files.iter().find(|file| file.id == id) else {
            return error(StatusCode::BAD_REQUEST, "invalid_mission_upload");
        };
        if files.contains_key(&id) {
            return error(StatusCode::BAD_REQUEST, "invalid_mission_upload");
        }
        let received = match uploads::receive(field).await {
            Ok(value) => value,
            Err(_) => return error(StatusCode::BAD_REQUEST, "invalid_mission_upload"),
        };
        if received.hash != expected.sha256 || received.size != expected.byte_size {
            return error(StatusCode::BAD_REQUEST, "invalid_mission_upload");
        }
        files.insert(id, received);
    }
    if files.len() != intent.files.len() || !state.sessions.is_live(&session) {
        return error(StatusCode::BAD_REQUEST, "upload_changed_or_session_expired");
    }
    // The detached task owns cleanup until a definitive Core result. An HTTP
    // disconnect cannot remove a source that may already be bound to a Mission.
    let task = tokio::spawn(async move {
        let _permit = permit;
        let attachments=intent.files.iter().map(|file|json!({"id":file.id,"sourcePath":files[&file.id].temporary.path(),"displayName":file.display_name,"mediaType":null})).collect::<Vec<_>>();
        let mut params = json!({"commandId":intent.command_id,"command":intent.command,"attachments":attachments});
        let method = if intent.operation == "missions.create" {
            "host.missions.createWithAttachments"
        } else {
            params["keepAttachmentIds"] = json!(intent.keep_attachment_ids);
            "host.missions.updateWithAttachments"
        };
        for value in files.values_mut() {
            value.temporary.retain = true
        }
        match state
            .core
            .request_for_editor(method, params, client.clone())
            .await
        {
            Ok(reply) if reply.error.is_none() => {
                let Some(result) = reply.result else {
                    return error(StatusCode::SERVICE_UNAVAILABLE, "mission_result_unknown");
                };
                if result["replayed"] == true || result["receipt"]["status"] == "rejected" {
                    for value in files.values_mut() {
                        value.temporary.retain = false
                    }
                }
                Json(json!({"result":public_receipt(&intent.operation,result["receipt"].clone())}))
                    .into_response()
            }
            Ok(reply) => {
                // A post-commit error can still have a canonical receipt.
                match recorded(&state, &client, &intent).await {
                    Ok(Some(receipt)) => Json(json!({"result":receipt})).into_response(),
                    Ok(None) => {
                        for value in files.values_mut() {
                            value.temporary.retain = false
                        }
                        Json(json!({"error":reply.error})).into_response()
                    }
                    Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "mission_result_unknown"),
                }
            }
            Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "mission_result_unknown"),
        }
    });
    match task.await {
        Ok(response) => response,
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "mission_result_unknown"),
    }
}
