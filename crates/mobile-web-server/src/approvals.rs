use std::time::{SystemTime, UNIX_EPOCH};

use deepseek_mobile_agent_core::risk::RiskAssessment;
use deepseek_mobile_agent_core::ssh::SshCommandRequest;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::events::broadcast_event;
use crate::ssh_exec::{CommandRunError, CommandRunner, SshCommandOutput};
use crate::{
    AppState, ApprovalRespondRequest, ApprovalResponse, AuditEntry, CommandPrepareRequest, Message,
    MessagePart, PendingApproval,
    types::{ToolPartData, text_part, tool_part},
};

#[derive(Clone, Debug)]
pub struct ApprovalService<R> {
    runner: R,
}

impl<R> ApprovalService<R>
where
    R: CommandRunner,
{
    #[must_use]
    pub const fn new(runner: R) -> Self {
        Self { runner }
    }

    pub fn prepare(&self, state: &AppState, request: CommandPrepareRequest) -> PendingApproval {
        let now = now_ms();
        let approval = PendingApproval::remote_shell(
            format!("approval-{}", Uuid::new_v4()),
            request.session_id,
            request.command,
            request.cwd,
            now,
            "pending".to_string(),
        );
        state.insert_pending_approval(approval.clone());
        state.push_audit(approval_audit(
            &approval,
            "prepared",
            "pending",
            "once",
            json!({}),
            None,
        ));
        broadcast_event(
            state,
            "approval.asked",
            serde_json::to_value(&approval).expect("pending approval must serialize"),
        );
        approval
    }

    pub fn respond(
        &self,
        state: &AppState,
        approval_id: &str,
        request: ApprovalRespondRequest,
    ) -> Result<ApprovalResponse, ApprovalError> {
        let normalized_response = normalize_approval_response(&request.response)?;
        let mut approval =
            state
                .remove_pending_approval(approval_id)
                .ok_or_else(|| ApprovalError::NotFound {
                    approval_id: approval_id.to_string(),
                })?;

        match normalized_response {
            ApprovalResponseAction::RejectStop => {
                let stopped = approval
                    .agent_turn_id()
                    .map(|agent_turn_id| {
                        state.remove_pending_approvals_for_agent_turn(&agent_turn_id)
                    })
                    .unwrap_or_default();
                let stopped_count = stopped.len();
                approval.status = "rejected".to_string();
                append_agent_rejection_summary(state, &approval);
                for mut stopped_approval in stopped {
                    stopped_approval.status = "stopped".to_string();
                    append_agent_stopped_summary(state, &stopped_approval);
                    broadcast_reply(state, &stopped_approval);
                }
                let fallback_summary = rejected_agent_summary(&approval.command);
                let assistant_summary =
                    append_agent_turn_stopped_summary_if_complete(state, &approval)
                        .unwrap_or(fallback_summary);
                let mut result = rejected_command_result(&approval.command, &assistant_summary);
                result["stopped_count"] = json!(stopped_count);
                state.push_audit(approval_audit(
                    &approval,
                    "rejected",
                    "reject_stop",
                    "once",
                    result.clone(),
                    Some(stopped_count),
                ));
                broadcast_reply(state, &approval);
                Ok(ApprovalResponse {
                    approval,
                    status: "rejected".to_string(),
                    result,
                })
            }
            ApprovalResponseAction::ApproveOnce | ApprovalResponseAction::ApproveSession => {
                let approve_session =
                    matches!(normalized_response, ApprovalResponseAction::ApproveSession);
                let output = match self.execute_approval(state, &approval) {
                    Ok(output) => output,
                    Err(error) => {
                        if approve_session {
                            state.grant_session_allow(
                                &approval.session_id,
                                &approval.command,
                                approval.cwd.as_deref(),
                            );
                            state.push_audit(approval_audit(
                                &approval,
                                "approved",
                                "approve_session",
                                "session",
                                json!({
                                    "status": "failed",
                                    "error": redact_text(&error.to_string()),
                                }),
                                None,
                            ));
                        }
                        return Err(error);
                    }
                };
                if approve_session {
                    state.grant_session_allow(
                        &approval.session_id,
                        &approval.command,
                        approval.cwd.as_deref(),
                    );
                }
                let summary = approved_agent_summary(&approval.command, &output);
                approval.status = "approved".to_string();
                append_agent_approval_summary(state, &approval, summary, &output);
                let assistant_summary =
                    append_agent_turn_final_summary_if_complete(state, &approval)
                        .unwrap_or_else(|| approved_agent_summary(&approval.command, &output));
                let mut result = command_result(&approval.command, &output, &assistant_summary);
                if approve_session {
                    result["scope"] = json!("session");
                }
                let response = if approve_session {
                    "approve_session"
                } else {
                    "approve_once"
                };
                state.push_audit(approval_audit(
                    &approval,
                    "approved",
                    response,
                    if approve_session { "session" } else { "once" },
                    result.clone(),
                    None,
                ));
                broadcast_reply(state, &approval);
                Ok(ApprovalResponse {
                    approval,
                    status: "approved".to_string(),
                    result,
                })
            }
        }
    }

    fn execute_approval(
        &self,
        state: &AppState,
        approval: &PendingApproval,
    ) -> Result<SshCommandOutput, ApprovalError> {
        let command = SshCommandRequest {
            command: approval.command.clone(),
            cwd: approval.cwd.clone(),
            timeout_ms: None,
            risk: RiskAssessment::high("advanced command requires explicit approval"),
        };
        broadcast_event(
            state,
            "tool.started",
            json!({
                "session_id": approval.session_id,
                "approval_id": approval.id,
                "tool": "remote.shell.exec",
                "command": approval.command,
            }),
        );

        let output = self
            .runner
            .run(&state.ssh_target(), &command)
            .map_err(ApprovalError::Command)?;

        if !output.stdout.is_empty() {
            broadcast_event(
                state,
                "tool.stdout",
                json!({
                    "session_id": approval.session_id,
                    "approval_id": approval.id,
                    "stdout": output.stdout,
                    "text": output.stdout,
                }),
            );
        }
        if !output.stderr.is_empty() {
            broadcast_event(
                state,
                "tool.stderr",
                json!({
                    "session_id": approval.session_id,
                    "approval_id": approval.id,
                    "stderr": output.stderr,
                    "text": output.stderr,
                }),
            );
        }

        broadcast_event(
            state,
            if output.timed_out {
                "tool.failed"
            } else {
                "tool.completed"
            },
            json!({
                "session_id": approval.session_id,
                "approval_id": approval.id,
                "command": approval.command,
                "exit_code": output.exit_code,
                "duration_ms": duration_ms(output.duration),
                "timed_out": output.timed_out,
            }),
        );
        Ok(output)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApprovalResponseAction {
    ApproveOnce,
    ApproveSession,
    RejectStop,
}

fn normalize_approval_response(response: &str) -> Result<ApprovalResponseAction, ApprovalError> {
    match response {
        "approve_once" => Ok(ApprovalResponseAction::ApproveOnce),
        "approve_session" | "always" => Ok(ApprovalResponseAction::ApproveSession),
        "reject_stop" | "reject" => Ok(ApprovalResponseAction::RejectStop),
        other => Err(ApprovalError::UnsupportedResponse {
            response: other.to_string(),
        }),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    #[error("approval not found or already consumed: {approval_id}")]
    NotFound { approval_id: String },
    #[error("unsupported approval response: {response}")]
    UnsupportedResponse { response: String },
    #[error("approved command failed: {0}")]
    Command(CommandRunError),
}

fn broadcast_reply(state: &AppState, approval: &PendingApproval) {
    broadcast_event(
        state,
        "approval.replied",
        json!({
            "id": approval.id,
            "session_id": approval.session_id,
            "status": approval.status,
        }),
    );
}

fn approval_audit(
    approval: &PendingApproval,
    action: &str,
    response: &str,
    scope: &str,
    result: Value,
    stopped_count: Option<usize>,
) -> AuditEntry {
    let mut metadata = json!({
        "approval_id": approval.id,
        "action": action,
        "response": response,
        "scope": scope,
        "command": redact_text(&approval.command),
        "cwd": approval.cwd.as_deref().map(redact_text),
        "status": action,
        "risk": {
            "level": approval.risk_level(),
            "reason": approval.risk_reason(),
        },
        "target": approval.target(),
        "target_label": approval.target_label(),
        "result": redact_value(&result),
    });
    if let Some(stopped_count) = stopped_count {
        metadata["stopped_count"] = json!(stopped_count);
    }
    AuditEntry {
        id: format!("audit-{}", Uuid::new_v4()),
        session_id: Some(approval.session_id.clone()),
        kind: format!("approval.{action}"),
        created_at_ms: now_ms(),
        summary: format!("approval {} {action}", approval.id),
        metadata,
    }
}

fn append_agent_approval_summary(
    state: &AppState,
    approval: &PendingApproval,
    summary: String,
    output: &SshCommandOutput,
) {
    let Some(agent_turn_id) = approval.agent_turn_id() else {
        return;
    };
    append_agent_summary_message(
        state,
        approval,
        agent_turn_id,
        summary,
        Some(approval_result_tool_part(
            approval,
            "completed",
            Some(output),
        )),
    );
}

fn append_agent_rejection_summary(state: &AppState, approval: &PendingApproval) {
    let Some(agent_turn_id) = approval.agent_turn_id() else {
        return;
    };
    append_agent_summary_message(
        state,
        approval,
        agent_turn_id,
        rejected_agent_summary(&approval.command),
        Some(approval_result_tool_part(approval, "rejected", None)),
    );
}

fn append_agent_stopped_summary(state: &AppState, approval: &PendingApproval) {
    let Some(agent_turn_id) = approval.agent_turn_id() else {
        return;
    };
    append_agent_summary_message(
        state,
        approval,
        agent_turn_id,
        stopped_agent_summary(&approval.command),
        Some(approval_result_tool_part(approval, "stopped", None)),
    );
}

fn append_agent_summary_message(
    state: &AppState,
    approval: &PendingApproval,
    agent_turn_id: String,
    text: String,
    tool_part: Option<MessagePart>,
) {
    let mut parts = vec![text_part(
        text,
        json!({
            "approval_id": approval.id,
            "agent_turn_id": agent_turn_id,
            "command": approval.command,
        }),
    )];
    if let Some(tool_part) = tool_part
        && !upsert_origin_tool_part(state, approval, &tool_part)
    {
        parts.push(tool_part);
    }
    let message = Message {
        id: format!("message-{}", Uuid::new_v4()),
        session_id: approval.session_id.clone(),
        role: "assistant".to_string(),
        created_at_ms: now_ms(),
        parts,
    };
    state.push_message(message.clone());
    broadcast_event(
        state,
        "message.updated",
        serde_json::to_value(&message).expect("message must serialize"),
    );
    for part in message.parts.iter().filter(|part| part.kind == "tool") {
        broadcast_message_part_updated(state, &approval.session_id, &message.id, part);
    }
}

fn upsert_origin_tool_part(
    state: &AppState,
    approval: &PendingApproval,
    tool_part: &MessagePart,
) -> bool {
    let Some(message_id) = approval.agent_message_id() else {
        return false;
    };
    if !state.upsert_message_part(&approval.session_id, &message_id, tool_part.clone()) {
        return false;
    }
    broadcast_message_part_updated(state, &approval.session_id, &message_id, tool_part);
    true
}

fn approval_result_tool_part(
    approval: &PendingApproval,
    status: &str,
    output: Option<&SshCommandOutput>,
) -> MessagePart {
    let agent_turn_id = approval.agent_turn_id().unwrap_or_default();
    let redacted_stdout = output.map(|output| redact_text(&output.stdout));
    let redacted_stderr = output.map(|output| redact_text(&output.stderr));
    let mut part = tool_part(ToolPartData {
        turn_id: agent_turn_id.clone(),
        agent_turn_id,
        tool_call_id: format!("approval-call-{}", approval.id),
        approval_id: Some(approval.id.clone()),
        tool: "remote.shell.exec".to_string(),
        title: "Remote shell command".to_string(),
        status: status.to_string(),
        requires_approval: true,
        command: approval.command.clone(),
        input: json!({ "command": approval.command }),
        output: redacted_stdout.clone(),
        stdout: redacted_stdout,
        stderr: redacted_stderr,
        exit_code: output.and_then(|output| output.exit_code),
        duration_ms: output.map(|output| duration_ms(output.duration)),
        timed_out: output.map(|output| output.timed_out),
    });
    part.id = approval
        .agent_tool_part_id()
        .unwrap_or_else(|| format!("part-tool-{}", Uuid::new_v4()));
    part
}

fn broadcast_message_part_updated(
    state: &AppState,
    session_id: &str,
    message_id: &str,
    part: &MessagePart,
) {
    broadcast_event(
        state,
        "message.part.updated",
        json!({
            "session_id": session_id,
            "message_id": message_id,
            "part": part,
        }),
    );
}

fn append_agent_turn_final_summary_if_complete(
    state: &AppState,
    approval: &PendingApproval,
) -> Option<String> {
    append_agent_turn_terminal_summary_if_complete(state, approval, "completed")
}

fn append_agent_turn_stopped_summary_if_complete(
    state: &AppState,
    approval: &PendingApproval,
) -> Option<String> {
    append_agent_turn_terminal_summary_if_complete(state, approval, "stopped")
}

fn append_agent_turn_terminal_summary_if_complete(
    state: &AppState,
    approval: &PendingApproval,
    status: &str,
) -> Option<String> {
    let agent_turn_id = approval.agent_turn_id()?;
    let has_remaining_turn_approvals = state
        .pending_approvals()
        .into_iter()
        .any(|pending| pending.agent_turn_id().as_deref() == Some(agent_turn_id.as_str()));
    if has_remaining_turn_approvals {
        return None;
    }

    let final_summary =
        terminal_agent_turn_summary(state, &approval.session_id, &agent_turn_id, status);
    let message = Message {
        id: format!("message-{}", Uuid::new_v4()),
        session_id: approval.session_id.clone(),
        role: "assistant".to_string(),
        created_at_ms: now_ms(),
        parts: vec![text_part(
            final_summary.clone(),
            json!({
                "agent_turn_id": agent_turn_id,
                "status": status,
            }),
        )],
    };
    state.push_message(message.clone());
    broadcast_event(
        state,
        "message.updated",
        serde_json::to_value(&message).expect("message must serialize"),
    );
    broadcast_event(
        state,
        "assistant.completed",
        json!({
            "session_id": approval.session_id,
            "turn_id": agent_turn_id,
            "status": status,
        }),
    );
    Some(final_summary)
}

fn terminal_agent_turn_summary(
    state: &AppState,
    session_id: &str,
    agent_turn_id: &str,
    status: &str,
) -> String {
    let summaries = state
        .messages(session_id)
        .into_iter()
        .flat_map(|message| message.parts)
        .filter(|part| {
            part.data.get("agent_turn_id").and_then(Value::as_str) == Some(agent_turn_id)
                && part
                    .data
                    .get("approval_id")
                    .and_then(Value::as_str)
                    .is_some()
        })
        .filter_map(|part| part.text)
        .collect::<Vec<_>>();

    if summaries.is_empty() {
        if status == "stopped" {
            return "本轮已按用户要求停止，没有继续执行后续远程命令。".to_string();
        }
        return "本轮远程命令已全部执行完成。".to_string();
    }

    let mut text = if status == "stopped" {
        "本轮已按用户要求停止，未继续执行后续远程命令。已记录结果如下：".to_string()
    } else {
        "本轮远程命令已全部执行完成。结果如下：".to_string()
    };
    for (index, summary) in summaries.iter().enumerate() {
        text.push_str("\n\n");
        text.push_str(&(index + 1).to_string());
        text.push_str(". ");
        text.push_str(summary.trim_end());
    }
    text
}

fn approved_agent_summary(command: &str, output: &SshCommandOutput) -> String {
    let exit_code = output
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let mut summary = format!("Approved command `{command}` completed with exit code {exit_code}.");
    if !output.stdout.is_empty() {
        summary.push_str("\nstdout:\n");
        summary.push_str(&redact_text(&output.stdout));
    }
    if !output.stderr.is_empty() {
        summary.push_str("\nstderr:\n");
        summary.push_str(&redact_text(&output.stderr));
    }
    if output.timed_out {
        summary.push_str("\nCommand timed out.");
    }
    summary
}

fn rejected_agent_summary(command: &str) -> String {
    format!("Rejected command `{command}`; it was not executed.")
}

fn stopped_agent_summary(command: &str) -> String {
    format!("Stopped pending command `{command}`; it was not executed.")
}

fn command_result(command: &str, output: &SshCommandOutput, summary: &str) -> Value {
    json!({
        "command": command,
        "stdout": redact_text(&output.stdout),
        "stderr": redact_text(&output.stderr),
        "exit_code": output.exit_code,
        "duration_ms": duration_ms(output.duration),
        "timed_out": output.timed_out,
        "summary": summary,
        "assistant_text": summary,
    })
}

fn rejected_command_result(command: &str, summary: &str) -> Value {
    json!({
        "command": command,
        "status": "rejected",
        "summary": summary,
        "assistant_text": summary,
    })
}

fn redact_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(redact_text(text)),
        Value::Array(items) => Value::Array(items.iter().map(redact_value).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if is_secret_key(key) {
                        (key.clone(), Value::String("[REDACTED]".to_string()))
                    } else {
                        (key.clone(), redact_value(value))
                    }
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn redact_text(text: &str) -> String {
    let mut redacted = text.lines().map(redact_line).collect::<Vec<_>>().join("\n");
    if text.ends_with('\n') {
        redacted.push('\n');
    }
    redacted
}

fn redact_line(line: &str) -> String {
    for separator in ['=', ':'] {
        if let Some((key, _value)) = line.split_once(separator)
            && is_secret_key(key.trim())
        {
            return format!("{}{}[REDACTED]", key, separator);
        }
    }
    let lower = line.to_ascii_lowercase();
    if ["api_key", "apikey", "token", "secret", "bearer"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        return "[REDACTED]".to_string();
    }
    line.to_string()
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["token", "nonce", "secret", "bearer", "lease", "idempotency"]
        .iter()
        .any(|needle| key.contains(needle))
}

fn duration_ms(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(duration_ms)
        .unwrap_or_default()
}
