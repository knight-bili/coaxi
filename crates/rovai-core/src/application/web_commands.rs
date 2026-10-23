//! Read-only reconciliation of individually admitted user commands. A missing
//! receipt remains unknown; this path never dispatches the original command.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reconcile {
    #[serde(deserialize_with = "crate::thread_compat::deserialize_host_method")]
    operation: String,
    params: Value,
}

fn receipt<C: rovai_core::command::DomainCommand>(
    database: &Database,
    envelope: CommandEnvelope<C>,
) -> Result<Value> {
    Ok(DomainCommandGateway
        .replay_if_recorded(database, &envelope)?
        .map(|execution| json!({"state":"recorded", "result":execution.result}))
        .unwrap_or_else(|| json!({"state":"unknown"})))
}

pub(super) fn reconcile(
    database: &Database,
    data_dir: &Path,
    client: &rovai_core::draft_client::DraftClient,
    value: Value,
) -> Result<Value> {
    let query: Reconcile = serde_json::from_value(value)?;
    macro_rules! camp {
        ($kind:ty) => {{
            let params: UserCommandParams<$kind> = serde_json::from_value(query.params)?;
            receipt(
                database,
                user_camp_command_envelope(
                    params.command_id,
                    params.command.camp_id.clone(),
                    params.command,
                ),
            )
        }};
    }
    macro_rules! user {
        ($kind:ty) => {{
            let params: UserCommandParams<$kind> = serde_json::from_value(query.params)?;
            receipt(
                database,
                user_command_envelope(params.command_id, params.command),
            )
        }};
    }
    match query.operation.as_str() {
        "configuration.providers.upsert"
        | "configuration.providers.remove"
        | "configuration.memberConnection.set" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Original {
                command_id: String,
            }
            let input: Original = serde_json::from_value(query.params)?;
            crate::command::portable_user_receipt(
                database.connection(),
                &query.operation,
                &input.command_id,
                client,
            )
        }
        "runtime.custom.upsert"
        | "runtime.custom.remove"
        | "runtime.custom.install"
        | "runtime.steer.policy.set" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Original {
                command_id: String,
            }
            let input: Original = serde_json::from_value(query.params)?;
            uuid::Uuid::parse_str(&input.command_id)?;
            if query.operation == "runtime.custom.install" {
                let path =
                    crate::custom_runtime::installer::receipt_path(data_dir, &input.command_id)?;
                return if path.exists() {
                    Ok(
                        json!({"state":"recorded","result":crate::custom_runtime::installer::receipt(data_dir,&input.command_id)?}),
                    )
                } else {
                    Ok(json!({"state":"unknown"}))
                };
            }
            let result =
                crate::command::load_stored_result(database.connection(), &input.command_id)?;
            if let Some(result) = result {
                anyhow::ensure!(
                    result.command_type == query.operation && result.is_local_user_scoped(),
                    "custom_runtime.receipt_scope_changed"
                );
                Ok(json!({"state":"recorded","result":result}))
            } else {
                Ok(json!({"state":"unknown"}))
            }
        }
        "runtime.interactions.resolve" => runtime_interactions::reconcile(database, query.params),
        "delegations.resume" => delegation_queries::reconcile_resume(database, query.params),
        "delegations.settings.update" => delegation_queries::reconcile(database, query.params),
        "delegations.takeover" | "delegations.cancel" | "delegations.conversation.end" => {
            delegation_control::reconcile(database, &query.operation, query.params)
        }
        "delegations.message.send" | "delegations.pendingInputs.edit" => {
            delegation_input::reconcile(database, client, &query.operation, query.params)
        }
        "delegations.composer.mutate" => {
            delegation_composer::reconcile(database, client, query.params)
        }
        "singleChat.composerDraft.mutate" => {
            private_composer::reconcile(database, client, query.params)
        }
        "skillPacks.install" | "skillPacks.custom.install" => {
            user!(crate::skill_pack_installation::InstallSkillPackCommand)
        }
        "skillPacks.uninstall" => user!(crate::skill_pack_installation::UninstallSkillPackCommand),
        "skillPacks.office.install" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct OfficeInstall {
                command_id: String,
                command: crate::skill_pack_installation::InstallSkillPackCommand,
                plan_id: String,
            }
            let params: OfficeInstall = serde_json::from_value(query.params)?;
            uuid::Uuid::parse_str(&params.plan_id)?;
            receipt(
                database,
                user_command_envelope(params.command_id, params.command),
            )
        }
        "canvas.boards.create" => user!(rovai_core::canvas_board::CreateCanvasBoardCommand),
        "canvas.boards.update" => user!(rovai_core::canvas_board::UpdateCanvasBoardCommand),
        "canvas.boards.delete" => user!(rovai_core::canvas_board::DeleteCanvasBoardCommand),
        "canvas.nodes.apply" => user!(rovai_core::canvas_node::ApplyCanvasNodesCommand),
        "missions.create" => {
            let mut params: UserCommandParams<crate::mission::CreateMissionCommand> =
                serde_json::from_value(query.params)?;
            if params.command.project_binding_kind == ProjectBindingKind::QuickChat {
                params.command.project_path =
                    data_dir.join("quick-chat").to_string_lossy().into_owned();
            }
            receipt(
                database,
                user_command_envelope(params.command_id, params.command),
            )
        }
        "missions.update" => user!(crate::mission::UpdateMissionCommand),
        "missions.attachments.remove" => user!(crate::mission::RemoveMissionAttachmentCommand),
        "missions.status" => user!(crate::mission::StatusMissionCommand),
        "missions.start" => user!(crate::mission::StartMissionCommand),
        "missions.linkPr" => user!(crate::mission::LinkMissionPrCommand),
        "camps.rename" => camp!(RenameThreadCommand),
        "camps.delete" => camp!(DeleteThreadCommand),
        "camps.retryDeletion" => user!(RetryThreadDeletionCommand),
        "camps.discardPending" => camp!(DiscardPendingThreadCommand),
        "camps.members.fast.set" => camp!(rovai_core::camp_fast::SetThreadMemberFastCommand),
        "members.remove" => user!(RemoveMemberCommand),
        "members.reorder" => user!(ReorderAgentProfilesCommand),
        "members.presence.set" => user!(SetMemberPresenceCommand),
        "notifications.preference.update" => user!(UpdateNotificationPreferenceCommand),
        "notifications.acknowledge" => user!(AcknowledgeNotificationEpisodeCommand),
        "notifications.acknowledgeVisibleSources" => {
            user!(AcknowledgeVisibleNotificationSourcesCommand)
        }
        "skills.reconcile" => user!(ReconcileSkillProjectionsCommand),

        "singleChat.open" => {
            let mut params: UserCommandParams<OpenSingleChatCommand> =
                serde_json::from_value(query.params)?;
            params.command.draft_client = client.clone();
            receipt(
                database,
                user_camp_command_envelope(
                    params.command_id,
                    params.command.camp_id.clone(),
                    params.command,
                ),
            )
        }
        "singleChat.send" => {
            let mut params: UserCommandParams<SendSingleChatMessageCommand> =
                serde_json::from_value(query.params)?;
            params.command.draft_client = client.clone();
            receipt(
                database,
                user_camp_command_envelope(
                    params.command_id,
                    params.command.camp_id.clone(),
                    params.command,
                ),
            )
        }
        "singleChat.pendingInputs.edit" => {
            let mut params: UserCommandParams<EditSingleChatPendingInputCommand> =
                serde_json::from_value(query.params)?;
            params.command.draft_client = client.clone();
            receipt(
                database,
                user_camp_command_envelope(
                    params.command_id,
                    params.command.camp_id.clone(),
                    params.command,
                ),
            )
        }
        "singleChat.end" => camp!(EndSingleChatCommand),
        "skills.import.commit" => user!(CommitSkillImportCommand),
        "skills.setEnabled" => user!(SetSkillEnabledCommand),
        "skills.setGroupAssignments" => user!(SetSkillGroupAssignmentsCommand),
        "skills.delete" => user!(DeleteSkillCommand),

        "memory.create" => user!(CreateMemoryCommand),
        "memory.revise" => user!(ReviseMemoryCommand),
        "memory.retire" => user!(RetireMemoryCommand),
        "memory.reactivate" => user!(ReactivateMemoryCommand),
        "memory.forget" => user!(ForgetMemoryCommand),
        "memory.supersede" => user!(SupersedeMemoriesCommand),
        "memory.review.schedule" => user!(ScheduleMemoryReviewCommand),
        "memory.hearthReviewItems.accept" => user!(AcceptHearthReviewItemCommand),
        "memory.hearthReviewItems.reject" => user!(RejectHearthReviewItemCommand),
        "automations.create" => user!(CreateAutomationCommand),
        "automations.update" => user!(UpdateAutomationCommand),
        "automations.close" => user!(CloseAutomationCommand),
        "automations.delete" => user!(DeleteAutomationCommand),
        "automations.run" => user!(RunAutomationCommand),
        "automations.cancelRun" => user!(CancelAutomationRunCommand),
        "tasks.create" => {
            let params: CreateTaskParams = serde_json::from_value(query.params)?;
            receipt(
                database,
                user_camp_command_envelope(
                    params.command_id,
                    params.camp_id.to_string(),
                    CreateTaskCommand {
                        camp_id: params.camp_id.to_string(),
                        title: params.title,
                        description: params.description,
                        assignee_agent_id: params.assignee_agent_id,
                    },
                ),
            )
        }
        "tasks.update" => {
            let params: UpdateTaskParams = serde_json::from_value(query.params)?;
            receipt(
                database,
                user_camp_command_envelope(
                    params.command_id,
                    params.camp_id.to_string(),
                    UpdateTaskCommand {
                        task_id: params.task_id,
                        title: params.title,
                        description: params.description,
                        status: params.status,
                        assignee: params.assignee,
                        blocked_reason: params.blocked_reason,
                        completion_summary: params.completion_summary,
                        cancel_reason: params.cancel_reason,
                    },
                ),
            )
        }

        "camp.messages.send" => {
            let params: SendThreadMessageParams = serde_json::from_value(query.params)?;
            let mut value = receipt(database, params.envelope())?;
            if value["state"] == "recorded" {
                value["result"] = json!({"commandResult":value["result"].take(), "replayed":true, "preflight":null, "pendingExecution":null});
            }
            Ok(value)
        }
        "camp.messages.withdraw" => camp!(WithdrawThreadMessageCommand),
        "action.approvals.resolve" => {
            let params: ResolveActionApprovalParams = serde_json::from_value(query.params)?;
            receipt(database, params.envelope())
        }
        "agentRuns.cancel" => camp!(CancelAgentRunCommand),
        "agentRuns.continue" => camp!(crate::run_continuation::ContinueAgentRunCommand),
        "camps.changeDefaultLead" => camp!(ChangeDefaultLeadCommand),
        "camps.members.add" => camp!(AddThreadMemberCommand),
        "camps.members.remove" => camp!(RemoveThreadMemberCommand),
        "members.create" => user!(CreateAgentProfileCommand),
        "members.update" => user!(UpdateAgentProfileCommand),
        "members.avatar.set" => user!(SetAgentProfileAvatarCommand),
        "members.runtime.set" => user!(SetMemberRuntimeConfigurationCommand),
        "members.runtime.clear" => user!(ClearMemberRuntimeConfigurationCommand),
        "messageQuotes.mutateDraft" => {
            let mut params: UserCommandParams<rovai_core::message_quote::MutateQuoteDraftCommand> =
                serde_json::from_value(query.params)?;
            params.command.draft_client = client.clone();
            let value = receipt(
                database,
                user_camp_command_envelope(
                    params.command_id,
                    params.command.camp_id.clone(),
                    params.command.clone(),
                ),
            )?;
            if value["state"] == "recorded" {
                // The command's receipt proves whether it ran; the API response
                // is the current authorized Draft, like the original handler.
                if value["result"]["status"] == "rejected" {
                    // A durable rejection is a known outcome, not a failure to
                    // read the receipt. Preserve the original handler's error
                    // while letting a disconnected client settle its promise.
                    let code = &value["result"]["code"];
                    return Ok(json!({"state":"recorded", "error":{"code":code,"message":code}}));
                }
                let result = if let Some(conversation_id) = params.command.conversation_id {
                    serde_json::to_value(
                        SingleChatService::for_client(client.clone())
                            .snapshot(database, &conversation_id)?,
                    )?
                } else {
                    serde_json::to_value(
                        ThreadAttachmentStore::for_client(data_dir, client.clone())
                            .load_draft(database, &params.command.camp_id)?,
                    )?
                };
                Ok(json!({"state":"recorded", "result":result}))
            } else {
                Ok(value)
            }
        }
        "camps.create" => {
            let params: CreateThreadParams = serde_json::from_value(query.params)?;
            let (project_binding_kind, path) = match params.workspace {
                Some(workspace) => (
                    ProjectBindingKind::Directory,
                    // Web admission already required this exact canonical
                    // spelling. Receipt reads must not inspect an arbitrary
                    // path or depend on the workspace still existing.
                    PathBuf::from(workspace.project_path),
                ),
                None => (
                    ProjectBindingKind::QuickChat,
                    std::fs::canonicalize(data_dir.join("quick-chat"))
                        .context("Quick Chat workspace is unavailable for reconciliation")?,
                ),
            };
            receipt(
                database,
                user_command_envelope(
                    params.command_id,
                    CreateThreadCommand {
                        name: params.name,
                        project_binding_kind,
                        project_path: path.to_string_lossy().into_owned(),
                        member_agent_ids: params.member_agent_ids,
                        default_lead_agent_id: params.default_lead_agent_id,
                        collaboration_mode: params.collaboration_mode,
                        activation_state: params.activation_state,
                    },
                ),
            )
        }
        _ => anyhow::bail!("command reconciliation is not admitted"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_reconciliation_rejects_removed_input_fields() {
        let create = serde_json::from_value::<CreateTaskParams>(json!({
            "commandId": "command-old-create",
            "threadId": "camp-old",
            "title": "Old create",
            "description": "Old payload",
            "acceptanceCriteria": ["legacy"],
            "assigneeAgentId": "agent-old"
        }));
        assert!(create.is_err());

        let update = serde_json::from_value::<UpdateTaskParams>(json!({
            "commandId": "command-old-update",
            "threadId": "camp-old",
            "taskId": "task-old",
            "expectedVersion": 1,
            "clearAcceptanceCriteria": true
        }));
        assert!(update.is_err());
    }
}
