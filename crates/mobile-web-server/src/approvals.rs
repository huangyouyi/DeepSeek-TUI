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
        let approval = PendingApproval {
            id: format!("approval-{}", Uuid::new_v4()),
            session_id: request.session_id,
            command: request.command,
            cwd: request.cwd,
            created_at_ms: now,
            status: "pending".to_string(),
        };
        state.insert_pending_approval(approval.clone());
        state.push_audit(approval_audit(&approval, "prepared", json!({})));
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
        let mut approval =
            state
                .remove_pending_approval(approval_id)
                .ok_or_else(|| ApprovalError::NotFound {
                    approval_id: approval_id.to_string(),
                })?;

        match request.response.as_str() {
            "reject" => {
                approval.status = "rejected".to_string();
                append_agent_rejection_summary(state, &approval);
                let _ = append_agent_turn_final_summary_if_complete(state, &approval);
                state.push_audit(approval_audit(&approval, "rejected", json!({})));
                broadcast_reply(state, &approval);
                Ok(ApprovalResponse {
                    approval,
                    status: "rejected".to_string(),
                    result: json!({}),
                })
            }
            "approve_once" => {
                let output = self.execute_approval(state, &approval)?;
                let summary = approved_agent_summary(&approval.command, &output);
                approval.status = "approved".to_string();
                append_agent_approval_summary(state, &approval, summary);
                let assistant_summary =
                    append_agent_turn_final_summary_if_complete(state, &approval)
                        .unwrap_or_else(|| approved_agent_summary(&approval.command, &output));
                let result = command_result(&approval.command, &output, &assistant_summary);
                state.push_audit(approval_audit(&approval, "approved", result.clone()));
                broadcast_reply(state, &approval);
                Ok(ApprovalResponse {
                    approval,
                    status: "approved".to_string(),
                    result,
                })
            }
            other => Err(ApprovalError::UnsupportedResponse {
                response: other.to_string(),
            }),
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

fn approval_audit(approval: &PendingApproval, action: &str, result: Value) -> AuditEntry {
    AuditEntry {
        id: format!("audit-{}", Uuid::new_v4()),
        session_id: Some(approval.session_id.clone()),
        kind: format!("approval.{action}"),
        created_at_ms: now_ms(),
        summary: format!("approval {} {action}", approval.id),
        metadata: json!({
            "approval_id": approval.id,
            "command": approval.command,
            "status": action,
            "result": redact_value(&result),
        }),
    }
}

fn append_agent_approval_summary(state: &AppState, approval: &PendingApproval, summary: String) {
    let Some(agent_turn_id) = approval.agent_turn_id() else {
        return;
    };
    append_agent_summary_message(state, approval, agent_turn_id, summary);
}

fn append_agent_rejection_summary(state: &AppState, approval: &PendingApproval) {
    let Some(agent_turn_id) = approval.agent_turn_id() else {
        return;
    };
    append_agent_summary_message(
        state,
        approval,
        agent_turn_id,
        format!(
            "Rejected command `{}`; it was not executed.",
            approval.command
        ),
    );
}

fn append_agent_summary_message(
    state: &AppState,
    approval: &PendingApproval,
    agent_turn_id: String,
    text: String,
) {
    let message = Message {
        id: format!("message-{}", Uuid::new_v4()),
        session_id: approval.session_id.clone(),
        role: "assistant".to_string(),
        created_at_ms: now_ms(),
        parts: vec![MessagePart {
            id: format!("part-{}", Uuid::new_v4()),
            kind: "text".to_string(),
            text: Some(text),
            data: json!({
                "approval_id": approval.id,
                "agent_turn_id": agent_turn_id,
                "command": approval.command,
            }),
        }],
    };
    state.push_message(message.clone());
    broadcast_event(
        state,
        "message.updated",
        serde_json::to_value(&message).expect("message must serialize"),
    );
}

fn append_agent_turn_final_summary_if_complete(
    state: &AppState,
    approval: &PendingApproval,
) -> Option<String> {
    let agent_turn_id = approval.agent_turn_id()?;
    let has_remaining_turn_approvals = state
        .pending_approvals()
        .into_iter()
        .any(|pending| pending.agent_turn_id().as_deref() == Some(agent_turn_id.as_str()));
    if has_remaining_turn_approvals {
        return None;
    }

    let final_summary = final_agent_turn_summary(state, &approval.session_id, &agent_turn_id);
    let message = Message {
        id: format!("message-{}", Uuid::new_v4()),
        session_id: approval.session_id.clone(),
        role: "assistant".to_string(),
        created_at_ms: now_ms(),
        parts: vec![MessagePart {
            id: format!("part-{}", Uuid::new_v4()),
            kind: "text".to_string(),
            text: Some(final_summary.clone()),
            data: json!({
                "agent_turn_id": agent_turn_id,
                "status": "completed",
            }),
        }],
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
            "status": "completed",
        }),
    );
    Some(final_summary)
}

fn final_agent_turn_summary(state: &AppState, session_id: &str, agent_turn_id: &str) -> String {
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
        return "本轮远程命令已全部执行完成。".to_string();
    }

    let mut text = "本轮远程命令已全部执行完成。结果如下：".to_string();
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
