//! Explicit delegated channel controls, using the existing Core command services.
use super::channel_adapter_commands::{ChannelCommand, effective_callback, sender_scope};
use super::channel_adapter_messages::Inbound;
use super::channel_adapters::{Adapter, Binding};
use super::*;
use anyhow::ensure;
use rusqlite::OptionalExtension;

pub(super) fn menu(
    transaction: &rusqlite::Transaction<'_>,
    input: &Inbound,
    config: &Adapter,
    binding: &Binding,
    command: &str,
    args: &str,
) -> Result<(Vec<String>, Vec<Value>)> {
    let mut lines = Vec::new();
    let mut choices = Vec::new();
    let allowed = if command == "cancel" {
        &config.control_senders
    } else {
        &config.approval_senders
    };
    if !allowed.contains(&input.sender_id) {
        return Ok((
            vec!["当前发送人没有这项控制权限，请在应用中由工作区所有者授权。".into()],
            choices,
        ));
    }
    if command == "cancel" {
        let mut statement=transaction.prepare("SELECT ar.id,ar.version,ar.status,c.agent_id FROM agent_run ar JOIN conversation c ON c.id=ar.conversation_id WHERE ar.camp_id=?1 AND ar.status NOT IN ('succeeded','failed','cancelled') AND ar.cancel_requested_at IS NULL ORDER BY ar.created_at DESC LIMIT 20")?;
        let rows = statement
            .query_map([&binding.thread_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, version, status, agent) in rows {
            if !binding.agent_ids.contains(&agent) || (!args.is_empty() && args != id) {
                continue;
            }
            lines.push(format!(
                "停止确认：{id} · {agent} · {status} · 版本{version}"
            ));
            choices.push(json!({"operation":"cancel","threadId":binding.thread_id,"agentRunId":id,"expectedVersion":version,"label":format!("停止 {agent}")}));
        }
    } else {
        let mut statement=transaction.prepare("SELECT a.id,a.version,a.native_options_json,ae.agent_run_id,ae.action_summary,ae.canonical_input_json,ae.action_digest,c.agent_id FROM approval a JOIN action_execution ae ON ae.id=a.action_id JOIN agent_run ar ON ar.id=ae.agent_run_id JOIN conversation c ON c.id=ar.conversation_id WHERE ar.camp_id=?1 AND a.status='pending' AND a.requested_for_user_id=?2 ORDER BY a.requested_at DESC LIMIT 10")?;
        let rows = statement
            .query_map(
                rusqlite::params![binding.thread_id, CURRENT_USER_ID],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, version, raw, run, summary, canonical, digest, agent) in rows {
            if !binding.agent_ids.contains(&agent) {
                continue;
            }
            lines.push(format!(
                "审批 {id} · {agent} · 版本{version}\n{summary}\n{canonical}\n操作摘要：{digest}"
            ));
            if canonical.len() > 16 * 1024 {
                lines.pop();
                lines.push(format!(
                    "审批 {id} 的内容较长，请在应用中查看完整内容并处理；此处不提供确认按钮。"
                ));
                continue;
            }
            let options: Vec<crate::action::RuntimePermissionOption> = serde_json::from_str(&raw)?;
            for option in options {
                if (command == "approve") != option.allows_action
                    || command == "approve"
                        && args.eq_ignore_ascii_case("always")
                        && option.kind != "allow_session"
                {
                    continue;
                }
                if choices.len() >= 20 {
                    break;
                }
                lines.push(format!("选项：{}\n{}", option.label, option.consequence));
                choices.push(json!({"operation":"approval","threadId":binding.thread_id,"approvalId":id,"agentRunId":run,"expectedVersion":version,"optionId":option.option_id,"actionDigest":digest,"label":option.label,"consequence":option.consequence}));
            }
        }
    }
    if choices.is_empty() {
        lines.push("没有可执行的匹配选项。没有更改任何执行或审批。".into())
    } else {
        lines.push("请核对上述对象和选项，再点对应按钮；按钮10分钟内有效。".into())
    }
    Ok((lines, choices))
}
impl Core {
    pub(super) async fn channel_adapter_control(
        &self,
        input: &Inbound,
        config: &Adapter,
        binding: &Binding,
        command_id: &str,
    ) -> Result<Option<Value>> {
        let callback = effective_callback(input, config);
        let Some(callback) = callback
            .as_deref()
            .filter(|value| value.starts_with("rv:control:"))
        else {
            return Ok(None);
        };
        let parts = callback.split(':').collect::<Vec<_>>();
        let scope = sender_scope(input, config)?;
        let prepared = (|| -> Result<(&str, usize)> {
            ensure!(parts.len() == 4, "channel_adapter.invalid_callback");
            uuid::Uuid::parse_str(parts[2])?;
            Ok((parts[2], parts[3].parse()?))
        })();
        let outcome = match prepared {
            Err(_) => "按钮格式无效，没有执行操作。".to_owned(),
            Ok((source, index)) => {
                let mut database = self.database.lock().await;
                let previous=database.connection().query_row("SELECT result_payload_json,created_at FROM event_log WHERE event_type='command.result' AND command_id=?1 AND command_type='channel.adapter.command' AND result_status='applied'",[source],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).optional()?;
                if let Some((raw, created_at)) = previous {
                    let previous: Value = serde_json::from_str(&raw)?;
                    if previous["senderScope"] != scope
                        || previous["revision"] != input.expected_revision
                    {
                        "按钮归属或渠道配置已变化，没有执行操作。".into()
                    } else if let Some(choice) = previous["controlChoices"]
                        .as_array()
                        .and_then(|values| values.get(index))
                    {
                        let kind = choice["operation"].as_str().unwrap_or_default();
                        let approved = if kind == "cancel" {
                            config.control_senders.contains(&input.sender_id)
                        } else {
                            kind == "approval" && config.approval_senders.contains(&input.sender_id)
                        };
                        let thread = choice["threadId"].as_str().unwrap_or_default();
                        let authorized = config.bindings.iter().any(|candidate| {
                            candidate.thread_id == thread
                                && candidate.chat_id
                                    == input.target.chat_id.as_deref().unwrap_or(&config.chat_id)
                                && candidate.thread_key == input.target.thread_key
                        });
                        if !approved || !authorized {
                            "当前发送人或对话没有此项控制权限，没有执行操作。".into()
                        } else {
                            let mut bytes = *uuid::Uuid::parse_str(command_id)?.as_bytes();
                            bytes[15] ^= 0x5a;
                            let effect_id = uuid::Uuid::from_bytes(bytes).to_string();
                            let recorded = crate::command::load_stored_result(
                                database.connection(),
                                &effect_id,
                            )?
                            .is_some();
                            let fresh = chrono::DateTime::parse_from_rfc3339(&created_at)
                                .map(|time| {
                                    (0..=600).contains(
                                        &(chrono::Utc::now().timestamp() - time.timestamp()),
                                    )
                                })
                                .unwrap_or(false);
                            if !recorded && !fresh {
                                "按钮已过期，请重新获取当前选项；没有执行操作。".into()
                            } else {
                                let version = choice["expectedVersion"]
                                    .as_i64()
                                    .context("channel_adapter.invalid_control")?;
                                let result = if kind == "cancel" {
                                    let command = crate::runtime::CancelAgentRunCommand {
                                        camp_id: thread.into(),
                                        agent_run_id: choice["agentRunId"]
                                            .as_str()
                                            .context("channel_adapter.invalid_control")?
                                            .into(),
                                        expected_version: version,
                                    };
                                    ExecutionRuntimeService::default()
                                        .request_agent_run_cancellation(
                                            &mut database,
                                            &user_camp_command_envelope(
                                                effect_id,
                                                thread.into(),
                                                command,
                                            ),
                                        )?
                                        .result
                                } else {
                                    let command = ResolveActionApprovalCommand {
                                        approval_id: choice["approvalId"]
                                            .as_str()
                                            .context("channel_adapter.invalid_control")?
                                            .into(),
                                        option_id: choice["optionId"]
                                            .as_str()
                                            .context("channel_adapter.invalid_control")?
                                            .into(),
                                        expected_version: version,
                                        reason: Some(format!(
                                            "渠道委托审批：{} / {} / {}",
                                            config.id, input.sender_id, input.external_id
                                        )),
                                    };
                                    ActionSafetyService::default()
                                        .resolve_approval(
                                            &mut database,
                                            &CommandEnvelope {
                                                command_id: effect_id,
                                                actor: ActorRef::User {
                                                    user_id: CURRENT_USER_ID.into(),
                                                },
                                                camp_id: Some(thread.into()),
                                                expected_versions: vec![],
                                                execution_epoch: None,
                                                payload: command,
                                            },
                                        )?
                                        .result
                                };
                                if result.status == CommandResultStatus::Applied {
                                    self.agent_run_cancellation_notify.notify_one();
                                    self.delivery_batch_scheduler_notify.notify_one();
                                    self.execution_wake.execution_changed();
                                }
                                format!(
                                    "操作回执：{} · {}\n{}",
                                    serde_json::to_string(&result.status)?,
                                    result.code,
                                    serde_json::to_string(&result.payload)?
                                )
                            }
                        }
                    } else {
                        "原按钮不存在，请重新获取列表；没有执行操作。".into()
                    }
                } else {
                    "原操作说明不存在，请重新获取列表；没有执行操作。".into()
                }
            }
        };
        let envelope = CommandEnvelope {
            command_id: command_id.into(),
            actor: ActorRef::System {
                component_id: "channel-adapter".into(),
            },
            camp_id: Some(binding.thread_id.clone()),
            expected_versions: vec![],
            execution_epoch: None,
            payload: ChannelCommand(input.clone()),
        };
        let mut database = self.database.lock().await;
        let result=DomainCommandGateway.execute(&mut database,&envelope,|_|Ok(crate::command::CommandHandlerResult::applied("channel.adapter.control.completed",json!({"adapterId":config.id,"threadId":binding.thread_id,"senderId":input.sender_id,"externalId":input.external_id,"target":input.target,"senderScope":scope,"revision":input.expected_revision,"response":{"body":outcome,"buttons":[]}}),None)))?;
        Ok(Some(serde_json::to_value(result.result)?))
    }
}
