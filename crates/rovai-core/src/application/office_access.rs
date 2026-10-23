use super::*;
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    scope: workbench::Scope,
    operation: String,
    #[serde(default)]
    hold_id: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    editable: bool,
    #[serde(default)]
    attachment_locator: Option<LocalAttachmentOwnerLocator>,
}
impl Core {
    pub(super) async fn office_access(
        &self,
        value: Value,
        client: &crate::draft_client::DraftClient,
    ) -> Result<Value> {
        let request: Request = serde_json::from_value(value)?;
        let root = self.workbench_root(&request.scope).await?;
        let scope = crate::command::canonical_json_digest(
            &json!({"scope":request.scope,"root":root,"core":self.data_dir,"attachment":request.attachment_locator}),
        )?;
        if request.operation == "detect" {
            return Ok(crate::office_preview::detect());
        }
        let id = request.hold_id.context("office.hold_required")?;
        match request.operation.as_str() {
            "start" => {
                if let Some(locator) = request.attachment_locator {
                    ensure!(
                        !request.editable && locator.camp_id() == request.scope.thread_id,
                        "office.attachment_readonly_owner_required"
                    );
                    ensure!(
                        matches!(
                            locator,
                            LocalAttachmentOwnerLocator::Message { .. }
                                | LocalAttachmentOwnerLocator::SingleChatMessage { .. }
                                | LocalAttachmentOwnerLocator::DelegationChildMessage { .. }
                                | LocalAttachmentOwnerLocator::Mission { .. }
                        ),
                        "office.published_attachment_required"
                    );
                    let target = {
                        let database = self.database.lock().await;
                        let source =
                            rovai_core::local_attachment_source::load_source_attachment_for_client(
                                &database, &locator, client,
                            )?;
                        if let Some(source) = source {
                            Some(desktop_target_for_source_attachment(&source)?)
                        } else {
                            ensure!(
                                legacy_attachment_belongs_to_owner(&database, &locator)?,
                                "office.attachment_not_owned"
                            );
                            let store =
                                ThreadAttachmentStore::for_client(&self.data_dir, client.clone());
                            match store.desktop_open_candidate(
                                &database,
                                locator.camp_id(),
                                locator.attachment_ref_id(),
                            )? {
                                Some(candidate) => {
                                    Some(store.verify_desktop_open_candidate(candidate)?)
                                }
                                None => None,
                            }
                        }
                    }
                    .context("office.attachment_unavailable")?;
                    crate::office_preview::start_attachment(
                        &scope,
                        &id,
                        &self.data_dir,
                        target.path.as_path(),
                    )
                    .await
                } else {
                    let path = request.path.context("office.path_required")?;
                    workbench::relative(&root, &path, false)?;
                    crate::office_preview::start(&scope, &id, &root, &path, request.editable).await
                }
            }
            "heartbeat" => crate::office_preview::heartbeat(&scope, &id),
            "stop" => crate::office_preview::release(&scope, &id).await,
            _ => anyhow::bail!("office.operation_not_admitted"),
        }
    }
}
